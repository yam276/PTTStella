//! Main-window, strict-loopback conformance. Toolkit events are not OS proof.
use super::StationColor;
use crate::{
    StationColorState, TerminalWindow,
    verification::{Verification, capture, click, foreground, key, require, wait},
};
use slint::{
    ComponentHandle, Model,
    platform::{Key, PointerEventButton as Button, WindowEvent as Event},
};
use std::{
    error::Error,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<StationColor>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(ui, directory, "editor-color", move |ui, dir| async move {
        verify(&ui, &owner, &dir).await
    })
}
fn point(
    ui: &TerminalWindow,
    owner: &StationColor,
    col: f32,
    row: f32,
) -> Result<slint::LogicalPosition, String> {
    let g = owner
        .output
        .geometry(ui)
        .and_then(crate::pointer::SurfaceGeometry::terminal)
        .ok_or("station color geometry missing")?;
    Ok(slint::LogicalPosition::new(
        ui.get_terminal_origin_x() + (g.left + g.size.width as f32 * col / 80.) / g.dpi,
        ui.get_terminal_origin_y() + (g.top + g.size.height as f32 * row / 24.) / g.dpi,
    ))
}
fn select(ui: &TerminalWindow, owner: &StationColor) -> Result<(), String> {
    let a = point(ui, owner, 1.5, 0.5)?;
    let b = point(ui, owner, 5.5, 2.5)?;
    ui.window().dispatch_event(Event::PointerPressed {
        position: a,
        button: Button::Left,
    });
    ui.window()
        .dispatch_event(Event::PointerMoved { position: b });
    ui.window().dispatch_event(Event::PointerReleased {
        position: b,
        button: Button::Left,
    });
    require(
        owner.output.selection_key().is_some(),
        "native selection TouchArea did not retain the three rows",
    )
}
async fn verify(ui: &TerminalWindow, owner: &StationColor, dir: &Path) -> Result<(), String> {
    let case = std::env::var("PTTSTELLA_STATION_COLOR_CASE").ok();
    wait(
        || {
            foreground(ui, true);
            owner.output.current(ui).is_some() && !ui.get_frame_paint_pending()
        },
        "initial editor frame not presented",
    )
    .await?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    foreground(ui, true);
    ui.invoke_focus_terminal();
    require(
        ui.get_terminal_input(),
        "station-colour probe did not mount native input",
    )?;
    select(ui, owner)?;
    capture(ui, dir, "selection.png")?;
    let before = owner.connection.updates.borrow().received;
    let b = point(ui, owner, 5.5, 2.5)?;
    click(ui, b, Button::Right);
    wait(
        || ui.get_context_open(),
        "station editor context menu did not open",
    )
    .await?;
    let entries = ui.get_context_entries();
    let label = pttstella::msg!("editor-color-title");
    let index = (0..entries.row_count())
        .find(|i| {
            entries
                .row_data(*i)
                .is_some_and(|e| e.enabled && e.label.as_str() == label)
        })
        .ok_or("missing enabled station colour action")?;
    ui.invoke_context_choose(index as i32);
    wait(
        || ui.get_station_color_open(),
        "station palette not mounted",
    )
    .await?;
    require(
        !ui.get_keep_alive_surface_ready() && ui.get_terminal_pointer_blocked(),
        "station palette failed to block background input/timer",
    )?;
    let state = ui.global::<StationColorState>();
    state.invoke_choose(9);
    state.invoke_choose(20);
    state.invoke_action(3);
    require(
        state.get_foreground() == 7 && state.get_background() == 0,
        "station palette Defaults differ",
    )?;
    capture(ui, dir, "preview.png")?;
    key(ui, Key::Escape);
    require(!ui.get_station_color_open(), "Esc did not cancel preview")?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    require(
        owner.connection.updates.borrow().received == before,
        "preview/cancel sent station input",
    )?;
    state.invoke_action(0);
    require(state.get_open(), "cancel lost reusable selection")?;
    ui.invoke_clear_selection();
    require(
        !state.get_open(),
        "selection change failed to revoke palette",
    )?;
    state.invoke_action(1);
    require(!state.get_busy(), "stale apply started a worker")?;
    select(ui, owner)?;
    state.invoke_action(0);
    require(state.get_open(), "station palette did not reopen")?;
    state.invoke_choose(9);
    state.invoke_choose(16);
    let initial = owner.connection.updates.borrow().frame.revision;
    state.invoke_action(1);
    require(
        state.get_busy(),
        "explicit Apply did not start common Connection job",
    )?;
    if case.as_deref() == Some("cancel") {
        wait(
            || owner.connection.updates.borrow().frame.revision > initial,
            "station cancellation never entered a confirmed source step",
        )
        .await?;
        state.invoke_action(2);
    }
    for _ in 0..9000 {
        if !state.get_busy() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    require(
        !state.get_busy() && !state.get_open(),
        "station color did not finish within source deadline",
    )?;
    let expected = match case.as_deref() {
        Some("cancel") => pttstella::msg!("editor-color-stopped"),
        Some("unsupported") => pttstella::msg!("editor-color-unsupported"),
        Some("partial") => pttstella::msg!("editor-color-stopped"),
        _ => pttstella::msg!("editor-color-done"),
    };
    require(
        ui.get_input_notice().as_str().starts_with(&expected),
        &format!(
            "station colouring result differs: {}",
            ui.get_input_notice()
        ),
    )?;
    if case.is_some() {
        tokio::time::sleep(Duration::from_millis(400)).await;
        let settled = owner.connection.updates.borrow().received;
        tokio::time::sleep(Duration::from_millis(500)).await;
        require(
            owner.connection.updates.borrow().received == settled,
            "failed/cancelled station colour continued working or retried",
        )?;
    }
    require(
        owner.job.borrow().is_none()
            && owner.cancel.borrow().is_none()
            && owner.source.borrow().is_none(),
        "station coloring retained task/source owners",
    )?;
    wait(
        || owner.output.current(ui).is_some() && !ui.get_frame_paint_pending(),
        "final color frame not presented",
    )
    .await?;
    capture(ui, dir, "colored.png")?;
    std::fs::write(dir.join("editor-color.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","native_drag":"synthetic toolkit TouchArea","preview_cancel_no_station_input":true,"stale_selection_rejected":true,"negative_case":case,"common_station_flow_completed":case.is_none(),"scope":"Full Slint main window with strict independent peer byte/mode check at Close; actual OS input remains separate; partial-error/cancel are explicit fixture branches"})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
