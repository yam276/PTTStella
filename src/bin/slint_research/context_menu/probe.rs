//! Context menu conformance through the complete native application's owners.
use crate::{
    TerminalWindow,
    output::Output,
    verification::{Verification, capture, click, key, require, wait},
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
    directory: PathBuf,
    station: bool,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "context-menu",
        move |ui, directory| async move { verify(&ui, &connection, &output, &directory, station).await },
    )
}
async fn open(ui: &TerminalWindow, output: &Output, station: bool) -> Result<(), String> {
    let geometry = output
        .geometry(ui)
        .and_then(crate::pointer::SurfaceGeometry::terminal)
        .ok_or("context probe missing geometry")?;
    let point = slint::LogicalPosition::new(
        ui.get_terminal_origin_x()
            + (geometry.left + geometry.size.width as f32 * 12.5 / 80.) / geometry.dpi,
        ui.get_terminal_origin_y()
            + (geometry.top + geometry.size.height as f32 * 5.5 / 24.) / geometry.dpi,
    );
    eprintln!(
        "context probe open: window {:?}, point ({},{}), selection {:?}, source {:?}",
        ui.window().size(),
        point.x,
        point.y,
        output.selection_key(),
        output.current(ui)
    );
    if station {
        ui.window().dispatch_event(Event::KeyPressed {
            text: Key::Shift.into(),
        });
    }
    click(ui, point, Button::Right);
    if station {
        ui.window().dispatch_event(Event::KeyReleased {
            text: Key::Shift.into(),
        });
    }
    wait(
        || ui.get_context_open(),
        "context menu did not open through terminal TouchArea",
    )
    .await
    .map_err(|error| {
        format!(
            "{error}; tools={}, context-pending={}, foreground={}, notice={}",
            ui.get_tools_open(),
            ui.get_context_pending(),
            ui.get_native_foreground(),
            ui.get_input_notice()
        )
    })
}
fn item_point(ui: &TerminalWindow, index: usize) -> slint::LogicalPosition {
    let dpi = ui.window().scale_factor();
    let size = ui.window().size();
    let width = size.width as f32 / dpi;
    let height = size.height as f32 / dpi;
    let menu_width = 300f32.min(width - 16.);
    let menu_height = (ui.get_context_entries().row_count() as f32 * 34. + 12.).min(height - 16.);
    slint::LogicalPosition::new(
        (ui.get_terminal_origin_x() + ui.get_context_x())
            .clamp(8., (width - menu_width - 8.).max(8.))
            + 18.,
        (ui.get_terminal_origin_y() + ui.get_context_y())
            .clamp(8., (height - menu_height - 8.).max(8.))
            + 6.
            + index as f32 * 34.
            + 17.,
    )
}
async fn verify(
    ui: &TerminalWindow,
    connection: &pttstella::connection::Connection,
    output: &Output,
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
            ready && output.current(ui).is_some() && !ui.get_frame_paint_pending()
        },
        "native app did not present a usable frame",
    )
    .await?;
    capture(ui, directory, "initial-layout.png")?;
    // Drain native startup focus/resize events before applying the explicit
    // synthetic activation used by this toolkit-only conformance case.
    tokio::time::sleep(Duration::from_millis(300)).await;
    crate::verification::foreground(ui, true);
    wait(
        || output.current(ui).is_some() && !ui.get_frame_paint_pending(),
        "initial native context layout did not settle",
    )
    .await?;
    ui.invoke_focus_terminal();
    open(ui, output, station).await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    require(
        ui.get_context_open(),
        "late mouse policy dismissed the menu",
    )?;
    require(
        ui.get_context_entries().row_count() == 3,
        "URL menu has wrong actions",
    )?;
    require(
        !ui.get_terminal_input_focused(),
        "terminal retained focus while menu open",
    )?;
    if station {
        require(
            ui.get_context_entries()
                .row_data(1)
                .is_some_and(|e| !e.enabled),
            "station open-link action was enabled",
        )?;
        click(ui, item_point(ui, 1), Button::Left);
        require(ui.get_context_open(), "disabled item dismissed the menu")?;
    }
    capture(ui, directory, "url-menu.png")?;
    key(ui, Key::DownArrow);
    require(
        ui.get_context_selected() == if station { 2 } else { 1 },
        "keyboard did not skip disabled action",
    )?;
    if !station {
        key(ui, Key::DownArrow);
    }
    key(ui, Key::Return);
    require(
        !ui.get_context_open() && ui.get_terminal_input_focused(),
        "menu did not restore terminal focus",
    )?;
    let selected = output
        .selection_key()
        .ok_or("select-link produced no selection")?;
    require(
        selected
            == (
                5..6,
                vec![pttstella::selection::Span {
                    row: 5,
                    start: 5,
                    end: 34,
                }],
            ),
        "select-link did not select the complete URL",
    )?;
    open(ui, output, station).await?;
    require(
        ui.get_context_entries().row_count() == 7,
        "right click lost the current selection",
    )?;
    key(ui, Key::End);
    require(
        ui.get_context_selected() == 6,
        "End did not select last enabled action",
    )?;
    key(ui, Key::Home);
    require(
        ui.get_context_selected() == 0,
        "Home did not select first enabled action",
    )?;
    key(ui, Key::Escape);
    require(
        output.selection_key() == Some(selected),
        "Escape discarded the retained selection",
    )?;
    ui.invoke_clear_selection();
    open(ui, output, station).await?;
    click(ui, slint::LogicalPosition::new(4., 4.), Button::Left);
    require(
        !ui.get_context_open() && !ui.get_selection_active(),
        "outside gesture leaked through menu",
    )?;
    open(ui, output, station).await?;
    crate::verification::foreground(ui, false);
    ui.invoke_context_updated();
    require(!ui.get_context_open(), "foreground loss retained menu")?;
    crate::verification::foreground(ui, true);
    // Independently require a visible Settings label on a background menu.
    let g = output
        .geometry(ui)
        .and_then(crate::pointer::SurfaceGeometry::terminal)
        .ok_or("background menu geometry missing")?;
    let p = slint::LogicalPosition::new(
        ui.get_terminal_origin_x() + (g.left + g.size.width as f32 * 40.5 / 80.) / g.dpi,
        ui.get_terminal_origin_y() + (g.top + g.size.height as f32 * 10.5 / 24.) / g.dpi,
    );
    require(
        output
            .link_at(
                ui,
                (
                    (p.x - ui.get_terminal_origin_x()) * g.dpi,
                    (p.y - ui.get_terminal_origin_y()) * g.dpi,
                ),
            )
            .is_none(),
        "background menu test point is a link",
    )?;
    ui.window().dispatch_event(Event::KeyPressed {
        text: Key::Shift.into(),
    });
    click(ui, p, Button::Right);
    ui.window().dispatch_event(Event::KeyReleased {
        text: Key::Shift.into(),
    });
    wait(|| ui.get_context_open(), "background menu did not open").await?;
    let entries = ui.get_context_entries();
    require(
        (0..entries.row_count())
            .filter_map(|i| entries.row_data(i))
            .all(|e| !e.label.trim().is_empty()),
        "background menu exposed an unlabeled action",
    )?;
    require(
        (0..entries.row_count())
            .filter_map(|i| entries.row_data(i))
            .any(|e| e.label == pttstella::msg!("settings-center-title")),
        "background Settings action has no localized label",
    )?;
    capture(ui, directory, "background-settings-label.png")?;
    key(ui, Key::Escape);
    open(ui, output, station).await?;
    ui.set_zoom(1.25);
    wait(
        || !ui.get_context_open() && output.current(ui).is_some() && !ui.get_frame_paint_pending(),
        "zoom did not revoke source and repaint",
    )
    .await?;
    ui.window().set_size(slint::LogicalSize::new(680., 520.));
    tokio::time::sleep(Duration::from_millis(150)).await;
    wait(
        || output.current(ui).is_some(),
        "small window did not present a frame",
    )
    .await?;
    open(ui, output, station).await?;
    capture(ui, directory, "small-menu.png")?;
    key(ui, Key::Tab);
    require(!ui.get_context_open(), "Tab did not dismiss menu")?;
    capture(ui, directory, "closed.png")?;
    Ok(())
}
