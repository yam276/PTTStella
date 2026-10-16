//! One PTT connection owner, including explicit single-attempt login. No shell.
use crate::{
    input::MAX_INPUT_BYTES,
    login::Credentials,
    navigation::{Intent, Stamp},
    screen::{Frame, Screen},
};
use std::{sync::Mutex, time::Duration};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
    time::timeout,
};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

pub const PTT_ENDPOINT: &str = "wss://ws.ptt.cc/bbs";
type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type Dial = JoinHandle<Result<Socket, String>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Disconnected,
    Connecting,
    Connected,
    Error,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseOutcome {
    NoConnection,
    Acknowledged,
    PeerClosedWithoutReply,
    Unconfirmed,
}
#[derive(Clone)]
pub struct View {
    pub keep_alive: bool,
    pub keep_alive_ready: bool,
    pub keep_alive_sent: u64,
    pub keep_alive_failure: Option<String>,
    pub input_epoch: u64,
    pub input_pending: bool,
    pub login_pending: bool,
    pub frame: Frame,
    pub generation: u64,
    pub phase: Phase,
    pub message: String,
    pub received: usize,
}
impl View {
    pub fn stamp(&self) -> Stamp {
        Stamp {
            generation: self.generation,
            revision: self.frame.revision,
            input_epoch: self.input_epoch,
        }
    }
}
enum Command {
    KeepAlive(bool),
    Connect,
    Disconnect,
    Input {
        generation: u64,
        bytes: Vec<u8>,
    },
    Login {
        stamp: Stamp,
        credentials: Credentials,
        reply: oneshot::Sender<Result<(), String>>,
    },
    CancelLogin,
    Navigate {
        stamp: Stamp,
        intent: Intent,
        keyboard: bool,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Shutdown(oneshot::Sender<CloseOutcome>),
}
pub struct Connection {
    commands: mpsc::Sender<Command>,
    keep_alive_ui: watch::Sender<Option<(Stamp, bool)>>,
    pub updates: watch::Receiver<View>,
    worker: Mutex<Option<JoinHandle<()>>>,
    probe_peer: Mutex<Option<JoinHandle<Result<(), String>>>>,
    shutdown_result: Mutex<Option<Result<CloseOutcome, String>>>,
}
/// Already enqueued in the single writer. Waiting never changes input order.
pub struct Reply {
    answer: oneshot::Receiver<Result<(), String>>,
    deadline: tokio::time::Instant,
    expired: &'static str,
    closed: &'static str,
}
impl Reply {
    pub async fn wait(self) -> Result<(), String> {
        tokio::time::timeout_at(self.deadline, self.answer)
            .await
            .map_err(|_| self.expired.to_owned())?
            .map_err(|_| self.closed.to_owned())?
    }
}
impl Connection {
    pub fn start(screen: Screen) -> Result<Self, String> {
        Self::start_at(screen, PTT_ENDPOINT.to_owned())
    }
    fn start_at(screen: Screen, endpoint: String) -> Result<Self, String> {
        Self::start_with_timing(screen, endpoint, liveness::Timing::default())
    }
    fn start_with_timing(
        screen: Screen,
        endpoint: String,
        timing: liveness::Timing,
    ) -> Result<Self, String> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let (commands, receiver) = mpsc::channel(64);
        let initial = View {
            keep_alive: true,
            keep_alive_ready: false,
            keep_alive_sent: 0,
            keep_alive_failure: None,
            input_epoch: 0,
            input_pending: false,
            login_pending: false,
            frame: screen.frame(),
            generation: 0,
            phase: Phase::Disconnected,
            message: "尚未連線 — 點選連線後手動登入 PTT".into(),
            received: 0,
        };
        let (sender, updates) = watch::channel(initial);
        let (keep_alive_ui, gates) = watch::channel(None);
        let worker = runtime.spawn(worker::run(
            screen, endpoint, receiver, sender, timing, gates,
        ));
        Ok(Self {
            commands,
            keep_alive_ui,
            updates,
            worker: Mutex::new(Some(worker)),
            probe_peer: Mutex::new(None),
            shutdown_result: Mutex::new(None),
        })
    }
    /// An isolated browser-to-WebSocket conformance session; no external network.
    pub fn start_probe(bytes: Vec<u8>, reload_settings: bool) -> Result<Self, String> {
        Self::start_owned_probe(bytes, probe::Scenario::Desktop { reload_settings })
    }
    /// Native-window regression, with a local peer checking translated Vim bytes.
    pub fn start_window_probe() -> Result<Self, String> {
        let connection = Self::start_owned_probe(probe::window_page(0)?, probe::Scenario::Window)?;
        connection.keep_alive(false)?;
        Ok(connection)
    }
    pub fn start_login_probe() -> Result<Self, String> {
        Self::start_owned_probe(probe::login_page()?, probe::Scenario::Login)
    }
    pub fn start_filters_probe() -> Result<Self, String> {
        let connection =
            Self::start_owned_probe(probe::filters_page(false)?, probe::Scenario::Filters)?;
        connection.keep_alive(false)?;
        Ok(connection)
    }
    /// Fault/admission regression: local peer rejects every station input byte.
    pub fn start_surface_probe() -> Result<Self, String> {
        Self::start_ui_probe(probe::window_page(0)?)
    }
    /// Explicit synthetic UI fixture; the peer accepts Close and no input bytes.
    pub fn start_ui_probe(bytes: Vec<u8>) -> Result<Self, String> {
        let connection = Self::start_owned_probe(bytes, probe::Scenario::Surface)?;
        connection.keep_alive(false)?;
        Ok(connection)
    }
    fn start_owned_probe(bytes: Vec<u8>, scenario: probe::Scenario) -> Result<Self, String> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let endpoint = format!(
            "ws://{}/bbs",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        let listener = tokio::net::TcpListener::from_std(listener).map_err(|e| e.to_string())?;
        let connection = Self::start_at(Screen::new(80, 24).map_err(str::to_owned)?, endpoint)?;
        let peer = runtime.spawn(probe::serve(listener, bytes, scenario));
        *connection
            .probe_peer
            .lock()
            .map_err(|_| "probe owner lock failed")? = Some(peer);
        Ok(connection)
    }
    pub fn connect(&self) -> Result<(), String> {
        self.enqueue(Command::Connect)
    }
    pub fn disconnect(&self) -> Result<(), String> {
        self.enqueue(Command::Disconnect)
    }
    pub fn keep_alive(&self, enabled: bool) -> Result<(), String> {
        self.enqueue(Command::KeepAlive(enabled))
    }
    pub fn keep_alive_gate(&self, stamp: Stamp, ready: bool) {
        // A blocked UI cannot lose its pause request to a full input queue.
        self.keep_alive_ui.send_replace(Some((stamp, ready)));
    }
    pub fn input(&self, generation: u64, bytes: Vec<u8>) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
            return Err("輸入長度超過限制".into());
        }
        self.enqueue(Command::Input { generation, bytes })
    }
    pub async fn login(&self, stamp: Stamp, credentials: Credentials) -> Result<(), String> {
        self.request_login(stamp, credentials)?.wait().await
    }
    pub fn request_login(&self, stamp: Stamp, credentials: Credentials) -> Result<Reply, String> {
        let (reply, answer) = oneshot::channel();
        self.enqueue(Command::Login {
            stamp,
            credentials,
            reply,
        })?;
        Ok(Reply {
            answer,
            deadline: tokio::time::Instant::now() + Duration::from_secs(6),
            expired: "登入送出結果未確認；不會重試",
            closed: "登入服務未回覆",
        })
    }
    pub fn cancel_login(&self) -> Result<(), String> {
        self.enqueue(Command::CancelLogin)
    }
    pub async fn navigate(&self, stamp: Stamp, intent: Intent) -> Result<(), String> {
        self.request_navigate(stamp, intent)?.wait().await
    }
    pub async fn browse_key(&self, stamp: Stamp, intent: Intent) -> Result<(), String> {
        self.request_browse_key(stamp, intent)?.wait().await
    }
    pub fn request_navigate(&self, stamp: Stamp, intent: Intent) -> Result<Reply, String> {
        self.request_navigation(stamp, intent, false)
    }
    pub fn request_browse_key(&self, stamp: Stamp, intent: Intent) -> Result<Reply, String> {
        self.request_navigation(stamp, intent, true)
    }
    fn request_navigation(
        &self,
        stamp: Stamp,
        intent: Intent,
        keyboard: bool,
    ) -> Result<Reply, String> {
        let (reply, answer) = oneshot::channel();
        self.enqueue(Command::Navigate {
            stamp,
            intent,
            keyboard,
            reply,
        })?;
        Ok(Reply {
            answer,
            deadline: tokio::time::Instant::now() + Duration::from_secs(6),
            expired: "導覽傳送逾時，結果未確認；不會自動重送",
            closed: "連線服務未回報導覽結果",
        })
    }
    fn enqueue(&self, command: Command) -> Result<(), String> {
        self.commands
            .try_send(command)
            .map_err(|_| "操作佇列已滿或連線服務已結束".into())
    }
    pub async fn shutdown(&self) -> Result<CloseOutcome, String> {
        if self
            .worker
            .lock()
            .map_err(|_| "worker ownership lock failed")?
            .is_none()
        {
            return self
                .shutdown_result
                .lock()
                .map_err(|_| "shutdown result lock failed".to_owned())
                .and_then(|result| result.clone().ok_or("連線清理結果尚未確認".to_owned())?);
        }
        let (done, receiver) = oneshot::channel();
        // Keep the enqueue alive if the bounded queue is full: canceling it on
        // a deadline would leave the worker without a shutdown request.
        let outcome = match self.commands.send(Command::Shutdown(done)).await {
            Ok(()) => timeout(Duration::from_secs(5), receiver)
                .await
                .map_err(|_| "連線清理逾時".to_owned())
                .and_then(|result| result.map_err(|_| "連線清理未完成".to_owned())),
            Err(_) => Err("連線服務已結束".to_owned()),
        };
        let mut errors = Vec::new();
        let task = self
            .worker
            .lock()
            .map_err(|_| "worker ownership lock failed".to_owned())?
            .take();
        if let Some(task) = task {
            // Preserve the owner even when the acknowledgement failed. Its
            // bounded transport work still has to finish and join its dial.
            if let Err(error) = task.await {
                errors.push(format!("連線工作失敗: {error}"));
            }
        }
        let peer = self
            .probe_peer
            .lock()
            .map_err(|_| "probe owner lock failed")?
            .take();
        if let Some(mut peer) = peer {
            match timeout(Duration::from_secs(5), &mut peer).await {
                Ok(Ok(Ok(()))) => {}
                Ok(Ok(Err(error))) => errors.push(error),
                Ok(Err(error)) => errors.push(error.to_string()),
                Err(_) => {
                    peer.abort();
                    let _ = peer.await;
                    errors.push("probe peer cleanup timed out".into());
                }
            }
        }
        if let Err(error) = &outcome {
            errors.push(error.clone());
        }
        let result = if errors.is_empty() {
            outcome
        } else {
            Err(errors.join("；"))
        };
        *self
            .shutdown_result
            .lock()
            .map_err(|_| "shutdown result lock failed".to_owned())? = Some(result.clone());
        result
    }
}

mod liveness;
mod probe;
#[cfg(test)]
mod tests;
mod transport;
mod worker;

/// Explicit no-credential network probe, separate from ordinary gates.
pub async fn probe_live() -> Result<serde_json::Value, String> {
    let connection = Connection::start(Screen::new(80, 24).map_err(str::to_owned)?)?;
    let mut updates = connection.updates.clone();
    connection.connect()?;
    let observed = timeout(Duration::from_secs(20), async {
        loop {
            updates.changed().await.map_err(|e| e.to_string())?;
            let view = updates.borrow_and_update().clone();
            if view.phase == Phase::Error {
                return Err(view.message);
            }
            if view.received > 0 {
                let text: String = view.frame.glyphs.iter().map(|g| g.text.as_str()).collect();
                if text.contains("請輸入代號") {
                    return Ok(
                        serde_json::json!({"status":"passed","endpoint":PTT_ENDPOINT,
                        "received_bytes":view.received,"login_prompt_detected":true,
                        "credential_input_sent":false,"scope":"pre-login only; no authentication"}),
                    );
                }
            }
        }
    })
    .await
    .map_err(|_| "未於期限內確認登入前提示；沒有送出帳密".to_owned())
    .and_then(|r| r);
    connection.shutdown().await?;
    observed
}
