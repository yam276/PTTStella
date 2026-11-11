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
    let state = || {
        serde_json::json!({
            "action":label,"category":category,"open":ui.get_tools_open(),
            "blocked":ui.get_tools_source_blocked(),"foreground":ui.get_native_foreground(),
            "composing":ui.get_native_composing(),"closing":ui.get_app_closing(),
            "modal":{"manager":ui.get_manager_open(),"settings":ui.get_settings_open(),
                "editor":ui.get_editor_open(),"station_color":ui.get_station_color_open(),
                "media":ui.get_media_open(),"find":ui.get_find_open(),"about":ui.get_about_open(),
                "login":ui.get_login_open(),"login_busy":ui.get_login_busy(),
                "paste":ui.get_paste_open(),"paste_reading":ui.get_paste_reading(),"output":ui.get_output_busy()}
        })
    };
    eprintln!("shell_action_before_toggle {}", state());
    ui.invoke_tools_toggle();
    if let Err(error) = wait(|| ui.get_tools_open(), "shell floating tools did not open").await {
        return Err(format!("{error}: {}", state()));
    }
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
    notices: Rc<crate::notice::Owner>,
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
            let window_width = window.width as f32 / dpi;
            let window_height = window.height as f32 / dpi;
            require(
                !ui.get_article_active()
                    && (dimensions.0 - window_width).abs() < 1.
                    && (dimensions.1 - window_height).abs() < 1.,
                &format!(
                    "idle terminal {dimensions:?} must fill the {window_width}×{window_height} logical window without a bottom status row"
                ),
            )?;
            require(
                !ui.get_shell_status_visible(),
                "connected idle shell retained a status banner",
            )?;
            verification::capture(&ui, &directory, "shell-idle.png")?;
            let idle_image = ui.get_terminal_image();

            ui.invoke_about_show();
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
            ui.set_zoom(1.25);
            wait(
                || ui.get_zoom() == 1.25 && !ui.get_frame_paint_pending(),
                "viewport zoom did not install geometry",
            )
            .await?;
            ui.set_zoom(1.);
            wait(
                || ui.get_zoom() == 1. && !ui.get_frame_paint_pending(),
                "viewport zoom did not return to 100 percent",
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
            crate::notice_probe::run(&ui, &notices, &directory).await?;
            require(
                connection.updates.borrow().input_epoch == 0,
                "notification Tab/hover sent station input",
            )?;
            verification::foreground(&ui, true);
            ui.invoke_tools_toggle();
            wait(
                || ui.get_tools_open(),
                "shell tools did not reopen after notice tests",
            )
            .await?;
            ui.invoke_tools_category_choose(0);
            wait(
                || ui.get_tools_category() == 0,
                "shell navigation panel did not open",
            )
            .await?;
            let labels = ui
                .get_tools_actions()
                .iter()
                .map(|item| item.label.to_string())
                .collect::<Vec<_>>();
            require(
                labels
                    == [
                        pttstella::msg!("draft-open"),
                        pttstella::msg!("destination-open"),
                    ],
                &format!("non-reading navigation controls differ: {labels:?}"),
            )?;
            ui.invoke_tools_key(7, false);
            ui.invoke_tools_category_choose(3);
            let session = ui.get_tools_actions();
            require(
                session.row_count() == 5
                    && session
                        .row_data(0)
                        .is_some_and(|item| item.label == pttstella::msg!("connect").as_str())
                    && session
                        .row_data(1)
                        .is_some_and(|item| item.label == pttstella::msg!("disconnect").as_str())
                    && session
                        .row_data(2)
                        .is_some_and(|item| item.label == pttstella::msg!("sites-title").as_str())
                    && session.row_data(3).is_some_and(|item| {
                        item.detail && !item.enabled && item.label.contains("Big5-UAO")
                    })
                    && session
                        .row_data(4)
                        .is_some_and(|item| item.label == pttstella::msg!("login").as_str()),
                "session menu did not match Dioxus order and station details",
            )?;
            ui.invoke_tools_dismiss(true);
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
            crate::notice::copied(&ui, "pending at close");
            require(
                notices.timer_active(),
                "exit probe has no live notification timer to cancel",
            )?;
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(directory.join("shell-result.json"))
                .map_err(|e| e.to_string())?;
            serde_json::to_writer_pretty(file,&serde_json::json!({"status":"passed","viewport_logical":[dimensions.0,dimensions.1],
            "window_logical":[window_width,window_height],"scale_factor":dpi,
            "prototype_chrome":false,"about_escape":true,"viewport_zoom":true,"invalid_export_visible":true,"disconnected_visible":true,
            "scope":"production owners with synthetic toolkit callbacks/foreground; OS focus, clipboard and pointer untested"})).map_err(|e|e.to_string())
        },
    )
}
