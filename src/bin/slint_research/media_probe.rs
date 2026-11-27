//! Explicit full-component toolkit gestures, owned HTTP and isolated settings.
use super::*;
use crate::verification::{self, Verification, require, wait};
use slint::platform::{PointerEventButton as Button, WindowEvent as Event};
use std::{error::Error, path::PathBuf, sync::Arc, time::Duration};

fn point(
    ui: &TerminalWindow,
    output: &Output,
    row: usize,
    col: f32,
) -> Result<slint::LogicalPosition, String> {
    let g = output
        .geometry(ui)
        .and_then(crate::pointer::SurfaceGeometry::terminal)
        .ok_or("media geometry missing")?;
    Ok(slint::LogicalPosition::new(
        ui.get_terminal_origin_x() + (g.left + g.size.width as f32 * col / 80.) / g.dpi,
        ui.get_terminal_origin_y()
            + (g.top + g.size.height as f32 * (row as f32 + 0.5) / 24.) / g.dpi,
    ))
}
fn moved(ui: &TerminalWindow, p: slint::LogicalPosition) {
    ui.window()
        .dispatch_event(Event::PointerMoved { position: p });
}
fn source(ui: &TerminalWindow, output: &Output) -> Result<Vec<u8>, String> {
    output
        .with_frame(ui, serde_json::to_vec)
        .ok_or("media source missing")?
        .map_err(|e| e.to_string())
}
fn bounds(ui: &TerminalWindow) -> Result<(), String> {
    let b = ui.get_media_hover_bounds();
    require(
        b.x >= 11.9
            && b.y >= 11.9
            && b.x + b.width <= ui.get_view_width() - 11.9
            && b.y + b.height <= ui.get_view_height() - 11.9
            && b.width <= 560.1
            && b.width <= ui.get_view_width() * 0.55 + 0.1
            && b.height <= ui.get_view_height() * 0.6 + 0.1,
        "hover card escaped viewport/budget",
    )
}
async fn preference(ui: &TerminalWindow, value: i32) -> Result<(), String> {
    ui.set_media_hover_choice(value);
    ui.invoke_media_hover_preference(value);
    wait(
        || !ui.get_output_busy() && ui.get_media_hover_position() == value,
        "hover preference did not save/apply",
    )
    .await
}
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<Media>,
    output: Rc<Output>,
    directory: PathBuf,
    settings_path: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "media-hover",
        move |ui, directory| async move {
            wait(
                || {
                    verification::foreground(&ui, true);
                    output.current(&ui).is_some()
                },
                "media input never became ready",
            )
            .await?;
            tokio::time::sleep(Duration::from_millis(300)).await;
            verification::foreground(&ui, true);
            ui.invoke_focus_terminal();
            wait(
                || ui.get_terminal_input_focused() && output.current(&ui).is_some(),
                "media toolkit focus did not settle",
            )
            .await?;
            let settings_probe = std::env::var_os("PTTSTELLA_SLINT_SETTINGS_PROBE");
            if settings_probe
                .as_ref()
                .is_some_and(|mode| mode == "releases")
            {
                return owner.preferences.verify_releases(&ui, &directory).await;
            }
            if let Some(mode) = settings_probe {
                owner
                    .preferences
                    .verify(&ui, &directory, &settings_path)
                    .await?;
                verification::foreground(&ui, true);
                ui.invoke_focus_terminal();
                wait(
                    || {
                        ui.get_terminal_input_focused()
                            && !ui.get_media_hover_blocked()
                            && output.current(&ui).is_some()
                    },
                    "media focus and source did not resume after settings",
                )
                .await?;
                // The settings-search case ends after restoring terminal focus.
                // The media probe has its own pointer and HTTP assertions.
                if mode == "search" {
                    return Ok(());
                }
            }
            moved(&ui, slint::LogicalPosition::new(2., 2.));
            let original = source(&ui, &output)?;
            verification::capture(&ui, &directory, "before.png")?;
            let png = point(&ui, &output, 2, 20.)?;
            moved(&ui, png);
            tokio::time::sleep(Duration::from_millis(60)).await;
            moved(&ui, slint::LogicalPosition::new(2., 2.));
            tokio::time::sleep(Duration::from_millis(410)).await;
            require(
                owner.sequence.get() == 0
                    && owner.worker.borrow().is_none()
                    && !owner.dwell.running(),
                "quick pass started HTTP/worker or retained a timer",
            )?;
            moved(&ui, png);
            tokio::time::sleep(Duration::from_millis(200)).await;
            require(owner.sequence.get() == 0, "hover started before dwell")?;
            moved(&ui, point(&ui, &output, 2, 21.)?);
            tokio::time::sleep(Duration::from_millis(220)).await;
            require(
                ui.get_media_hover_open(),
                &format!(
                    "same-link dwell did not open: foreground={}, focused={}, blocked={}, timer={}, candidate={}, sequence={}",
                    ui.get_native_foreground(),
                    ui.get_terminal_input_focused(),
                    ui.get_media_hover_blocked(),
                    owner.dwell.running(),
                    owner.candidate.borrow().is_some(),
                    owner.sequence.get()
                ),
            )?;
            wait(|| ui.get_media_ready(), "PNG hover did not decode").await?;
            require(
                ui.get_terminal_input_focused()
                    && !ui.get_media_open()
                    && source(&ui, &output)? == original,
                "hover changed focus, source or modal state",
            )?;
            bounds(&ui)?;
            verification::capture(&ui, &directory, "top-right.png")?;
            let request = owner.sequence.get();
            let image = owner
                .state
                .borrow()
                .original
                .clone()
                .ok_or("PNG hover lacks original")?;
            verification::click(&ui, png, Button::Left);
            wait(
                || ui.get_media_open() && owner.state.borrow().inspect,
                "PNG hover did not promote",
            )
            .await?;
            require(
                owner.sequence.get() == request
                    && Arc::ptr_eq(
                        &image,
                        owner
                            .state
                            .borrow()
                            .original
                            .as_ref()
                            .ok_or("promoted original missing")?,
                    ),
                "promotion replaced original/request",
            )?;
            verification::capture(&ui, &directory, "promoted.png")?;

            // The address row ends with 34px buttons and an 8px gap, inside
            // the preview panel's 12px margin and 12px padding.
            let width = ui
                .window()
                .size()
                .to_logical(ui.window().scale_factor())
                .width;
            let close = slint::LogicalPosition::new(width - 41., 41.);
            moved(&ui, close);
            slint::platform::update_timers_and_animations();
            require(
                ui.get_media_toolbar_tooltip() == ui.get_media_close_label(),
                "Close tooltip did not appear on the pointer event",
            )?;
            verification::capture(&ui, &directory, "toolbar-hover.png")?;
            moved(&ui, slint::LogicalPosition::new(close.x - 42., close.y));
            slint::platform::update_timers_and_animations();
            require(
                ui.get_media_toolbar_tooltip() == ui.get_media_external_label(),
                "Browser tooltip did not replace Close immediately",
            )?;
            moved(&ui, slint::LogicalPosition::new(2., 2.));
            slint::platform::update_timers_and_animations();
            require(
                ui.get_media_toolbar_tooltip().is_empty(),
                "Toolbar tooltip persisted after pointer exit",
            )?;
            ui.invoke_media_action(4);
            wait(
                || !ui.get_output_busy() && ui.get_media_status() == msg!("export-copied"),
                "shared preview PNG copy failed",
            )
            .await?;
            verification::click(&ui, close, Button::Left);
            wait(
                || !ui.get_media_open(),
                "Close tool did not dismiss preview",
            )
            .await?;
            moved(&ui, slint::LogicalPosition::new(2., 2.));
            let gif = point(&ui, &output, 4, 20.)?;
            moved(&ui, gif);
            wait(
                || ui.get_media_hover_open() && ui.get_media_ready(),
                "GIF hover did not decode",
            )
            .await?;
            let first = owner.state.borrow().clone();
            require(
                !first.playing
                    && first
                        .pixels
                        .as_ref()
                        .is_some_and(|p| p.as_bytes()[..4] == [255, 0, 0, 255]),
                "GIF hover is not red first frame",
            )?;
            tokio::time::sleep(Duration::from_millis(650)).await;
            require(
                owner.state.borrow().frame == first.frame && !ui.get_media_playing(),
                "GIF advanced while hovering",
            )?;
            verification::capture(&ui, &directory, "gif-first.png")?;
            verification::click(&ui, gif, Button::Left);
            wait(
                || ui.get_media_open() && owner.state.borrow().playing,
                "GIF promotion never started playback",
            )
            .await?;
            require(
                owner.sequence.get() == first.request,
                "GIF promotion replaced request",
            )?;
            ui.invoke_media_action(4);
            wait(
                || !ui.get_output_busy() && ui.get_media_status() == msg!("export-copied"),
                "shared preview GIF copy failed",
            )
            .await?;
            tokio::time::sleep(Duration::from_millis(340)).await;
            require(
                ui.get_media_status() == msg!("export-copied"),
                "GIF frame replaced copy feedback",
            )?;
            ui.invoke_media_action(0);
            moved(&ui, slint::LogicalPosition::new(2., 2.));
            preference(&ui, 2).await?;
            let last = owner.sequence.get();
            moved(&ui, png);
            tokio::time::sleep(Duration::from_millis(410)).await;
            require(
                owner.sequence.get() == last && !ui.get_media_hover_open(),
                "disabled hover downloaded a new image",
            )?;
            require(
                ui.get_pointer_cursor() == crate::PointerKind::Action,
                "disabled media hover removed the link cursor",
            )?;
            preference(&ui, 1).await?;
            moved(&ui, png);
            wait(
                || ui.get_media_hover_open() && ui.get_media_ready(),
                "follow hover did not open",
            )
            .await?;
            bounds(&ui)?;
            verification::capture(&ui, &directory, "follow.png")?;
            // Geometry-only boundary cases are synthetic properties, not hit tests.
            for (x, y) in [
                (0., 0.),
                (ui.get_view_width(), 0.),
                (0., ui.get_view_height()),
                (ui.get_view_width(), ui.get_view_height()),
            ] {
                ui.set_media_hover_x(x);
                ui.set_media_hover_y(y);
                bounds(&ui)?;
            }
            verification::capture(&ui, &directory, "corner-clamp.png")?;
            verification::foreground(&ui, false);
            tokio::time::sleep(Duration::from_millis(410)).await;
            require(
                !ui.get_media_hover_open()
                    && owner.active.borrow().is_none()
                    && !owner.dwell.running(),
                "background hover survived cancellation",
            )?;
            verification::foreground(&ui, true);
            tokio::time::sleep(Duration::from_millis(410)).await;
            require(
                !ui.get_media_hover_open(),
                "focus alone revived stale hover",
            )?;
            let saved = owner
                .preferences
                .model
                .borrow()
                .store()?
                .read()?
                .ok_or("hover settings were not written")?;
            let settings = pttstella::settings::AppSettings::decode(&saved)?;
            require(
                settings.hover == pttstella::settings::HoverPosition::FollowCursor,
                "saved hover preference differs",
            )?;
            std::fs::write(directory.join("saved-settings.json"), saved)
                .map_err(|e| e.to_string())?;
            // A concurrent same-field write must not silently win in this UI.
            let baseline = owner.preferences.model.borrow().app().clone();
            let mut external = baseline.clone();
            external.hover = pttstella::settings::HoverPosition::Off;
            owner
                .preferences
                .model
                .borrow()
                .store()?
                .save_from(&baseline, external)?;
            ui.set_media_hover_choice(0);
            ui.invoke_media_hover_preference(0);
            wait(
                || !ui.get_output_busy(),
                "failed preference worker did not drain",
            )
            .await?;
            require(
                ui.get_media_hover_position() == 1
                    && ui.get_media_hover_choice() == 0
                    && ui
                        .get_input_notice()
                        .starts_with(&msg!("settings-save-failed"))
                    && owner.preferences.model.borrow().store()?.load()?.hover
                        == pttstella::settings::HoverPosition::Off,
                "same-field conflict changed the applied value, lost the draft, or overwrote external data",
            )?;
            wait(
                || output.current(&ui).is_some(),
                "failed-save layout did not settle",
            )
            .await?;
            require(
                source(&ui, &output)? == original,
                "hover operations modified terminal source",
            )?;
            // Explicitly withdraw this failed edit back to the still-effective
            // choice. The external conflicting file remains unchanged.
            preference(&ui, 1).await?;
            std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","scope":"actual Slint component toolkit pointer gestures and isolated settings, synthetic foreground; OS pointer/focus not covered","quick_pass_worker_started":false,"dwell_ms":350,"same_link_dwell_preserved":true,"png_promoted_same_request_and_arc":true,"gif_hover_static_ms":650,"gif_promotion_playing":true,"disabled_hover_starts":0,"follow_and_four_synthetic_corner_bounds":true,"focus_alone_revives":false,"terminal_source_unchanged":true,"settings_written_via_shared_store":true,"disabled_media_preserves_link_hints":true,"save_conflict_preserves_draft_applied_value_and_external_data":true})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            if std::env::var("PTTSTELLA_SLINT_SETTINGS_PROBE").is_ok_and(|v| v == "shutdown") {
                owner.preferences.prepare_shutdown(&directory)?;
            }
            Ok(())
        },
    )
}
