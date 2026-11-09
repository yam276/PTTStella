//! Full product shell checks; toolkit actions only, with the production owners.
use crate::{
    TerminalWindow,
    verification::{self, Verification, require, wait},
};
use pttstella::connection::{Connection, Phase};
use slint::{
    ComponentHandle, Model,
    platform::{Key, PointerEventButton},
};
use std::{error::Error, path::PathBuf, rc::Rc};

async fn choose(ui: &TerminalWindow, category: i32, label: &str) -> Result<(), String> {
    ui.invoke_tools_toggle();
    wait(|| ui.get_tools_open(), "shell floating tools did not open").await?;
    ui.invoke_tools_category_choose(category);
    let items = ui.get_tools_actions();
    let index = (0..items.row_count())
        .find(|i| {
            items
                .row_data(*i)
                .is_some_and(|item| item.enabled && item.label == label)
        })
        .ok_or_else(|| format!("shell action missing or disabled: {label}"))?;
    ui.invoke_tools_action_choose(index as i32);
    require(!ui.get_tools_open(), "shell action did not dismiss tools")
}

pub fn start(
    ui: &TerminalWindow,
    connection: Rc<Connection>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "product-shell",
        move |ui, directory| async move {
            wait(
                || directory.join("prepared.json").is_file(),
                "shell preparation did not complete",
            )
            .await?;
            verification::foreground(&ui, true);
            ui.invoke_focus_terminal();
            wait(
                || !ui.get_tools_source_blocked() && ui.get_terminal_input_focused(),
                "shell input and tools were not ready",
            )
            .await?;
            require(
                !ui.get_prototype_chrome(),
                "full-service shell retained prototype chrome",
            )?;
            let dimensions = (ui.get_view_width(), ui.get_view_height());
            let window = ui.window().size();
            let dpi = ui.window().scale_factor();
            require(
                (dimensions.0 - window.width as f32 / dpi).abs() < 1.
                    && (dimensions.1 - window.height as f32 / dpi).abs() < 1.,
                "idle terminal does not fill the product window",
            )?;
            require(
                !ui.get_shell_status_visible(),
                "connected idle shell retained a status banner",
            )?;
            verification::capture(&ui, &directory, "shell-idle.png")?;
            let idle_image = ui.get_terminal_image();

            choose(&ui, 4, &pttstella::msg!("settings-about")).await?;
            require(
                ui.get_about_open()
                    && !ui.get_terminal_input_focused()
                    && !ui.get_keep_alive_surface_ready(),
                "license panel did not own the modal scope",
            )?;
            verification::capture(&ui, &directory, "shell-about.png")?;
            verification::key(&ui, Key::Escape);
            wait(
                || !ui.get_about_open() && ui.get_terminal_input_focused(),
                "license Escape did not restore terminal focus",
            )
            .await?;
            require(
                idle_image == ui.get_terminal_image(),
                "license modal repainted the terminal bitmap",
            )?;
            choose(&ui, 0, "125%").await?;
            wait(
                || ui.get_zoom() == 1.25 && !ui.get_frame_paint_pending(),
                "floating zoom did not install geometry",
            )
            .await?;
            choose(&ui, 0, "100%").await?;
            wait(
                || ui.get_zoom() == 1. && !ui.get_frame_paint_pending(),
                "floating zoom did not return to 100 percent",
            )
            .await?;

            ui.invoke_clear_selection();
            ui.invoke_output_action(3); // Actual row-export owner must reject no selection before OS I/O.
            wait(
                || !ui.get_input_notice().is_empty() && ui.get_shell_status_visible(),
                "export failure was hidden with prototype chrome",
            )
            .await?;
            require(
                !ui.get_output_busy(),
                "invalid selected-row export started an OS worker",
            )?;
            require(
                dimensions == (ui.get_view_width(), ui.get_view_height()),
                "notification changed terminal viewport",
            )?;
            verification::capture(&ui, &directory, "shell-output-error.png")?;
            require(
                ui.get_shell_notice_dismissible(),
                "local error cannot be acknowledged",
            )?;
            // Hit the actual standard button: calling the callback directly
            // would skip the focus acquired by a real notification click.
            let width = ui.window().size().width as f32 / ui.window().scale_factor();
            let panel_width = 620_f32.min(width - 32.);
            verification::click(
                &ui,
                slint::LogicalPosition::new(width / 2. + panel_width / 2. - 24., 36.),
                PointerEventButton::Left,
            );
            require(
                !ui.get_shell_notice_dismissible(),
                "persistent connection state exposes a no-op dismiss button",
            )?;
            require(
                !ui.get_shell_status_visible(),
                "acknowledged local error stayed open",
            )?;
            wait(
                || ui.get_terminal_input_focused(),
                "notice dismissal did not restore terminal focus",
            )
            .await?;
            require(
                connection.updates.borrow().input_epoch == 0,
                "shell controls sent station input",
            )?;
            choose(&ui, 3, &pttstella::msg!("disconnect")).await?;
            wait(
                || {
                    connection.updates.borrow().phase == Phase::Disconnected
                        && ui.get_connection_attention()
                        && ui.get_shell_status_visible()
                },
                "disconnected shell status is not persistent",
            )
            .await?;
            ui.invoke_tools_updated();
            ui.invoke_notice_dismiss();
            require(
                !ui.get_connection_status().is_empty()
                    && dimensions == (ui.get_view_width(), ui.get_view_height()),
                "disconnected status lost text or changed geometry",
            )?;
            verification::capture(&ui, &directory, "shell-disconnected.png")?;
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join("shell-result.json"))
                .map_err(|e| e.to_string())?;
            serde_json::to_writer_pretty(file,&serde_json::json!({"status":"passed","viewport_logical":[dimensions.0,dimensions.1],
            "prototype_chrome":false,"about_escape":true,"floating_zoom":true,"invalid_export_visible":true,"disconnected_visible":true,
            "scope":"production owners with synthetic toolkit callbacks/foreground; OS focus, clipboard and pointer untested"})).map_err(|e|e.to_string())
        },
    )
}
