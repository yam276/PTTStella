//! The single session owner. Event polling stays separate from state transitions.
use super::{
    CloseOutcome, Command, Dial, Phase, Socket, View, liveness, mouse_control, output, transport,
};
use crate::{
    login::{self, Credentials, Prompt},
    navigation::{self, Context, Intent, Stamp},
    navigation_job::{self, Destination, Flow, Progress as NavigationProgress},
    screen::Screen,
    telnet,
};
use std::{
    future::pending,
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, watch},
    time::{MissedTickBehavior, interval, timeout},
};
use tokio_tungstenite::tungstenite::{Error as SocketError, Message};

enum PendingInput {
    Mouse {
        revision: u64,
        deadline: Instant,
    },
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
            Self::Manual { revision, .. }
            | Self::Refresh { revision, .. }
            | Self::Mouse { revision, .. } => *revision,
        }
    }
    fn expired(&self, now: Instant) -> bool {
        match self {
            Self::Manual { deadline, .. }
            | Self::Refresh { deadline, .. }
            | Self::Mouse { deadline, .. } => now >= *deadline,
        }
    }
}

struct LoginAttempt {
    credentials: Credentials,
    deadline: Instant,
}

struct NavigationJob {
    flow: Flow,
    operation: Stamp,
    serial: u64,
    epoch: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputEffect {
    Command,
    PointerMotion,
}

struct Session {
    mouse: crate::mouse::input::State,
    screen: Screen,
    endpoint: String,
    socket: Option<Socket>,
    dialing: Option<Dial>,
    framing: telnet::Framing,
    view: View,
    updates: watch::Sender<View>,
    dirty: bool,
    pending_input: Option<PendingInput>,
    login_attempt: Option<LoginAttempt>,
    login_followup: Option<login::FollowUp>,
    navigation_job: Option<NavigationJob>,
    navigation_serial: u64,
    visits: crate::recent_boards::Visits,
    visit_serial: u64,
    last_close: CloseOutcome,
    idle: liveness::Idle,
    timing: liveness::Timing,
    output: output::Deadline,
}

pub(super) async fn run(
    screen: Screen,
    endpoint: String,
    mut commands: mpsc::Receiver<Command>,
    updates: watch::Sender<View>,
    timing: liveness::Timing,
    mut gates: watch::Receiver<Option<(Stamp, bool)>>,
    mut mouse_gates: watch::Receiver<mouse_control::Controls>,
) {
    let mut session = Session::new(screen, endpoint, updates, timing);
    let mut clock = interval(Duration::from_millis(16));
    let mut idle_clock = interval(timing.poll);
    idle_clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            command = commands.recv() => {
                if matches!(mouse_gates.has_changed(),Ok(true)) {session.mouse_controls(*mouse_gates.borrow_and_update());}
                if session.command(command).await { break; }
            }
            changed=mouse_gates.changed()=>{
                if changed.is_err() {session.stop().await;break;}
                session.mouse_controls(*mouse_gates.borrow_and_update());
            }
            changed = gates.changed() => {
                if changed.is_err() { session.stop().await; break; }
                if let Some((stamp, ready)) = *gates.borrow_and_update() {
                    session.gate(stamp, ready);
                }
            }
            result = async {
                match session.dialing.as_mut() { Some(task) => task.await, None => pending().await }
            } => session.connected(result),
            incoming = async {
                match session.socket.as_mut() { Some(socket) => socket.next().await, None => pending().await }
            } => session.receive(incoming).await,
            _ = clock.tick(), if session.dirty || session.output.active() || session.navigation_job.is_some() => {
                if session.output.expire(Instant::now()) {
                    session.screen.release_synchronized_output();
                    session.continue_login().await;
                    session.dirty = true;
                }
                if let Some(message) = session.navigation_job.as_ref().filter(|job|job.flow.expired(Instant::now())).map(|job|job.flow.timeout_message()) {
                    session.stop_navigation(message);
                }
                // Automatic steps yield to already queued manual input/UI state.
                if commands.is_empty() && matches!(gates.has_changed(),Ok(false)) {
                    session.advance_navigation(Instant::now()).await;
                }
                if session.dirty { session.publish(); }
            },
            _ = idle_clock.tick() => {
                let now = Instant::now();
                session.idle.tick(now);
                session.expire_login(now);
                // Automatic work yields to queued input/UI state, without a biased select.
                if !commands.is_empty() || !matches!(gates.has_changed(), Ok(false)) { continue; }
                session.refresh(now).await;
            }
        }
        // A response wait cannot outlive its connection or replace its close reason.
        if session.view.phase != Phase::Connected {
            session.mouse.interrupt();
            session.pending_input = None;
            session.login_attempt = None;
            session.login_followup = None;
            session.view.login_guidance = login::Guidance::Idle;
            session.navigation_job = None;
            session.view.destination = NavigationProgress::Idle;
            session.screen.release_synchronized_output();
            session.output.track(None, Instant::now());
        }
    }
}

impl Session {
    fn new(
        screen: Screen,
        endpoint: String,
        updates: watch::Sender<View>,
        timing: liveness::Timing,
    ) -> Self {
        let view = updates.borrow().clone();
        Self {
            mouse: crate::mouse::input::State::default(),
            screen,
            endpoint,
            view,
            updates,
            timing,
            socket: None,
            dialing: None,
            framing: telnet::Framing::default(),
            dirty: false,
            pending_input: None,
            login_attempt: None,
            login_followup: None,
            navigation_job: None,
            navigation_serial: 0,
            visits: crate::recent_boards::Visits::default(),
            visit_serial: 0,
            last_close: CloseOutcome::NoConnection,
            idle: liveness::Idle::new(Instant::now(), timing),
            output: output::Deadline::default(),
        }
    }

    /// The authoritative screen may be newer than the last published View.
    fn stamp(&self) -> Stamp {
        Stamp {
            revision: self.screen.revision(),
            ..self.view.stamp()
        }
    }
    fn mouse_context(&self) -> crate::mouse::input::Context {
        let connected = self.view.phase == Phase::Connected && self.socket.is_some();
        let (cols, rows) = self.screen.dimensions();
        crate::mouse::input::Context {
            stamp: self.stamp(),
            modes: if connected {
                self.screen.mouse_modes()
            } else {
                crate::mouse::Modes::default()
            },
            ready: connected
                && self.screen.control_complete()
                && self.login_attempt.is_none()
                && self.login_followup.is_none()
                && self.navigation_job.is_none()
                && matches!(self.pending_input, None | Some(PendingInput::Mouse { .. })),
            cols,
            rows,
        }
    }
    fn sync_mouse(&mut self) {
        let context = self.mouse_context();
        self.mouse.observe(context, Instant::now());
        self.view.mouse = self.mouse.snapshot(context);
    }
    fn mouse_controls(&mut self, controls: mouse_control::Controls) {
        self.sync_mouse();
        if let Some(preference) = controls
            .preference
            .filter(|p| p.generation == self.view.generation)
        {
            self.mouse.enable(preference.serial, preference.enabled);
        }
        if let Some(revoke) = controls
            .revoke
            .filter(|r| r.generation == self.view.generation)
        {
            self.mouse.revoke(revoke.serial, None);
        }
        if let Some(cancel) = controls
            .cancel
            .filter(|c| c.lease.stamp.generation == self.view.generation)
        {
            self.mouse.revoke(cancel.serial, Some(cancel.lease));
        }
        self.sync_mouse();
        self.dirty = true;
    }
    async fn mouse_report(&mut self, event: crate::mouse::input::Event) {
        // Like manual keys, late pointer events cannot overwrite a close reason.
        if self.view.phase != Phase::Connected
            || self.socket.is_none()
            || event.lease.stamp.generation != self.view.generation
        {
            return;
        }
        self.clear_answered_input();
        match self
            .mouse
            .accept(self.mouse_context(), event, Instant::now())
        {
            Ok(bytes) => {
                self.idle.activity(Instant::now());
                self.sync_mouse();
                if self.write_input(&bytes).await.is_ok()
                    && event.report.action == crate::mouse::Action::Press
                {
                    // The station discards KEY_MOUSE_RELEASE; it does not owe
                    // a redraw for an up edge. Preserve, never extend, a press wait.
                    self.pending_input = Some(PendingInput::Mouse {
                        revision: self.screen.revision(),
                        deadline: Instant::now() + self.timing.response_timeout,
                    });
                }
            }
            Err(error) => self.view.message = error.message(),
        }
        self.dirty = true;
    }

    async fn mouse_sample(&mut self, sample: crate::mouse::input::Sample) {
        use crate::mouse::input::{SampleOutcome, SampleReport};
        if self.view.phase != Phase::Connected
            || self.socket.is_none()
            || sample.lease.stamp.generation != self.view.generation
        {
            return;
        }
        self.clear_answered_input();
        match self
            .mouse
            .accept_sample(self.mouse_context(), sample, Instant::now())
        {
            Ok(SampleOutcome::Bytes(bytes)) => {
                self.idle.activity(Instant::now());
                self.sync_mouse();
                let effect = match sample.report {
                    SampleReport::Motion(_) => InputEffect::PointerMotion,
                    SampleReport::Wheel(_) => InputEffect::Command,
                };
                if self.send_input(&bytes, effect).await.is_ok()
                    && matches!(sample.report, SampleReport::Wheel(_))
                    && self.pending_input.is_none()
                {
                    // Hover/motion need not redraw. Wheel may wait for a reply,
                    // but neither sample extends an existing press/wheel wait.
                    self.pending_input = Some(PendingInput::Mouse {
                        revision: self.screen.revision(),
                        deadline: Instant::now() + self.timing.response_timeout,
                    });
                }
            }
            Ok(SampleOutcome::SameCell | SampleOutcome::RateLimited) => {}
            Err(error) => self.view.message = error.message(),
        }
        self.dirty = true;
    }

    /// Returns true only when the command closes this worker.
    async fn command(&mut self, command: Option<Command>) -> bool {
        match command {
            Some(Command::Mouse(event)) => self.mouse_report(event).await,
            Some(Command::MouseSample(sample)) => self.mouse_sample(sample).await,
            Some(Command::KeepAlive(enabled)) => self.keep_alive(enabled),
            Some(Command::Connect) => self.connect(),
            Some(Command::SwitchSite(target)) => {
                self.disconnect().await;
                self.endpoint = target.endpoint.as_str().to_owned();
                self.screen.set_encoding(target.encoding);
                self.view.site = target;
                self.keep_alive(self.view.site.ptt_features());
                self.connect();
            }
            Some(Command::Disconnect) => self.disconnect().await,
            Some(Command::Input { generation, bytes }) => self.input(generation, &bytes).await,
            Some(Command::CancelLogin) => {
                self.login_attempt = None;
                self.login_followup = None;
                self.view.login_guidance = login::Guidance::Idle;
                self.view.message = crate::msg!("login-cancelled");
                self.dirty = true;
            }
            Some(Command::GoTo {
                stamp,
                destination,
                reply,
            }) => {
                if !reply.is_closed() {
                    let result = self.go_to(stamp, destination).await;
                    let started = result.is_ok();
                    if reply.send(result).is_err() && started {
                        self.cancel_navigation(stamp);
                    }
                }
            }
            Some(Command::CancelNavigation { operation }) => self.cancel_navigation(operation),
            Some(Command::Login {
                stamp,
                credentials,
                reply,
            }) => {
                if !reply.is_closed() {
                    let result = self.login(stamp, credentials).await;
                    self.dirty = true;
                    let _ = reply.send(result);
                }
            }
            Some(Command::Navigate {
                stamp,
                intent,
                source,
                reply,
            }) => {
                // An expired request must never execute later from the queue.
                if !reply.is_closed() {
                    let _ = reply.send(self.navigate(stamp, intent, source).await);
                }
            }
            Some(Command::Shutdown(done)) => {
                let outcome = self.stop().await;
                let _ = done.send(if outcome == CloseOutcome::NoConnection {
                    self.last_close
                } else {
                    outcome
                });
                return true;
            }
            None => {
                self.stop().await;
                return true;
            }
        }
        false
    }

    fn connect(&mut self) {
        if self.socket.is_some() || self.dialing.is_some() {
            return;
        }
        self.last_close = CloseOutcome::NoConnection;
        self.login_attempt = None;
        self.login_followup = None;
        self.view.login_guidance = login::Guidance::Idle;
        self.navigation_job = None;
        self.view.destination = NavigationProgress::Idle;
        self.view.generation = self.view.generation.saturating_add(1);
        self.view.board_visit = None;
        self.idle.activity(Instant::now());
        self.view.keep_alive_sent = 0;
        self.view.keep_alive_failure = None;
        self.screen.reset_session();
        self.output.track(None, Instant::now());
        self.framing = telnet::Framing::default();
        self.view.received = 0;
        self.pending_input = None;
        self.view.input_epoch = 0;
        self.view.input_pending = false;
        self.view.phase = Phase::Connecting;
        self.sync_mouse();
        self.view.message = if self.view.site.ptt_features() {
            crate::msg!("status-connecting")
        } else {
            crate::msg!(
                "site-connecting",
                address = self.view.site.endpoint.as_str()
            )
        };
        self.dialing = Some(tokio::spawn(transport::dial(self.endpoint.clone())));
        self.dirty = true;
    }

    async fn stop(&mut self) -> CloseOutcome {
        self.mouse.interrupt();
        let context = crate::mouse::input::Context {
            ready: false,
            modes: crate::mouse::Modes::default(),
            ..self.mouse_context()
        };
        self.mouse.observe(context, Instant::now());
        self.view.mouse = self.mouse.snapshot(context);
        self.updates.send_replace(self.view.clone());
        transport::stop(&mut self.socket, &mut self.dialing).await
    }

    async fn disconnect(&mut self) {
        self.login_attempt = None;
        self.login_followup = None;
        self.view.login_guidance = login::Guidance::Idle;
        self.navigation_job = None;
        self.view.destination = NavigationProgress::Idle;
        self.idle.activity(Instant::now());
        self.view.generation = self.view.generation.saturating_add(1);
        self.pending_input = None;
        self.view.input_pending = false;
        self.last_close = self.stop().await;
        self.view.phase = Phase::Disconnected;
        self.view.message = match self.last_close {
            CloseOutcome::Acknowledged => crate::msg!("disconnect-acknowledged"),
            CloseOutcome::TransportClosed => crate::msg!("disconnect-transport-closed"),
            CloseOutcome::PeerClosedWithoutReply => {
                crate::msg!("disconnect-peer-ended")
            }
            CloseOutcome::NoConnection => crate::msg!("disconnect-no-connection"),
            CloseOutcome::Unconfirmed => crate::msg!("disconnect-unconfirmed"),
        };
        self.dirty = true;
    }

    fn connected(&mut self, result: Result<Result<Socket, String>, tokio::task::JoinError>) {
        self.dialing = None;
        match result {
            Ok(Ok(connection)) => {
                self.socket = Some(connection);
                self.view.phase = Phase::Connected;
                self.view.message = if self.view.site.ptt_features() {
                    crate::msg!("status-connected")
                } else {
                    crate::msg!("site-connected", address = self.view.site.endpoint.as_str())
                };
            }
            Ok(Err(error)) => self.fail(error),
            Err(error) => self.fail(crate::msg!(
                "connection-task-failed",
                error = error.to_string()
            )),
        }
        self.dirty = true;
    }

    fn fail(&mut self, error: String) {
        self.mouse.interrupt();
        self.socket = None;
        self.view.phase = Phase::Error;
        self.view.message = error;
    }

    /// Explicit input advances the command epoch once; passive cell motion does
    /// not invalidate the next click on the same painted frame.
    async fn write_input(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.send_input(bytes, InputEffect::Command).await
    }

    /// One serialized socket path, one escaping pass, no retry.
    async fn send_input(&mut self, bytes: &[u8], effect: InputEffect) -> Result<(), String> {
        self.sync_mouse();
        let Some(socket) = self.socket.as_mut() else {
            return Err(navigation::Error::Disconnected.to_string());
        };
        if effect == InputEffect::Command {
            self.view.input_epoch = self.view.input_epoch.saturating_add(1);
            self.view.input_pending = true;
            self.view.keep_alive_ready = false;
        }
        // Publish explicit-command lease invalidation before the write. Passive
        // motion retains the command epoch and cannot steal a following click.
        self.updates.send_replace(self.view.clone());
        match transport::send(socket, Message::Binary(telnet::escape(bytes).into())).await {
            Ok(()) => Ok(()),
            Err(error) => {
                self.fail(error.clone());
                Err(error)
            }
        }
    }

    fn wait_for_input(&mut self, streaming: bool) {
        self.pending_input = Some(PendingInput::Manual {
            revision: self.screen.revision(),
            deadline: Instant::now() + self.timing.response_timeout,
            streaming,
        });
    }

    async fn input(&mut self, generation: u64, bytes: &[u8]) {
        self.login_attempt = None;
        // Preserve the peer/transport's last reason when typing after closure.
        if self.socket.is_none() && self.view.generation > 0 {
            return;
        }
        if generation != self.view.generation || self.socket.is_none() {
            self.view.message = crate::msg!("input-stale");
            self.dirty = true;
            return;
        }
        self.mouse.intervene();
        if self.navigation_job.is_some() {
            self.stop_navigation(crate::msg!("navigation-job-cancelled"));
        }
        self.idle.activity(Instant::now());
        if self.write_input(bytes).await.is_ok() {
            self.wait_for_input(false);
            if bytes.contains(&b'\r')
                && let Some(state) = self
                    .login_followup
                    .as_mut()
                    .and_then(|followup| followup.submitted_code(self.screen.revision()))
            {
                self.show_login_guidance(state);
            }
        }
        self.dirty = true;
    }

    async fn login(&mut self, stamp: Stamp, credentials: Credentials) -> Result<(), String> {
        if !self.view.site.ptt_features() {
            return Err(crate::msg!("site-ptt-only"));
        }
        if self.view.phase != Phase::Connected || self.socket.is_none() || stamp != self.stamp() {
            return Err(crate::msg!("login-stale"));
        }
        if self.login_attempt.is_some()
            || self.navigation_job.is_some()
            || self.pending_input.is_some()
            || login::prompt(&self.screen.frame()) != Some(Prompt::Account)
        {
            return Err(crate::msg!("login-not-account-prompt"));
        }
        self.idle.activity(Instant::now());
        self.mouse.intervene();
        self.write_input(&credentials.account_input()).await?;
        self.login_followup = None;
        self.view.login_guidance = login::Guidance::Idle;
        self.login_attempt = Some(LoginAttempt {
            credentials,
            deadline: Instant::now() + Duration::from_secs(15),
        });
        self.view.message = crate::msg!("login-await-password");
        Ok(())
    }

    async fn go_to(&mut self, stamp: Stamp, target: Destination) -> Result<(), String> {
        if !self.view.site.ptt_features() {
            return Err(crate::msg!("site-ptt-only"));
        }
        if self.view.phase != Phase::Connected || self.socket.is_none() || stamp != self.stamp() {
            return Err(navigation::Error::Stale.to_string());
        }
        if self.navigation_job.is_some()
            || self.login_attempt.is_some()
            || self.login_followup.is_some()
        {
            return Err(crate::msg!("navigation-job-busy"));
        }
        self.clear_answered_input();
        if self.pending_input.is_some() {
            return Err(navigation::Error::AwaitingScreen.to_string());
        }
        let start = Flow::begin(&self.screen.frame(), target.clone(), Instant::now())?;
        self.mouse.intervene();
        self.navigation_serial = self.navigation_serial.saturating_add(1);
        match start {
            navigation_job::Start::Reached(target) => {
                self.reached_destination(self.navigation_serial, target)
            }
            navigation_job::Start::Run { flow, bytes } => {
                self.view.destination = NavigationProgress::Active {
                    serial: self.navigation_serial,
                    target: target.clone(),
                    operation: stamp,
                };
                self.view.message = match &target {
                    Destination::Board { board } => {
                        crate::msg!("board-switch-active", board = board.as_str())
                    }
                    Destination::Article(article) => crate::msg!(
                        "article-open-active",
                        board = article.board().as_str(),
                        aid = article.aid().as_str()
                    ),
                };
                self.navigation_job = Some(NavigationJob {
                    flow,
                    operation: stamp,
                    serial: self.navigation_serial,
                    epoch: self.view.input_epoch,
                });
                self.send_navigation_step(&bytes).await?;
            }
        }
        self.dirty = true;
        Ok(())
    }

    async fn send_navigation_step(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.idle.activity(Instant::now());
        match self.write_input(bytes).await {
            Ok(()) => {
                if let Some(job) = self.navigation_job.as_mut() {
                    job.epoch = self.view.input_epoch;
                }
                self.wait_for_input(false);
                self.dirty = true;
                Ok(())
            }
            Err(error) => {
                self.stop_navigation(error.clone());
                Err(error)
            }
        }
    }

    async fn advance_navigation(&mut self, now: Instant) {
        let Some(job) = self.navigation_job.as_mut() else {
            return;
        };
        if job.operation.generation != self.view.generation || job.epoch != self.view.input_epoch {
            self.stop_navigation(crate::msg!("navigation-job-cancelled"));
            return;
        }
        if self.screen.synchronized_since().is_some()
            || !job.flow.needs_frame(self.screen.revision())
        {
            return;
        }
        let serial = job.serial;
        match job.flow.observe(&self.screen.frame(), now) {
            Ok(navigation_job::Advance::Wait) => {}
            Ok(navigation_job::Advance::Send(bytes)) => {
                let _ = self.send_navigation_step(&bytes).await;
            }
            Ok(navigation_job::Advance::Reached(target)) => {
                self.reached_destination(serial, target)
            }
            Err(error) => self.stop_navigation(error.to_string()),
        }
    }

    fn reached_destination(&mut self, serial: u64, target: Destination) {
        self.navigation_job = None;
        self.view.message = match &target {
            Destination::Board { board } => {
                crate::msg!("board-switch-reached", board = board.as_str())
            }
            Destination::Article(article) => crate::msg!(
                "article-open-reached",
                board = article.board().as_str(),
                aid = article.aid().as_str()
            ),
        };
        self.view.destination = NavigationProgress::Reached { serial, target };
        self.dirty = true;
    }

    fn cancel_navigation(&mut self, operation: Stamp) {
        if self.navigation_job.as_ref().is_some_and(|job| {
            job.operation == operation && operation.generation == self.view.generation
        }) {
            self.stop_navigation(crate::msg!("navigation-job-cancelled"));
        }
    }

    fn stop_navigation(&mut self, message: String) {
        if self.navigation_job.take().is_none() {
            return;
        }
        self.view.destination = NavigationProgress::Idle;
        self.view.message = message;
        if self
            .pending_input
            .as_ref()
            .is_some_and(|pending| pending.expired(Instant::now()))
        {
            self.pending_input = None;
        }
        self.dirty = true;
    }

    async fn navigate(
        &mut self,
        stamp: Stamp,
        intent: Intent,
        source: navigation::Source,
    ) -> Result<(), String> {
        if !self.view.site.ptt_features() {
            return Err(crate::msg!("site-ptt-only"));
        }
        if source == navigation::Source::Pointer
            && self.mouse.snapshot(self.mouse_context()).route != crate::mouse::input::Route::Local
        {
            return Err(crate::msg!("station-mouse-local-blocked"));
        }
        self.login_attempt = None;
        let keyboard = source == navigation::Source::Keyboard;
        if stamp == self.stamp() && self.navigation_job.is_some() {
            self.stop_navigation(crate::msg!("navigation-job-cancelled"));
        }
        let context = navigation::analyze(&self.screen.frame());
        let list = matches!(
            context,
            Context::MainMenu { .. }
                | Context::SystemMenu { .. }
                | Context::DocumentList { .. }
                | Context::BoardList { .. }
                | Context::ArticleList { .. }
                | Context::CommandHelp { .. }
        );
        let list_move = list
            && matches!(
                intent,
                Intent::First
                    | Intent::Last
                    | Intent::NextPage
                    | Intent::PreviousPage
                    | Intent::Lines { .. }
            );
        // Boundary moves can have no redraw. Only explicit keyboard movement
        // may continue, and a forward/back transition gates the next operation.
        let continuation = matches!(intent, Intent::Lines { .. })
            || list_move
            || (list && matches!(intent, Intent::Forward | Intent::Back));
        let streaming = keyboard
            && continuation
            && matches!(
                self.pending_input,
                Some(PendingInput::Manual {
                    streaming: true,
                    ..
                })
            );
        if stamp.generation != self.view.generation
            || self.socket.is_none()
            || self.view.phase != Phase::Connected
        {
            return Err(navigation::Error::Disconnected.to_string());
        }
        if stamp.revision != self.screen.revision()
            || (!streaming && stamp.input_epoch != self.view.input_epoch)
        {
            return Err(navigation::Error::Stale.to_string());
        }
        if !streaming
            && self
                .pending_input
                .as_ref()
                .is_some_and(|p| p.revision() == self.screen.revision())
        {
            return Err(navigation::Error::AwaitingScreen.to_string());
        }
        let bytes = navigation::plan(&context, intent).map_err(|e| e.to_string())?;
        self.mouse.intervene();
        self.dirty = true;
        if bytes.is_empty() {
            return Ok(());
        }
        self.idle.activity(Instant::now());
        let result = self.write_input(&bytes).await;
        if result.is_ok() {
            self.wait_for_input(keyboard && (list_move || matches!(intent, Intent::Lines { .. })));
        }
        self.dirty = true;
        result
    }

    async fn receive(&mut self, incoming: Option<Result<Message, SocketError>>) {
        match incoming {
            Some(Ok(Message::Binary(bytes))) => self.receive_bytes(&bytes).await,
            Some(Ok(Message::Text(_))) => self.fail(crate::msg!("connection-text-frame")),
            Some(Ok(Message::Ping(payload))) => {
                if let Some(socket) = self.socket.as_mut()
                    && let Err(error) = transport::send(socket, Message::Pong(payload)).await
                {
                    self.fail(error);
                }
            }
            Some(Ok(Message::Pong(_))) => {}
            Some(Ok(Message::Close(frame))) => {
                // Reading queued the peer's close acknowledgement; flush it before drop.
                self.last_close = if let Some(mut socket) = self.socket.take() {
                    match timeout(Duration::from_secs(2), socket.flush()).await {
                        Ok(Ok(())) => CloseOutcome::Acknowledged,
                        _ => CloseOutcome::Unconfirmed,
                    }
                } else {
                    CloseOutcome::Unconfirmed
                };
                self.view.phase = Phase::Disconnected;
                self.view.message = match frame {
                    Some(frame) if frame.reason.is_empty() => {
                        crate::msg!("peer-closed-code", value1 = u16::from(frame.code))
                    }
                    Some(frame) => crate::msg!(
                        "peer-closed-reason",
                        value1 = u16::from(frame.code),
                        value2 = frame.reason.to_string()
                    ),
                    None => crate::msg!("peer-closed-no-reason"),
                };
            }
            None => {
                self.socket = None;
                self.view.phase = Phase::Disconnected;
                self.view.message = crate::msg!("peer-ended");
            }
            Some(Err(error)) => self.fail(crate::msg!(
                "connection-interrupted",
                error = error.to_string()
            )),
            Some(Ok(_)) => {}
        }
        self.dirty = true;
    }

    async fn receive_bytes(&mut self, bytes: &[u8]) {
        self.view.received = self.view.received.saturating_add(bytes.len());
        let (display, reply) = self.framing.feed(bytes, 80, 24);
        self.screen.feed(&display);
        self.sync_mouse();
        self.output
            .track(self.screen.synchronized_since(), Instant::now());
        if !display.is_empty() {
            // Revoke leases before awaiting any write and before the UI reads
            // this change. The retained frame may still be visible until flush.
            self.view.output_pending = true;
            if !self.updates.borrow().output_pending {
                let mouse = self.view.mouse;
                self.updates.send_modify(|view| {
                    view.output_pending = true;
                    view.mouse = mouse;
                });
            } else {
                let mouse = self.view.mouse;
                self.updates.send_modify(|view| view.mouse = mouse);
            }
        }
        if !reply.is_empty()
            && let Some(socket) = self.socket.as_mut()
            && let Err(error) = transport::send(socket, Message::Binary(reply.into())).await
        {
            self.fail(error);
        }
        self.continue_login().await;
    }

    async fn continue_login(&mut self) {
        if self.login_attempt.as_ref().is_some_and(|attempt| {
            login::password_prompt(&self.screen.frame(), &attempt.credentials.account)
        }) {
            if let Some(attempt) = self.login_attempt.take() {
                if Instant::now() < attempt.deadline && self.view.phase == Phase::Connected {
                    if self
                        .write_input(&attempt.credentials.password_input())
                        .await
                        .is_ok()
                    {
                        self.login_followup = Some(login::FollowUp::after_password(
                            self.screen.revision(),
                            Instant::now(),
                        ));
                        self.show_login_guidance(login::Guidance::Waiting);
                        self.wait_for_input(false);
                    }
                } else {
                    self.view.message = crate::msg!("login-password-timeout");
                }
            }
        } else if self.login_attempt.is_some()
            && !matches!(
                navigation::analyze(&self.screen.frame()),
                Context::Unknown { .. }
            )
        {
            self.login_attempt = None;
            self.view.message = crate::msg!("login-left-prompt");
        }
        self.observe_login(Instant::now());
    }

    fn show_login_guidance(&mut self, state: login::Guidance) {
        self.view.login_guidance = state;
        if let Some(message) = state.message() {
            self.view.message = message;
        }
        self.dirty = true;
    }

    fn observe_login(&mut self, now: Instant) {
        if let Some(state) = self
            .login_followup
            .as_mut()
            .and_then(|followup| followup.observe(&self.screen.frame(), now))
        {
            if !state.active() {
                self.login_followup = None;
            }
            self.show_login_guidance(state);
        }
    }

    fn gate(&mut self, stamp: Stamp, ready: bool) {
        if stamp.generation == self.view.generation {
            self.idle
                .gate(stamp, ready && stamp == self.stamp(), Instant::now());
            self.dirty = true;
        }
    }

    fn keep_alive(&mut self, enabled: bool) {
        if let Some(failure) = self.view.keep_alive_failure.take()
            && self.view.message == failure
            && self.view.phase == Phase::Connected
        {
            self.view.message = if enabled {
                crate::msg!("status-keepalive-on")
            } else {
                crate::msg!("status-keepalive-off")
            };
        }
        self.idle.enable(enabled, Instant::now());
        self.view.keep_alive = enabled;
        self.dirty = true;
        if matches!(self.pending_input, Some(PendingInput::Refresh { .. })) {
            self.pending_input = None;
        }
    }

    fn clear_answered_input(&mut self) {
        if self.screen.synchronized_since().is_some() {
            return;
        }
        if self
            .pending_input
            .as_ref()
            .is_some_and(|p| p.revision() != self.screen.revision())
        {
            self.pending_input = None;
        }
    }

    fn publish(&mut self) {
        self.clear_answered_input();
        self.sync_mouse();
        self.view.input_pending = (self.pending_input.is_some() || self.navigation_job.is_some())
            && self.view.phase == Phase::Connected;
        self.view.login_pending = self.login_attempt.is_some() || self.login_followup.is_some();
        self.view.output_pending = self.screen.synchronized_since().is_some();
        if !self.view.output_pending {
            self.view.frame = self.screen.frame();
            if self.view.site.ptt_features()
                && self.view.phase == Phase::Connected
                && let Some(board) = self.visits.observe(self.view.generation, &self.view.frame)
            {
                self.visit_serial = self.visit_serial.saturating_add(1);
                self.view.board_visit = Some(crate::recent_boards::BoardVisit {
                    generation: self.view.generation,
                    serial: self.visit_serial,
                    board,
                });
            }
        }
        self.view.keep_alive_ready = self.idle.ready(self.stamp())
            && self.socket.is_some()
            && !self.view.input_pending
            && !self.view.login_pending
            && !self.mouse.active()
            && self.view.context().allows_keepalive();
        self.updates.send_replace(self.view.clone());
        self.dirty = false;
    }

    fn expire_login(&mut self, now: Instant) {
        self.observe_login(now);
        if self
            .login_attempt
            .as_ref()
            .is_some_and(|attempt| now >= attempt.deadline)
        {
            self.login_attempt = None;
            self.view.message = crate::msg!("login-expired");
            self.dirty = true;
        }
    }

    async fn refresh(&mut self, now: Instant) {
        self.clear_answered_input();
        if self.mouse.expire(now) {
            self.dirty = true;
        }
        if self.navigation_job.is_none()
            && self.pending_input.as_ref().is_some_and(|p| p.expired(now))
        {
            if matches!(self.pending_input, Some(PendingInput::Refresh { .. })) {
                self.idle.enable(false, now);
                self.view.keep_alive = false;
                self.view.message = crate::msg!("keepalive-unanswered");
                self.view.keep_alive_failure = Some(self.view.message.clone());
            } else if !matches!(self.pending_input, Some(PendingInput::Mouse { .. })) {
                self.view.message = crate::msg!("input-unanswered");
            }
            self.pending_input = None;
            self.dirty = true;
        }
        let stamp = self.stamp();
        if self.view.site.ptt_features()
            && self.socket.is_some()
            && self.view.phase == Phase::Connected
            && self.pending_input.is_none()
            && self.login_attempt.is_none()
            && self.login_followup.is_none()
            && self.navigation_job.is_none()
            && !self.mouse.active()
            && self.idle.due(now, stamp)
            && navigation::analyze(&self.screen.frame()).allows_keepalive()
        {
            self.idle.activity(now);
            if self.write_input(liveness::REFRESH).await.is_ok() {
                self.pending_input = Some(PendingInput::Refresh {
                    revision: self.screen.revision(),
                    deadline: Instant::now() + self.timing.response_timeout,
                });
                self.view.keep_alive_sent = self.view.keep_alive_sent.saturating_add(1);
            }
            self.dirty = true;
        } else if self.view.keep_alive_ready && !self.idle.ready(stamp) {
            self.dirty = true;
        }
    }
}
