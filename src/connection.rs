//! One PTT connection owner. No login automation, credentials storage or shell.
use crate::{
    input::MAX_INPUT_BYTES,
    screen::{Frame, Screen},
    telnet,
};
use futures_util::{SinkExt, StreamExt};
use std::{
    future::pending,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
    time::{interval, timeout},
};
use tokio_tungstenite::{
    Connector, MaybeTlsStream, WebSocketStream, connect_async_tls_with_config,
    tungstenite::{Message, client::IntoClientRequest, protocol::WebSocketConfig},
};

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
#[derive(Clone)]
pub struct View {
    pub frame: Frame,
    pub generation: u64,
    pub phase: Phase,
    pub message: String,
    pub received: usize,
}
enum Command {
    Connect,
    Disconnect,
    Input { generation: u64, bytes: Vec<u8> },
    Shutdown(oneshot::Sender<()>),
}
pub struct Connection {
    commands: mpsc::Sender<Command>,
    pub updates: watch::Receiver<View>,
    worker: Mutex<Option<JoinHandle<()>>>,
    probe_peer: Mutex<Option<JoinHandle<Result<(), String>>>>,
}
impl Connection {
    pub fn start(screen: Screen) -> Result<Self, String> {
        Self::start_at(screen, PTT_ENDPOINT.to_owned())
    }
    fn start_at(screen: Screen, endpoint: String) -> Result<Self, String> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let (commands, receiver) = mpsc::channel(64);
        let initial = View {
            frame: screen.frame(),
            generation: 0,
            phase: Phase::Disconnected,
            message: "尚未連線 — 點選連線後手動登入 PTT".into(),
            received: 0,
        };
        let (sender, updates) = watch::channel(initial);
        let worker = runtime.spawn(run(screen, endpoint, receiver, sender));
        Ok(Self {
            commands,
            updates,
            worker: Mutex::new(Some(worker)),
            probe_peer: Mutex::new(None),
        })
    }
    /// An isolated browser-to-WebSocket conformance session; no external network.
    pub fn start_probe(bytes: Vec<u8>, reload_settings: bool) -> Result<Self, String> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let endpoint = format!(
            "ws://{}/bbs",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        let listener = tokio::net::TcpListener::from_std(listener).map_err(|e| e.to_string())?;
        let connection = Self::start_at(Screen::new(80, 24).map_err(str::to_owned)?, endpoint)?;
        let peer = runtime.spawn(probe::serve(listener, bytes, reload_settings));
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
    pub fn input(&self, generation: u64, bytes: Vec<u8>) -> Result<(), String> {
        if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
            return Err("輸入長度超過限制".into());
        }
        self.enqueue(Command::Input { generation, bytes })
    }
    fn enqueue(&self, command: Command) -> Result<(), String> {
        self.commands
            .try_send(command)
            .map_err(|_| "操作佇列已滿或連線服務已結束".into())
    }
    pub async fn shutdown(&self) -> Result<(), String> {
        if self
            .worker
            .lock()
            .map_err(|_| "worker ownership lock failed")?
            .is_none()
        {
            return Ok(());
        }
        let (done, receiver) = oneshot::channel();
        self.commands
            .send(Command::Shutdown(done))
            .await
            .map_err(|_| "連線服務已結束".to_owned())?;
        timeout(Duration::from_secs(5), receiver)
            .await
            .map_err(|_| "連線清理逾時".to_owned())?
            .map_err(|_| "連線清理未完成".to_owned())?;
        let task = self
            .worker
            .lock()
            .map_err(|_| "worker ownership lock failed".to_owned())?
            .take();
        if let Some(task) = task {
            task.await.map_err(|e| format!("連線工作失敗: {e}"))?;
        }
        let peer = self
            .probe_peer
            .lock()
            .map_err(|_| "probe owner lock failed")?
            .take();
        if let Some(mut peer) = peer {
            match timeout(Duration::from_secs(5), &mut peer).await {
                Ok(result) => result.map_err(|e| e.to_string())??,
                Err(_) => {
                    peer.abort();
                    let _ = peer.await;
                    return Err("probe peer cleanup timed out".into());
                }
            }
        }
        Ok(())
    }
}

async fn dial(endpoint: String) -> Result<Socket, String> {
    let mut request = endpoint.into_client_request().map_err(|e| e.to_string())?;
    let connector = if request.uri().scheme_str() == Some("wss") {
        Some(tls_connector()?)
    } else {
        None
    };
    request.headers_mut().insert(
        "Origin",
        "https://term.ptt.cc"
            .parse()
            .map_err(|_| "invalid fixed origin")?,
    );
    let config = WebSocketConfig::default()
        .max_message_size(Some(64 * 1024))
        .max_frame_size(Some(64 * 1024));
    timeout(
        Duration::from_secs(15),
        connect_async_tls_with_config(request, Some(config), false, connector),
    )
    .await
    .map_err(|_| "連線逾時".to_owned())?
    .map(|(socket, _)| socket)
    .map_err(|e| format!("無法連線: {e}"))
}

fn tls_connector() -> Result<Connector, String> {
    let mut roots = rustls::RootCertStore::empty();
    let certificates = rustls_native_certs::load_native_certs();
    roots.add_parsable_certificates(certificates.certs);
    if roots.is_empty() {
        return Err("系統沒有可用的 TLS 信任根憑證".into());
    }
    // Desktop dependencies can enable both Rustls providers. Select locally,
    // without installing a mutable process-wide default or bypassing verification.
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| e.to_string())?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(Connector::Rustls(Arc::new(config)))
}

async fn stop(socket: &mut Option<Socket>, dialing: &mut Option<Dial>) -> bool {
    if let Some(task) = dialing.take() {
        task.abort();
        let _ = task.await;
    }
    if let Some(mut connection) = socket.take() {
        return matches!(
            timeout(Duration::from_secs(2), connection.close(None)).await,
            Ok(Ok(()))
        );
    }
    true
}

async fn send(socket: &mut Socket, message: Message) -> Result<(), String> {
    timeout(Duration::from_secs(3), socket.send(message))
        .await
        .map_err(|_| "送出逾時；站端結果未知，未自動重送".to_owned())?
        .map_err(|e| format!("送出失敗，未自動重送: {e}"))
}

async fn run(
    mut screen: Screen,
    endpoint: String,
    mut commands: mpsc::Receiver<Command>,
    updates: watch::Sender<View>,
) {
    let mut socket: Option<Socket> = None;
    let mut dialing: Option<Dial> = None;
    let mut framing = telnet::Framing::default();
    let mut view = updates.borrow().clone();
    let mut dirty = false;
    let mut clock = interval(Duration::from_millis(16));
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Connect) if socket.is_none() && dialing.is_none() => {
                    view.generation = view.generation.saturating_add(1);
                    screen.reset_session(); framing = telnet::Framing::default(); view.received = 0;
                    view.phase = Phase::Connecting; view.message = "正在連線 PTT…".into();
                    dialing = Some(tokio::spawn(dial(endpoint.clone()))); dirty = true;
                },
                Some(Command::Connect) => {},
                Some(Command::Disconnect) => {
                    view.generation = view.generation.saturating_add(1);
                    let closed = stop(&mut socket, &mut dialing).await;
                    view.phase = Phase::Disconnected;
                    view.message = if closed { "本機已斷線（已送出關閉訊息）" } else { "本機連線已關閉；遠端關閉未確認" }.into(); dirty = true;
                },
                Some(Command::Input { generation, bytes }) => {
                    // Preserve the peer/transport's last reason when typing after closure.
                    if socket.is_none() && view.generation > 0 {
                        continue;
                    }
                    if generation != view.generation || socket.is_none() {
                        view.message = "尚未連線或輸入屬於舊連線；未送出".into(); dirty = true; continue;
                    }
                    if let Some(connection) = socket.as_mut()
                        && let Err(error) = send(connection, Message::Binary(telnet::escape(&bytes).into())).await {
                            socket = None; view.phase = Phase::Error; view.message = error; dirty = true;
                        }
                },
                Some(Command::Shutdown(done)) => {
                    stop(&mut socket, &mut dialing).await;
                    let _ = done.send(()); break;
                },
                None => { stop(&mut socket, &mut dialing).await; break; },
            },
            result = async { match dialing.as_mut() { Some(task) => task.await, None => pending().await } } => {
                dialing = None;
                match result {
                    Ok(Ok(connection)) => {
                        socket = Some(connection); view.phase = Phase::Connected;
                        view.message = "已連線 — 請在畫面內手動登入 PTT".into();
                    },
                    Ok(Err(error)) => { view.phase = Phase::Error; view.message = error; },
                    Err(error) => { view.phase = Phase::Error; view.message = format!("連線工作失敗: {error}"); },
                }
                dirty = true;
            },
            incoming = async { match socket.as_mut() { Some(connection) => connection.next().await, None => pending().await } } => {
                let bytes = match incoming {
                    Some(Ok(Message::Binary(bytes))) => Some(bytes),
                    Some(Ok(Message::Text(_))) => {
                        socket = None; view.phase = Phase::Error; view.message = "站端回傳非預期文字 framing；未轉碼猜測".into(); None
                    },
                    Some(Ok(Message::Ping(payload))) => {
                        if let Some(connection) = socket.as_mut()
                            && let Err(error) = send(connection, Message::Pong(payload)).await {
                                socket = None; view.phase = Phase::Error; view.message = error;
                            }
                        None
                    },
                    Some(Ok(Message::Pong(_))) => None,
                    Some(Ok(Message::Close(frame))) => {
                        // Tungstenite queues the peer's close acknowledgement during read.
                        if let Some(mut connection) = socket.take() {
                            let _ = timeout(Duration::from_secs(2), connection.flush()).await;
                        }
                        view.phase = Phase::Disconnected;
                        view.message = match frame {
                            Some(frame) if frame.reason.is_empty() => format!("站端已關閉連線（{}）", u16::from(frame.code)),
                            Some(frame) => format!("站端已關閉連線（{}）：{}", u16::from(frame.code), frame.reason),
                            None => "站端已關閉連線（未提供原因）".into(),
                        }; None
                    },
                    None => {
                        socket = None; view.phase = Phase::Disconnected; view.message = "站端連線已結束（未收到關閉原因）".into(); None
                    },
                    Some(Err(error)) => { socket = None; view.phase = Phase::Error; view.message = format!("連線中斷: {error}"); None },
                    Some(Ok(_)) => None,
                };
                if let Some(bytes) = bytes {
                    view.received = view.received.saturating_add(bytes.len());
                    let (display, reply) = framing.feed(&bytes, 80, 24); screen.feed(&display);
                    if !reply.is_empty() && let Some(connection) = socket.as_mut()
                        && let Err(error) = send(connection, Message::Binary(reply.into())).await {
                            socket = None; view.phase = Phase::Error; view.message = error;
                        }
                }
                dirty = true;
            },
            _ = clock.tick(), if dirty => {
                view.frame = screen.frame(); updates.send_replace(view.clone()); dirty = false;
            },
        }
    }
}

mod probe;
#[cfg(test)]
mod tests;

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
