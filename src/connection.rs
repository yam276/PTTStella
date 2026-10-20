//! One PTT connection owner, including explicit single-attempt login. No shell.
use crate::{
    board_navigation::BoardName,
    input::MAX_INPUT_BYTES,
    login::Credentials,
    navigation::{self, Intent, Stamp},
    navigation_job::{Destination, Progress as NavigationProgress},
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

pub const PTT_ENDPOINT: &str = crate::sites::PTT;
pub use probe::MouseSampleProbe;
type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
use transport::Socket;
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
    TransportClosed,
    Unconfirmed,
}
#[derive(Clone)]
pub struct View {
    pub site: crate::sites::Target,
    pub mouse: crate::mouse::input::Snapshot,
    /// The retained frame is not an admissible source for interactions yet.
    pub output_pending: bool,
    pub keep_alive: bool,
    pub keep_alive_ready: bool,
    pub keep_alive_sent: u64,
    pub keep_alive_failure: Option<String>,
    pub input_epoch: u64,
    pub input_pending: bool,
    pub login_pending: bool,
    pub login_guidance: crate::login::Guidance,
    pub destination: NavigationProgress,
    pub board_visit: Option<crate::recent_boards::BoardVisit>,
    pub frame: Frame,
    pub generation: u64,
    pub phase: Phase,
    pub message: String,
    pub received: usize,
}
impl View {
    pub fn context(&self) -> crate::navigation::Context {
        if !self.site.ptt_features() {
            crate::navigation::Context::Unknown {
                reason: crate::navigation::UnknownReason::DifferentSite,
            }
        } else if self.output_pending {
            crate::navigation::Context::Unknown {
                reason: crate::navigation::UnknownReason::Incomplete,
            }
        } else {
            crate::navigation::analyze(&self.frame)
        }
    }
    pub fn stamp(&self) -> Stamp {
        Stamp {
            generation: self.generation,
            revision: self.frame.revision,
            input_epoch: self.input_epoch,
        }
    }
}
enum Command {
    Mouse(crate::mouse::input::Event),
    MouseSample(crate::mouse::input::Sample),
    KeepAlive(bool),
    Connect,
    SwitchSite(crate::sites::Target),
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
    GoTo {
        stamp: Stamp,
        destination: Destination,
        reply: oneshot::Sender<Result<(), String>>,
    },
    CancelNavigation {
        operation: Stamp,
    },
    Navigate {
        stamp: Stamp,
        intent: Intent,
        source: navigation::Source,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Shutdown(oneshot::Sender<CloseOutcome>),
}
pub struct Connection {
    commands: mpsc::Sender<Command>,
    keep_alive_ui: watch::Sender<Option<(Stamp, bool)>>,
    mouse_ui: watch::Sender<mouse_control::Controls>,
    pub updates: watch::Receiver<View>,
    worker: Mutex<Option<JoinHandle<()>>>,
    probe_peer: Mutex<Option<JoinHandle<Result<(), String>>>>,
    shutdown_result: Mutex<Option<Result<CloseOutcome, String>>>,
}
/// Already enqueued in the single writer. Waiting never changes input order.
pub struct Reply {
    answer: oneshot::Receiver<Result<(), String>>,
    deadline: tokio::time::Instant,
    expired: String,
    closed: String,
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
    /// The serialized session owner closes the old transport before opening another.
    pub fn connect_to(&self, target: crate::sites::Target) -> Result<(), String> {
        self.enqueue(Command::SwitchSite(target))
    }
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
            site: crate::sites::Target::default(),
            mouse: crate::mouse::input::Snapshot::default(),
            output_pending: false,
            keep_alive: true,
            keep_alive_ready: false,
            keep_alive_sent: 0,
            keep_alive_failure: None,
            input_epoch: 0,
            input_pending: false,
            login_pending: false,
            login_guidance: crate::login::Guidance::Idle,
            destination: NavigationProgress::Idle,
            board_visit: None,
            frame: screen.frame(),
            generation: 0,
            phase: Phase::Disconnected,
            message: crate::msg!("status-offline"),
            received: 0,
        };
        let (sender, updates) = watch::channel(initial);
        let (keep_alive_ui, gates) = watch::channel(None);
        let (mouse_ui, mouse_gates) = watch::channel(mouse_control::Controls::default());
        let worker = runtime.spawn(worker::run(
            screen,
            endpoint,
            receiver,
            sender,
            timing,
            gates,
            mouse_gates,
        ));
        Ok(Self {
            commands,
            keep_alive_ui,
            mouse_ui,
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
    pub fn start_verification_probe() -> Result<Self, String> {
        Self::start_owned_probe(probe::login_page()?, probe::Scenario::Login2fa)
    }
    pub fn start_board_probe() -> Result<Self, String> {
        let connection =
            Self::start_owned_probe(probe::board_page("main")?, probe::Scenario::Boards)?;
        connection.keep_alive(false)?;
        Ok(connection)
    }
    pub fn start_article_probe() -> Result<Self, String> {
        let connection =
            Self::start_owned_probe(probe::article_page("main")?, probe::Scenario::Articles)?;
        connection.keep_alive(false)?;
        Ok(connection)
    }
    pub fn start_bookmarks_probe() -> Result<Self, String> {
        let connection =
            Self::start_owned_probe(probe::article_page("main")?, probe::Scenario::Bookmarks)?;
        connection.keep_alive(false)?;
        Ok(connection)
    }
    pub fn start_mouse_probe(boundary: bool) -> Result<Self, String> {
        Self::start_mouse_scene(1000, probe::Scenario::Mouse { boundary })
    }
    pub fn start_mouse_samples_probe(case: MouseSampleProbe) -> Result<Self, String> {
        let tracking = if matches!(case, MouseSampleProbe::Drag | MouseSampleProbe::RoundTrip) {
            1002
        } else {
            1000
        };
        Self::start_mouse_scene(tracking, probe::Scenario::MouseSamples(case))
    }
    fn start_mouse_scene(tracking: usize, scenario: probe::Scenario) -> Result<Self, String> {
        let mut bytes = [
            probe::article_page("articles")?,
            format!("\x1b[?{tracking};1006h").into_bytes(),
        ]
        .concat();
        if matches!(
            scenario,
            probe::Scenario::MouseSamples(MouseSampleProbe::RoundTrip)
        ) {
            bytes.extend_from_slice(&probe::round_trip::scene());
        }
        let connection = Self::start_owned_probe(bytes, scenario)?;
        connection.keep_alive(false)?;
        Ok(connection)
    }
    pub fn start_pointer_rows_probe() -> Result<Self, String> {
        let connection =
            Self::start_owned_probe(probe::pointer_menu()?, probe::Scenario::PointerRows)?;
        connection.keep_alive(false)?;
        Ok(connection)
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
    pub fn start_inline_pages_probe(first: Vec<u8>, second: Vec<u8>) -> Result<Self, String> {
        let connection = Self::start_owned_probe(first, probe::Scenario::InlinePages { second })?;
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
        let connection = Self::start_at(
            Screen::new(80, 24).map_err(|error| error.to_string())?,
            endpoint,
        )?;
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
            return Err(crate::msg!("input-too-large"));
        }
        self.enqueue(Command::Input { generation, bytes })
    }
    pub fn mouse_report(&self, event: crate::mouse::input::Event) -> Result<(), String> {
        // Keep capacity for an admitted release, manual keys and shutdown.
        if event.report.action == crate::mouse::Action::Press && self.commands.capacity() <= 8 {
            return Err(crate::msg!("connection-queue-full"));
        }
        self.enqueue(Command::Mouse(event))
    }
    pub fn mouse_sample(&self, sample: crate::mouse::input::Sample) -> Result<(), String> {
        // Continuous input cannot consume the capacity reserved for owned up
        // edges, explicit keyboard input and shutdown.
        if self.commands.capacity() <= 8 {
            return Err(crate::msg!("connection-queue-full"));
        }
        self.enqueue(Command::MouseSample(sample))
    }
    pub fn mouse_enabled(&self, generation: u64, enabled: bool) {
        self.mouse_ui
            .send_modify(|controls| controls.enable(generation, enabled));
    }
    pub fn cancel_mouse(&self, lease: crate::mouse::input::Lease) {
        self.mouse_ui.send_modify(|controls| controls.cancel(lease));
    }
    pub fn revoke_mouse(&self, generation: u64) {
        self.mouse_ui
            .send_modify(|controls| controls.revoke(generation));
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
            expired: crate::msg!("login-send-unconfirmed"),
            closed: crate::msg!("login-service-unavailable"),
        })
    }
    pub fn cancel_login(&self) -> Result<(), String> {
        self.enqueue(Command::CancelLogin)
    }
    /// Acknowledges starting one workflow. Only View::destination reports arrival.
    pub fn request_switch_board(&self, stamp: Stamp, board: BoardName) -> Result<Reply, String> {
        self.request_go_to(stamp, Destination::Board { board })
    }
    pub fn request_open_article(
        &self,
        stamp: Stamp,
        target: crate::article_target::ArticleTarget,
    ) -> Result<Reply, String> {
        self.request_go_to(stamp, Destination::Article(target))
    }
    pub fn request_go_to(&self, stamp: Stamp, destination: Destination) -> Result<Reply, String> {
        let (reply, answer) = oneshot::channel();
        self.enqueue(Command::GoTo {
            stamp,
            destination,
            reply,
        })?;
        Ok(Reply {
            answer,
            deadline: tokio::time::Instant::now() + Duration::from_secs(6),
            expired: crate::msg!("board-switch-start-timeout"),
            closed: crate::msg!("navigation-service-unavailable"),
        })
    }
    pub fn cancel_navigation(&self, operation: Stamp) -> Result<(), String> {
        self.enqueue(Command::CancelNavigation { operation })
    }
    pub async fn navigate(&self, stamp: Stamp, intent: Intent) -> Result<(), String> {
        self.request_navigate(stamp, intent, navigation::Source::Pointer)?
            .wait()
            .await
    }
    pub async fn browse_key(&self, stamp: Stamp, intent: Intent) -> Result<(), String> {
        self.request_browse_key(stamp, intent)?.wait().await
    }
    pub fn request_browse_key(&self, stamp: Stamp, intent: Intent) -> Result<Reply, String> {
        self.request_navigate(stamp, intent, navigation::Source::Keyboard)
    }
    pub fn request_navigate(
        &self,
        stamp: Stamp,
        intent: Intent,
        source: navigation::Source,
    ) -> Result<Reply, String> {
        let (reply, answer) = oneshot::channel();
        self.enqueue(Command::Navigate {
            stamp,
            intent,
            source,
            reply,
        })?;
        Ok(Reply {
            answer,
            deadline: tokio::time::Instant::now() + Duration::from_secs(6),
            expired: crate::msg!("navigation-timeout"),
            closed: crate::msg!("navigation-service-unavailable"),
        })
    }
    fn enqueue(&self, command: Command) -> Result<(), String> {
        self.commands
            .try_send(command)
            .map_err(|_| crate::msg!("connection-queue-full"))
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
                .and_then(|result| {
                    result
                        .clone()
                        .ok_or(crate::msg!("connection-cleanup-unconfirmed").to_owned())?
                });
        }
        let (done, receiver) = oneshot::channel();
        // Keep the enqueue alive if the bounded queue is full: canceling it on
        // a deadline would leave the worker without a shutdown request.
        let outcome = match self.commands.send(Command::Shutdown(done)).await {
            Ok(()) => timeout(Duration::from_secs(5), receiver)
                .await
                .map_err(|_| crate::msg!("connection-cleanup-timeout").to_owned())
                .and_then(|result| {
                    result.map_err(|_| crate::msg!("connection-cleanup-incomplete").to_owned())
                }),
            Err(_) => Err(crate::msg!("connection-service-stopped").to_owned()),
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
                errors.push(crate::msg!(
                    "connection-task-failed",
                    error = error.to_string()
                ));
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
mod mouse_control;
mod output;
mod probe;
#[cfg(test)]
mod tests;
mod transport;
mod worker;

/// Explicit no-credential network probes, separate from ordinary gates.
pub async fn probe_live() -> Result<serde_json::Value, String> {
    probe_prelogin(crate::sites::Target::default(), true).await
}
pub async fn probe_site(target: crate::sites::Target) -> Result<serde_json::Value, String> {
    probe_prelogin(target, false).await
}
async fn probe_prelogin(
    target: crate::sites::Target,
    require_ptt_prompt: bool,
) -> Result<serde_json::Value, String> {
    let connection = Connection::start(Screen::new(80, 24).map_err(str::to_owned)?)?;
    let mut updates = connection.updates.clone();
    connection.connect_to(target.clone())?;
    let observed = timeout(Duration::from_secs(20), async {
        let mut started = false;
        loop {
            updates.changed().await.map_err(|e| e.to_string())?;
            let view = updates.borrow_and_update().clone();
            started |= matches!(view.phase, Phase::Connecting | Phase::Connected);
            if view.phase == Phase::Error || started && view.phase == Phase::Disconnected {
                return Err(view.message);
            }
            let visible = view.frame.glyphs.iter().any(|glyph| !glyph.text.trim().is_empty());
            if visible && !view.output_pending {
                let prompt = view.frame.glyphs.iter()
                    .map(|glyph| glyph.text.as_str()).collect::<String>()
                    .contains("請輸入代號");
                if !require_ptt_prompt || prompt {
                    return Ok(serde_json::json!({
                        "status": "passed", "endpoint": target.endpoint, "encoding": target.encoding,
                        "received_bytes": view.received,
                        "unmapped_glyphs": view.frame.glyphs.iter().filter(|glyph| glyph.unmapped).count(),
                        "login_prompt_detected": prompt, "credential_input_sent": false,
                        "scope": "transport banner only; no authentication or raw traffic saved"
                    }));
                }
            }
        }
    })
    .await
    .map_err(|_| "No pre-login response within the deadline; no credentials were sent".to_owned())
    .and_then(|result| result);
    let close = connection.shutdown().await?;
    let mut report = observed?;
    report["disconnect"] = serde_json::json!(close);
    if !matches!(
        close,
        CloseOutcome::Acknowledged | CloseOutcome::TransportClosed
    ) {
        report["status"] = "failed".into();
    }
    Ok(report)
}
