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

pub enum Fixture {
    ParityGrid,
    DraftBoard(Rc<pttstella::connection::Connection>),
}
impl Fixture {
    fn name(&self) -> &'static str {
        match self {
            Self::ParityGrid => "benchmark",
            Self::DraftBoard(_) => "draft-board",
        }
    }
    fn accepts(&self, frame: &pttstella::screen::Frame, presented: FrameId) -> bool {
        match self {
            Self::ParityGrid => pttstella::replay::verify_parity_grid(frame).is_ok(),
            Self::DraftBoard(connection) => {
                draft_board_ready(&connection.updates.borrow(), frame, presented)
            }
        }
    }
}

fn draft_board_ready(
    view: &pttstella::connection::View,
    frame: &pttstella::screen::Frame,
    presented: FrameId,
) -> bool {
    frame.cols == 80
        && frame.rows == 24
        && frame.control_complete
        && frame.glyphs.iter().all(|glyph| !glyph.unmapped)
        && presented.generation == view.generation
        && presented.revision == frame.revision
        && frame.revision == view.frame.revision
        && pttstella::drafts::scope(view)
            .is_some_and(|scope| scope.account == "StellaTest" && scope.board.as_str() == "Stella")
}

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
        fixture: Fixture,
    ) -> Self {
        ui.window().set_size(slint::LogicalSize::new(1040., 740.));
        let timer = Rc::new(Timer::default());
        let outcome = Rc::new(RefCell::new(None));
        let (weak_ui, weak_timer, result) = (ui.as_weak(), Rc::downgrade(&timer), outcome.clone());
        let started = Instant::now();
        timer.start(TimerMode::Repeated, Duration::from_millis(20), move || {
            let Some(ui) = weak_ui.upgrade() else { return; };
            let ready = presented.get().is_some_and(|id| id.revision == renderer.borrow().frame().revision
                    && fixture.accepts(renderer.borrow().frame(), id))
                && size.get().is_some() && !ui.get_frame_paint_pending()
                ;
            if !ready && started.elapsed() < Duration::from_secs(5) { return; }
            if let Some(timer) = weak_timer.upgrade() { timer.stop(); }
            let checked = (|| -> Result<(), String> {
                if !ready { return Err(format!("full-service {} frame was not installed",fixture.name())); }
                if !services.as_object().is_some_and(|flags| !flags.is_empty() && flags.values().all(|v| v.as_bool() == Some(true))) {
                    return Err("full-service benchmark is missing a production service owner".into());
                }
                let dimensions = ui.window().size();
                let installed = size.get().ok_or("benchmark grid dimensions missing")?;
                let report = serde_json::json!({
                    "status":"prepared", "services":services,
                    "fixture":fixture.name(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_preparation_requires_its_connected_scope_and_installed_frame() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let connection = runtime.block_on(async {
            let connection =
                Rc::new(pttstella::connection::Connection::start_drafts_probe().unwrap());
            connection.connect().unwrap();
            connection
        });
        let view = runtime.block_on(async {
            let mut updates = connection.updates.clone();
            tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let view = updates.borrow_and_update().clone();
                    if pttstella::drafts::scope(&view).is_some() {
                        break view;
                    }
                    updates.changed().await.unwrap();
                }
            })
            .await
            .unwrap()
        });
        let id = FrameId {
            generation: view.generation,
            revision: view.frame.revision,
        };
        assert!(draft_board_ready(&view, &view.frame, id));
        assert!(Fixture::DraftBoard(connection.clone()).accepts(&view.frame, id));
        assert!(!Fixture::ParityGrid.accepts(&view.frame, id));
        let mut wrong = view.clone();
        wrong.account = Some("OtherAccount".into());
        assert!(!draft_board_ready(&wrong, &view.frame, id));
        wrong = view.clone();
        wrong.output_pending = true;
        assert!(!draft_board_ready(&wrong, &view.frame, id));
        wrong = view.clone();
        wrong.input_pending = true;
        assert!(!draft_board_ready(&wrong, &view.frame, id));
        assert!(!draft_board_ready(
            &view,
            &view.frame,
            FrameId {
                generation: id.generation + 1,
                ..id
            }
        ));
        assert!(!draft_board_ready(
            &view,
            &view.frame,
            FrameId {
                revision: id.revision + 1,
                ..id
            }
        ));
        let mut frame = view.frame.clone();
        frame.control_complete = false;
        assert!(!draft_board_ready(&view, &frame, id));
        frame = view.frame.clone();
        frame.glyphs[0].unmapped = true;
        assert!(!draft_board_ready(&view, &frame, id));
        assert_eq!(
            runtime.block_on(connection.shutdown()).unwrap(),
            pttstella::connection::CloseOutcome::Acknowledged
        );
    }
}
