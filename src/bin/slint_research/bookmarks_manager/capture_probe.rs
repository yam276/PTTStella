//! Strict peer conformance of the production native capture and local reader.
use super::*;
use crate::verification::{self, Verification, require, wait};
use pttstella::{article_cache::LoadState, navigation::Context};
use slint::Model;
use std::{error::Error, path::PathBuf, time::Duration};

async fn full(ui: &TerminalWindow, o: &Manager) -> Result<(), String> {
    wait(
        || {
            verification::foreground(ui, true);
            ui.invoke_focus_article();
            o.article.progress().is_some_and(|(_, first, last, state)| {
                first == 1 && last == 45 && state == Some(LoadState::Complete)
            }) && o.current_available()
        },
        "full article did not reach a current native ACK",
    )
    .await
}
fn independent_file(o: &Manager, headerless: bool) -> Result<(), String> {
    let data = o.store.clone()?.load()?;
    require(
        data.entries().len() == 1,
        "capture did not save exactly one article",
    )?;
    let e = &data.entries()[0];
    require(
        e.target().aid().as_str() == "00000101" && e.target().board().as_str() == "Stella",
        "wrong captured identity",
    )?;
    let a = e.archive().ok_or("archive absent")?;
    require(a.row_count() == 45, "partial archive was persisted")?;
    let frame = a.filtered(&pttstella::filters::Rules::default());
    let rows = pttstella::text::rows(&frame).ok_or("archive glyph layout invalid")?;
    let text = |row: usize| rows[row].text.as_str();
    require(text(44).contains("REPLY 0045"), "last original row missing")?;
    if headerless {
        require(
            text(0).contains("無標頭的 ANSI 圖") && e.label().is_none(),
            "headerless original/title changed",
        )?;
    }
    Ok(())
}
pub fn start(
    ui: &TerminalWindow,
    o: Rc<Manager>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "bookmark-capture",
        move |ui, directory| async move {
            let case = std::env::var("PTTSTELLA_SLINT_CAPTURE_PROBE").map_err(|e| e.to_string())?;
            let list = case == "list";
            if list {
                wait(
                    || {
                        verification::foreground(&ui, true);
                        o.output.current(&ui).is_some()
                    },
                    "list source not acknowledged",
                )
                .await?;
                let (row, col) = match o.connection.updates.borrow().context() {
                    Context::ArticleList {
                        articles,
                        cursor_row,
                        ..
                    } => {
                        let target = articles
                            .iter()
                            .find(|a| a.bookmark && a.row != cursor_row)
                            .ok_or("different clicked row missing")?;
                        (target.row, target.title_start + 2)
                    }
                    _ => return Err("list not recognized".into()),
                };
                let identity = o
                    .article
                    .output_identity(&o.connection.updates.borrow())
                    .ok_or("list identity missing")?;
                let stamp = o.connection.updates.borrow().stamp();
                require(
                    o.list_source(identity, stamp, (row, 0)).is_none(),
                    "non-title cell was admitted",
                )?;
                let source = o
                    .list_source(identity, stamp, (row, col))
                    .ok_or("title lease unavailable")?;
                let mut stale = source;
                stale.stamp.revision += 1;
                o.capture_source(stale);
                require(
                    o.capture.borrow().is_none(),
                    "stale list lease started acquisition",
                )?;
                let g = o
                    .output
                    .geometry(&ui)
                    .and_then(crate::pointer::SurfaceGeometry::terminal)
                    .ok_or("list geometry absent")?;
                let point = slint::LogicalPosition::new(
                    ui.get_terminal_origin_x()
                        + (g.left + g.size.width as f32 * (col as f32 + 0.5) / 80.) / g.dpi,
                    ui.get_terminal_origin_y()
                        + (g.top + g.size.height as f32 * (row as f32 + 0.5) / 24.) / g.dpi,
                );
                verification::click(&ui, point, slint::platform::PointerEventButton::Right);
                wait(
                    || ui.get_context_open(),
                    "list title right-click opened no content menu",
                )
                .await?;
                let index = ui
                    .get_context_entries()
                    .iter()
                    .position(|e| e.label == msg!("bookmarks-list-current"))
                    .ok_or("list bookmark action missing")?;
                ui.invoke_context_choose(index as i32);
            } else {
                full(&ui, &o).await?;
                ui.invoke_bookmarks_capture_current();
            }
            require(o.capture.borrow().is_some(), "capture action did not start")?;
            if matches!(case.as_str(), "cancel" | "blur") {
                wait(
                    || o.connection.updates.borrow().input_epoch >= 2,
                    "owned Q was not sent",
                )
                .await?;
                if case == "blur" {
                    verification::foreground(&ui, false);
                } else {
                    ui.invoke_bookmarks_capture_cancel();
                }
                wait(
                    || o.capture.borrow().is_none(),
                    "cancel did not retire acquisition",
                )
                .await?;
                tokio::time::sleep(Duration::from_millis(6300)).await;
                require(
                    o.store.clone()?.load()?.entries().is_empty()
                        && !directory.join("bookmarks.json").exists(),
                    "cancel or late Q wrote an archive",
                )?;
            } else {
                wait(
                    || {
                        verification::foreground(&ui, true);
                        ui.invoke_focus_article();
                        o.capture.borrow().is_none()
                            && !o.output.busy()
                            && o.result.borrow().is_none()
                    },
                    "capture did not finish its joined save",
                )
                .await?;
                independent_file(&o, list || case == "headerless").map_err(|e| {
                    format!(
                        "{e}; notice={}; diagnostic={}",
                        ui.get_input_notice(),
                        o.article.diagnostics()
                    )
                })?;
                // The archive opens through the local-only reader. The strict peer
                // rejects every station byte after its nine/thirteen expected steps.
                ui.invoke_bookmarks_show();
                wait(
                    || ui.get_bookmarks_open() && !o.output.busy(),
                    "saved bookmark form did not open",
                )
                .await?;
                let w = o.window().ok_or("bookmark window absent")?;
                w.invoke_choose(1);
                w.invoke_action(2);
                wait(
                    || w.get_offline() && !o.output.busy(),
                    "local reader did not load",
                )
                .await?;
                w.invoke_page_key(5);
                require(
                    o.offline.borrow().as_ref().is_some_and(|r| r.top == 22),
                    "offline End did not reach row 45",
                )?;
                verification::capture(&w, &directory, "offline-last-page.png")?;
                w.invoke_page_key(6);
                w.invoke_close_bookmarks();
            }
            std::fs::write(directory.join("capture-result.json"), serde_json::to_vec_pretty(&serde_json::json!({"case":case,"strict_station_peer":true,"actual_os_input":false,"saved":!matches!(case.as_str(),"cancel"|"blur")})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(())
        },
    )
}
pub fn audit_closed(o: &Manager, directory: &std::path::Path) -> Result<(), String> {
    require(
        o.stopped.get()
            && o.capture.borrow().is_none()
            && !o.capture_timer.running()
            && o.result.borrow().is_none()
            && o.window.borrow().is_none()
            && !o.output.busy(),
        "capture close retained an owner",
    )?;
    std::fs::write(directory.join("capture-closed.json"), b"{\"capture_retired\":true,\"timer_stopped\":true,\"file_worker_joined\":true,\"result_collected\":true,\"window_released\":true}").map_err(|e|e.to_string())
}
