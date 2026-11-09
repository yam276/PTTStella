//! Main-window native video lifetime. One session plus one replaceable pending
//! request; GL retirement precedes cancellation/join and late status is scoped.
#[path = "media_exit_probe.rs"]
pub mod exit_probe;
#[path = "media_video_probe.rs"]
pub mod probe;

use crate::{TerminalWindow, media::Media};
use pttstella::{
    msg,
    native_video::{
        Command,
        gl_state::Saved,
        mpv,
        session::{Session, Stage},
        software::{Software, View},
        surface::Surface,
    },
};
use slint::{ComponentHandle, GraphicsAPI, RenderingState};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::Notify;

struct Offer {
    sequence: u64,
    core: Arc<mpv::Client>,
    gate: pttstella::native_video::session::RenderGate,
}
#[derive(Default)]
struct Bridge {
    software: Cell<bool>,
    desired: Cell<Option<u64>>,
    offer: RefCell<Option<Offer>>,
    rendered: Cell<Option<u64>>,
    wake: Arc<Notify>,
    capture: Cell<bool>,
    samples: RefCell<Vec<serde_json::Value>>,
    error: RefCell<Option<String>>,
}
struct Live {
    sequence: u64,
    // Drop/join the CPU renderer before Session drops its normal API owner.
    software: Option<Software>,
    session: Session,
    offered: bool,
    frame_serial: u64,
}
pub struct Video {
    ui: slint::Weak<TerminalWindow>,
    media: Weak<Media>,
    pending: RefCell<Option<(u64, String)>>,
    current: RefCell<Option<Live>>,
    bridge: Rc<Bridge>,
    closed: Cell<bool>,
    done: Notify,
    cleanup_error: RefCell<Option<String>>,
    task: RefCell<Option<slint::JoinHandle<()>>>,
}
impl Video {
    pub fn mount(ui: &TerminalWindow, media: Weak<Media>) -> Result<Rc<Self>, String> {
        let bridge = Rc::new(Bridge::default());
        if ui.get_media_video_software() {
            bridge.software.set(true);
        } else {
            match mount_renderer(ui, bridge.clone()) {
                Ok(()) => {}
                Err(slint::SetRenderingNotifierError::Unsupported) => bridge.software.set(true),
                Err(error) => return Err(error.to_string()),
            }
        }
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            media,
            pending: RefCell::new(None),
            current: RefCell::new(None),
            bridge,
            closed: Cell::new(false),
            done: Notify::new(),
            cleanup_error: RefCell::new(None),
            task: RefCell::new(None),
        });
        ui.on_media_seek({
            let weak = Rc::downgrade(&owner);
            move |seconds| {
                if let Some(owner) = weak.upgrade() {
                    owner.seek(seconds);
                }
            }
        });
        let weak = Rc::downgrade(&owner);
        let task=slint::spawn_local(async move {
            loop {
                let Some(owner)=weak.upgrade() else {break;};
                let mut updates=owner.current.borrow().as_ref().map(|v|v.session.updates());
                if let Some(updates)=&mut updates{updates.borrow_and_update();}
                let mut software_updates=owner.current.borrow().as_ref().and_then(|v|v.software.as_ref()).map(Software::updates);
                if let Some(updates)=&mut software_updates{updates.borrow_and_update();}
                owner.step();
                if owner.closed.get() && owner.current.borrow().is_none() {owner.done.notify_waiters();break;}
                let bridge=owner.bridge.clone();drop(owner);
                tokio::select! {
                    _=bridge.wake.notified()=>{},
                    _=async {if let Some(updates)=&mut updates{let _=updates.changed().await;}else{std::future::pending::<()>().await;}}=>{}
                    _=async {if let Some(updates)=&mut software_updates{let _=updates.changed().await;}else{std::future::pending::<()>().await;}}=>{}
                }
            }
        }).map_err(|e|e.to_string())?;
        owner.task.replace(Some(task));
        Ok(owner)
    }
    pub fn open(&self, sequence: u64, url: String) {
        if self.closed.get() {
            return;
        }
        self.pending.replace(Some((sequence, url)));
        self.bridge.wake.notify_one();
    }
    pub fn close(&self) {
        self.pending.borrow_mut().take();
        self.bridge.desired.set(None);
        self.bridge.wake.notify_one();
        if let Some(ui) = self.ui.upgrade() {
            ui.window().request_redraw();
        }
    }
    pub fn permit(&self, value: bool) {
        if let Some(live) = self.current.borrow().as_ref() {
            live.session.permit(value);
        }
        self.bridge.wake.notify_one();
    }
    pub fn toggle(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        self.command(if ui.get_media_playing() {
            Command::Pause
        } else {
            Command::Play
        });
    }
    fn capture(&self) {
        self.bridge.capture.set(true);
        self.bridge.wake.notify_one();
        if let Some(ui) = self.ui.upgrade() {
            ui.window().request_redraw();
        }
    }
    fn seek(&self, seconds: f32) {
        self.command(Command::Seek(f64::from(seconds)));
    }
    fn command(&self, command: Command) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if !ui.get_media_play_allowed() {
            return;
        }
        let result = self
            .current
            .borrow()
            .as_ref()
            .map(|v| v.session.command(command));
        if let Some(Err(_)) = result {
            ui.set_media_status(msg!("video-play-failed").into());
        }
    }
    fn step(&self) {
        let ui = self.ui.upgrade();
        // Session cleanup must continue after the root component/GL surface
        // has gone. No new offer may be created for an absent window.
        if ui.is_none() {
            self.close();
        }
        let sequence = self.current.borrow().as_ref().map(|v| v.sequence);
        if let Some(sequence) = sequence
            && self
                .pending
                .borrow()
                .as_ref()
                .is_some_and(|p| p.0 == sequence)
            && !self
                .media
                .upgrade()
                .is_some_and(|m| m.video_valid(sequence))
        {
            self.close();
        }
        let wanted = self.pending.borrow().as_ref().map(|v| v.0);
        let mut current = self.current.borrow_mut();
        if let Some(live) = current.as_mut() {
            if wanted != Some(live.sequence) {
                self.bridge.desired.set(None);
                // Unclaimed offers can be dropped here: no render context was
                // created for them. Claimed render references retire in GL.
                self.bridge.offer.borrow_mut().take();
                if let Some(ui) = &ui {
                    ui.window().request_redraw();
                }
                if let Some(software) = &mut live.software {
                    software.stop();
                    if software.updates().borrow().stopped {
                        self.retire_software(live);
                    }
                }
                if self.bridge.rendered.get().is_none() && live.software.is_none() {
                    live.session.stop();
                }
            }
            let state = live.session.updates().borrow().clone();
            if state.stage == Stage::Stopped {
                let report = live.session.finish();
                if let Err(error) = self.record(live.sequence, report)
                    && let Some(ui) = &ui
                {
                    ui.set_input_notice(error.into());
                }
                if wanted == Some(live.sequence) {
                    self.pending.borrow_mut().take();
                }
                current.take();
                self.bridge.wake.notify_one();
                self.done.notify_waiters();
                return;
            }
            if wanted == Some(live.sequence)
                && state.stage == Stage::Ready
                && let Some(ui) = &ui
            {
                if !live.offered {
                    match live.session.take_core() {
                        Ok(Some(core)) => {
                            self.bridge.offer.replace(Some(Offer {
                                sequence: live.sequence,
                                core,
                                gate: live.session.render_gate(),
                            }));
                            self.bridge.desired.set(Some(live.sequence));
                            live.offered = true;
                            ui.window().request_redraw();
                        }
                        _ => ui.set_media_status(msg!("video-play-failed").into()),
                    }
                }
                if self.bridge.software.get() {
                    self.step_software(live, ui);
                }
                ui.set_media_ready(
                    self.bridge.rendered.get() == Some(live.sequence)
                        && state.error.is_none()
                        && self.bridge.error.borrow().is_none(),
                );
                ui.set_media_playing(state.status.loaded && !state.status.paused);
                ui.set_media_motion_label(
                    if state.status.loaded && !state.status.paused {
                        msg!("video-pause")
                    } else {
                        msg!("video-play")
                    }
                    .into(),
                );
                ui.set_media_position(state.status.position as f32);
                ui.set_media_duration(state.status.duration as f32);
                ui.set_media_status(
                    if state.error.is_some() || self.bridge.error.borrow().is_some() {
                        msg!("video-unsupported")
                    } else if state.status.loading {
                        msg!("video-buffering")
                    } else if !state.status.loaded {
                        msg!("video-ready")
                    } else if state.status.paused {
                        msg!("media-paused")
                    } else {
                        msg!("media-playing")
                    }
                    .into(),
                );
                live.session.permit(ui.get_media_play_allowed());
            }
            return;
        }
        let Some(ui) = ui else {
            return;
        };
        let pending = self.pending.borrow().clone();
        if let Some((sequence, url)) = pending {
            let weak = ui.as_weak();
            let notify = Arc::new(move || {
                let _ = weak.upgrade_in_event_loop(|_| {});
            });
            match Session::start(url, mpv::Layout::VideoOnly, notify) {
                Ok(session) => {
                    *current = Some(Live {
                        sequence,
                        session,
                        offered: false,
                        software: None,
                        frame_serial: 0,
                    });
                    self.bridge.wake.notify_one();
                }
                Err(_) => {
                    self.pending.borrow_mut().take();
                    ui.set_media_status(msg!("video-unsupported").into());
                }
            }
        }
    }
    fn step_software(&self, live: &mut Live, ui: &TerminalWindow) {
        ui.set_media_video_software(true);
        if live.software.is_none()
            && let Some(offer) = self.bridge.offer.borrow_mut().take()
        {
            let wake = self.bridge.wake.clone();
            let weak = ui.as_weak();
            let queued = Arc::new(AtomicBool::new(false));
            let notify = Arc::new(move || {
                wake.notify_one();
                if !queued.swap(true, Ordering::AcqRel) {
                    let queued = queued.clone();
                    let _ = weak.upgrade_in_event_loop(move |_| {
                        queued.store(false, Ordering::Release);
                    });
                }
            });
            match Software::start(offer.core, offer.gate, notify) {
                Ok(software) => live.software = Some(software),
                Err(error) => {
                    self.bridge.error.replace(Some(error));
                    self.close();
                }
            }
        }
        let Some(software) = &live.software else {
            return;
        };
        let snapshot = software.updates().borrow().clone();
        if snapshot.stopped {
            if let Some(error) = &snapshot.error {
                self.bridge.error.replace(Some(error.clone()));
            }
            self.retire_software(live);
            self.close();
            return;
        }
        if snapshot.ready {
            self.bridge.rendered.set(Some(live.sequence));
        }
        let bounds = ui.get_media_video_bounds();
        let dpi = ui.window().scale_factor();
        if let Err(error) = software.set_view(View {
            width: (bounds.width * dpi).max(0.) as u32,
            height: (bounds.height * dpi).max(0.) as u32,
            active: ui.get_media_video_visible() && ui.get_media_play_allowed(),
        }) {
            self.bridge.error.replace(Some(error));
            self.close();
            return;
        }
        if let Some(frame) = &snapshot.frame {
            if frame.serial() != live.frame_serial {
                let [width, height] = frame.size();
                let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                    frame.pixels(),
                    width,
                    height,
                );
                ui.set_media_image(slint::Image::from_rgba8_premultiplied(buffer));
                live.frame_serial = frame.serial();
            }
            if self.bridge.capture.replace(false) && self.bridge.samples.borrow().len() < 8 {
                self.bridge
                    .samples
                    .borrow_mut()
                    .push(probe::capture_software(frame));
            }
        }
    }
    fn retire_software(&self, live: &mut Live) {
        if let Some(mut software) = live.software.take() {
            match software.finish() {
                Ok(report) => {
                    eprintln!(
                        "{}",
                        serde_json::json!({"native_video_software":report,"sequence":live.sequence})
                    );
                }
                Err(error) => {
                    self.cleanup_error.replace(Some(error));
                }
            }
        }
        self.bridge.rendered.set(None);
    }
    pub async fn shutdown(&self) -> Result<(), String> {
        self.closed.set(true);
        self.close();
        // Install notification before testing state, avoiding a lost wake.
        loop {
            let wake = self.done.notified();
            if self.current.borrow().is_none() {
                break;
            }
            wake.await;
        }
        self.cleanup_result()
    }
    fn cleanup_result(&self) -> Result<(), String> {
        self.cleanup_error.borrow().clone().map_or(Ok(()), Err)
    }
    fn record(
        &self,
        sequence: u64,
        report: Result<pttstella::native_video::session::Report, String>,
    ) -> Result<(), String> {
        let result = match report {
            Ok(report) => {
                eprintln!(
                    "{}",
                    serde_json::json!({"native_video_session":report,"sequence":sequence})
                );
                if !report.core_destroyed
                    || report.control_joined == Some(false)
                    || report.stream.as_ref().is_some_and(|s| !s.stopped)
                {
                    Err("native video did not complete resource cleanup".into())
                } else {
                    report.error.map_or(Ok(()), Err)
                }
            }
            Err(error) => Err(error),
        };
        if let Err(error) = &result
            && self.cleanup_error.borrow().is_none()
        {
            self.cleanup_error.replace(Some(error.clone()));
        }
        result
    }
    pub fn finish(&self) -> Result<(), String> {
        self.closed.set(true);
        self.close();
        if let Some(live) = self.current.borrow_mut().as_mut()
            && let Some(software) = &live.software
        {
            software.stop();
            if !software.updates().borrow().stopped {
                return Err("software video renderer has not retired".into());
            }
            self.retire_software(live);
        }
        if self.bridge.rendered.get().is_some() {
            return Err("native video renderer has not retired".into());
        }
        self.bridge.offer.borrow_mut().take();
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        if let Some(mut live) = self.current.borrow_mut().take() {
            self.record(live.sequence, live.session.finish())?;
        }
        self.cleanup_result()
    }
}

struct Rendered {
    sequence: u64,
    player: mpv::Render,
    gate: pttstella::native_video::session::RenderGate,
    surface: Option<Surface>,
    dirty: Arc<AtomicBool>,
}
fn mount_renderer(
    ui: &TerminalWindow,
    bridge: Rc<Bridge>,
) -> Result<(), slint::SetRenderingNotifierError> {
    let weak = ui.as_weak();
    let mut gl = None;
    let mut render: Option<Rendered> = None;
    ui.window().set_rendering_notifier(move |state, api| {
        if matches!(state, RenderingState::RenderingSetup) {
            if let GraphicsAPI::NativeOpenGL { get_proc_address } = api {
                // SAFETY: loader/current context are supplied by Slint here.
                gl = Some(unsafe {
                    glow::Context::from_loader_function_cstr(|n| get_proc_address(n))
                });
            } else {
                bridge.software.set(true);
                bridge.wake.notify_one();
            }
            return;
        }
        let Some(gl) = &gl else {
            return;
        };
        let retire = matches!(state, RenderingState::RenderingTeardown)
            || (matches!(state, RenderingState::BeforeRendering)
                && render
                    .as_ref()
                    .is_some_and(|r| Some(r.sequence) != bridge.desired.get()));
        if retire {
            // SAFETY: Slint guarantees the same current context at retirement.
            unsafe {
                let saved = Saved::take(gl).ok();
                if let Some(s) = &saved {
                    s.defaults(gl);
                }
                if let Some(mut owned) = render.take() {
                    if let Some(surface) = owned.surface.take() {
                        surface.destroy(gl);
                    }
                    let gate = owned.gate.clone();
                    drop(owned);
                    gate.ready(false);
                }
                bridge.rendered.set(None);
                bridge.wake.notify_waiters();
                if let Some(saved) = saved {
                    saved.restore(gl);
                }
            }
        }
        if matches!(state, RenderingState::RenderingTeardown) {
            return;
        }
        if matches!(state, RenderingState::BeforeRendering)
            && render.is_none()
            && let Some(offer) = bridge.offer.borrow_mut().take()
        {
            if Some(offer.sequence) != bridge.desired.get() {
                return;
            }
            let GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
                return;
            };
            // SAFETY: current GL; owned render is destroyed above on this context.
            let result = unsafe {
                (|| {
                    let saved = Saved::take(gl)?;
                    saved.defaults(gl);
                    let dirty = Arc::new(AtomicBool::new(true));
                    let flag = dirty.clone();
                    let redraw = weak.clone();
                    bridge.error.borrow_mut().take();
                    let player = mpv::Render::new(
                        offer.core,
                        get_proc_address,
                        Box::new(move || {
                            if flag.swap(true, Ordering::AcqRel) {
                                return;
                            }
                            let _ = redraw.upgrade_in_event_loop(|ui| ui.window().request_redraw());
                        }),
                    );
                    saved.restore(gl);
                    player.map(|player| Rendered {
                        sequence: offer.sequence,
                        player,
                        gate: offer.gate,
                        surface: None,
                        dirty,
                    })
                })()
            };
            match result {
                Ok(owner) => {
                    owner.gate.ready(true);
                    bridge.rendered.set(Some(owner.sequence));
                    render = Some(owner);
                    bridge.wake.notify_one();
                }
                Err(error) => {
                    bridge.error.replace(Some(error));
                    if let Some(ui) = weak.upgrade() {
                        ui.set_media_status(msg!("video-unsupported").into());
                    }
                }
            }
        }
        if !matches!(state, RenderingState::AfterRendering) {
            return;
        }
        let (Some(render), Some(ui)) = (&mut render, weak.upgrade()) else {
            return;
        };
        if !ui.get_media_video_visible() {
            return;
        }
        let rect = ui.get_media_video_bounds();
        let dpi = ui.window().scale_factor();
        let size = ui.window().size();
        let x = (rect.x * dpi).round() as i32;
        let y = (rect.y * dpi).round() as i32;
        let w = (rect.width * dpi).round() as i32;
        let h = (rect.height * dpi).round() as i32;
        if x < 0
            || y < 0
            || w <= 0
            || h <= 0
            || x + w > size.width as i32
            || y + h > size.height as i32
        {
            return;
        }
        // SAFETY: live current GL; rect lies in drawable, owned FBO pixel budget
        // is checked before allocation, all shared Slint state restored below.
        let result = unsafe {
            (|| {
                let saved = Saved::take(gl)?;
                let target = saved.framebuffer();
                saved.defaults(gl);
                let result = (|| {
                    if render.surface.as_ref().is_none_or(|s| s.size() != [w, h]) {
                        if let Some(surface) = render.surface.take() {
                            surface.destroy(gl);
                        }
                        render.surface = Some(Surface::new(gl, w, h)?);
                        render.dirty.store(true, Ordering::Release);
                    }
                    if let Some(surface) = &render.surface {
                        if render.dirty.swap(false, Ordering::AcqRel) {
                            surface.draw(gl, &render.player)?;
                        }
                        surface.blit(
                            gl,
                            target,
                            [x, size.height as i32 - y - h, x + w, size.height as i32 - y],
                        );
                    }
                    if bridge.capture.replace(false) && bridge.samples.borrow().len() < 8 {
                        bridge.samples.borrow_mut().push(probe::capture(
                            gl,
                            target,
                            [x, y, w, h],
                            size.height as i32,
                        )?);
                        bridge.wake.notify_one();
                    }
                    Ok::<(), String>(())
                })();
                saved.restore(gl);
                result
            })()
        };
        if let Err(error) = result {
            bridge.error.replace(Some(error));
            ui.set_media_status(msg!("video-unsupported").into());
            ui.set_media_ready(false);
        }
    })
}
