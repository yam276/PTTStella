//! Passive, explicit loopback benchmark preparation. Never synthesizes focus or input.
use crate::{
    TerminalWindow,
    renderer::{Renderer, Size},
    session::FrameId,
};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

pub struct Preparation {
    timer: Rc<Timer>,
    outcome: Rc<RefCell<Option<Result<(), String>>>>,
}

impl Preparation {
    pub fn start(
        ui: &TerminalWindow,
        renderer: Rc<RefCell<Renderer>>,
        size: Rc<Cell<Option<Size>>>,
        presented: Rc<Cell<Option<FrameId>>>,
        directory: PathBuf,
        services: serde_json::Value,
    ) -> Self {
        ui.window().set_size(slint::LogicalSize::new(1040., 740.));
        let timer = Rc::new(Timer::default());
        let outcome = Rc::new(RefCell::new(None));
        let (weak_ui, weak_timer, result) = (ui.as_weak(), Rc::downgrade(&timer), outcome.clone());
        let started = Instant::now();
        timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
            let Some(ui) = weak_ui.upgrade() else { return; };
            let ready = presented.get().is_some_and(|id| id.revision == renderer.borrow().frame().revision)
                && size.get().is_some() && !ui.get_frame_paint_pending()
                && pttstella::replay::verify_parity_grid(renderer.borrow().frame()).is_ok();
            if !ready && started.elapsed() < Duration::from_secs(5) { return; }
            if let Some(timer) = weak_timer.upgrade() { timer.stop(); }
            let checked = (|| -> Result<(), String> {
                if !ready { return Err("full-service benchmark frame was not installed".into()); }
                if !services.as_object().is_some_and(|flags| !flags.is_empty() && flags.values().all(|v| v.as_bool() == Some(true))) {
                    return Err("full-service benchmark is missing a production service owner".into());
                }
                let dimensions = ui.window().size();
                let installed = size.get().ok_or("benchmark grid dimensions missing")?;
                let report = serde_json::json!({
                    "status":"prepared", "services":services,
                    "scope":"frame and geometry installed; not OS presentation, foreground or input latency",
                    "window_physical":[dimensions.width,dimensions.height],
                    "scale_factor":ui.window().scale_factor(),
                    "viewport_logical":[ui.get_view_width(),ui.get_view_height()],
                    "grid_physical":[installed.width,installed.height],
                    "zoom":ui.get_zoom(), "native_foreground_at_preparation":ui.get_native_foreground(),
                    "frame":{"generation":presented.get().map(|id|id.generation),"revision":renderer.borrow().frame().revision},
                });
                let pixels = renderer.borrow().paint(Size{width:960,height:576},false).map_err(str::to_owned)?;
                crate::save_png(&pixels, &directory.join("source.png")).map_err(|e|e.to_string())?;
                crate::verification::capture(&ui, &directory, "snapshot.png")?;
                let file = std::fs::OpenOptions::new().write(true).create_new(true)
                    .open(directory.join("prepared.json")).map_err(|e|e.to_string())?;
                serde_json::to_writer_pretty(file,&report).map_err(|e|e.to_string())
            })();
            let failed = checked.is_err();
            result.replace(Some(checked));
            if failed { ui.invoke_app_quit(); }
        });
        Self { timer, outcome }
    }

    pub fn finish(self) -> Result<(), String> {
        self.timer.stop();
        self.outcome
            .take()
            .ok_or("benchmark closed before frame preparation")?
    }
}
