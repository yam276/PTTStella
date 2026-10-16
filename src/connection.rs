//! One PTT connection owner. No login automation, credentials storage or shell.
use crate::{
    input::MAX_INPUT_BYTES,
    navigation::{self, Intent, Stamp},
    screen::{Frame, Screen},
    telnet,
};
use futures_util::{SinkExt, StreamExt};
use std::{
    future::pending,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
    time::{MissedTickBehavior, interval, timeout},
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
    pub keep_alive: bool,
    pub keep_alive_ready: bool,
    pub keep_alive_sent: u64,
    pub keep_alive_failure: Option<String>,
    pub input_epoch: u64,
    pub input_pending: bool,
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
    Navigate {
        stamp: Stamp,
        intent: Intent,
        keyboard: bool,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Shutdown(oneshot::Sender<()>),
}
enum PendingInput {
    Manual {
        revision: u64,
        deadline: Instant,
        streaming: bool,
    },
    Refresh {
        revision: u64,
        deadline: Instant,
    },
}
impl PendingInput {
    fn revision(&self) -> u64 {
        match self {
            Self::Manual { revision, .. } | Self::Refresh { revision, .. } => *revision,
        }
    }
    fn expired(&self, now: Instant) -> bool {
        match self {
            Self::Manual { deadline, .. } | Self::Refresh { deadline, .. } => now >= *deadline,
        }
    }
}
pub struct Connection {
    commands: mpsc::Sender<Command>,
    keep_alive_ui: watch::Sender<Option<(Stamp, bool)>>,
    pub updates: watch::Receiver<View>,
    worker: Mutex<Option<JoinHandle<()>>>,
    probe_peer: Mutex<Option<JoinHandle<Result<(), String>>>>,
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
            frame: screen.frame(),
            generation: 0,
            phase: Phase::Disconnected,
            message: "尚未連線 — 點選連線後手動登入 PTT".into(),
            received: 0,
        };
        let (sender, updates) = watch::channel(initial);
        let (keep_alive_ui, gates) = watch::channel(None);
        let worker = runtime.spawn(run(screen, endpoint, receiver, sender, timing, gates));
        Ok(Self {
            commands,
            keep_alive_ui,
            updates,
            worker: Mutex::new(Some(worker)),
            probe_peer: Mutex::new(None),
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
    pub async fn navigate(&self, stamp: Stamp, intent: Intent) -> Result<(), String> {
        self.navigate_from(stamp, intent, false).await
    }
    pub async fn browse_key(&self, stamp: Stamp, intent: Intent) -> Result<(), String> {
        self.navigate_from(stamp, intent, true).await
    }
    async fn navigate_from(
        &self,
        stamp: Stamp,
        intent: Intent,
        keyboard: bool,
    ) -> Result<(), String> {
        let (reply, answer) = oneshot::channel();
        self.enqueue(Command::Navigate {
            stamp,
            intent,
            keyboard,
            reply,
        })?;
        timeout(Duration::from_secs(6), answer)
            .await
            .map_err(|_| "導覽傳送逾時，結果未確認；不會自動重送".to_owned())?
            .map_err(|_| "連線服務未回報導覽結果".to_owned())?
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
    Ok(Connector::Rustls(Arc::new(crate::tls::client_config()?)))
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
    timing: liveness::Timing,
    mut gates: watch::Receiver<Option<(Stamp, bool)>>,
) {
    let mut socket: Option<Socket> = None;
    let mut dialing: Option<Dial> = None;
    let mut framing = telnet::Framing::default();
    let mut view = updates.borrow().clone();
    let mut dirty = false;
    let mut pending_input: Option<PendingInput> = None;
    let mut clock = interval(Duration::from_millis(16));
    let mut idle = liveness::Idle::new(Instant::now(), timing);
    let mut idle_clock = interval(timing.poll);
    idle_clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::KeepAlive(enabled)) => {
                    if let Some(failure)=view.keep_alive_failure.take() && view.message==failure && view.phase==Phase::Connected {
                        view.message=if enabled {"已連線 — 防閒置已開啟"} else {"已連線 — 防閒置已暫停"}.into();
                    }
                    idle.enable(enabled,Instant::now());view.keep_alive=enabled;dirty=true;
                    if matches!(pending_input,Some(PendingInput::Refresh {..})) {pending_input=None;}
                },
                Some(Command::Connect) if socket.is_none() && dialing.is_none() => {
                    view.generation = view.generation.saturating_add(1);
                    idle.activity(Instant::now());view.keep_alive_sent=0;
                    view.keep_alive_failure=None;
                    screen.reset_session(); framing = telnet::Framing::default(); view.received = 0;
                    pending_input=None;view.input_epoch=0;view.input_pending=false;
                    view.phase = Phase::Connecting; view.message = "正在連線 PTT…".into();
                    dialing = Some(tokio::spawn(dial(endpoint.clone()))); dirty = true;
                },
                Some(Command::Connect) => {},
                Some(Command::Disconnect) => {
                    idle.activity(Instant::now());
                    view.generation = view.generation.saturating_add(1);
                    pending_input=None;view.input_pending=false;
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
                    idle.activity(Instant::now());
                    if write_input(&mut socket,&mut view,&bytes).await.is_ok() { pending_input=Some(PendingInput::Manual {revision:screen.revision(),deadline:Instant::now()+timing.response_timeout,streaming:false}); }
                    dirty=true;
                },
                Some(Command::Navigate {stamp,intent,keyboard,reply}) => {
                    // An expired request must never execute later from the command queue.
                    if reply.is_closed() { continue; }
                    let streaming=keyboard && matches!(intent,Intent::Lines{..}) && matches!(pending_input,Some(PendingInput::Manual{streaming:true,..}));
                    let result=if stamp.generation!=view.generation || socket.is_none() || view.phase!=Phase::Connected {
                        Err(navigation::Error::Disconnected.to_string())
                    } else if stamp.revision!=screen.revision() || (!streaming && stamp.input_epoch!=view.input_epoch) {
                        Err(navigation::Error::Stale.to_string())
                    } else if !streaming && pending_input.as_ref().is_some_and(|p|p.revision()==screen.revision()) {
                        Err(navigation::Error::AwaitingScreen.to_string())
                    } else {
                        match navigation::plan(&navigation::analyze(&screen.frame()),intent) {
                            Err(error)=>Err(error.to_string()),
                            Ok(bytes)=> {
                                if bytes.is_empty() {let _=reply.send(Ok(()));continue;}
                                idle.activity(Instant::now());
                                let result=write_input(&mut socket,&mut view,&bytes).await;
                                if result.is_ok() { pending_input=Some(PendingInput::Manual {revision:screen.revision(),deadline:Instant::now()+timing.response_timeout,streaming:keyboard && matches!(intent,Intent::Lines{..})}); }
                                dirty=true;result
                            },
                        }
                    };
                    let _=reply.send(result);
                },
                Some(Command::Shutdown(done)) => {
                    stop(&mut socket, &mut dialing).await;
                    let _ = done.send(()); break;
                },
                None => { stop(&mut socket, &mut dialing).await; break; },
            },
            changed = gates.changed() => {
                if changed.is_err() {stop(&mut socket,&mut dialing).await;break;}
                if let Some((stamp,ready))=*gates.borrow_and_update() && stamp.generation==view.generation {
                    let current=Stamp {revision:screen.revision(),..view.stamp()};
                    idle.gate(stamp,ready && stamp==current,Instant::now());dirty=true;
                }
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
                if pending_input.as_ref().is_some_and(|p| p.revision()!=screen.revision()) { pending_input=None; }
                view.input_pending=pending_input.is_some() && view.phase==Phase::Connected;
                view.keep_alive_ready=idle.ready(Stamp {revision:screen.revision(),..view.stamp()})
                    && socket.is_some() && !view.input_pending
                    && navigation::analyze(&screen.frame()).allows_keepalive();
                view.frame = screen.frame(); updates.send_replace(view.clone()); dirty = false;
            },
            _ = idle_clock.tick() => {
                let now=Instant::now();idle.tick(now);
                // Defer automatic work to pending input/UI state without starving
                // incoming data or frame publication through a biased select.
                if !commands.is_empty() || !matches!(gates.has_changed(),Ok(false)) {continue;}
                if pending_input.as_ref().is_some_and(|p|p.revision()!=screen.revision()) {pending_input=None;}
                if pending_input.as_ref().is_some_and(|p|p.expired(now)) {
                    if matches!(pending_input,Some(PendingInput::Refresh{..})) {
                        idle.enable(false,now);view.keep_alive=false;
                        view.message="防閒置未收到畫面更新，已暫停；手動操作仍可使用".into();
                        view.keep_alive_failure=Some(view.message.clone());
                    } else {view.message="未收到畫面更新，操作結果未確認；可繼續手動操作，不會自動重送".into();}
                    pending_input=None;dirty=true;
                }
                let stamp=Stamp {revision:screen.revision(),..view.stamp()};
                if socket.is_some() && view.phase==Phase::Connected && pending_input.is_none()
                    && idle.due(now,stamp)
                    && navigation::analyze(&screen.frame()).allows_keepalive() {
                    idle.activity(now);
                    if write_input(&mut socket,&mut view,liveness::REFRESH).await.is_ok() {
                        pending_input=Some(PendingInput::Refresh {revision:screen.revision(),deadline:Instant::now()+timing.response_timeout});
                        view.keep_alive_sent=view.keep_alive_sent.saturating_add(1);
                    }
                    dirty=true;
                } else if view.keep_alive_ready && !idle.ready(stamp) {dirty=true;}
            },
        }
        // A response wait belongs to a live connection. Its deadline must not
        // replace a later peer close or transport failure with a generic notice.
        if view.phase != Phase::Connected {
            pending_input = None;
        }
    }
}

async fn write_input(
    socket: &mut Option<Socket>,
    view: &mut View,
    bytes: &[u8],
) -> Result<(), String> {
    let Some(connection) = socket.as_mut() else {
        return Err(navigation::Error::Disconnected.to_string());
    };
    view.input_epoch = view.input_epoch.saturating_add(1);
    match send(connection, Message::Binary(telnet::escape(bytes).into())).await {
        Ok(()) => Ok(()),
        Err(error) => {
            *socket = None;
            view.phase = Phase::Error;
            view.message = error.clone();
            Err(error)
        }
    }
}

mod liveness;
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
