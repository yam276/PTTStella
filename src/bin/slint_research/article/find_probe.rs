//! Full native component proof; toolkit text is distinct from real OS IME.
use super::session::ArticleSession;
use crate::{
    TerminalWindow,
    output::{self, Output},
    verification::{self, Verification, require, wait},
};
use pttstella::{connection::Connection, navigation::Context};
use slint::{
    ComponentHandle,
    platform::{Key, WindowEvent},
};
use std::{error::Error, path::PathBuf, rc::Rc, time::Duration};

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
        "article-find",
        move |ui, directory| async move {
            wait(
                || {
                    verification::foreground(&ui, true);
                    ui.invoke_focus_terminal();
                    output.current(&ui).is_some() && ui.get_find_available()
                },
                "find source not installed",
            )
            .await?;
            require(
                owner.requests.get() == 0,
                "complete find fixture unexpectedly preloaded",
            )?;
            let original = owner
                .core
                .borrow()
                .last_displayed()
                .ok_or("find displayed source missing")?
                .frame
                .clone();
            let display = pttstella::replay::parity_display_profile().resolve()?;
            let before = output::render_png(original.clone(), display.clone(), 0..24)?;
            ui.invoke_find_action(0);
            require(
                ui.get_find_open() && ui.get_find_focused(),
                "find did not take native field focus",
            )?;
            require(
                !ui.get_article_reading_ready() && !ui.get_keep_alive_surface_ready(),
                "finding retained BBS background readiness",
            )?;
            ui.window()
                .dispatch_event(WindowEvent::KeyPressed { text: "中".into() });
            ui.window()
                .dispatch_event(WindowEvent::KeyReleased { text: "中".into() });
            wait(
                || {verification::foreground(&ui,true);owner.core.borrow().find().reply(0).total == 3},
                "visible Chinese hit count differs (mask/concealed)",
            )
            .await.map_err(|e| {
                let _=verification::capture(&ui,&directory,"find-timeout.png");
                let core=owner.core.borrow();let find=core.find();
                format!("{e}: {}",serde_json::json!({"readiness":owner.diagnostics(),
                    "ui_query":ui.get_find_query().as_str(),"query":find.query(),"find":find.reply(0),
                    "focused":ui.get_find_focused(),"composing":ui.get_find_composing(),"blocked":ui.get_find_blocked()}))
            })?;
            let hits = {
                let core = owner.core.borrow();
                let r = core.find().reply(0);
                r.hits.ok_or("find hits missing")?.to_vec()
            };
            require(
                hits.iter()
                    .map(|h| (h.row, h.start, h.end))
                    .collect::<Vec<_>>()
                    == [(5, 6, 8), (6, 0, 2), (6, 3, 5)],
                "independent Chinese cell ranges differ",
            )?;
            // Changing language must not rebuild/search the index, consume a
            // synthetic preedit, reveal another hit, or send station input.
            let find_state = || {
                let core = owner.core.borrow();
                let find = core.find();
                (
                    find.version(),
                    find.data_version(),
                    find.query().to_owned(),
                    find.reply(0).active,
                    find.reply(0).total,
                )
            };
            let original_find = find_state();
            ui.set_find_composing(true);
            let mut previous = pttstella::i18n::application().language();
            for (language, title) in [
                (pttstella::i18n::Language::English, "Find in article"),
                (
                    pttstella::i18n::Language::TraditionalChinese,
                    "在文章中尋找",
                ),
            ] {
                pttstella::i18n::application().select(language);
                crate::locale::refresh(&ui);
                crate::locale::refresh_statuses(&ui, previous);
                owner.refresh_language();
                require(
                    find_state() == original_find
                        && ui.get_find_query() == "中"
                        && ui.get_find_composing()
                        && ui.get_find_focused()
                        && owner.requests.get() == 0,
                    "language changed active query/index/preedit/focus or sent a page",
                )?;
                require(
                    ui.get_find_title() == title && ui.get_find_status() == "1 / 3",
                    "localized search chrome/status differs",
                )?;
                previous = language;
            }
            ui.set_find_composing(false);
            std::fs::write(directory.join("language-find.json"),br#"{"status":"passed","query_index_active_total_preedit_focus_requests_unchanged":true,"scope":"actual search widgets; synthetic preedit and direct diagnostic language selection through production refresh methods"}"#).map_err(|e|e.to_string())?;
            tokio::time::sleep(Duration::from_millis(80)).await;
            verification::capture(&ui, &directory, "chinese-find.png")?;
            // Retain an accepted edit across deactivation without searching or
            // sending bytes while blocked. Resume the same bounded draft.
            // A snapshot/native activation event may have revoked our earlier
            // synthetic foreground. Establish an eligible edit first; otherwise
            // a correctly rejected edit leaves old hits and proves no pause bug.
            wait(
                || {
                    verification::foreground(&ui, true);
                    ui.invoke_focus_find(false);
                    ui.get_find_open()
                        && ui.get_find_focused()
                        && !ui.get_find_blocked()
                        && !ui.get_find_composing()
                },
                "search edit precondition did not become ready",
            )
            .await?;
            let before_edit = owner.diagnostics();
            ui.invoke_find_edit("中".into());
            let after_edit = owner.diagnostics();
            require(
                after_edit["find"]["draft_retained"].as_bool() == Some(true)
                    && after_edit["find"]["hits"].as_u64() == Some(0),
                "foreground edit was not accepted as pending",
            )?;
            verification::foreground(&ui, false);
            let after_deactivation = owner.diagnostics();
            tokio::time::sleep(Duration::from_millis(180)).await;
            let after_background = owner.diagnostics();
            std::fs::write(
                directory.join("find-pause.json"),
                serde_json::to_vec_pretty(&serde_json::json!({
                    "before_edit":before_edit,"after_edit":after_edit,
                    "after_deactivation":after_deactivation,"after_background":after_background,
                    "scope":"synthetic toolkit activation; no OS foreground claim"
                }))
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            require(
                !ui.get_native_foreground() && ui.get_find_blocked(),
                "synthetic deactivation was not retained",
            )?;
            require(
                after_background["find"]["draft_retained"].as_bool() == Some(true)
                    && owner.core.borrow().find().reply(0).total == 0,
                "background discarded or applied the pending query",
            )?;
            verification::foreground(&ui, true);
            wait(
                || {
                    verification::foreground(&ui, true);
                    owner.core.borrow().find().reply(0).total == 3
                },
                "foreground did not resume retained query",
            )
            .await?;
            verification::key(&ui, Key::Return);
            require(
                owner.core.borrow().find().reply(0).active == Some(1),
                "native Enter failed to step",
            )?;
            ui.invoke_find_action(1);
            require(
                owner.core.borrow().find().reply(0).active == Some(0),
                "previous match failed",
            )?;
            ui.invoke_find_action(1);
            require(
                owner.core.borrow().find().reply(0).active == Some(2),
                "previous match did not wrap",
            )?;
            ui.set_find_composing(true);
            verification::key(&ui, Key::Escape);
            verification::key(&ui, Key::Return);
            require(
                ui.get_find_open() && owner.core.borrow().find().reply(0).active == Some(2),
                "candidate keys escaped local composition",
            )?;
            ui.set_find_composing(false);
            require(
                owner
                    .core
                    .borrow()
                    .last_displayed()
                    .is_some_and(|p| p.frame == original),
                "highlight mutated source cells",
            )?;
            let after = output::render_png(
                owner
                    .core
                    .borrow()
                    .last_displayed()
                    .ok_or("find source gone")?
                    .frame
                    .clone(),
                display,
                0..24,
            )?;
            require(
                before.png == after.png,
                "display highlights leaked into source PNG",
            )?;
            std::fs::write(directory.join("source.png"), after.png).map_err(|e| e.to_string())?;
            ui.invoke_find_edit("missing".into());
            require(
                owner.core.borrow().find().reply(0).total == 0,
                "pending edit kept old highlights",
            )?;
            ui.invoke_find_action(3);
            tokio::time::sleep(Duration::from_millis(180)).await;
            require(
                !ui.get_find_open() && owner.core.borrow().find().indexed_rows() == 0,
                "close retained find timer/index",
            )?;
            require(
                if ui.get_article_active() {
                    ui.get_article_focused()
                } else {
                    ui.get_terminal_input_focused()
                },
                "find close lost terminal focus",
            )?;
            ui.invoke_find_action(0);
            ui.set_find_query("中".into());
            ui.invoke_find_edit("中".into());
            wait(
                || owner.core.borrow().find().reply(0).total == 3,
                "reopened find did not rebuild",
            )
            .await?;
            ui.invoke_find_action(3);
            verification::capture(&ui, &directory, "closed-find.png")?;
            // Station input enters the production typed path. OS Winit keys
            // require their own evidence, separate from toolkit text above.
            let generation = connection.updates.borrow().generation;
            owner.route_input(generation, b"/".to_vec())?;
            wait(
                || {
                    matches!(
                        connection.updates.borrow().context(),
                        Context::MainMenu { .. }
                    )
                },
                "explicit station slash did not reach strict peer",
            )
            .await?;
            wait(
                || !ui.get_find_available(),
                "foreign screen retained local reader search",
            )
            .await?;
            let result = serde_json::json!({"status":"passed","events":"Slint toolkit; synthetic foreground; no OS IME claim","hits":hits,"source_unchanged":true,"png_unchanged":true,"queries_release_index":true,"preload_requests":owner.requests.get(),"peer_bytes":[47],"mode_at_start":if original.rows==24 {"confirmed reading"}else{"cached reading"}});
            std::fs::write(
                directory.join("article-find.json"),
                serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        },
    )
}
