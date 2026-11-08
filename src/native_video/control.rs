//! One sleeping control thread; bounded commands and coalesced UI status.
use super::mpv::Client;
use serde::Serialize;
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
};
#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub loaded: bool,
    pub loading: bool,
    pub paused: bool,
    pub eof: bool,
    pub position: f64,
    pub duration: f64,
    pub audio_channels: f64,
    pub decoder: String,
    pub error: Option<String>,
    pub events: u64,
}
pub enum Command {
    Play,
    Pause,
    Seek(f64),
    Wake,
    Stop,
}
pub struct Control {
    pub core: Arc<Client>,
    pub status: Arc<Mutex<Status>>,
    send: SyncSender<Command>,
    join: Option<JoinHandle<Result<(), String>>>,
}
impl Control {
    pub fn start(
        url: String,
        notify: Arc<dyn Fn() + Send + Sync>,
        layout: super::mpv::Layout,
    ) -> Result<Self, String> {
        let (send, receive) = mpsc::sync_channel(8);
        let (ready, result) = mpsc::sync_channel(1);
        let status = Arc::new(Mutex::new(Status {
            paused: true,
            ..Default::default()
        }));
        let state = status.clone();
        let wake = send.clone();
        let reschedule = send.clone();
        let join = std::thread::Builder::new()
            .name("slint-player-control".into())
            .spawn(move || {
                let core = match Client::new(layout) {
                    Ok(core) => core,
                    Err(e) => {
                        let _ = ready.send(Err(e.clone()));
                        return Err(e);
                    }
                };
                core.set_wakeup(Box::new(move || {
                    let _ = wake.try_send(Command::Wake);
                }))?;
                for name in [
                    c"time-pos",
                    c"duration",
                    c"pause",
                    c"eof-reached",
                    c"audio-params",
                    c"hwdec-current",
                ] {
                    core.observe(name)?;
                }
                ready
                    .send(Ok(core.clone()))
                    .map_err(|_| "UI closed during player setup")?;
                let mut loaded = false;
                let mut started = false;
                let mut last_error = None;
                let mut events = 0u64;
                while let Ok(command) = receive.recv() {
                    let action = match command {
                        Command::Stop => break,
                        Command::Wake => Ok(()),
                        Command::Play => {
                            if !started || last_error.is_some() {
                                last_error = None;
                                started = true;
                                loaded = false;
                                core.command(&["loadfile", &url, "replace"])
                                    .and_then(|_| core.flag(c"pause", false))
                            } else {
                                if core.get_flag(c"eof-reached") == Some(true) {
                                    core.command(&["seek", "0", "absolute+exact"])?;
                                }
                                core.flag(c"pause", false)
                            }
                        }
                        Command::Pause => core.flag(c"pause", true),
                        Command::Seek(seconds) if seconds.is_finite() && seconds >= 0. => {
                            core.command(&["seek", &seconds.to_string(), "absolute+exact"])
                        }
                        Command::Seek(_) => Err("seek must be finite and non-negative".into()),
                    };
                    let mut error = action.err().or(last_error.take());
                    let mut drained = 0;
                    // A bounded drain prevents a malformed event storm monopolizing
                    // the worker. Every wake/command checks all current properties.
                    for _ in 0..64 {
                        let Some((kind, code)) = core.event() else {
                            break;
                        };
                        events = events.saturating_add(1);
                        drained += 1;
                        if kind == mpv_sys::mpv_event_id_MPV_EVENT_FILE_LOADED {
                            loaded = true;
                        }
                        if code < 0 {
                            loaded = false;
                            error = Some(format!("libmpv event error {code}"));
                        }
                    }
                    if drained == 64 {
                        let _ = reschedule.try_send(Command::Wake);
                    }
                    last_error = error.clone();
                    let value = Status {
                        loaded,
                        loading: started && !loaded && error.is_none(),
                        paused: core.get_flag(c"pause").unwrap_or(true),
                        eof: core.get_flag(c"eof-reached").unwrap_or(false),
                        position: core.number(c"time-pos").unwrap_or(0.),
                        duration: core.number(c"duration").unwrap_or(0.),
                        audio_channels: core.number(c"audio-params/channel-count").unwrap_or(0.),
                        decoder: core.text(c"hwdec-current").unwrap_or_default(),
                        error,
                        events,
                    };
                    *state.lock().map_err(|_| "player status poisoned")? = value;
                    notify();
                }
                // Render context is destroyed by Slint before this normal-API stop.
                core.command(&["stop"])?;
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        let core = match result.recv() {
            Ok(Ok(core)) => core,
            value => {
                let reason = value.err().map(|e| e.to_string());
                let failure = match join.join() {
                    Ok(Err(e)) => e,
                    Ok(Ok(())) => reason.unwrap_or_else(|| "player setup ended".into()),
                    Err(_) => "player setup worker panicked".into(),
                };
                return Err(failure);
            }
        };
        Ok(Self {
            core,
            status,
            send,
            join: Some(join),
        })
    }
    pub fn command(&self, command: Command) -> Result<(), String> {
        self.send
            .try_send(command)
            .map_err(|e| format!("player command not queued: {e}"))
    }
    pub fn finish(&mut self) -> Result<(), String> {
        // Called after GL teardown; blocking here cannot hold the render thread.
        if let Some(join) = self.join.take() {
            let _ = self.send.send(Command::Stop);
            join.join()
                .map_err(|_| "player control worker panicked")??;
        }
        Ok(())
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
