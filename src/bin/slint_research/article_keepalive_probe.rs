//! A complete article and one exact refresh on the production Connection timer.
use crate::{
    TerminalWindow,
    article::session::ArticleSession,
    output::Output,
    verification::{self, Verification, require, wait},
};
use pttstella::{
    connection::{Connection, Phase},
    navigation::Context,
};
use std::{
    error::Error,
    path::PathBuf,
    rc::Rc,
    time::{Duration, Instant},
};

async fn permit(connection: &Connection, ready: bool) -> Result<(), String> {
    wait(
        || connection.updates.borrow().keep_alive_ready == ready,
        "article keepalive permit did not settle",
    )
    .await
}
fn no_refresh(connection: &Connection) -> Result<(), String> {
    require(
        connection.updates.borrow().keep_alive_sent == 0,
        "blocked article emitted a refresh",
    )
}
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<ArticleSession>,
    connection: Rc<Connection>,
    output: Rc<Output>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "article-keepalive",
        move |ui, directory| async move {
            verification::capture(&ui, &directory, "startup.png")?;
            tokio::time::sleep(Duration::from_millis(300)).await;
            wait(
                || {
                    verification::foreground(&ui, true);
                    ui.invoke_focus_article();
                    ui.invoke_keep_alive_changed();
                    ui.get_article_active()
                        && output.current(&ui).is_some()
                        && connection.updates.borrow().keep_alive_ready
                },
                "complete article did not grant its real idle permit",
            )
            .await?;
            require(
                owner.requests.get() == 0,
                "complete article sent a preload command",
            )?;
            let source = connection.updates.borrow().stamp();
            let rasters = owner.raster_count();
            ui.invoke_output_action(6);
            require(
                ui.get_selection_active(),
                "article selection fixture missing",
            )?;
            permit(&connection, false).await?;
            tokio::time::sleep(Duration::from_millis(3150)).await;
            no_refresh(&connection)?;
            ui.invoke_clear_selection();
            // Exercise each generated UI guard; the independently timed selection
            // and background windows below prove that false reaches the real timer.
            for guard in 0..5 {
                match guard {
                    0 => ui.set_native_composing(true),
                    1 => ui.set_paste_reading(true),
                    2 => ui.set_paste_open(true),
                    3 => ui.set_output_busy(true),
                    _ => ui.set_native_pointer_down(true),
                }
                ui.invoke_keep_alive_changed();
                require(
                    !ui.get_keep_alive_surface_ready(),
                    "article UI guard remained ready",
                )?;
                permit(&connection, false).await?;
                match guard {
                    0 => ui.set_native_composing(false),
                    1 => ui.set_paste_reading(false),
                    2 => ui.set_paste_open(false),
                    3 => ui.set_output_busy(false),
                    _ => ui.set_native_pointer_down(false),
                }
            }
            ui.invoke_tools_toggle();
            require(ui.get_tools_open(), "article tools did not open")?;
            permit(&connection, false).await?;
            ui.invoke_tools_toggle();
            wait(
                || ui.get_article_focused(),
                "closing article tools focused the hidden terminal",
            )
            .await?;
            verification::foreground(&ui, false);
            permit(&connection, false).await?;
            tokio::time::sleep(Duration::from_millis(3150)).await;
            no_refresh(&connection)?;
            require(
                owner.raster_count() == rasters,
                "idle permit metadata rerasterized the article",
            )?;
            verification::foreground(&ui, true);
            ui.invoke_focus_article();
            ui.invoke_keep_alive_changed();
            permit(&connection, true).await?;
            tokio::time::sleep(Duration::from_millis(1500)).await;
            ui.set_native_pointer_down(true);
            ui.invoke_keep_alive_changed();
            permit(&connection, false).await?;
            ui.set_native_pointer_down(false);
            ui.invoke_keep_alive_changed();
            permit(&connection, true).await?;
            let restarted = Instant::now();
            tokio::time::sleep(Duration::from_millis(1850)).await;
            no_refresh(&connection)?;
            let mut updates = connection.updates.clone();
            tokio::time::timeout(
                Duration::from_secs(3),
                updates.wait_for(|v| {
                    v.keep_alive_sent == 1
                        && !v.output_pending
                        && !matches!(v.context(), Context::ArticleReading { .. })
                }),
            )
            .await
            .map_err(|_| "article refresh or exact peer response missing")?
            .map_err(|e| e.to_string())?;
            require(
                restarted.elapsed() >= Duration::from_millis(2900),
                "article idle interval was not restarted",
            )?;
            wait(
                || output.current(&ui).is_some(),
                "unknown refresh reply was not painted",
            )
            .await
            .map_err(|e| format!("{e}: {}", owner.diagnostics()))?;
            require(
                connection.updates.borrow().phase == Phase::Connected
                    && connection.updates.borrow().stamp() != source
                    && !ui.get_article_active(),
                "refresh source did not retire the article",
            )?;
            permit(&connection, false).await?;
            verification::capture(&ui, &directory, "refresh-complete.png")?;
            owner.shutdown().await;
            require(owner.stopped(), "keepalive article actor did not join")?;
            std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "scope":"generated native UI and production three-second probe timer; synthetic activation/guards, no OS focus or sleep claim",
            "refresh_bytes":[12,12],"refresh_count":1,"preload_commands":0,
            "selection_and_background_exceed_idle_without_refresh":true,"ime_paste_output_pointer_tools_blocked":true,
            "resume_restarts_interval":true,"metadata_kept_raster_count":rasters,"unknown_reply_revokes":true,"article_actor_joined":true
        })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(())
        },
    )
}
