//! One native-preview lifetime. Source HTTP and coordination share one sleeping
//! runtime thread; the existing libmpv controller owns normal API calls.
//! The UI transfers the single render-core reference and retires it in GL
//! context before stopping/joining this session. Snapshots never own the core.
use super::{
    control::{Command, Control, Status},
    mpv::{Client, Layout},
};
use crate::media::video_stream::{Report as StreamReport, Stream};
use serde::Serialize;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};
use tokio::sync::{Notify, mpsc, oneshot, watch};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Stage {
    Starting,
    Ready,
    Stopped,
}
#[derive(Clone, Default, Debug, Serialize)]
pub struct Report {
    pub stream: Option<StreamReport>,
    pub control_joined: Option<bool>,
    pub core_destroyed: bool,
    pub error: Option<String>,
}
#[derive(Clone)]
pub struct Snapshot {
    pub stage: Stage,
    pub status: Status,
    pub report: Option<Report>,
    pub error: Option<String>,
}
struct Gate {
    closed: AtomicBool,
    render: AtomicBool,
    permit: AtomicBool,
    wake: Notify,
}
#[derive(Clone)]
pub struct RenderGate(Arc<Gate>);
impl RenderGate {
    /// A readiness fence, not a GL operation. Set only after render setup;
    /// clear only after the context and all transferred core refs are retired.
    pub fn ready(&self, value: bool) {
        self.0.render.store(value, Ordering::Release);
        self.0.wake.notify_one();
    }
}
pub struct Session {
    send: mpsc::Sender<Command>,
    gate: Arc<Gate>,
    core: Arc<Mutex<Option<Arc<Client>>>>,
    pub status: Arc<Mutex<Status>>,
    updates: watch::Receiver<Snapshot>,
    join: Option<JoinHandle<Report>>,
}
impl Session {
    pub fn start(
        url: String,
        layout: Layout,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        if !crate::media::classify(&url)?.is_some_and(|t| t.kind == crate::media::Kind::Video) {
            return Err("native session requires a direct video URL".into());
        }
        let (send, receive) = mpsc::channel(8);
        let status = Arc::new(Mutex::new(Status {
            paused: true,
            ..Default::default()
        }));
        let (output, updates) = watch::channel(Snapshot {
            stage: Stage::Starting,
            status: Status {
                paused: true,
                ..Default::default()
            },
            report: None,
            error: None,
        });
        let gate = Arc::new(Gate {
            closed: AtomicBool::new(false),
            render: AtomicBool::new(false),
            permit: AtomicBool::new(true),
            wake: Notify::new(),
        });
        let core = Arc::new(Mutex::new(None));
        let (owned, slot, state) = (gate.clone(), core.clone(), status.clone());
        let join = std::thread::Builder::new()
            .name("slint-video-stream".into())
            .spawn(move || {
                let result = (|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| e.to_string())?;
                    let result = runtime.block_on(run(
                        url,
                        layout,
                        receive,
                        WorkerContext {
                            gate: owned,
                            slot,
                            status: state.clone(),
                        },
                        &output,
                        notify.clone(),
                    ));
                    drop(runtime);
                    result
                })();
                let report = result.unwrap_or_else(|error| Report {
                    core_destroyed: true,
                    error: Some(error),
                    ..Default::default()
                });
                let final_status = state.lock().map(|v| v.clone()).unwrap_or_default();
                output.send_replace(Snapshot {
                    stage: Stage::Stopped,
                    status: final_status,
                    report: Some(report.clone()),
                    error: report.error.clone(),
                });
                notify();
                report
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            send,
            gate,
            core,
            status,
            updates,
            join: Some(join),
        })
    }
    pub fn updates(&self) -> watch::Receiver<Snapshot> {
        self.updates.clone()
    }
    pub fn render_gate(&self) -> RenderGate {
        RenderGate(self.gate.clone())
    }
    pub fn take_core(&self) -> Result<Option<Arc<Client>>, String> {
        self.core
            .lock()
            .map(|mut v| v.take())
            .map_err(|_| "native render transfer poisoned".into())
    }
    /// Used by the standalone probe before showing its window; UI integrations
    /// can await the same watch and transfer via take_core without blocking.
    pub async fn wait_core(&mut self) -> Result<Arc<Client>, String> {
        {
            let state = self
                .updates
                .wait_for(|s| s.stage != Stage::Starting)
                .await
                .map_err(|_| "native session setup ended")?;
            if state.stage != Stage::Ready {
                return Err(state
                    .error
                    .clone()
                    .unwrap_or_else(|| "native session stopped during setup".into()));
            }
        }
        self.take_core()?
            .ok_or_else(|| "native render core already transferred".into())
    }
    pub fn command(&self, command: Command) -> Result<(), String> {
        if self.gate.closed.load(Ordering::Acquire) {
            return Err("native session closed".into());
        }
        if matches!(command, Command::Play)
            && (!self.gate.render.load(Ordering::Acquire)
                || !self.gate.permit.load(Ordering::Acquire))
        {
            return Err("native video is not ready to play".into());
        }
        if matches!(command, Command::Stop | Command::Wake) {
            return Err("reserved native session command".into());
        }
        self.send
            .try_send(command)
            .map_err(|_| "native video command queue full".into())
    }
    pub fn permit(&self, permit: bool) {
        self.gate.permit.store(permit, Ordering::Release);
        self.gate.wake.notify_one();
    }
    pub fn stop(&self) {
        self.gate.closed.store(true, Ordering::Release);
        self.gate.wake.notify_one();
    }
    /// Call after the render gate is retired. A UI awaits Stage::Stopped before
    /// joining: network, libmpv and runtime work are finished before that signal.
    pub fn finish(&mut self) -> Result<Report, String> {
        if self.gate.render.load(Ordering::Acquire) {
            return Err("native render context must be retired before join".into());
        }
        self.stop();
        match self.join.take() {
            Some(join) => join
                .join()
                .map_err(|_| "native video worker panicked".into()),
            None => Err("native video already joined".into()),
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
struct WorkerContext {
    gate: Arc<Gate>,
    slot: Arc<Mutex<Option<Arc<Client>>>>,
    status: Arc<Mutex<Status>>,
}
async fn run(
    url: String,
    layout: Layout,
    mut receive: mpsc::Receiver<Command>,
    context: WorkerContext,
    output: &watch::Sender<Snapshot>,
    notify: Arc<dyn Fn() + Send + Sync>,
) -> Result<Report, String> {
    let WorkerContext { gate, slot, status } = context;
    if gate.closed.load(Ordering::Acquire) {
        return Ok(Report {
            core_destroyed: true,
            ..Default::default()
        });
    }
    let source = Stream::bind(&url).await?;
    let endpoint = source.url().to_owned();
    let (stop, closed) = oneshot::channel();
    let transfer = source.serve(async {
        let _ = closed.await;
    });
    tokio::pin!(transfer);
    let dirty = Arc::new(Notify::new());
    let wake = dirty.clone();
    let mut control = Control::start(endpoint, Arc::new(move || wake.notify_one()), layout)?;
    let lifetime = Arc::downgrade(&control.core);
    let mut error = None;
    let registered = match slot.lock() {
        Ok(mut slot) => {
            *slot = Some(control.core.clone());
            true
        }
        Err(_) => {
            error = Some("native core transfer poisoned".into());
            false
        }
    };
    let initial = match status.lock() {
        Ok(value) => value.clone(),
        Err(_) => {
            error = Some("native status poisoned".into());
            Status::default()
        }
    };
    let initialized = registered && error.is_none();
    if initialized {
        output.send_replace(Snapshot {
            stage: Stage::Ready,
            status: initial,
            report: None,
            error: None,
        });
        notify();
    }
    let (mut last_permit, mut stream_done) = (true, None);
    while initialized && !gate.closed.load(Ordering::Acquire) {
        let permit = gate.permit.load(Ordering::Acquire);
        if !permit
            && last_permit
            && let Err(e) = control.command(Command::Pause)
        {
            error = Some(e);
        }
        last_permit = permit;
        tokio::select! {biased;
            _=gate.wake.notified()=>{},
            value=receive.recv()=>{
                let Some(command)=value else{break;};
                if (!matches!(command,Command::Play)||gate.render.load(Ordering::Acquire)&&gate.permit.load(Ordering::Acquire))
                    && let Err(e)=control.command(command){error=Some(e);}
            },
            _=dirty.notified()=>{
                let value=match control.status.lock(){Ok(value)=>value.clone(),Err(_)=>{error=Some("native control status poisoned".into());break;}};
                match status.lock(){Ok(mut status)=>*status=value.clone(),Err(_)=>{error=Some("native status poisoned".into());break;}}
                output.send_replace(Snapshot {stage:Stage::Ready,error:error.clone().or_else(||value.error.clone()),status:value,report:None});notify();
            },
            report=&mut transfer=>{stream_done=Some(report);error=Some("native stream ended unexpectedly".into());break;}
        }
    }
    match slot.lock() {
        Ok(mut slot) => {
            slot.take();
        }
        Err(poisoned) => {
            poisoned.into_inner().take();
            error = Some("native core transfer poisoned".into());
        }
    }
    let _ = stop.send(());
    let stream = match stream_done {
        Some(report) => report,
        None => transfer.await,
    };
    let joined = control.finish();
    if let Err(e) = &joined {
        error = Some(e.clone());
    }
    drop(control);
    let core_destroyed = lifetime.upgrade().is_none();
    if !core_destroyed {
        error = Some("native render core retained after session shutdown".into());
    }
    Ok(Report {
        stream: Some(stream),
        control_joined: Some(joined.is_ok()),
        core_destroyed,
        error,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn ready_does_not_fetch_and_unrendered_play_is_rejected() {
        let file = tempfile::Builder::new().suffix(".mp4").tempfile().unwrap();
        std::fs::write(file.path(), b"synthetic not played").unwrap();
        let mut source = crate::media::probe_server::Server::start(file.path(), false).unwrap();
        let mut session = Session::start(source.url(), Layout::VideoOnly, Arc::new(|| {})).unwrap();
        let core = tokio::time::timeout(std::time::Duration::from_secs(5), session.wait_core())
            .await
            .unwrap()
            .unwrap();
        assert!(session.command(Command::Play).is_err());
        assert_eq!(source.requests.load(Ordering::Acquire), 0);
        drop(core);
        session.stop();
        let mut updates = session.updates();
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            updates.wait_for(|s| s.stage == Stage::Stopped),
        )
        .await
        .unwrap()
        .unwrap();
        let report = session.finish().unwrap();
        assert!(
            report.core_destroyed
                && report.control_joined == Some(true)
                && report.stream.unwrap().stopped
        );
        assert!(source.finish().unwrap().joined);
    }
    #[test]
    fn cancelled_startup_joins_without_network_or_unclaimed_core() {
        let mut session = Session::start(
            "http://127.0.0.1:9/not-played.mp4".into(),
            Layout::VideoOnly,
            Arc::new(|| {}),
        )
        .unwrap();
        let report = session.finish().unwrap();
        assert!(report.core_destroyed);
        assert!(session.take_core().unwrap().is_none());
    }
}
