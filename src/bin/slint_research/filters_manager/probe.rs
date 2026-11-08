//! Explicit, isolated full-component acceptance. Picker/OS input are separate.
use super::*;
use crate::verification::{self, Verification, require, wait};
use pttstella::selection::{self, Selection};
use std::{error::Error, path::PathBuf, time::Duration};
async fn idle(o: &Manager) -> Result<(), String> {
    wait(
        || !o.output.busy() && o.result.borrow().is_none(),
        "filter file worker did not join",
    )
    .await
}
fn fill(w: &FiltersWindow, id: &str, note: &str) {
    w.set_account(w.invoke_field_edit(0, id.into()));
    w.set_note(w.invoke_field_edit(1, note.into()));
}
pub fn start(
    ui: &TerminalWindow,
    o: Rc<Manager>,
    directory: PathBuf,
    path: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(ui, directory, "filters", move |ui, directory| async move {
        wait(
            || {
                verification::foreground(&ui, true);
                o.output.current(&ui).is_some()
            },
            "filter source paint missing",
        )
        .await?;
        let web = std::env::var("PTTSTELLA_SLINT_FILTERS_PROBE").is_ok_and(|v| v == "web");
        require(
            ui.get_article_active() == web,
            "filter probe mounted the wrong reading mode",
        )?;
        ui.invoke_filters_show();
        require(
            ui.get_filters_open() && ui.get_manager_open() && !ui.get_keep_alive_surface_ready(),
            "filter modal did not gate the station",
        )?;
        let w = o.window().ok_or("filter form missing")?;
        w.window()
            .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));
        // A printable root key must be rejected by the same production gate.
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: "x".into() });
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: "x".into() });
        fill(&w, "1invalid", "draft");
        w.invoke_action(0);
        require(
            o.rules().entries.is_empty() && !path.exists(),
            "invalid ID wrote a filter document",
        )?;
        fill(&w, "Blocked", "synthetic note");
        w.set_policy(0);
        w.invoke_action(0);
        idle(&o).await?;
        require(
            o.rules().entries.len() == 1 && o.rules().entries[0].policy == Policy::Mask,
            "content mask was not committed",
        )?;
        wait(
            || {
                o.output
                    .with_frame(&ui, |frame| {
                        selection::extract(frame, &Selection::Line { row: 4 })
                            .ok()
                            .is_some_and(|s| {
                                !s.text.contains("hidden") && s.text.contains("Blocked")
                            })
                    })
                    .unwrap_or(false)
            },
            "saved mask did not reach the acknowledged projection",
        )
        .await?;
        let original = std::fs::read(&path).map_err(|e| e.to_string())?;
        fill(&w, "bLoCkEd", "updated");
        w.set_policy(1);
        w.invoke_action(0);
        idle(&o).await?;
        require(
            o.rules().entries.len() == 1
                && o.rules().entries[0].id == "bLoCkEd"
                && o.rules().entries[0].policy == Policy::MaskRow,
            "case-insensitive edit duplicated the account",
        )?;
        require(
            std::fs::read(path.with_extension("json.bak")).map_err(|e| e.to_string())? == original,
            "filter transaction did not preserve the prior bytes",
        )?;
        wait(
            || {
                o.output
                    .with_frame(&ui, |frame| {
                        selection::extract(frame, &Selection::Line { row: 4 })
                            .ok()
                            .is_some_and(|s| s.text.chars().all(|c| c == '█'))
                    })
                    .unwrap_or(false)
            },
            "whole-row mask did not reach the acknowledged projection",
        )
        .await?;
        w.invoke_tab(true);
        fill(&w, "Alice", "individual");
        w.set_entry_color(13);
        w.invoke_action(0);
        idle(&o).await?;
        w.invoke_new_entry();
        fill(&w, "Bob", "shared");
        w.set_entry_color(0);
        w.invoke_action(0);
        idle(&o).await?;
        w.set_shared_color(10);
        w.invoke_action(2);
        idle(&o).await?;
        require(
            o.rules().allow_color == 11
                && o.rules()
                    .entries
                    .iter()
                    .find(|e| e.id == "Alice")
                    .is_some_and(|e| e.color == Some(13))
                && o.rules()
                    .entries
                    .iter()
                    .find(|e| e.id == "Bob")
                    .is_some_and(|e| e.color.is_none()),
            "shared color reset an individual override",
        )?;
        fill(&w, "Unsubmitted", "kept while sorting");
        w.invoke_sort();
        require(
            w.get_account() == "Unsubmitted" && w.get_note() == "kept while sorting",
            "sorting discarded the form draft",
        )?;
        require(
            o.rows
                .borrow()
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>()
                == ["Bob", "Alice"],
            "allow category sort did not use effective ANSI color",
        )?;
        w.invoke_tab(false);
        require(
            w.get_account().is_empty(),
            "changing tabs retained an unsaved edit",
        )?;
        let saved = o.rules();
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let request = o.begin_import()?;
        o.start(Box::new(move ||Ok(Value::Imported(request,Some(pttstella::filters::import::parse(b"Blocked=C:row\nAlice=W2:green\nALICE=B:must not win\nCarol=W:shared\ninvalid!=W:x")?)))),Operation::Import(request));
        idle(&o).await?;
        require(
            w.get_preview()
                && o.rules() == saved
                && std::fs::read(&path).map_err(|e| e.to_string())? == bytes,
            "import preview changed effective rules or disk",
        )?;
        w.invoke_tab(true);
        w.invoke_sort();
        require(
            o.rows
                .borrow()
                .iter()
                .find(|e| e.id.eq_ignore_ascii_case("Alice"))
                .is_some_and(|e| e.policy == Policy::Allow && e.color == Some(10)),
            "preview lost whitelist precedence or imported color",
        )?;
        w.invoke_action(5);
        require(
            !w.get_preview() && o.rules() == saved,
            "cancelled preview became effective",
        )?;
        let request = o.begin_import()?;
        o.start(
            Box::new(move || {
                std::thread::sleep(Duration::from_millis(100));
                Ok(Value::Imported(
                    request,
                    Some(pttstella::filters::import::parse(b"Late=W:cancelled")?),
                ))
            }),
            Operation::Import(request),
        );
        w.invoke_close_filters();
        require(
            !ui.get_filters_open(),
            "closing a reading import did not hide the form",
        )?;
        idle(&o).await?;
        require(
            o.rules() == saved && o.preview.borrow().is_none(),
            "late import survived close",
        )?;
        ui.invoke_filters_show();
        w.window()
            .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));
        let request = o.begin_import()?;
        o.start(
            Box::new(move || {
                Ok(Value::Imported(
                    request,
                    Some(pttstella::filters::import::parse(
                        b"Alice=W2:green\nCarol=W:shared",
                    )?),
                ))
            }),
            Operation::Import(request),
        );
        idle(&o).await?;
        w.invoke_action(4);
        idle(&o).await?;
        require(
            o.rules().entries.iter().any(|e| e.id == "Carol")
                && o.rules()
                    .entries
                    .iter()
                    .find(|e| e.id == "Alice")
                    .is_some_and(|e| e.color == Some(10)),
            "confirmed import did not save the preview",
        )?;
        // Owned synthetic backup obstruction: no other task's path is touched.
        let before = o.rules();
        let before_bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        std::fs::remove_file(path.with_extension("json.bak")).map_err(|e| e.to_string())?;
        std::fs::create_dir(path.with_extension("json.bak")).map_err(|e| e.to_string())?;
        w.invoke_tab(false);
        fill(&w, "Failed", "retain this draft");
        w.invoke_action(0);
        idle(&o).await?;
        require(
            o.rules() == before
                && std::fs::read(&path).map_err(|e| e.to_string())? == before_bytes
                && w.get_account() == "Failed"
                && w.get_note() == "retain this draft",
            "save failure lost its draft or changed effective/durable rules",
        )?;
        std::fs::write(
            directory.join("backup-failure-message.txt"),
            w.get_message().as_str(),
        )
        .map_err(|e| e.to_string())?;
        std::fs::remove_dir(path.with_extension("json.bak")).map_err(|e| e.to_string())?;
        w.invoke_action(0);
        idle(&o).await?;
        require(
            o.rules().entries.iter().any(|e| e.id == "Failed"),
            "explicit retry did not save the retained draft",
        )?;
        w.invoke_action(1);
        idle(&o).await?;
        require(
            !o.rules().entries.iter().any(|e| e.id == "Failed"),
            "remove did not commit",
        )?;
        let intact = std::fs::read(&path).map_err(|e| e.to_string())?;
        std::fs::write(&path, b"broken synthetic file").map_err(|e| e.to_string())?;
        w.invoke_action(6);
        idle(&o).await?;
        require(
            !w.get_writable(),
            "reload error left a corrupt file writable",
        )?;
        fill(&w, "Overwrite", "must be denied");
        w.invoke_action(0);
        idle(&o).await?;
        require(
            std::fs::read(&path).map_err(|e| e.to_string())? == b"broken synthetic file",
            "corrupt filter document was overwritten",
        )?;
        std::fs::write(&path, &intact).map_err(|e| e.to_string())?;
        w.invoke_action(6);
        idle(&o).await?;
        require(
            w.get_writable() && o.rules() == Store::at(path.clone()).load()?,
            "explicit reload did not restore the validated rules",
        )?;
        require(
            w.get_writable() && !w.get_busy() && !w.get_importing() && !w.get_composing(),
            "form controls stayed disabled after reload",
        )?;
        // A pending import error is also stale after cancellation.
        let request = o.begin_import()?;
        o.start(
            Box::new(move || {
                std::thread::sleep(Duration::from_millis(80));
                Err("cancelled synthetic read error".into())
            }),
            Operation::Import(request),
        );
        w.invoke_action(5);
        let cancelled = w.get_message();
        idle(&o).await?;
        require(
            w.get_message() == cancelled && o.preview.borrow().is_none(),
            "cancelled import published a late error",
        )?;
        fill(&w, "Abandoned", "not saved");
        w.invoke_close_filters();
        ui.invoke_filters_show();
        require(
            w.get_account().is_empty() && !o.rules().entries.iter().any(|e| e.id == "Abandoned"),
            "close retained an unsaved draft",
        )?;
        require(
            o.article.requests.get() == 0,
            "local filter management requested a station page",
        )?;
        verification::capture(&w, &directory, "filter-form.png")?;
        verification::capture(&ui, &directory, "filtered-terminal.png")?;
        std::fs::write(
            directory.join("saved-filters.json"),
            std::fs::read(&path).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","scope":"full native components, synthetic foreground and local zero-input peer; OS picker/IME and other platforms pending","crud":true,"case_insensitive":true,"mask_projection":true,"row_projection":true,"colors":true,"tab_sort":true,"preview_cancel_apply":true,"late_import_close":true,"backup_failure_preserves_draft":true,"corrupt_read_only_reload":true,"root_key_blocked":true,"article_mode":if web {"web"}else{"terminal"},"full_load_requests":0,"late_import_error_cancelled":true})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        if std::env::var("PTTSTELLA_SLINT_FILTERS_PROBE").is_ok_and(|v| v == "shutdown") {
            let base = o.rules();
            let next = base.edit("Shutdown", "explicit pending save", false)?;
            let store = o.store.clone()?;
            o.start(
                Box::new(move || {
                    std::thread::sleep(Duration::from_millis(120));
                    store.save_from(&base, next).map(Value::Saved)
                }),
                Operation::Save,
            );
            require(
                o.result.borrow().is_some(),
                "shutdown did not retain the pending save slot",
            )?;
        } else {
            w.invoke_close_filters();
        }
        Ok(())
    })
}

/// After normal event-loop exit: the exact joined worker result must survive
/// without a UI callback, and the durable document must match effective rules.
pub fn audit_closed(o: &Manager, directory: &std::path::Path, path: PathBuf) -> Result<(), String> {
    let saved = Store::at(path).load()?;
    require(
        o.stopped.get()
            && o.window.borrow().is_none()
            && o.result.borrow().is_none()
            && !o.output.busy()
            && o.request.get().is_none()
            && o.preview.borrow().is_none(),
        "filter manager exit retained work/window/preview",
    )?;
    require(
        saved == o.rules(),
        "joined filter write differs from the durable document",
    )?;
    let pending = std::env::var("PTTSTELLA_SLINT_FILTERS_PROBE").is_ok_and(|v| v == "shutdown");
    if pending {
        require(
            saved
                .entries
                .iter()
                .any(|e| e.id == "Shutdown" && e.note == "explicit pending save"),
            "app exit lost an authorized pending filter save",
        )?;
    }
    std::fs::write(directory.join("filter-close-audit.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","worker_joined":true,"result_collected":true,"window_dropped":true,"preview_cleared":true,"durable_matches_effective":true,"pending_shutdown_write":pending})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
