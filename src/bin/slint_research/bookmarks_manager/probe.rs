//! Explicit synthetic full-component acceptance; no OS input claims.
use super::*;
use crate::verification::{self, Verification, require, wait};
use pttstella::{
    bookmarks::archive::Archive,
    filters::{Policy, Rules},
    screen::{Frame, Screen},
};
use slint::Model;
use std::{error::Error, path::PathBuf, time::Duration};

/// Only the explicit synthetic native mode calls this before mounting the form.
pub fn prepare_native(store: &Result<Store, String>) -> Result<(), String> {
    if std::env::var("PTTSTELLA_SLINT_BOOKMARKS_PROBE").as_deref() != Ok("native") {
        return Ok(());
    }
    let Some(input) = std::env::var_os("PTTSTELLA_SLINT_BOOKMARKS_SOURCE") else {
        return Ok(());
    };
    let source = Store::at(input.into()).load()?;
    let store = store.clone()?;
    let base = store.load()?;
    store.save_from(&base, source)?;
    Ok(())
}

pub fn audit_closed(o: &Manager, directory: &std::path::Path) -> Result<(), String> {
    require(
        o.stopped.get()
            && o.result.borrow().is_none()
            && o.window.borrow().is_none()
            && o.offline.borrow().is_none()
            && !o.output.busy(),
        "bookmark close retained a worker, window or article",
    )?;
    let saved = o.store.clone()?.load()?;
    let mode = std::env::var("PTTSTELLA_SLINT_BOOKMARKS_PROBE").unwrap_or_default();
    if mode == "native" {
        return std::fs::write(directory.join("bookmarks-closed.json"),serde_json::to_vec_pretty(&serde_json::json!({"worker_joined":true,"result_collected":true,"window_released":true,"offline_released":true,"entries":saved.entries().len(),"manual_outcome":"independent_file_and_UI_check_required"})).map_err(|e|e.to_string())?).map_err(|e|e.to_string());
    }
    let label = if mode == "save-exit" {
        "退出前保存末字"
    } else {
        "保存失敗保留末字"
    };
    require(
        saved.entries().len() == 1
            && saved.entries()[0].label() == Some(label)
            && saved.entries()[0]
                .archive()
                .is_some_and(|a| a.row_count() == 45),
        "closed bookmark file did not preserve the expected save and archive",
    )?;
    std::fs::write(directory.join("bookmarks-closed.json"),serde_json::to_vec_pretty(&serde_json::json!({"worker_joined":true,"result_collected":true,"window_released":true,"offline_released":true,"label":label,"archive_rows":45})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}

async fn idle(o: &Manager) -> Result<(), String> {
    wait(
        || {
            !o.output.busy()
                && o.result.borrow().is_none()
                && o.window().is_none_or(|w| !w.get_busy())
        },
        "bookmark file worker did not join",
    )
    .await
}
fn fill(w: &BookmarksWindow, target: &str, label: &str) {
    w.set_target(w.invoke_field_edit(0, target.into()));
    w.set_label(w.invoke_field_edit(1, label.into()));
}
fn source() -> Result<Frame, String> {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../../tests/fixtures/filters.json"))
            .map_err(|e| e.to_string())?;
    let text = fixture["reading"]
        .as_str()
        .ok_or("reading fixture missing")?;
    let mut screen = Screen::new(80, 24).map_err(|e| e.to_string())?;
    screen.feed(&pttstella::encoding::encode(text).map_err(|e| e.to_string())?);
    let mut frame = screen.frame();
    let chinese = frame
        .glyphs
        .iter_mut()
        .find(|g| g.row == 1 && g.width == 2)
        .ok_or("dual style fixture missing")?;
    chinese.left.fg = 1;
    chinese.right.fg = 4;
    let mut glyphs = Vec::new();
    for row in 0..45 {
        glyphs.extend(
            frame
                .glyphs
                .iter()
                .filter(|g| g.row == row % 23)
                .cloned()
                .map(|mut g| {
                    g.row = row;
                    g
                }),
        );
    }
    glyphs.extend(
        frame
            .glyphs
            .iter()
            .filter(|g| g.row == 23)
            .cloned()
            .map(|mut g| {
                g.row = 45;
                g
            }),
    );
    frame.rows = 46;
    frame.cursor = (0, 45);
    frame.glyphs = glyphs;
    frame.hyperlinks.clear();
    Ok(frame)
}
pub fn start(
    ui: &TerminalWindow,
    o: Rc<Manager>,
    connection: Rc<pttstella::connection::Connection>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "bookmarks",
        move |ui, directory| async move {
            wait(
                || {
                    verification::foreground(&ui, true);
                    o.output.current(&ui).is_some()
                },
                "bookmark source paint missing",
            )
            .await?;
            let path = directory.join("bookmarks.json");
            let store = o.store.clone()?;
            let base = store.load()?;
            let target = ArticleTarget::parse("#00000101@Stella").map_err(|e| e.to_string())?;
            let mode = std::env::var("PTTSTELLA_SLINT_BOOKMARKS_PROBE").unwrap_or_default();
            if matches!(mode.as_str(), "restart" | "save-exit") {
                let input = std::env::var_os("PTTSTELLA_SLINT_BOOKMARKS_SOURCE")
                    .ok_or("bookmark restart source missing")?;
                let source = Store::at(input.into()).load()?;
                require(
                    source.entries().len() == 1
                        && source.entries()[0].label() == Some("保存失敗保留末字")
                        && source.entries()[0].created_at() == 100
                        && source.entries()[0]
                            .archive()
                            .is_some_and(|a| a.row_count() == 45),
                    "new process did not decode the expected previous data",
                )?;
                store.save_from(&base, source)?;
                connection.disconnect()?;
                wait(
                    || {
                        connection.updates.borrow().phase
                            == pttstella::connection::Phase::Disconnected
                    },
                    "restart probe did not disconnect",
                )
                .await?;
            } else {
                let seed = base.archive(
                    target.clone(),
                    "離線測試",
                    100,
                    Archive::new(&source()?, 100)?,
                )?;
                store.save_from(&base, seed)?;
            }
            wait(
                || {
                    verification::foreground(&ui, true);
                    !ui.get_tools_source_blocked() && o.output.current(&ui).is_some()
                },
                "bookmark tools did not admit a foreground source",
            )
            .await?;
            require(
                !matches!(
                    connection.updates.borrow().context(),
                    pttstella::navigation::Context::ArticleReading { .. }
                ),
                "bookmark background fixture unexpectedly became an article",
            )?;
            let geometry = o
                .output
                .geometry(&ui)
                .and_then(crate::pointer::SurfaceGeometry::terminal)
                .ok_or("bookmark background geometry missing")?;
            let point = slint::LogicalPosition::new(
                ui.get_terminal_origin_x()
                    + (geometry.left + geometry.size.width as f32 * 40.5 / 80.) / geometry.dpi,
                ui.get_terminal_origin_y()
                    + (geometry.top + geometry.size.height as f32 * 15.5 / 24.) / geometry.dpi,
            );
            verification::click(&ui, point, slint::platform::PointerEventButton::Right);
            wait(
                || ui.get_tools_open() && !ui.get_context_open(),
                "non-reading background did not open floating tools",
            )
            .await?;
            verification::key(&ui, slint::platform::Key::Escape);
            ui.invoke_tools_toggle();
            require(
                ui.get_tools_categories()
                    .iter()
                    .nth(1)
                    .is_some_and(|category| category.enabled),
                "bookmark tool category is disabled",
            )?;
            ui.invoke_tools_category_choose(1);
            require(
                ui.get_tools_category() == 1,
                "bookmark category did not open",
            )?;
            ui.invoke_tools_action_choose(0);

            idle(&o).await?;
            require(
                ui.get_bookmarks_open()
                    && ui.get_manager_open()
                    && !ui.get_keep_alive_surface_ready(),
                "bookmark window did not gate the station",
            )?;
            let w = o.window().ok_or("bookmark window missing")?;
            w.window()
                .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: "x".into() });
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: "x".into() });
            require(o.rows.borrow().len() == 1, "saved archive did not load")?;
            require(
                w.get_choice() == 0
                    && !w.get_selected()
                    && !w.get_has_copy()
                    && w.get_target().is_empty(),
                "new bookmark draft impersonated a selected entry",
            )?;
            w.invoke_choose(1);
            o.article.replace_rules(Rules::default().edit_policy(
                "Alice",
                "",
                Policy::MaskRow,
                false,
            )?);
            w.invoke_action(2);
            idle(&o).await?;
            wait(
                || {
                    o.offline
                        .borrow()
                        .as_ref()
                        .is_some_and(|a| a.painted.is_some())
                },
                "offline page did not paint",
            )
            .await?;
            {
                let offline = o.offline.borrow();
                let a = offline.as_ref().ok_or("offline data missing")?;
                require(
                    a.article.frame.rows == 45 && a.top == 0,
                    "archive row count changed",
                )?;
                require(
                    a.article
                        .frame
                        .glyphs
                        .iter()
                        .filter(|g| g.row == 5)
                        .all(|g| g.text.chars().all(|c| c == '█')),
                    "current row-mask policy was not applied offline",
                )?;
                require(
                    a.article
                        .frame
                        .glyphs
                        .iter()
                        .any(|g| g.row == 1 && g.width == 2 && g.left.fg == 1 && g.right.fg == 4),
                    "archive lost the two glyph styles",
                )?;
                require(
                    a.renderer
                        .as_ref()
                        .is_some_and(|r| r.frame().rows == 23 && r.frame().cols == 80),
                    "offline viewport was not 80 by 23",
                )?;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
            verification::capture(&w, &directory, "offline.png")?;
            w.window()
                .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                    text: slint::platform::Key::PageDown.into(),
                });
            w.window()
                .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                    text: slint::platform::Key::PageDown.into(),
                });
            require(
                o.offline.borrow().as_ref().is_some_and(|a| a.top == 22),
                "offline page did not overlap by one line",
            )?;
            w.invoke_page_key(0);
            require(
                o.offline.borrow().as_ref().is_some_and(|a| a.top == 21),
                "offline up did not move one line",
            )?;
            w.invoke_page_key(5);
            require(
                o.offline.borrow().as_ref().is_some_and(|a| a.top == 22),
                "offline end was unreachable",
            )?;
            w.invoke_page_key(4);
            require(
                o.offline.borrow().as_ref().is_some_and(|a| a.top == 0),
                "offline home failed",
            )?;
            w.invoke_page_key(6);
            require(
                !w.get_offline() && o.offline.borrow().is_none(),
                "offline close retained the article or raster",
            )?;
            if matches!(mode.as_str(), "restart" | "save-exit") {
                require(
                    connection.updates.borrow().phase == pttstella::connection::Phase::Disconnected,
                    "offline reader reconnected",
                )?;
                if mode == "save-exit" {
                    let write = o
                        .state
                        .borrow()
                        .as_ref()
                        .ok_or("bookmark state missing")?
                        .edit("#00000101@Stella", "退出前保存末字", false)?;
                    o.start(
                        Box::new(move || {
                            std::thread::sleep(Duration::from_millis(120));
                            write
                                .store
                                .save_from(&write.base, write.draft)
                                .map(Value::Saved)
                        }),
                        Operation::Save,
                    );
                    require(
                        o.output.busy() && o.result.borrow().is_some(),
                        "exit test missed the in-flight save",
                    )?;
                }
                std::fs::write(directory.join("bookmarks-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"new_process_decode":true,"disconnected_read":true,"archive_rows":45,"current_rules":true,"dual_styles":true,"page_step":22,"pending_save_on_exit":mode=="save-exit","os_input_tested":false})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
                return Ok(());
            }
            require(
                w.get_message() != msg!("bookmarks-loading"),
                "completed archive read retained its loading notice",
            )?;
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            fill(&w, "Stella", "invalid");
            w.invoke_action(0);
            idle(&o).await?;
            require(
                std::fs::read(&path).map_err(|e| e.to_string())? == bytes,
                "invalid destination wrote bookmarks",
            )?;
            fill(&w, "#00000102@Stella", "新書籤末字");
            w.invoke_action(0);
            idle(&o).await?;
            require(o.rows.borrow().len() == 2, "manual bookmark did not save")?;
            w.invoke_choose(1);
            fill(&w, "#00000102@stella", "renamed");
            w.invoke_action(0);
            idle(&o).await?;
            let saved = store.load()?;
            require(
                saved.entries().len() == 2
                    && saved.entries()[0].label() == Some("renamed")
                    && saved.entries()[1].created_at() == 100
                    && saved.entries()[1].archive().is_some(),
                "case-insensitive update duplicated or dropped an archive",
            )?;
            fill(&w, "#00000101@Stella", "changed unsaved identity");
            w.invoke_action(1);
            idle(&o).await?;
            require(
                store.load()?.entries().len() == 1
                    && same(store.load()?.entries()[0].target(), &target),
                "remove followed the edited field instead of the selected bookmark",
            )?;
            w.invoke_choose(1);
            fill(&w, "#00000101@Stella", "保存失敗保留末字");
            let original = std::fs::read(&path).map_err(|e| e.to_string())?;
            let backup = path.with_extension("json.bak");
            let retained = directory.join("backup-before-fault.json");
            std::fs::rename(&backup, &retained).map_err(|e| e.to_string())?;
            std::fs::create_dir(&backup).map_err(|e| e.to_string())?;
            w.invoke_action(0);
            w.invoke_close_bookmarks();
            idle(&o).await?;
            require(
                ui.get_bookmarks_open()
                    && w.get_label() == "保存失敗保留末字"
                    && !w.get_message().is_empty(),
                "failed save closed the form or discarded its draft",
            )?;
            require(
                std::fs::read(&path).map_err(|e| e.to_string())? == original
                    && o.rows.borrow()[0].label() == Some("離線測試"),
                "failed backup changed the disk or effective state",
            )?;
            std::fs::remove_dir(&backup).map_err(|e| e.to_string())?;
            std::fs::rename(&retained, &backup).map_err(|e| e.to_string())?;
            w.invoke_action(0);
            idle(&o).await?;
            require(
                store.load()?.entries()[0].label() == Some("保存失敗保留末字"),
                "retry did not save retained label",
            )?;
            let read = o.state.borrow().as_ref().ok_or("state missing")?.read(
                "#00000101@Stella",
                o.article.rules(),
                o.article.settings(),
            )?;
            o.start(
                Box::new(move || {
                    std::thread::sleep(Duration::from_millis(100));
                    Ok(Value::Read(read.run()))
                }),
                Operation::Read,
            );
            w.invoke_close_bookmarks();
            require(
                !ui.get_bookmarks_open(),
                "closing an archive read did not hide the window",
            )?;
            idle(&o).await?;
            require(
                !ui.get_bookmarks_open() && !w.get_offline() && o.offline.borrow().is_none(),
                "late archive result reopened the window",
            )?;
            ui.invoke_bookmarks_show();
            idle(&o).await?;
            let valid = std::fs::read(&path).map_err(|e| e.to_string())?;
            std::fs::write(&path, b"broken").map_err(|e| e.to_string())?;
            w.invoke_action(3);
            idle(&o).await?;
            require(
                !w.get_writable()
                    && o.rows.borrow().is_empty()
                    && std::fs::read(&path).map_err(|e| e.to_string())? == b"broken",
                "corrupt document was not read-only",
            )?;
            fill(&w, "#00000103@Stella", "must not write");
            w.invoke_action(0);
            idle(&o).await?;
            require(
                std::fs::read(&path).map_err(|e| e.to_string())? == b"broken",
                "read-only manager overwrote corrupt data",
            )?;
            std::fs::write(&path, &valid).map_err(|e| e.to_string())?;
            w.invoke_action(3);
            idle(&o).await?;
            require(
                w.get_writable() && o.rows.borrow().len() == 1,
                "explicit reload did not recover repaired data",
            )?;
            tokio::time::sleep(Duration::from_millis(30)).await;
            verification::capture(&w, &directory, "manager.png")?;
            std::fs::write(directory.join("bookmarks-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"manual_crud":true,"saved_identity_remove":true,"archive_rows":45,"viewport":[80,23],"page_step":22,"current_rules":true,"dual_styles":true,"backup_failure_retained":true,"late_read_rejected":true,"corrupt_readonly_reload":true,"os_input_tested":false})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(())
        },
    )
}
