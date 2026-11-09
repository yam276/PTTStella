//! Explicit bounded GPU checks against the owned 320x180 quadrant fixtures.
//! Toolkit gestures prove the production adapter, separately from native input.
use super::*;
use crate::{
    output::Output,
    verification::{self, Verification, require, wait},
};
use glow::HasContext;
use std::{error::Error, path::PathBuf, time::Duration};

/// # Safety
/// Called only after the production blit with the same live current GL context.
pub(super) unsafe fn capture(
    gl: &glow::Context,
    target: i32,
    rect: [i32; 4],
    height: i32,
) -> Result<serde_json::Value, String> {
    let [x, y, w, h] = rect;
    let scale = (f64::from(w) / 320.).min(f64::from(h) / 180.);
    let vw = 320. * scale;
    let vh = 180. * scale;
    let left = f64::from(x) + (f64::from(w) - vw) / 2.;
    let bottom = f64::from(height - y - h) + (f64::from(h) - vh) / 2.;
    let expected = [[255u8, 0, 0], [0, 0, 255], [255, 255, 0], [0, 255, 255]];
    let mut pixels = [[0u8; 4]; 4];
    // SAFETY: rect is validated inside the drawable by the producer. PACK
    // buffer is unbound and default state set. Each read writes four bytes.
    unsafe {
        gl.bind_framebuffer(
            glow::READ_FRAMEBUFFER,
            std::num::NonZeroU32::new(target as u32).map(glow::NativeFramebuffer),
        );
        let previous = gl.get_parameter_i32(glow::READ_BUFFER);
        gl.read_buffer(if target == 0 {
            glow::BACK
        } else {
            glow::COLOR_ATTACHMENT0
        });
        for (pixel, (fx, fy)) in
            pixels
                .iter_mut()
                .zip([(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)])
        {
            gl.read_pixels(
                (left + vw * fx) as i32,
                (bottom + vh * (1. - fy)) as i32,
                1,
                1,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(pixel)),
            );
        }
        gl.read_buffer(previous as u32);
    }
    let passed = pixels
        .iter()
        .zip(expected)
        .all(|(a, b)| a[..3].iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 30));
    Ok(serde_json::json!({"rect":rect,"pixels":pixels,"expected":expected,"passed":passed}))
}
pub fn start(
    ui: &TerminalWindow,
    media: Rc<Media>,
    output: Rc<Output>,
    directory: PathBuf,
    _settings_path: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "native-video",
        move |ui, directory| async move {
            let checked = async {
                let video = ready(&ui, &media, &output).await?;
            if std::env::var_os("PTTSTELLA_VIDEO_PROBE_FAIL_READY").is_some() {
                return Err("deliberate video verifier failure after renderer setup".into());
            }
            play(&ui).await?;
            if std::env::var_os("PTTSTELLA_VIDEO_STALL").is_some() {
                wait(
                    || {
                        video
                            .current
                            .borrow()
                            .as_ref()
                            .is_some_and(|v| v.session.status.lock().is_ok_and(|s| s.loading))
                    },
                    "stalled video never started",
                )
                .await?;
                tokio::time::sleep(Duration::from_millis(120)).await;
            } else if std::env::var_os("PTTSTELLA_VIDEO_REJECT").is_some() {
                wait(
                    || ui.get_media_status() == msg!("video-unsupported"),
                    "non-video payload was not rejected",
                )
                .await?;
                require(!ui.get_media_playing(), "rejected payload is playing")?;
            } else {
                wait(
                    || ui.get_media_playing() && ui.get_media_duration() > 2.9,
                    "video did not load/play",
                )
                .await?;
                ui.invoke_media_seek(1.);
                ui.invoke_media_action(2);
                wait(
                    || !ui.get_media_playing() && (ui.get_media_position() - 1.).abs() < 0.1,
                    "video seek/pause not acknowledged",
                )
                .await?;
                video.bridge.capture.set(true);
                ui.window().request_redraw();
                wait(
                    || !video.bridge.samples.borrow().is_empty(),
                    "GPU capture did not run",
                )
                .await?;
                require(
                    video
                        .bridge
                        .samples
                        .borrow()
                        .iter()
                        .all(|s| s["passed"] == true),
                    "main preview GPU pixels differ from fixture",
                )?;
                ui.invoke_media_action(2);
                wait(
                    || ui.get_media_playing() && ui.get_media_position() > 1.15,
                    "resume did not advance",
                )
                .await?;
                verification::foreground(&ui, false);
                wait(|| !ui.get_media_playing(), "focus loss did not pause video").await?;
                verification::foreground(&ui, true);
                tokio::time::sleep(Duration::from_millis(100)).await;
                require(
                    !ui.get_media_playing(),
                    "focus regain resumed without permission",
                )?;
            }
            media.close(true);
            wait(
                || video.current.borrow().is_none(),
                "video close did not join session",
            )
            .await?;
            require(
                video.bridge.rendered.get().is_none() && video.bridge.offer.borrow().is_none(),
                "video retained GPU/core references",
            )?;
            // Replacement during asynchronous startup must not leave a stale view.
            ui.invoke_media_request(3);
            ui.invoke_media_action(0);
            ui.invoke_media_request(0);
            wait(
                || ui.get_media_open() && !ui.get_media_video() && ui.get_media_ready(),
                "image did not replace cancelled video startup",
            )
            .await?;
            wait(
                || video.current.borrow().is_none(),
                "cancelled startup survived image replacement",
            )
            .await?;
            media.close(true);
            Ok::<_, String>(serde_json::json!({"scope":"main Slint window; actual GPU; synthetic toolkit controls; not OS input/audio audibility/performance","passed":true,"samples":&*video.bridge.samples.borrow(),"error":&*video.bridge.error.borrow(),"render_retired":video.bridge.rendered.get().is_none(),"session_joined":video.current.borrow().is_none()}))
            }.await;
            if let Err(error) = &checked {
                let state = media.video.borrow().as_ref().map(|video| {
                    let session = video.current.borrow().as_ref().map(|live| {
                        let snapshot = live.session.updates().borrow().clone();
                        serde_json::json!({"stage":format!("{:?}", snapshot.stage),
                            "loading":snapshot.status.loading,"loaded":snapshot.status.loaded,
                            "paused":snapshot.status.paused,
                            "status":snapshot.status})
                    });
                    serde_json::json!({"session":session,"rendered":video.bridge.rendered.get(),
                        "offer":video.bridge.offer.borrow().is_some(),
                        "renderer_error":video.bridge.error.borrow().is_some(),
                        "renderer_error_detail":&*video.bridge.error.borrow(),
                        "samples":&*video.bridge.samples.borrow()})
                });
                let audit = serde_json::json!({"status":"failed","error":error,
                    "foreground":ui.get_native_foreground(),"play_allowed":ui.get_media_play_allowed(),
                    "ready":ui.get_media_ready(),"position":ui.get_media_position(),
                    "duration":ui.get_media_duration(),"video":state});
                let saved = serde_json::to_vec_pretty(&audit)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        std::fs::write(directory.join("failure-state.json"), bytes)
                            .map_err(|e| e.to_string())
                    });
                if let Err(error) = saved {
                    eprintln!("video failure audit: {error}");
                }
            }
            // Retire the GL owner while Slint can still render, on every exit.
            // Keep the original assertion error even when cleanup also fails.
            let cleanup = media.shutdown().await;
            let summary = match (checked, cleanup) {
                (Ok(summary), Ok(())) => summary,
                (Err(error), Ok(())) | (Ok(_), Err(error)) => return Err(error),
                (Err(error), Err(cleanup)) => {
                    return Err(format!("{error}; media shutdown: {cleanup}"));
                }
            };
            std::fs::write(
                directory.join("video.json"),
                serde_json::to_vec_pretty(&summary).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())
        },
    )
}

/// Shared explicit toolkit setup for content and lifetime probes.
pub(super) async fn ready(
    ui: &TerminalWindow,
    media: &Media,
    output: &Output,
) -> Result<Rc<Video>, String> {
    if std::env::var_os("PTTSTELLA_VIDEO_PROBE_NATIVE_WAIT").is_some() {
        let mut active = false;
        for _ in 0..6_000 {
            if ui.get_native_foreground() {
                active = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        require(
            active,
            "native video probe did not receive foreground activation",
        )?;
    }
    wait(
        || {
            verification::foreground(ui, true);
            output.current(ui).is_some()
        },
        "video input never ready",
    )
    .await?;
    let video = media
        .video
        .borrow()
        .clone()
        .ok_or("native video owner missing")?;
    ui.invoke_media_request(3);
    wait(
        || ui.get_media_video() && ui.get_media_ready(),
        "video did not become ready without playback",
    )
    .await?;
    require(
        !ui.get_media_playing() && ui.get_media_duration() == 0.,
        "video autoloaded",
    )?;
    Ok(video)
}

pub(super) async fn play(ui: &TerminalWindow) -> Result<(), String> {
    // The diagnostic window never claims native focus at launch.
    // Explicitly activate this toolkit gesture, then give the same
    // production permit binding one UI turn before pressing Play.
    // Match other full-App probes: let initial Focused/Occluded
    // notifications settle before this explicit foreground gesture.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let activated = std::cell::Cell::new(false);
    wait(
        || {
            verification::foreground(ui, true);
            activated.replace(true) && ui.get_media_play_allowed()
        },
        "video toolkit Play was not admitted",
    )
    .await?;
    ui.invoke_media_action(2);
    Ok(())
}
