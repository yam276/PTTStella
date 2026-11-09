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

async fn pause(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}
fn move_to(ui: &TerminalWindow, x: f32, y: f32) {
    ui.window().dispatch_event(WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(x, y),
    });
}

pub async fn run(ui: &TerminalWindow, owner: &notice::Owner, dir: &Path) -> Result<(), String> {
    let size = ui.window().size();
    let dpi = ui.window().scale_factor();
    let (width, height) = (size.width as f32 / dpi, size.height as f32 / dpi);
    let close_x = width / 2. + 620_f32.min(width - 32.) / 2. - 24.;
    let outside = || move_to(ui, width / 2., height - 80.);
    ui.invoke_focus_terminal();
    outside();
    ui.set_tools_reduced_motion(false);
    notice::clear(ui);
    notice::copied(ui, "<img src=x> 複製通知");
    let started = Instant::now();
    require(
        owner.timer_active(),
        "copy did not arm the one product timer",
    )?;
    pause(1000).await;
    require(
        !ui.get_notice_fading() && !ui.get_input_notice().is_empty(),
        "copy expired before its readable interval",
    )?;
    // Hover over the child standard Button, not just empty panel background.
    move_to(ui, close_x, 36.);
    wait(
        || ui.get_notice_hovered(),
        "notification did not observe hover over its child button",
    )
    .await?;
    pause(1300).await;
    require(
        !ui.get_notice_fading() && !owner.timer_active() && !ui.get_input_notice().is_empty(),
        "hover consumed readable time or kept a polling timer",
    )?;
    // Real toolkit Tab traversal, rather than setting a focus property.
    for _ in 0..20 {
        verification::key(ui, Key::Tab);
        pause(10).await;
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
    pause(1200).await;
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
    pause(150).await;
    verification::capture(ui, dir, "notice-fading.png")?;
    move_to(ui, close_x, 36.);
    wait(
        || ui.get_notice_hovered() && !ui.get_notice_fading(),
        "hover did not restore opacity during fade",
    )
    .await?;
    pause(650).await;
    require(
        !ui.get_input_notice().is_empty() && !owner.timer_active(),
        "paused fade expired",
    )?;
    ui.set_tools_reduced_motion(true);
    pause(50).await;
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
    pause(1200).await;
    notice::copied(ui, "same");
    pause(1000).await;
    require(
        !ui.get_notice_fading() && ui.get_input_notice() == "same",
        "identical replacement inherited the old deadline",
    )?;
    notice::error(ui, "new failure");
    notice::copied(ui, "late copy");
    notice::success(ui, "late save");
    pause(2600).await;
    require(
        ui.get_input_notice() == "new failure" && ui.get_notice_error() && !owner.timer_active(),
        "a stale timer or late success replaced the error",
    )?;
    verification::capture(ui, dir, "notice-error-priority.png")?;
    ui.invoke_notice_dismiss();
    outside();
    ui.set_tools_reduced_motion(true);
    notice::copied(ui, "no motion");
    pause(1500).await;
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
    pause(2600).await;
    require(
        ui.get_input_notice() == "unclassified" && !owner.timer_active(),
        "unclassified notification was timed",
    )?;
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
