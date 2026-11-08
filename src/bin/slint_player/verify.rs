//! Explicit, finite toolkit probe of the real native rendering/control path.
//! This is not OS input, audio audibility, or a performance benchmark.
use super::{Metrics, PlayerWindow, Status};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

#[derive(Clone, Default, serde::Serialize)]
pub struct Report {
    pub passed: bool,
    pub checks: Vec<&'static str>,
    pub error: Option<String>,
}

pub struct Probe {
    timer: slint::Timer,
    report: Rc<RefCell<Report>>,
}
impl Probe {
    pub fn start(
        ui: &PlayerWindow,
        status: Arc<Mutex<Status>>,
        requests: Arc<AtomicUsize>,
        metrics: Rc<RefCell<Metrics>>,
        stall: bool,
    ) -> Self {
        let timer = slint::Timer::default();
        let report = Rc::new(RefCell::new(Report::default()));
        let output = report.clone();
        let weak = ui.as_weak();
        let started = Instant::now();
        let mut phase = 0;
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(20),
            move || {
                if phase == 99 {
                    return;
                }
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let value = status.lock().ok().map(|s| s.clone());
                let mut record = output.borrow_mut();
                let failure = metrics
                    .borrow()
                    .error
                    .clone()
                    .or_else(|| value.as_ref().and_then(|s| s.error.clone()))
                    .or_else(|| {
                        (started.elapsed() > Duration::from_secs(20))
                            .then(|| format!("verification timed out in phase {phase}"))
                    });
                if let Some(error) = failure {
                    record.error = Some(error.clone());
                    ui.set_status(error.into());
                    phase = 99;
                    ui.invoke_action(3);
                    return;
                }
                let Some(value) = value else {
                    return;
                };
                match phase {
                    0 if metrics.borrow().setups == 1 => {
                        if requests.load(Ordering::Acquire) != 0 || value.loaded || value.loading {
                            record.error =
                                Some("unexpected media load before explicit play".into());
                            phase = 99;
                            ui.invoke_action(3);
                            return;
                        }
                        record
                            .checks
                            .push("no HTTP or media load before explicit play");
                        ui.invoke_action(0);
                        phase = 1;
                    }
                    1 if stall && requests.load(Ordering::Acquire) == 1 && !value.loaded => {
                        record
                            .checks
                            .push("request in flight without a loaded frame; close now");
                        record.passed = true;
                        phase = 99;
                        ui.invoke_action(3);
                    }
                    1 if !stall
                        && value.loaded
                        && ui.get_playing()
                        && value.duration > 2.9
                        && value.audio_channels == 1. =>
                    {
                        record
                            .checks
                            .push("owned three-second video and mono audio metadata loaded");
                        ui.invoke_action(1);
                        ui.invoke_action(0); // UI was playing: pause through the same callback.
                        phase = 2;
                    }
                    2 if value.paused && (value.position - 1.).abs() < 0.10 => {
                        record
                            .checks
                            .push("seek to one second and pause acknowledged");
                        ui.invoke_action(4);
                        phase = 3;
                    }
                    3 if !ui.get_playing() && !metrics.borrow().pixel_samples.is_empty() => {
                        if metrics.borrow().pixel_samples.iter().all(|s| s.passed) {
                            record
                                .checks
                                .push("four independent GPU quadrant samples match");
                            ui.invoke_action(0);
                            phase = 4;
                        } else {
                            record.error =
                                Some("decoded GPU pixels differ from owned fixture".into());
                            phase = 99;
                            ui.invoke_action(3);
                        }
                    }
                    4 if !value.paused && ui.get_playing() && value.position >= 1.2 => {
                        record.checks.push("resumed clock advances");
                        ui.invoke_action(0);
                        phase = 5;
                    }
                    5 if value.paused => {
                        record
                            .checks
                            .push("pause acknowledged; normal render/control shutdown requested");
                        record.passed = true;
                        phase = 99;
                        ui.invoke_action(3);
                    }
                    _ => {}
                }
            },
        );
        Self { timer, report }
    }
    pub fn finish(&self) -> Report {
        self.timer.stop();
        self.report.borrow().clone()
    }
}
