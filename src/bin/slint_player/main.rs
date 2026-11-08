//! Explicit S1 native libmpv/Slint viability window. Loopback fixtures only.
use pttstella::media::probe_server as fixture;
mod pixel_probe;
mod verify;
use glow::HasContext;
use pttstella::native_video::{Command, Status, gl_state, mpv, session::Session};
use slint::{ComponentHandle, GraphicsAPI, RenderingState};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

slint::slint! {
    import {AboutSlint, Button} from "std-widgets.slint";
    export component PlayerWindow inherits Window {
        title: "PTTStella · Native player probe";
        preferred-width: 960px; preferred-height: 640px; min-width: 640px; min-height: 420px;
        background: transparent;
        in property <string> status: "原生影片驗證 · 尚未載入或播放";
        in property <string> progress;
        in property <string> render-error;
        in property <bool> loaded;
        in property <bool> playing;
        in property <string> pixel-result;
        property <bool> show-about: false;
        callback action(int);
        Rectangle { x:0px;y:0px; width: root.width; height: root.height * 0.10; background: #202733;
            Text {x:16px;y:16px;text:"PTTStella / libmpv · Native OpenGL";color:#e5eaf0;}
            Button {x:root.width - 144px; y:12px; width:128px; text:"關於 Slint"; clicked=>{root.show-about = !root.show-about;}}
        }
        if root.show-about: Rectangle {x:(root.width - self.width)/2; y:root.height*0.12;width:340px;height:180px;background:#202733;
            VerticalLayout {AboutSlint {} Button {text:"關閉關於";clicked=>{root.show-about=false;}}}
        }
        Rectangle { x:0px; y:root.height * 0.80; width:root.width; height:root.height * 0.20; background:#202733;
            VerticalLayout {padding:12px;spacing:5px;
                Text {text:root.render-error == "" ? root.status : root.render-error;color:#e5eaf0;wrap:word-wrap;}
                Text {text:root.progress + " " + root.pixel-result;color:#b6c4d5;}
                HorizontalLayout {spacing:8px;
                    Button {text:root.playing?"暫停":"播放";accessible-label:"播放或暫停影片";clicked=>{root.action(0);}}
                    Button {text:"前往 1 秒";enabled:root.loaded;clicked=>{root.action(1);}}
                    Button {text:"回到開頭";enabled:root.loaded;clicked=>{root.action(2);}}
                    Button {text:"核對四色";enabled:root.loaded;clicked=>{root.action(4);}}
                    Button {text:"關閉";clicked=>{root.action(3);}}
                }
            }
        }
    }
}
#[derive(Default, serde::Serialize)]
struct Metrics {
    setups: usize,
    teardowns: usize,
    frames: usize,
    render_us: Vec<u64>,
    gl_errors: Vec<u32>,
    error: Option<String>,
    gl_version: String,
    gl_renderer: String,
    pixel_samples: Vec<pixel_probe::Sample>,
}
fn source(value: &str) -> Result<String, String> {
    let url = pttstella::media::validate(value)?;
    let local = url.host_str().is_some_and(|host| {
        host.parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
    });
    if !local
        || url.scheme() != "http"
        || url.port().is_none()
        || value.len() > 4096
        || !pttstella::media::classify(value)?
            .is_some_and(|t| t.kind == pttstella::media::Kind::Video)
    {
        return Err("player probe accepts an explicit loopback HTTP video fixture only".into());
    }
    Ok(url.into())
}
fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let first = args
        .next()
        .ok_or("missing loopback URL or --fixture FILE")?;
    let fixture_path = if first == "--fixture" {
        Some(PathBuf::from(args.next().ok_or("missing fixture path")?))
    } else {
        None
    };
    let validated = if fixture_path.is_none() {
        Some(source(&first)?)
    } else {
        None
    };
    let output = PathBuf::from(args.next().ok_or("missing NEW output directory")?);
    let mut stall = false;
    let mut verification = false;
    for argument in args {
        match argument.as_str() {
            "--stall" if fixture_path.is_some() && !stall => stall = true,
            "--verify" if fixture_path.is_some() && !verification => verification = true,
            _ => return Err("unexpected or duplicate argument".into()),
        }
    }
    std::fs::create_dir(&output)?;
    let mut server = fixture_path
        .as_ref()
        .map(|path| fixture::Server::start(path, stall))
        .transpose()?;
    let url = if let Some(server) = &server {
        source(&server.url())?
    } else {
        validated.ok_or("missing source")?
    };
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("skia".into())
        .require_opengl_with_version(3, 3)
        .select()?;
    let ui = PlayerWindow::new()?;
    let requests = server.as_ref().map(|s| s.requests.clone());
    let verification_requests = requests.clone();
    let status_queued = Arc::new(AtomicBool::new(false));
    let status_cell: Arc<std::sync::Mutex<Option<Arc<std::sync::Mutex<Status>>>>> =
        Arc::new(std::sync::Mutex::new(None));
    let notify: Arc<dyn Fn() + Send + Sync> = {
        let weak = ui.as_weak();
        let queued = status_queued.clone();
        let status = status_cell.clone();
        let requests = requests.clone();
        Arc::new(move || {
            if queued.swap(true, Ordering::AcqRel) {
                return;
            }
            let queued = queued.clone();
            let status = status.clone();
            let requests = requests.clone();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                queued.store(false, Ordering::Release);
                let value = status
                    .lock()
                    .ok()
                    .and_then(|s| s.as_ref().and_then(|s| s.lock().ok().map(|s| s.clone())));
                if let Some(value) = value {
                    ui.set_loaded(value.loaded);
                    ui.set_playing(value.loaded && !value.paused);
                    ui.set_status(
                        value
                            .error
                            .unwrap_or_else(|| {
                                if !value.loaded {
                                    return if value.loading {
                                        "載入中"
                                    } else {
                                        "尚未載入或播放"
                                    }
                                    .into();
                                }
                                format!(
                                    "{} · decoder {} · audio channels {}",
                                    if value.paused {
                                        "已暫停"
                                    } else {
                                        "播放中"
                                    },
                                    value.decoder,
                                    value.audio_channels
                                )
                            })
                            .into(),
                    );
                    ui.set_progress(
                        format!(
                            "{:.2} / {:.2} 秒 · HTTP {}",
                            value.position,
                            value.duration,
                            requests
                                .as_ref()
                                .map(|v| v.load(Ordering::Acquire).to_string())
                                .unwrap_or_else(|| "未觀測".into())
                        )
                        .into(),
                    );
                }
            });
        })
    };
    let mut control = Session::start(url, mpv::Layout::ProbeChrome, notify)?;
    let core = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(control.wait_core())?;
    let render_gate = control.render_gate();
    *status_cell
        .lock()
        .map_err(|_| "status registration poisoned")? = Some(control.status.clone());
    let control = Rc::new(RefCell::new(control));
    let closing = Rc::new(Cell::new(false));
    let capture = Rc::new(Cell::new(false));
    ui.on_action({
        let weak = ui.as_weak();
        let control = control.clone();
        let closing = closing.clone();
        let capture = capture.clone();
        move |action| {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let result = match action {
                0 => control.borrow().command(if ui.get_playing() {
                    Command::Pause
                } else {
                    Command::Play
                }),
                1 => control.borrow().command(Command::Seek(1.)),
                2 => control.borrow().command(Command::Seek(0.)),
                3 => {
                    closing.set(true);
                    ui.window().request_redraw();
                    Ok(())
                }
                4 => {
                    capture.set(true);
                    ui.window().request_redraw();
                    Ok(())
                }
                _ => Ok(()),
            };
            if let Err(e) = result {
                ui.set_status(e.into());
            }
        }
    });
    ui.window().on_close_requested({
        let weak = ui.as_weak();
        let closing = closing.clone();
        move || {
            closing.set(true);
            if let Some(ui) = weak.upgrade() {
                ui.window().request_redraw();
            }
            slint::CloseRequestResponse::KeepWindowShown
        }
    });
    let core_lifetime = Arc::downgrade(&core);
    let metrics = Rc::new(RefCell::new(Metrics::default()));
    let frame_queued = Arc::new(AtomicBool::new(false));
    ui.window().set_rendering_notifier({
        let weak = ui.as_weak();
        let core = Arc::downgrade(&core);
        let closing = closing.clone();
        let metrics = metrics.clone();
        let mut render: Option<mpv::Render> = None;
        let mut gl = None;
        move |state, api| {
            match state {
                RenderingState::RenderingSetup => {
                    let GraphicsAPI::NativeOpenGL { get_proc_address } = api else {
                        metrics.borrow_mut().error = Some("native GL API unavailable".into());
                        return;
                    };
                    // SAFETY: Slint supplies the current GL 3.3+ context and its
                    // loader, used only in this notifier on the same UI thread.
                    let context = unsafe {
                        glow::Context::from_loader_function_cstr(|name| get_proc_address(name))
                    };
                    // SAFETY: initialized current GL context, diagnostic strings.
                    metrics.borrow_mut().gl_version =
                        unsafe { context.get_parameter_string(glow::VERSION) };
                    metrics.borrow_mut().gl_renderer =
                        unsafe { context.get_parameter_string(glow::RENDERER) };
                    let saved = match unsafe { gl_state::Saved::take(&context) } {
                        Ok(s) => s,
                        Err(e) => {
                            metrics.borrow_mut().error = Some(e);
                            return;
                        }
                    };
                    unsafe { saved.defaults(&context) };
                    let queued = frame_queued.clone();
                    let redraw = weak.clone();
                    let wake = Box::new(move || {
                        if queued.swap(true, Ordering::AcqRel) {
                            return;
                        }
                        let queued = queued.clone();
                        let _ = redraw.upgrade_in_event_loop(move |ui| {
                            queued.store(false, Ordering::Release);
                            ui.window().request_redraw();
                        });
                    });
                    let result = core
                        .upgrade()
                        .ok_or_else(|| "player core closed before rendering setup".to_owned())
                        .and_then(|core| {
                            // SAFETY: current Slint GL context; this closure frees
                            // the render context in BeforeRendering/Teardown.
                            unsafe { mpv::Render::new(core, get_proc_address, wake) }
                        });
                    unsafe { saved.restore(&context) };
                    match result {
                        Ok(owner) => {
                            render = Some(owner);
                            render_gate.ready(true);
                            gl = Some(context);
                            metrics.borrow_mut().setups += 1;
                        }
                        Err(e) => {
                            metrics.borrow_mut().error = Some(e.clone());
                            let _ =
                                weak.upgrade_in_event_loop(move |ui| ui.set_render_error(e.into()));
                        }
                    }
                }
                RenderingState::BeforeRendering => {
                    if closing.get() {
                        if let Some(context) = &gl {
                            let saved = unsafe { gl_state::Saved::take(context) }.ok();
                            if let Some(s) = &saved {
                                unsafe { s.defaults(context) };
                            }
                            drop(render.take());
                            render_gate.ready(false);
                            if let Some(saved) = saved {
                                unsafe { saved.restore(context) };
                            }
                        }
                        let _ = weak.upgrade_in_event_loop(|ui| {
                            let _ = ui.hide();
                            let _ = slint::quit_event_loop();
                        });
                        return;
                    }
                    if let (Some(owner), Some(gl), Some(ui)) = (&render, &gl, weak.upgrade()) {
                        let saved = match unsafe { gl_state::Saved::take(gl) } {
                            Ok(s) => s,
                            Err(e) => {
                                metrics.borrow_mut().error = Some(e);
                                return;
                            }
                        };
                        let framebuffer = saved.framebuffer();
                        unsafe { saved.defaults(gl) };
                        let size = ui.window().size();
                        let start = Instant::now();
                        let result = unsafe {
                            owner.draw(framebuffer, size.width as i32, size.height as i32)
                        };
                        unsafe { saved.restore(gl) };
                        let mut record = metrics.borrow_mut();
                        record.frames += 1;
                        if record.render_us.len() < 4096 {
                            record
                                .render_us
                                .push(start.elapsed().as_micros().min(u64::MAX as u128) as u64);
                        }
                        if let Err(e) = result {
                            record.error = Some(e.clone());
                            let _ =
                                weak.upgrade_in_event_loop(move |ui| ui.set_render_error(e.into()));
                        }
                        // SAFETY: same current GL context; scalar error query.
                        // SAFETY: AfterRendering still has the current GL context.
                        let error = unsafe { gl.get_error() };
                        if error != glow::NO_ERROR && record.gl_errors.len() < 32 {
                            record.gl_errors.push(error);
                        }
                    }
                }
                RenderingState::RenderingTeardown => {
                    // Slint guarantees the current GL context in this callback.
                    if let Some(context) = &gl {
                        let saved = unsafe { gl_state::Saved::take(context) }.ok();
                        if let Some(s) = &saved {
                            unsafe { s.defaults(context) };
                        }
                        drop(render.take());
                        render_gate.ready(false);
                        if let Some(saved) = saved {
                            unsafe { saved.restore(context) };
                        }
                    }
                    gl = None;
                    metrics.borrow_mut().teardowns += 1;
                }
                RenderingState::AfterRendering if capture.replace(false) && !closing.get() => {
                    if let (Some(gl), Some(ui)) = (&gl, weak.upgrade()) {
                        let mut record = metrics.borrow_mut();
                        if record.pixel_samples.len() >= 8 {
                            return;
                        }
                        let size = ui.window().size();
                        let label = match pixel_probe::capture(gl, size.width, size.height) {
                            Ok(sample) => {
                                let label = if sample.passed {
                                    "四色通過"
                                } else {
                                    "四色不符"
                                };
                                record.pixel_samples.push(sample);
                                label.to_owned()
                            }
                            Err(e) => e,
                        };
                        let error = unsafe { gl.get_error() };
                        if error != glow::NO_ERROR && record.gl_errors.len() < 32 {
                            record.gl_errors.push(error);
                        }
                        let _ =
                            weak.upgrade_in_event_loop(move |ui| ui.set_pixel_result(label.into()));
                    }
                }
                _ => {}
            }
        }
    })?;
    drop(core);
    let probe = if verification {
        Some(verify::Probe::start(
            &ui,
            control.borrow().status.clone(),
            verification_requests.ok_or("verification requires owned HTTP fixture")?,
            metrics.clone(),
            stall,
        ))
    } else {
        None
    };
    ui.run()?;
    let mut verification = probe.as_ref().map(verify::Probe::finish);
    drop(probe);
    // Prove session cancellation before unblocking the stalled test origin.
    let session_report = control.borrow_mut().finish()?;
    if let Some(server) = &server {
        server.cancel()?;
    }
    let status = control
        .borrow()
        .status
        .lock()
        .map_err(|_| "final status poisoned")?
        .clone();
    drop(ui);
    drop(control);
    let core_destroyed = core_lifetime.upgrade().is_none();
    let http = server.as_mut().map(fixture::Server::finish).transpose()?;
    if let Some(report) = &mut verification {
        let render = metrics.borrow();
        if !core_destroyed
            || !session_report.core_destroyed
            || session_report.control_joined != Some(true)
            || session_report.error.is_some()
            || !session_report
                .stream
                .as_ref()
                .is_some_and(|s| s.stopped && s.max_transfer_slice <= 64 * 1024)
            || render.setups != 1
            || render.teardowns != 1
            || render.error.is_some()
            || !render.gl_errors.is_empty()
            || !http
                .as_ref()
                .is_some_and(|h| h.joined && (!stall || h.bytes == 0))
        {
            report.passed = false;
            report.error = Some("final renderer/core/HTTP cleanup or GL checks failed".into());
        } else if report.passed {
            report
                .checks
                .push("render teardown, control/core and HTTP joined; no GL errors");
        }
    }
    std::fs::write(
        output.join("player.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"native GL viability; loopback-only, no production profile or BBS","render":&*metrics.borrow(),"control":status,"control_joined":session_report.control_joined,"native_session":session_report,"core_destroyed":core_destroyed,"http":http,"frame_copies_to_slint":0,"toolkit_verification":verification}),
        )?,
    )?;
    if verification.is_some_and(|report| !report.passed) {
        return Err("native player verification failed; see player.json".into());
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("native player probe: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn probe_cannot_open_external_sites_or_unclassified_urls() {
        assert!(source("http://127.0.0.1:1234/movie.mp4").is_ok());
        for url in [
            "https://example.com/a.mp4",
            "file:///tmp/a.mp4",
            "http://127.0.0.1/a.mp4",
            "http://127.0.0.1:1234/a.png",
            "http://x:y@127.0.0.1:1234/a.mp4",
        ] {
            assert!(source(url).is_err(), "{url}");
        }
    }
}
