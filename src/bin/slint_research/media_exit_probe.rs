//! Explicit full-window exit checks. This probe never drains video itself:
//! only the same normal app-quit/window-close owner may do that work.
use super::*;
use crate::{
    output::Output,
    verification::{Verification, require, wait},
};
use slint::{platform::WindowEvent, winit_030::WinitWindowAccessor};
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Duration,
};

pub fn start(
    ui: &TerminalWindow,
    media: Rc<Media>,
    output: Rc<Output>,
    directory: PathBuf,
    _settings_path: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    let route = std::env::var("PTTSTELLA_VIDEO_PROBE_EXIT")?;
    let stage =
        std::env::var("PTTSTELLA_VIDEO_PROBE_EXIT_STAGE").unwrap_or_else(|_| "playing".into());
    require(
        matches!(
            route.as_str(),
            "quit" | "close" | "native" | "minimized-quit"
        ),
        "unknown video exit route",
    )?;
    require(
        matches!(stage.as_str(), "ready" | "playing" | "stall"),
        "unknown video exit stage",
    )?;
    let start = if route == "native" {
        Verification::prepare_for_native_close
    } else {
        Verification::start
    };
    start(
        ui,
        directory,
        "video-exit-preparation",
        move |ui: TerminalWindow, directory: PathBuf| async move {
            let video = probe::ready(&ui, &media, &output).await?;
            if stage != "ready" {
                probe::play(&ui).await?;
                if stage == "stall" {
                    wait(
                        || {
                            video
                                .current
                                .borrow()
                                .as_ref()
                                .is_some_and(|v| v.session.status.lock().is_ok_and(|s| s.loading))
                        },
                        "exit fixture never reached stalled load",
                    )
                    .await?;
                    tokio::time::sleep(Duration::from_millis(120)).await;
                } else {
                    wait(
                        || {
                            ui.get_media_playing()
                                && ui.get_media_duration() > 2.9
                                && ui.get_media_position() > 0.2
                        },
                        "exit fixture did not play",
                    )
                    .await?;
                    video.bridge.capture.set(true);
                    ui.window().request_redraw();
                    wait(
                        || !video.bridge.samples.borrow().is_empty(),
                        "exit fixture GPU capture missing",
                    )
                    .await?;
                    let samples = serde_json::to_vec_pretty(&*video.bridge.samples.borrow())
                        .map_err(|e| e.to_string())?;
                    std::fs::write(directory.join("gpu-samples.json"), samples)
                        .map_err(|e| e.to_string())?;
                    require(
                        video
                            .bridge
                            .samples
                            .borrow()
                            .iter()
                            .all(|sample| sample["passed"] == true),
                        "exit fixture GPU pixels differ",
                    )?;
                }
            }
            require(
                video.bridge.rendered.get().is_some() && video.current.borrow().is_some(),
                "exit fixture has no active renderer/session",
            )?;
            std::fs::write(directory.join("video-exit-ready.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "status":"prepared", "route":route, "stage":stage,
            "renderer_owned":true, "session_owned":true, "playing":ui.get_media_playing(),
            "gpu_samples":&*video.bridge.samples.borrow(),
            "scope":"Preparation only; post-loop resource and peer audits determine exit success"
        })).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            if route == "native" {
                return Ok(());
            }
            if route == "minimized-quit" {
                ui.window()
                    .with_winit_window(|window| window.set_minimized(true));
                tokio::time::sleep(Duration::from_millis(150)).await;
                let minimized = ui
                    .window()
                    .with_winit_window(|window| window.is_minimized());
                require(
                    minimized == Some(Some(true)),
                    "backend did not confirm minimized window",
                )?;
            }
            let sequence = media.sequence.get();
            if route == "close" {
                ui.window().dispatch_event(WindowEvent::CloseRequested);
                ui.window().dispatch_event(WindowEvent::CloseRequested);
            } else {
                ui.invoke_app_quit();
                ui.invoke_app_quit();
            }
            require(
                ui.get_app_closing()
                    && !ui.get_media_play_allowed()
                    && !ui.get_keep_alive_surface_ready(),
                "exit did not immediately revoke input/media permissions",
            )?;
            // Late requests must not reopen managers or replace the closing view.
            ui.invoke_settings_show();
            ui.invoke_sites_show();
            ui.invoke_media_request(0);
            require(
                !ui.get_settings_open() && !ui.get_sites_open() && media.sequence.get() == sequence,
                "a late action entered during exit",
            )?;
            Ok(())
        },
    )
}

/// Called after normal Media.finish, never by the pre-exit probe future.
pub fn audit_closed(media: &Media, directory: &Path) -> Result<(), String> {
    let video = media.video.borrow();
    let video = video.as_ref().ok_or("exit video owner missing")?;
    require(
        media.closed.get()
            && media.worker.borrow().is_none()
            && media.task.borrow().is_none()
            && video.closed.get()
            && video.current.borrow().is_none()
            && video.pending.borrow().is_none()
            && video.bridge.rendered.get().is_none()
            && video.bridge.offer.borrow().is_none()
            && video.task.borrow().is_none()
            && video.cleanup_result().is_ok(),
        "normal exit did not retire all media owners",
    )?;
    std::fs::write(directory.join("video-exit-closed.json"), serde_json::to_vec_pretty(&serde_json::json!({
        "status":"passed", "normal_exit":true, "renderer_retired":true, "session_joined":true,
        "pending_empty":true, "offer_empty":true, "video_task_empty":true, "image_worker_empty":true,
        "scope":"Normal app exit owners; strict BBS Close and HTTP counts recorded separately"
    })).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
