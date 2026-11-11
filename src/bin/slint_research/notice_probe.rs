//! Product timer + standard widget events. This does not simulate OS input.
use crate::{
    TerminalWindow, notice,
    verification::{self, require, wait},
};
use slint::{
    ComponentHandle,
    platform::{Key, WindowEvent},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// Only explicit conformance runs sample these flags. No product timer or
/// callback is replaced and the original deadlines/assertions remain intact.
struct Trace {
    started: Instant,
    samples: Vec<serde_json::Value>,
    last: Option<serde_json::Value>,
    omitted: usize,
}
impl Trace {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            samples: Vec::new(),
            last: None,
            omitted: 0,
        }
    }
    fn sample(&mut self, ui: &TerminalWindow, owner: &notice::Owner, phase: &str) {
        let flags = serde_json::json!({
            "phase":phase,"hover":ui.get_notice_hovered(),"focus":ui.get_notice_focused(),
            "fading":ui.get_notice_fading(),"timer":owner.timer_active(),
            "notice_empty":ui.get_input_notice().is_empty(),"error":ui.get_notice_error(),
            "reduced_motion":ui.get_tools_reduced_motion(),"foreground":ui.get_native_foreground(),
            "tools_blocked":ui.get_tools_source_blocked(),"terminal_focus":ui.get_terminal_input_focused(),
            "shell_visible":ui.get_shell_status_visible(),"closing":ui.get_app_closing()
        });
        if self.last.as_ref() == Some(&flags) {
            return;
        }
        self.last = Some(flags.clone());
        if self.samples.len() < 128 {
            self.samples
                .push(serde_json::json!({"ms":self.started.elapsed().as_millis(),"flags":flags}));
        } else {
            self.omitted += 1;
        }
    }
    async fn pause(&mut self, ui: &TerminalWindow, owner: &notice::Owner, ms: u64, phase: &str) {
        let until = Instant::now() + Duration::from_millis(ms);
        self.sample(ui, owner, phase);
        while Instant::now() < until {
            tokio::time::sleep(
                until
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(20)),
            )
            .await;
            self.sample(ui, owner, phase);
        }
    }
    fn write(&self, dir: &Path, result: &Result<(), String>) -> Result<(), String> {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join("notice-state-trace.json"))
            .map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(file,&serde_json::json!({
            "scope":"conformance-only 20ms sampling of UI/timer flags; no OS event trace or text content",
            "max_samples":128,"omitted":self.omitted,"samples":self.samples,"outcome":result
        })).map_err(|e|e.to_string())
    }
}
fn move_to(ui: &TerminalWindow, x: f32, y: f32) {
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(x, y),
    });
}

pub async fn run(ui: &TerminalWindow, owner: &notice::Owner, dir: &Path) -> Result<(), String> {
    let mut trace = Trace::new();
    let result = run_inner(ui, owner, dir, &mut trace).await;
    trace.sample(ui, owner, "finished");
    let saved = trace.write(dir, &result);
    match (result, saved) {
        (Ok(()), result) => result,
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(trace)) => Err(format!(
            "{error}; failed to save notice state trace: {trace}"
        )),
    }
}
async fn run_inner(
    ui: &TerminalWindow,
    owner: &notice::Owner,
    dir: &Path,
    trace: &mut Trace,
) -> Result<(), String> {
    let size = ui.window().size();
    let dpi = ui.window().scale_factor();
    let (width, height) = (size.width as f32 / dpi, size.height as f32 / dpi);
    let close_x = width / 2. + 620_f32.min(width - 32.) / 2. - 35.;
    let outside = || move_to(ui, width / 2., height - 80.);
    ui.invoke_focus_terminal();
    outside();
    ui.set_tools_reduced_motion(false);
    notice::clear(ui);
    notice::copied(ui, "<img src=x> 複製通知");
    require(
        ui.get_notice_mark_ready() && ui.get_input_notice() == "<img src=x> 複製通知",
        "notice icon missing or message was interpreted as markup",
    )?;
    let started = Instant::now();
    require(
        owner.timer_active(),
        "copy did not arm the one product timer",
    )?;
    trace.pause(ui, owner, 1000, "initial_readable").await;
    require(
        !ui.get_notice_fading() && !ui.get_input_notice().is_empty(),
        "copy expired before its readable interval",
    )?;
    // Hover over the child standard Button, not just empty panel background.
    move_to(ui, close_x, 51.);
    wait(
        || ui.get_notice_hovered(),
        "notification did not observe hover over its child button",
    )
    .await?;
    trace.pause(ui, owner, 1300, "hover_readable").await;
    require(
        !ui.get_notice_fading() && !owner.timer_active() && !ui.get_input_notice().is_empty(),
        "hover consumed readable time or kept a polling timer",
    )?;
    // Real toolkit Tab traversal, rather than setting a focus property.
    for _ in 0..20 {
        verification::key(ui, Key::Tab);
        trace.pause(ui, owner, 10, "tab_to_notice").await;
        if ui.get_notice_focused() {
            break;
        }
    }
    require(
        ui.get_notice_focused(),
        "Tab could not reach a notification control",
    )?;
    outside();
    wait(
        || !ui.get_notice_hovered(),
        "notification hover did not leave",
    )
    .await?;
    trace.pause(ui, owner, 1200, "focused_without_hover").await;
    require(
        !owner.timer_active() && !ui.get_input_notice().is_empty(),
        "focus did not independently pause notification lifetime",
    )?;
    ui.invoke_focus_terminal();
    wait(
        || !ui.get_notice_focused(),
        "notification focus did not leave",
    )
    .await?;
    wait(
        || ui.get_notice_fading(),
        "resuming copy did not enter fade",
    )
    .await?;
    trace.pause(ui, owner, 150, "fade_resumed").await;
    verification::capture(ui, dir, "notice-fading.png")?;
    move_to(ui, close_x, 51.);
    wait(
        || ui.get_notice_hovered() && !ui.get_notice_fading(),
        "hover did not restore opacity during fade",
    )
    .await?;
    trace.pause(ui, owner, 650, "hover_fade").await;
    require(
        !ui.get_input_notice().is_empty() && !owner.timer_active(),
        "paused fade expired",
    )?;
    ui.set_tools_reduced_motion(true);
    trace
        .pause(ui, owner, 50, "reduced_motion_while_hovered")
        .await;
    require(
        !ui.get_input_notice().is_empty(),
        "reduced motion dismissed a hovered notice",
    )?;
    outside();
    wait(
        || ui.get_input_notice().is_empty(),
        "reduced motion did not clear resumed fade",
    )
    .await?;
    require(
        ui.get_terminal_input_focused(),
        "automatic expiry changed terminal focus",
    )?;
    let paused_copy_wall_ms = started.elapsed().as_millis();

    ui.set_tools_reduced_motion(false);
    notice::copied(ui, "same");
    trace
        .pause(ui, owner, 1200, "before_identical_replacement")
        .await;
    notice::copied(ui, "same");
    trace
        .pause(ui, owner, 1000, "after_identical_replacement")
        .await;
    require(
        !ui.get_notice_fading() && ui.get_input_notice() == "same",
        "identical replacement inherited the old deadline",
    )?;
    notice::error(ui, "new failure");
    notice::copied(ui, "late copy");
    notice::success(ui, "late save");
    trace.pause(ui, owner, 2600, "error_priority").await;
    require(
        ui.get_input_notice() == "new failure" && ui.get_notice_error() && !owner.timer_active(),
        "a stale timer or late success replaced the error",
    )?;
    verification::capture(ui, dir, "notice-error-priority.png")?;
    ui.invoke_notice_dismiss();
    outside();
    ui.set_tools_reduced_motion(true);
    notice::copied(ui, "no motion");
    trace
        .pause(ui, owner, 1500, "reduced_motion_readable")
        .await;
    require(
        ui.get_input_notice() == "no motion" && !ui.get_notice_fading(),
        "reduced motion skipped readable time",
    )?;
    wait(
        || ui.get_input_notice().is_empty(),
        "reduced-motion copy did not expire",
    )
    .await?;
    require(
        !ui.get_notice_fading() && ui.get_terminal_input_focused(),
        "reduced-motion expiry animated or took focus",
    )?;
    notice::persistent(ui, "unclassified");
    trace.pause(ui, owner, 2600, "persistent_unknown").await;
    require(
        ui.get_input_notice() == "unclassified" && !owner.timer_active(),
        "unclassified notification was timed",
    )?;
    notice::clear(ui);
    notice::copied(ui, pttstella::msg!("context-copied"));
    verification::capture(ui, dir, "notice-copy.png")?;
    notice::clear(ui);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join("notice-result.json"))
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(file,&serde_json::json!({"status":"passed_pending_exit_audit","copy_pause_wall_ms":paused_copy_wall_ms,
        "hover_on_standard_child":true,"toolkit_tab_focus":true,"fade_pause":true,"reduced_motion_change":true,
        "replacement_deadline":true,"error_priority":true,"persistent_unknown":true,"expiry_preserves_focus":true,
        "scope":"production timer/model/widgets with toolkit events and synthetic foreground; OS preference, OS input and other platforms untested"})).map_err(|e|e.to_string())
}
