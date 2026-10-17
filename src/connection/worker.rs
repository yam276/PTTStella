//! The single session owner. Event polling stays separate from state transitions.
use super::{CloseOutcome, Command, Dial, Phase, Socket, View, liveness, transport};
use crate::{
    login::{self, Credentials, Prompt},
    navigation::{self, Context, Intent, Stamp},
    screen::Screen,
    telnet,
};
use futures_util::{SinkExt, StreamExt};
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

struct LoginAttempt {
    credentials: Credentials,
    deadline: Instant,
}

struct Session {
    screen: Screen,
    endpoint: String,
    socket: Option<Socket>,
    dialing: Option<Dial>,
    framing: telnet::Framing,
    view: View,
    dirty: bool,
    pending_input: Option<PendingInput>,
    login_attempt: Option<LoginAttempt>,
    last_close: CloseOutcome,
    idle: liveness::Idle,
    timing: liveness::Timing,
}

pub(super) async fn run(
    screen: Screen,
    endpoint: String,
    mut commands: mpsc::Receiver<Command>,
    updates: watch::Sender<View>,
    timing: liveness::Timing,
    mut gates: watch::Receiver<Option<(Stamp, bool)>>,
) {
    let mut session = Session::new(screen, endpoint, updates.borrow().clone(), timing);
    let mut clock = interval(Duration::from_millis(16));
    let mut idle_clock = interval(timing.poll);
    idle_clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            command = commands.recv() => {
                if session.command(command).await { break; }
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
            _ = clock.tick(), if session.dirty => session.publish(&updates),
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
            session.pending_input = None;
            session.login_attempt = None;
        }
    }
}

impl Session {
    fn new(screen: Screen, endpoint: String, view: View, timing: liveness::Timing) -> Self {
        Self {
            screen,
            endpoint,
            view,
            timing,
            socket: None,
            dialing: None,
            framing: telnet::Framing::default(),
            dirty: false,
            pending_input: None,
            login_attempt: None,
            last_close: CloseOutcome::NoConnection,
            idle: liveness::Idle::new(Instant::now(), timing),
        }
    }

    /// The authoritative screen may be newer than the last published View.
    fn stamp(&self) -> Stamp {
        Stamp {
            revision: self.screen.revision(),
            ..self.view.stamp()
        }
    }

    /// Returns true only when the command closes this worker.
    async fn command(&mut self, command: Option<Command>) -> bool {
        match command {
            Some(Command::KeepAlive(enabled)) => self.keep_alive(enabled),
            Some(Command::Connect) => self.connect(),
            Some(Command::Disconnect) => self.disconnect().await,
            Some(Command::Input { generation, bytes }) => self.input(generation, &bytes).await,
            Some(Command::CancelLogin) => {
                self.login_attempt = None;
                self.view.message = crate::msg!("login-cancelled");
                self.dirty = true;
            }
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
                keyboard,
                reply,
            }) => {
                self.login_attempt = None;
                // An expired request must never execute later from the queue.
                if !reply.is_closed() {
                    let _ = reply.send(self.navigate(stamp, intent, keyboard).await);
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
        self.view.generation = self.view.generation.saturating_add(1);
        self.idle.activity(Instant::now());
        self.view.keep_alive_sent = 0;
        self.view.keep_alive_failure = None;
        self.screen.reset_session();
        self.framing = telnet::Framing::default();
        self.view.received = 0;
        self.pending_input = None;
        self.view.input_epoch = 0;
        self.view.input_pending = false;
        self.view.phase = Phase::Connecting;
        self.view.message = crate::msg!("status-connecting");
        self.dialing = Some(tokio::spawn(transport::dial(self.endpoint.clone())));
        self.dirty = true;
    }

    async fn stop(&mut self) -> CloseOutcome {
        transport::stop(&mut self.socket, &mut self.dialing).await
    }

    async fn disconnect(&mut self) {
        self.login_attempt = None;
        self.idle.activity(Instant::now());
        self.view.generation = self.view.generation.saturating_add(1);
        self.pending_input = None;
        self.view.input_pending = false;
        self.last_close = self.stop().await;
        self.view.phase = Phase::Disconnected;
        self.view.message = match self.last_close {
            CloseOutcome::Acknowledged => crate::msg!("disconnect-acknowledged"),
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
                self.view.message = crate::msg!("status-connected");
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
        self.socket = None;
        self.view.phase = Phase::Error;
        self.view.message = error;
    }

    /// The only path for application input: increment epoch once, escape once, never retry.
    async fn write_input(&mut self, bytes: &[u8]) -> Result<(), String> {
        let Some(socket) = self.socket.as_mut() else {
            return Err(navigation::Error::Disconnected.to_string());
        };
        self.view.input_epoch = self.view.input_epoch.saturating_add(1);
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
        self.idle.activity(Instant::now());
        if self.write_input(bytes).await.is_ok() {
            self.wait_for_input(false);
        }
        self.dirty = true;
    }

    async fn login(&mut self, stamp: Stamp, credentials: Credentials) -> Result<(), String> {
        if self.view.phase != Phase::Connected || self.socket.is_none() || stamp != self.stamp() {
            return Err(crate::msg!("login-stale"));
        }
        if self.login_attempt.is_some()
            || self.pending_input.is_some()
            || login::prompt(&self.screen.frame()) != Some(Prompt::Account)
        {
            return Err(crate::msg!("login-not-account-prompt"));
        }
        self.idle.activity(Instant::now());
        self.write_input(&credentials.account_input()).await?;
        self.login_attempt = Some(LoginAttempt {
            credentials,
            deadline: Instant::now() + Duration::from_secs(15),
        });
        self.view.message = crate::msg!("login-await-password");
        Ok(())
    }

    async fn navigate(
        &mut self,
        stamp: Stamp,
        intent: Intent,
        keyboard: bool,
    ) -> Result<(), String> {
        let context = navigation::analyze(&self.screen.frame());
        let list = matches!(
            context,
            Context::MainMenu { .. } | Context::BoardList { .. } | Context::ArticleList { .. }
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
                if let Some(mut socket) = self.socket.take() {
                    let _ = timeout(Duration::from_secs(2), socket.flush()).await;
                }
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
                        self.view.message = crate::msg!("login-sent");
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
        if self
            .pending_input
            .as_ref()
            .is_some_and(|p| p.revision() != self.screen.revision())
        {
            self.pending_input = None;
        }
    }

    fn publish(&mut self, updates: &watch::Sender<View>) {
        self.clear_answered_input();
        self.view.input_pending =
            self.pending_input.is_some() && self.view.phase == Phase::Connected;
        self.view.login_pending = self.login_attempt.is_some();
        self.view.frame = self.screen.frame();
        self.view.keep_alive_ready = self.idle.ready(self.stamp())
            && self.socket.is_some()
            && !self.view.input_pending
            && !self.view.login_pending
            && navigation::analyze(&self.view.frame).allows_keepalive();
        updates.send_replace(self.view.clone());
        self.dirty = false;
    }

    fn expire_login(&mut self, now: Instant) {
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
        if self.pending_input.as_ref().is_some_and(|p| p.expired(now)) {
            if matches!(self.pending_input, Some(PendingInput::Refresh { .. })) {
                self.idle.enable(false, now);
                self.view.keep_alive = false;
                self.view.message = crate::msg!("keepalive-unanswered");
                self.view.keep_alive_failure = Some(self.view.message.clone());
            } else {
                self.view.message = crate::msg!("input-unanswered");
            }
            self.pending_input = None;
            self.dirty = true;
        }
        let stamp = self.stamp();
        if self.socket.is_some()
            && self.view.phase == Phase::Connected
            && self.pending_input.is_none()
            && self.login_attempt.is_none()
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
