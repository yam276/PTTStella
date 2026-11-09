//! Toolkit-only checks use the actual application owner and its strict local peer.
use super::Tools;
use crate::{
    TerminalWindow,
    output::Output,
    verification::{self, Verification, capture, click, key, require, wait},
};
use slint::{
    ComponentHandle, Model,
    platform::{Key, PointerEventButton as Button, WindowEvent as Event},
};
use std::{error::Error, path::PathBuf, rc::Rc, time::Duration};
pub fn start(
    ui: &TerminalWindow,
    connection: Rc<pttstella::connection::Connection>,
    output: Rc<Output>,
    tools: Rc<Tools>,
    directory: PathBuf,
    station: bool,
) -> Result<Verification, Box<dyn Error>> {
    verification::Verification::start(
        ui,
        directory,
        "floating-tools",
        move |ui, directory| async move {
            verify(&ui, &connection, &output, &tools, &directory, station).await
        },
    )
}
fn ball(ui: &TerminalWindow) -> slint::LogicalPosition {
    slint::LogicalPosition::new(
        ui.get_tools_x() + 28. * ui.get_tools_scale(),
        ui.get_tools_y() + 28. * ui.get_tools_scale(),
    )
}
fn tile(ui: &TerminalWindow, index: usize) -> Result<slint::LogicalPosition, String> {
    let item = ui
        .get_tools_categories()
        .row_data(index)
        .ok_or("missing category")?;
    Ok(slint::LogicalPosition::new(
        item.x + item.width / 2.,
        item.y + item.height / 2.,
    ))
}
fn action_point(ui: &TerminalWindow, index: usize) -> Result<slint::LogicalPosition, String> {
    let item = ui
        .get_tools_actions()
        .row_data(index)
        .ok_or("missing action")?;
    let panel = ui.get_tools_panel();
    Ok(slint::LogicalPosition::new(
        panel.x + 12. + item.x + item.width / 2.,
        panel.y + 20. + 24. * ui.get_tools_scale() + item.y + item.height / 2.,
    ))
}
async fn open(ui: &TerminalWindow) -> Result<(), String> {
    click(ui, ball(ui), Button::Left);
    wait(|| ui.get_tools_open(), "floating ball did not open fan").await
}
async fn verify(
    ui: &TerminalWindow,
    connection: &pttstella::connection::Connection,
    output: &Output,
    tools: &Tools,
    directory: &std::path::Path,
    station: bool,
) -> Result<(), String> {
    wait(
        || {
            crate::verification::foreground(ui, true);
            let ready = {
                let view = connection.updates.borrow();
                view.received > 0 && !view.output_pending
            };
            ready && output.current(ui).is_some()
        },
        "native tools frame not ready",
    )
    .await?;
    capture(ui, directory, "initial-layout.png")?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    crate::verification::foreground(ui, true);
    wait(
        || output.current(ui).is_some() && !ui.get_frame_paint_pending(),
        "initial native tools layout did not settle",
    )
    .await?;
    ui.invoke_focus_terminal();
    require(
        ui.get_tools_opacity() == 0.2,
        "fixture opacity did not reach the tool owner",
    )?;
    if !station {
        let geometry = output
            .geometry(ui)
            .and_then(crate::pointer::SurfaceGeometry::terminal)
            .ok_or("closed ball link geometry missing")?;
        let point = slint::LogicalPosition::new(
            ui.get_terminal_origin_x()
                + (geometry.left + geometry.size.width as f32 * 12.5 / 80.) / geometry.dpi,
            ui.get_terminal_origin_y()
                + (geometry.top + geometry.size.height as f32 * 5.5 / 24.) / geometry.dpi,
        );
        click(ui, point, Button::Left);
        require(
            ui.get_input_notice() == "https://example.com/pttstella",
            "closed ball intercepted ordinary link click",
        )?;
        tokio::time::sleep(Duration::from_millis(30)).await;
        wait(
            || output.current(ui).is_some(),
            "link status layout did not settle",
        )
        .await?;
    }
    let image = ui.get_terminal_image();
    open(ui).await?;
    require(
        !ui.get_terminal_input_focused(),
        "tools did not own keyboard focus",
    )?;
    require(
        ui.get_tools_categories().row_count() == 5,
        "five category layout missing",
    )?;
    require(
        !ui.get_tools_categories()
            .row_data(1)
            .is_some_and(|v| v.enabled)
            && ui
                .get_tools_categories()
                .row_data(4)
                .is_some_and(|v| v.enabled),
        "bookmark category or always-available license entry is incorrect",
    )?;
    require(
        image == ui.get_terminal_image(),
        "opening tools repainted terminal bitmap",
    )?;
    capture(ui, directory, "fan.png")?;
    key(ui, Key::DownArrow);
    require(
        ui.get_tools_selected() == 2,
        "fan did not skip unavailable bookmark dialog",
    )?;
    key(ui, Key::Return);
    require(
        ui.get_tools_category() == 2,
        "keyboard did not open capture category",
    )?;
    capture(ui, directory, "capture.png")?;
    key(ui, Key::Return);
    require(
        !ui.get_tools_open() && ui.get_selection_active() && ui.get_terminal_input_focused(),
        "select-all action or focus restoration failed",
    )?;
    require(
        output
            .selection_key()
            .is_some_and(|(rows, _)| rows == (0..24)),
        "capture action did not select all rows",
    )?;
    ui.invoke_clear_selection();
    open(ui).await?;
    click(ui, tile(ui, 2)?, Button::Left);
    require(
        ui.get_tools_category() == 2,
        "pointer did not choose capture",
    )?;
    // The same visible button may not export/select a new, unpresented source.
    let action = action_point(ui, 0)?;
    ui.set_native_pointer_buttons(1);
    ui.set_native_pointer_down(true);
    ui.window().dispatch_event(Event::PointerPressed {
        position: action,
        button: Button::Left,
    });
    ui.set_frame_paint_pending(true);
    ui.set_native_pointer_buttons(0);
    ui.set_native_pointer_down(false);
    ui.window().dispatch_event(Event::PointerReleased {
        position: action,
        button: Button::Left,
    });
    require(
        !ui.get_selection_active(),
        "stale pressed capture acted on unpresented source",
    )?;
    ui.set_frame_paint_pending(false);
    ui.invoke_tools_updated();
    ui.invoke_tools_action_pointer(0, 0);
    ui.invoke_tools_cancel_gesture();
    ui.invoke_tools_action_pointer(2, 0);
    require(
        !ui.get_selection_active(),
        "geometry cancellation retained pressed action",
    )?;
    click(ui, action_point(ui, 0)?, Button::Left);
    require(
        ui.get_selection_active() && !ui.get_tools_open(),
        "pointer action did not select all",
    )?;
    ui.invoke_clear_selection();
    open(ui).await?;
    click(ui, tile(ui, 2)?, Button::Left);
    key(ui, Key::LeftArrow);
    require(
        ui.get_tools_category() == -1 && ui.get_tools_open() && ui.get_tools_selected() == 2,
        "Back did not return to category fan",
    )?;
    click(ui, slint::LogicalPosition::new(4., 4.), Button::Left);
    require(
        !ui.get_tools_open() && !ui.get_selection_active(),
        "outside click leaked to terminal",
    )?;
    ui.set_tools_reduced_motion(false);
    let from = ball(ui);
    ui.set_native_pointer_buttons(1);
    ui.set_native_pointer_down(true);
    ui.window().dispatch_event(Event::PointerPressed {
        position: from,
        button: Button::Left,
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    let to = slint::LogicalPosition::new(from.x - 240., from.y - 60.);
    ui.window()
        .dispatch_event(Event::PointerMoved { position: to });
    tokio::time::sleep(Duration::from_millis(30)).await;
    ui.set_native_pointer_buttons(0);
    ui.set_native_pointer_down(false);
    ui.window().dispatch_event(Event::PointerReleased {
        position: to,
        button: Button::Left,
    });
    require(!ui.get_tools_open(), "drag release opened menu")?;
    wait(
        || !tools.state.borrow().animating(),
        "coast/dock did not stop",
    )
    .await?;
    let parked = (ui.get_tools_x(), ui.get_tools_y());
    if !station {
        let geometry = output
            .geometry(ui)
            .and_then(crate::pointer::SurfaceGeometry::terminal)
            .ok_or("tools right-click geometry missing")?;
        let point = slint::LogicalPosition::new(
            ui.get_terminal_origin_x()
                + (geometry.left + geometry.size.width as f32 * 40.5 / 80.) / geometry.dpi,
            ui.get_terminal_origin_y()
                + (geometry.top + geometry.size.height as f32 * 12.5 / 24.) / geometry.dpi,
        );
        require(
            matches!(
                connection.updates.borrow().context(),
                pttstella::navigation::Context::ArticleReading { .. }
            ),
            "tools reading-priority fixture is not an article",
        )?;
        click(ui, point, Button::Right);
        wait(
            || ui.get_context_open() && !ui.get_tools_open(),
            "reading background did not retain the content menu",
        )
        .await?;
        key(ui, Key::Escape);
        require(
            (ui.get_tools_x(), ui.get_tools_y()) == parked,
            "visible temporary position replaced parked position",
        )?;
        ui.set_tools_visible(false);
        click(ui, point, Button::Right);
        wait(
            || ui.get_context_open() && !ui.get_tools_open(),
            "hidden floating button intercepted the reading content menu",
        )
        .await?;
        require(
            ui.get_context_open() && !ui.get_tools_open(),
            "reading background opened both menus",
        )?;
        capture(ui, directory, "hidden-trigger-content.png")?;
        key(ui, Key::Escape);
        require(
            (ui.get_tools_x(), ui.get_tools_y()) == parked,
            "temporary right-click position replaced parked position",
        )?;
        ui.set_tools_visible(true);
        ui.set_tools_right_click(false);
        click(ui, point, Button::Right);
        wait(
            || ui.get_context_open(),
            "disabled right-click tools did not restore content menu",
        )
        .await?;
        key(ui, Key::Escape);
        ui.set_tools_right_click(true);
    }
    ui.window().set_size(slint::LogicalSize::new(580., 360.));
    tokio::time::sleep(Duration::from_millis(150)).await;
    open(ui).await?;
    require(
        ui.get_tools_compact(),
        "small window did not use compact category layout",
    )?;
    capture(ui, directory, "compact.png")?;
    key(ui, Key::Escape);
    require(
        ui.get_terminal_input_focused(),
        "Escape did not restore BBS focus",
    )?;
    open(ui).await?;
    crate::verification::foreground(ui, false);
    ui.invoke_tools_updated();
    require(
        !ui.get_tools_open() && !tools.state.borrow().animating(),
        "focus loss retained floating tools",
    )?;
    crate::verification::foreground(ui, true);
    capture(ui, directory, "closed.png")?;
    require(
        connection.updates.borrow().input_epoch == 0,
        "tools sent terminal input",
    )?;
    Ok(())
}
