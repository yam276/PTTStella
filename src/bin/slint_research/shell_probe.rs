//! Full product shell checks; toolkit actions only, with the production owners.
use crate::{
    SelectionRect, TerminalWindow,
    verification::{self, Verification, require, wait},
};
use pttstella::connection::{Connection, Phase};
use slint::{
    ComponentHandle, Model, ModelRc, VecModel,
    platform::{Key, PointerEventButton},
};
use std::{
    error::Error,
    path::{Path, PathBuf},
    rc::Rc,
};

async fn settings_dropdowns(ui: &TerminalWindow, directory: &Path) -> Result<(), String> {
    let settings = ui.global::<crate::SettingsState>();
    settings.invoke_show_page(1);
    wait(
        || settings.get_families().row_count() > 0 && !ui.get_output_busy(),
        "installed font choices did not load on entry",
    )
    .await?;
    verification::capture(ui, directory, "settings-appearance.png")?;
    settings.invoke_focus_field(11);
    wait(
        || settings.get_focused_target() == 11,
        "font preset did not focus",
    )
    .await?;
    let original = settings.get_preset();
    verification::key(ui, Key::Return);
    verification::capture(ui, directory, "settings-preset-menu.png")?;
    verification::key(ui, Key::Home);
    verification::key(ui, Key::DownArrow);
    verification::key(ui, Key::Escape);
    require(
        ui.get_settings_open() && settings.get_preset() == original,
        "Escape changed the preset or closed settings",
    )?;
    verification::key(ui, Key::Return);
    verification::key(ui, Key::Home);
    verification::key(ui, Key::DownArrow);
    verification::key(ui, Key::Return);
    require(
        settings.get_preset() == 1,
        "keyboard did not select the preset",
    )?;

    let window = ui.window().size();
    let scale = ui.window().scale_factor();
    let height = window.height as f32 / scale;
    let panel_top = (height - 700_f32.min(height - 32.)) / 2.;
    let font =
        slint::LogicalPosition::new(window.width as f32 / scale / 2. + 85., panel_top + 337.);
    verification::click(ui, font, PointerEventButton::Left);
    verification::capture(ui, directory, "settings-local-font-menu.png")?;
    let families = settings.get_families();
    let last = families
        .row_data(families.row_count() - 1)
        .ok_or("missing last font choice")?;
    verification::key(ui, Key::End);
    verification::key(ui, Key::Return);
    require(
        settings.get_ascii() == last && settings.get_preset() == 8,
        "local font menu did not apply the selected family",
    )?;
    verification::capture(ui, directory, "settings-local-font-selected.png")?;
    settings.invoke_show_page(2);
    verification::capture(ui, directory, "settings-keyboard.png")?;
    settings.invoke_focus_field(6);
    wait(
        || settings.get_focused_target() == 6,
        "opacity slider did not take focus",
    )
    .await?;
    let opacity = settings.get_tool_opacity();
    let (key, step) = if opacity > 20 {
        (Key::LeftArrow, -1)
    } else {
        (Key::RightArrow, 1)
    };
    verification::key(ui, key);
    require(
        settings.get_tool_opacity() == opacity + step,
        "opacity slider did not update its value",
    )?;
    let track =
        slint::LogicalPosition::new(window.width as f32 / scale / 2. + 120., panel_top + 212.);
    verification::click(ui, track, PointerEventButton::Left);
    let selected_opacity = settings.get_tool_opacity();
    require(
        (20..=100).contains(&selected_opacity) && selected_opacity != opacity + step,
        "opacity slider did not respond to a track click",
    )?;
    settings.invoke_show_page(3);
    verification::capture(ui, directory, "settings-media.png")?;
    settings.invoke_show_page(4);
    wait(
        || ui.get_settings_open() && ui.get_sites_open() && ui.get_sites_embedded(),
        "sites form did not open within settings",
    )
    .await?;
    verification::capture(ui, directory, "settings-sites.png")?;

    settings.invoke_show_page(5);
    wait(
        || {
            ui.get_settings_open()
                && !ui.get_sites_open()
                && ui.get_filters_open()
                && ui.get_filters_embedded()
        },
        "filters form did not replace the inline sites form",
    )
    .await?;
    verification::capture(ui, directory, "settings-filters.png")?;

    settings.invoke_show_page(6);
    wait(
        || {
            ui.get_settings_open()
                && !ui.get_filters_open()
                && ui.get_login_open()
                && ui.get_login_embedded()
                && !ui.get_login_busy()
        },
        "account form did not replace the inline filters form",
    )
    .await?;
    verification::capture(ui, directory, "settings-account.png")?;
    settings.invoke_show_page(7);
    wait(
        || ui.get_settings_open() && !ui.get_login_open(),
        "switching to licenses did not close the inline account form",
    )
    .await?;
    verification::capture(ui, directory, "settings-about.png")?;
    settings.invoke_close_settings();
    wait(
        || !ui.get_settings_open() && !ui.get_login_open() && ui.get_terminal_input_focused(),
        "settings did not close the account form and restore terminal focus",
    )
    .await?;
    ui.invoke_settings_show();
    require(
        settings.get_tool_opacity() == selected_opacity,
        "opacity did not survive closing and reopening settings",
    )?;
    settings.invoke_close_settings();
    wait(
        || !ui.get_settings_open(),
        "reopened settings did not close",
    )
    .await
}

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

fn navigation_marker(ui: &TerminalWindow, directory: &Path) -> Result<(), String> {
    let width = ui.get_raster_width();
    let row = ui.get_raster_height() / 24.;
    let top = row * 5.;
    ui.set_navigation_rects(ModelRc::new(VecModel::from(vec![SelectionRect {
        x: 0.,
        y: top,
        width,
        height: row,
    }])));
    let snapshot = ui.window().take_snapshot();
    ui.set_navigation_rects(ModelRc::default());
    let pixels = snapshot.map_err(|e| e.to_string())?;
    let dpi = ui.window().scale_factor();
    let origin_x = ui.get_terminal_origin_x() + ui.get_raster_left();
    let y = ((ui.get_terminal_origin_y() + ui.get_raster_top() + top + row / 2.) * dpi).round()
        as usize;
    let is_gold = |x: usize| {
        let offset = (y * pixels.width() as usize + x) * 4;
        pixels.as_bytes().get(offset..offset + 4) == Some(&[217, 174, 91, 255])
    };
    let left_edge = ((origin_x + 1.5) * dpi).round() as usize;
    let right_edge = ((origin_x + width - 1.5) * dpi).round() as usize;
    let start = ((origin_x + 4.) * dpi).ceil() as usize;
    let end = ((origin_x + width - 4.) * dpi).floor() as usize;
    crate::save_png(&pixels, &directory.join("shell-navigation-marker.png"))
        .map_err(|error| error.to_string())?;
    require(
        start < end
            && end < pixels.width() as usize
            && is_gold(left_edge)
            && is_gold(right_edge)
            && !(start..end).any(is_gold),
        "navigation marker must have gold on both sides and none through the middle",
    )
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
            navigation_marker(&ui, &directory)?;
            let idle_image = ui.get_terminal_image();
            let fullscreen = ui.window().is_fullscreen();
            ui.invoke_tools_toggle();
            wait(|| ui.get_tools_open(), "settings tool did not open").await?;
            ui.invoke_tools_category_choose(4);
            wait(
                || {
                    ui.get_settings_open()
                        && ui.global::<crate::SettingsState>().get_focused_target() == 0
                },
                "settings panel did not focus its search field",
            )
            .await?;
            require(
                !ui.get_terminal_input_focused()
                    && ui.window().size() == window
                    && ui.window().is_fullscreen() == fullscreen,
                "opening settings changed the terminal window or left station focus active",
            )?;
            verification::capture(&ui, &directory, "shell-settings.png")?;
            if std::env::var_os("PTTSTELLA_SLINT_SETTINGS_LAYOUT").is_some() {
                return settings_dropdowns(&ui, &directory).await;
            }
            let scale = ui.window().scale_factor();
            let width = window.width as f32 / scale;
            let height = window.height as f32 / scale;
            let settings_close = slint::LogicalPosition::new(
                (width + 900_f32.min(width - 32.)) / 2. - 38.,
                (height + 700_f32.min(height - 32.)) / 2. - 31.,
            );
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerMoved {
                    position: settings_close,
                });
            verification::click(&ui, settings_close, PointerEventButton::Left);
            wait(
                || !ui.get_settings_open() && ui.get_terminal_input_focused(),
                "settings close button did not restore terminal focus",
            )
            .await?;

            ui.invoke_settings_show();
            if let Err(error) = wait(
                || {
                    ui.get_settings_open()
                        && ui.global::<crate::SettingsState>().get_focused_target() == 0
                },
                "reopened settings panel did not focus its search field",
            )
            .await
            {
                return Err(format!(
                    "{error}: open={}, foreground={}, target={}",
                    ui.get_settings_open(),
                    ui.get_native_foreground(),
                    ui.global::<crate::SettingsState>().get_focused_target()
                ));
            }
            verification::key(&ui, Key::Escape);
            wait(
                || !ui.get_settings_open() && ui.get_terminal_input_focused(),
                "settings Escape did not restore terminal focus",
            )
            .await?;

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
            let close = slint::LogicalPosition::new(width / 2. + panel_width / 2. - 36., 56.);
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerMoved { position: close });
            verification::click(&ui, close, PointerEventButton::Left);
            wait(
                || !ui.get_shell_notice_dismissible(),
                "notification close button did not dismiss the error",
            )
            .await?;
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
