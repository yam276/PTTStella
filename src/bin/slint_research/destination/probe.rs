//! Full native components over the production Session's independent strict peer.
use super::*;
use crate::verification::{self, Verification, require, wait};
use std::{error::Error, path::PathBuf, time::Duration};
async fn ready(ui: &TerminalWindow, o: &Manager) -> Result<(), String> {
    wait(
        || {
            verification::foreground(ui, true);
            o.output.current(ui).is_some() && !ui.get_frame_paint_pending()
        },
        "destination source did not reach paint ACK",
    )
    .await
}
fn fill(w: &TerminalWindow, value: &str) {
    w.global::<crate::DestinationState>().set_draft(
        w.global::<crate::DestinationState>()
            .invoke_field_edit(value.into()),
    );
}
async fn reached(ui: &TerminalWindow, o: &Manager, name: &str) -> Result<(), String> {
    wait(||{verification::foreground(ui,true);o.active.get().is_none() && o.pending.get().is_none() && matches!(&o.connection.updates.borrow().destination,Progress::Reached{target,..} if target.board().is_some_and(|b|b.as_str()==name))},"destination never reached its verified target").await?;
    ready(ui, o).await
}
async fn files(o: &Manager) -> Result<(), String> {
    wait(|| !o.history.busy(), "recent writer did not join").await
}
pub fn start(
    ui: &TerminalWindow,
    o: Rc<Manager>,
    book: Rc<crate::bookmarks_manager::Manager>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "destination",
        move |ui, directory| async move {
            ready(&ui, &o).await?;
            let case =
                std::env::var("PTTSTELLA_SLINT_DESTINATION_PROBE").map_err(|e| e.to_string())?;
            ui.invoke_destination_show();
            require(
                ui.get_destination_open()
                    && ui.get_manager_open()
                    && !ui.get_keep_alive_surface_ready(),
                "destination draft did not gate the BBS",
            )?;
            let w = o.window().ok_or("destination window missing")?;
            w.window()
                .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));
            for bad in [
                "StellaTooLongName",
                "https://example.org/bbs/Stella/M.1.A.001.html",
            ] {
                fill(&w, bad);
                w.global::<crate::DestinationState>().invoke_action(0);
                require(
                    o.pending.get().is_none()
                        && w.global::<crate::DestinationState>().get_draft() == bad
                        && !w
                            .global::<crate::DestinationState>()
                            .get_message()
                            .is_empty(),
                    "invalid draft was truncated, sent or discarded",
                )?;
            }
            fill(&w, "Stella");
            w.global::<crate::DestinationState>().set_composing(true);
            w.global::<crate::DestinationState>().invoke_action(0);
            require(
                o.pending.get().is_none(),
                "composition submitted a destination",
            )?;
            w.global::<crate::DestinationState>().set_composing(false);
            w.global::<crate::DestinationState>()
                .invoke_close_destination();
            require(
                !ui.get_destination_open() && o.draft.borrow().is_empty(),
                "cancel did not clear the unsent draft",
            )?;
            if case == "boards" {
                ui.invoke_destination_show();
                fill(&w, "Stella");
                w.global::<crate::DestinationState>().invoke_focus_draft();
                w.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                        text: slint::platform::Key::Return.into(),
                    });
                w.window()
                    .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                        text: slint::platform::Key::Return.into(),
                    });
                reached(&ui, &o, "Stella").await?;
                files(&o).await?;
                require(
                    o.history
                        .snapshot()
                        .boards
                        .iter()
                        .map(|b| b.as_str())
                        .collect::<Vec<_>>()
                        == ["Stella"],
                    "confirmed arrival was not persisted",
                )?;
                // Already at the target: zero station keys and an immediate Reached
                // must clear the receipt even if watch publication won the race.
                ui.invoke_destination_show();
                fill(&w, "Stella");
                w.global::<crate::DestinationState>().invoke_action(0);
                reached(&ui, &o, "Stella").await?;
                require(
                    !ui.get_destination_open() && !ui.get_destination_active(),
                    "immediate Reached left a stale active receipt",
                )?;
                ui.invoke_destination_show();
                w.global::<crate::DestinationState>().invoke_choose(1);
                require(
                    w.global::<crate::DestinationState>().get_draft() == "Stella"
                        && o.pending.get().is_none(),
                    "choosing recent navigated instead of filling",
                )?;
                w.global::<crate::DestinationState>()
                    .invoke_close_destination();
                ready(&ui, &o).await?;
                ui.invoke_destination_show();
                fill(&w, "Other");
                w.global::<crate::DestinationState>().invoke_action(0);
                reached(&ui, &o, "Other").await?;
                files(&o).await?;
                require(
                    o.history
                        .snapshot()
                        .boards
                        .iter()
                        .map(|b| b.as_str())
                        .collect::<Vec<_>>()
                        == ["Other", "Stella"],
                    "recent order changed",
                )?;
                let path = directory.join("recent-boards.json");
                let store = Store::at(path.clone());
                require(
                    store
                        .load()?
                        .boards()
                        .iter()
                        .map(|b| b.as_str())
                        .collect::<Vec<_>>()
                        == ["Other", "Stella"],
                    "recent disk snapshot differs",
                )?;
                ui.invoke_destination_show();
                w.global::<crate::DestinationState>().invoke_action(2);
                files(&o).await?;
                require(
                    o.history.snapshot().boards.is_empty(),
                    "clear re-added the current board",
                )?;
                for file in [&path, &path.with_extension("json.bak")] {
                    require(
                        Store::at(file.to_owned()).load()?.boards().is_empty(),
                        "clear did not erase both files",
                    )?;
                }
                let backup = path.with_extension("json.bak");
                let retained = directory.join("recent-empty-backup.json");
                std::fs::rename(&backup, &retained).map_err(|e| e.to_string())?;
                std::fs::create_dir(&backup).map_err(|e| e.to_string())?;
                let before = std::fs::read(&path).map_err(|e| e.to_string())?;
                w.global::<crate::DestinationState>().invoke_action(2);
                files(&o).await?;
                require(
                    !w.global::<crate::DestinationState>()
                        .get_history_message()
                        .is_empty()
                        && w.global::<crate::DestinationState>().get_history_message()
                            != msg!("recent-cleared"),
                    "clear failure was not visible",
                )?;
                require(
                    std::fs::read(&path).map_err(|e| e.to_string())? == before
                        && o.history.last_error().is_some(),
                    "failed clear mutated main or lost its error",
                )?;
                std::fs::remove_dir(&backup).map_err(|e| e.to_string())?;
                std::fs::rename(&retained, &backup).map_err(|e| e.to_string())?;
                w.global::<crate::DestinationState>().invoke_action(2);
                files(&o).await?;
                require(
                    o.history.last_error().is_none(),
                    "successful explicit retry retained failed result",
                )?;
                verification::capture(&w, &directory, "recent-cleared.png")?;
                w.global::<crate::DestinationState>()
                    .invoke_close_destination();
                let bookmarks =
                    pttstella::bookmarks::store::Store::at(directory.join("bookmarks.json"));
                let base = bookmarks.load()?;
                let saved = base.upsert(
                    pttstella::article_target::ArticleTarget::parse("#00000101@Stella")
                        .map_err(|e| e.to_string())?,
                    "saved target",
                    100,
                )?;
                bookmarks.save_from(&base, saved)?;
                ready(&ui, &o).await?;
                ui.invoke_bookmarks_show();
                wait(
                    || ui.get_bookmarks_open() && !o.output.busy(),
                    "bookmark handoff form did not load",
                )
                .await?;
                let bw = book.window().ok_or("bookmark handoff window missing")?;
                bw.global::<crate::BookmarksState>().invoke_choose(1);
                bw.global::<crate::BookmarksState>().set_target(
                    bw.global::<crate::BookmarksState>()
                        .invoke_field_edit(0, "#00000102@Other".into()),
                );
                bw.global::<crate::BookmarksState>().invoke_action(4);
                require(
                    !ui.get_bookmarks_open()
                        && ui.get_destination_open()
                        && w.global::<crate::DestinationState>().get_draft() == "#00000101@Stella"
                        && o.pending.get().is_none(),
                    "bookmark Go did not fill its selected saved identity without sending",
                )?;
                w.global::<crate::DestinationState>()
                    .invoke_close_destination();
                ready(&ui, &o).await?;
                ui.invoke_destination_show();
                fill(&w, "Beta");
                w.global::<crate::DestinationState>().invoke_action(0);
                wait(||matches!(&o.connection.updates.borrow().destination,Progress::Active{target,..} if target.board().is_some_and(|b|b.as_str()=="Beta")),"third navigation did not start").await?;
                ui.invoke_destination_stop();
                require(
                    o.active.get().is_none() && o.pending.get().is_none(),
                    "stop retained a request",
                )?;
                tokio::time::sleep(Duration::from_millis(3300)).await;
                require(
                    o.history.snapshot().boards.is_empty(),
                    "late cancelled frame was recorded as arrival",
                )?;
            } else if case == "articles" {
                ui.invoke_destination_show();
                fill(&w, "#00000101@Stella");
                w.global::<crate::DestinationState>().invoke_action(0);
                reached(&ui, &o, "Stella").await?;
                ui.invoke_destination_show();
                fill(&w, "https://www.ptt.cc/bbs/Stella/M.1.A.002.html");
                w.global::<crate::DestinationState>().invoke_action(0);
                wait(||o.active.get().is_none() && matches!(&o.connection.updates.borrow().destination,Progress::Reached{target:Destination::Article(t),..} if t.aid().as_str()=="00000102"),"second verified article did not open").await?;
                ready(&ui, &o).await?;
                ui.invoke_destination_show();
                fill(&w, "#00000103@Stella");
                w.global::<crate::DestinationState>().invoke_action(0);
                wait(
                    || {
                        o.active.get().is_none()
                            && matches!(o.connection.updates.borrow().destination, Progress::Idle)
                    },
                    "missing article did not stop",
                )
                .await?;
                ready(&ui, &o).await?;
                // Only this explicit user raw key recovers the station's pause.
                let generation = o.connection.updates.borrow().generation;
                o.connection.input(generation, b" ".to_vec())?;
                wait(
                    || {
                        matches!(
                            o.connection.updates.borrow().context(),
                            pttstella::navigation::Context::ArticleList { .. }
                        )
                    },
                    "manual recovery did not return to articles",
                )
                .await?;
                ready(&ui, &o).await?;
                ui.invoke_destination_show();
                fill(&w, "#00000104@Stella");
                w.global::<crate::DestinationState>().invoke_action(0);
                wait(||o.connection.updates.borrow().input_epoch==22 && matches!(&o.connection.updates.borrow().destination,Progress::Active{target:Destination::Article(t),..} if t.aid().as_str()=="00000104"),"delayed fourth article did not start").await?;
                // Wait until Q has been sent, then stop before the late identity.
                ui.invoke_destination_stop();
                tokio::time::sleep(Duration::from_millis(3300)).await;
                require(
                    o.active.get().is_none(),
                    "late AID reply reactivated navigation",
                )?;
            } else {
                return Err("unknown destination verifier case".into());
            }
            files(&o).await?;
            if case == "boards" {
                o.history.clear();
                o.history.clear();
                require(
                    o.history.pressure() == (true, true),
                    "exit case missed the active and pending snapshots",
                )?;
                require(
                    o.history.snapshot().notice == Some(msg!("recent-saving")),
                    "exit snapshot was already collected",
                )?;
            }
            std::fs::write(directory.join("destination-result.json"),serde_json::to_vec_pretty(&serde_json::json!({"case":case,"draft_only_until_explicit_go":true,"invalid_retained":true,"composition_rejected":true,"shared_receipt_slot":true,"cancel_late":true,"history_limit":20,"os_input_tested":false})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(())
        },
    )
}

pub fn audit_closed(o: &Manager, directory: &std::path::Path) -> Result<(), String> {
    require(
        o.stopping.get()
            && o.pending.get().is_none()
            && o.active.get().is_none()
            && o.window.borrow().is_none()
            && o.history.stopped(),
        "destination close retained a command, window or file worker",
    )?;
    require(
        o.history.last_error().is_none(),
        "recent persistence ended in a failure",
    )?;
    let saved = Store::at(directory.join("recent-boards.json")).load()?;
    if std::env::var("PTTSTELLA_SLINT_DESTINATION_PROBE").as_deref() == Ok("boards") {
        require(
            o.history.snapshot().notice == Some(msg!("recent-cleared"))
                && saved.boards().is_empty(),
            "latest pending clear was not collected through exit",
        )?;
        require(
            Store::at(directory.join("recent-boards.json.bak"))
                .load()?
                .boards()
                .is_empty(),
            "exit clear left a stale backup",
        )?;
    }
    std::fs::write(directory.join("destination-closed.json"),serde_json::to_vec_pretty(&serde_json::json!({"window_released":true,"receipt_released":true,"recent_worker_joined":true,"recent_result_collected":true,"pending_snapshot_drained":true,"recent":saved.boards()})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
