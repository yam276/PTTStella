//! Real main-window draft owners; filesystem faults and cross-process barriers
//! are explicit diagnostics, never production storage hooks.
use super::{BODY, TITLE, set_body, source};
use crate::{
    EditorState, TerminalWindow,
    editor::Editor,
    verification::{capture, require},
};
use pttstella::drafts::store::Store;
use slint::{ComponentHandle, Model};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

/// Bounded, text-free event evidence for the explicitly isolated native fixture.
/// Normal and live windows never enable this trace.
pub(crate) struct Trace {
    role: &'static str,
    remaining: Cell<u16>,
}
impl Trace {
    pub fn new(role: &'static str) -> Self {
        let enabled = std::env::args_os().any(|a| a == "--probe-drafts")
            && std::env::var("PTTSTELLA_SLINT_EDITOR_TRACE").as_deref() == Ok("1")
            && case().is_ok_and(|c| c == Some(Case::Backup))
            && std::env::var_os("PTTSTELLA_SLINT_DRAFTS_PROBE_DIR").is_some();
        Self {
            role,
            remaining: Cell::new(if enabled { 192 } else { 0 }),
        }
    }
    pub fn record(&self, kind: &str, detail: impl FnOnce() -> serde_json::Value) {
        let left = self.remaining.get();
        if left == 0 {
            return;
        }
        self.remaining.set(left - 1);
        let detail = detail();
        eprintln!(
            "{}",
            serde_json::json!({"editor_probe_trace":self.role,"sequence":192-left,"event":kind,"detail":detail})
        );
    }
    pub fn native(&self, event: &slint::winit_030::winit::event::WindowEvent) {
        use slint::winit_030::winit::{
            event::{Ime, WindowEvent},
            keyboard::Key,
        };
        if self.remaining.get() == 0 {
            return;
        }
        let detail = match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                serde_json::json!({"key":match &event.logical_key {Key::Named(n)=>format!("{n:?}"),_=>"text-redacted".into()},"state":format!("{:?}",event.state),"repeat":event.repeat,"synthetic":is_synthetic})
            }
            WindowEvent::Ime(Ime::Preedit(s, _)) => {
                serde_json::json!({"ime":"preedit","bytes":s.len()})
            }
            WindowEvent::Ime(Ime::Commit(s)) => serde_json::json!({"ime":"commit","bytes":s.len()}),
            WindowEvent::Ime(Ime::Disabled) => serde_json::json!({"ime":"disabled"}),
            WindowEvent::Ime(Ime::Enabled) => serde_json::json!({"ime":"enabled"}),
            WindowEvent::Focused(f) => serde_json::json!({"focused":f}),
            WindowEvent::MouseInput { state, button, .. } => {
                serde_json::json!({"mouse":format!("{button:?}"),"state":format!("{state:?}")})
            }
            _ => return,
        };
        self.record("native", || detail);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Case {
    Backup,
    Corrupt,
    CrashStage,
    CrashRecover,
    SameA,
    SameB,
    DisjointA,
    DisjointB,
}
pub(super) fn case() -> Result<Option<Case>, String> {
    let value = std::env::var("PTTSTELLA_DRAFTS_STORAGE_CASE").ok();
    let case = match value.as_deref() {
        None => return Ok(None),
        Some("backup") => Case::Backup,
        Some("corrupt") => Case::Corrupt,
        Some("crash-stage") => Case::CrashStage,
        Some("crash-recover") => Case::CrashRecover,
        Some("same-a") => Case::SameA,
        Some("same-b") => Case::SameB,
        Some("disjoint-a") => Case::DisjointA,
        Some("disjoint-b") => Case::DisjointB,
        _ => return Err("unknown draft storage fixture".into()),
    };
    if [
        "PTTSTELLA_DRAFTS_CANCEL",
        "PTTSTELLA_SLINT_DRAFTS_RESTORE",
        "PTTSTELLA_DRAFTS_SOURCE_CASE",
    ]
    .iter()
    .any(|v| std::env::var_os(v).is_some())
    {
        return Err(
            "storage fixture cannot combine cancellation/restore/station failure modes".into(),
        );
    }
    Ok(Some(case))
}
fn path() -> Result<PathBuf, String> {
    std::env::var_os("PTTSTELLA_SLINT_DRAFTS_PROBE_DIR")
        .map(|p| PathBuf::from(p).join("probe-drafts.json"))
        .ok_or("storage fixtures require an explicit isolated profile".into())
}
async fn until(mut condition: impl FnMut() -> bool, message: &str) -> Result<(), String> {
    for _ in 0..1500 {
        if condition() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(message.into())
}
fn write(output: &Path, name: &str, value: serde_json::Value) -> Result<(), String> {
    std::fs::write(
        output.join(name),
        serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn saved(owner: &Editor) -> bool {
    owner
        .document
        .borrow()
        .as_ref()
        .is_some_and(|d| owner.saved.get() == Some(d.version()))
}
async fn save(ui: &TerminalWindow, owner: &Editor) -> Result<(), String> {
    ui.global::<EditorState>().invoke_action(1);
    until(
        || saved(owner),
        "local storage did not acknowledge current Document version",
    )
    .await
}
pub(super) fn choose(ui: &TerminalWindow, case: Case) -> Result<(), String> {
    let state = ui.global::<EditorState>();
    match case {
        Case::Backup | Case::Corrupt | Case::CrashStage => state.invoke_choose("".into()),
        Case::CrashRecover => {
            let rows = state.get_drafts();
            require(
                rows.row_count() == 1,
                "crash recovery must find exactly one saved draft",
            )?;
            let item = rows.row_data(0).ok_or("missing recovered draft")?;
            require(
                item.title.as_str() == TITLE,
                "crash recovery changed saved title",
            )?;
            state.invoke_choose(item.id);
        }
        _ => {
            let id = if case == Case::DisjointB {
                "two"
            } else {
                "one"
            };
            let rows = state.get_drafts();
            let item = (0..rows.row_count())
                .filter_map(|i| rows.row_data(i))
                .find(|e| e.id.as_str() == id)
                .ok_or("shared draft seed not found")?;
            state.invoke_choose(item.id);
        }
    }
    Ok(())
}
pub(super) async fn verify(
    ui: &TerminalWindow,
    owner: &Rc<Editor>,
    output: &Path,
    case: Case,
) -> Result<(), String> {
    let path = path()?;
    match case {
        Case::Backup | Case::Corrupt => disk(ui, owner, output, case, &path).await?,
        Case::CrashStage => {
            set_body(ui, BODY)?;
            ui.global::<EditorState>().invoke_title_edit(TITLE.into());
            save(ui, owner).await?;
            require(
                Store::at(path.clone()).load()?.entries[0].body == BODY,
                "pre-crash durable body differs",
            )?;
            set_body(ui, &format!("{BODY}\n尚未保存"))?;
            require(
                !saved(owner),
                "crash fixture accidentally saved the pending edit",
            )?;
            // Parent terminates this exact PID only after inspecting this marker.
            // Returning would run normal main cleanup and invalidate a crash test.
            write(
                output,
                "crash-ready.json",
                serde_json::json!({"status":"ready_for_forced_termination","pid":std::process::id(),"saved_body":BODY,"pending_body":source(owner),"scope":"saved acknowledgement then pending debounce edit; not arbitrary mid-persist/power-loss coverage"}),
            )?;
            std::future::pending::<()>().await;
        }
        Case::CrashRecover => {
            require(
                source(owner) == BODY && saved(owner),
                "crash recovery did not restore the last confirmed save",
            )?;
            set_body(ui, &format!("{BODY}\n恢復後修改"))?;
            save(ui, owner).await?;
            require(
                Store::at(path.clone()).load()?.entries[0].body == format!("{BODY}\n恢復後修改"),
                "recovered editor could not save subsequent changes",
            )?;
        }
        _ => concurrent(ui, owner, output, case).await?,
    }
    capture(ui, output, "storage.png")?;
    write(
        output,
        "drafts.json",
        serde_json::json!({"status":"passed","storage_case":format!("{case:?}"),"saved_current_version":saved(owner),"scope":"real main-window owners with synthetic widget input; parent independently checks durable documents and process outcomes; OS input separate"}),
    )
}
async fn disk(
    ui: &TerminalWindow,
    owner: &Rc<Editor>,
    output: &Path,
    case: Case,
    path: &Path,
) -> Result<(), String> {
    set_body(ui, BODY)?;
    let state = ui.global::<EditorState>();
    state.invoke_title_edit(TITLE.into());
    save(ui, owner).await?;
    let before = std::fs::read(path).map_err(|e| e.to_string())?;
    let backup = path.with_extension("json.bak");
    let held = path.with_extension("json.probe-held-backup");
    require(!held.exists(), "backup fault staging already exists")?;
    let had_backup = backup.exists();
    if case == Case::Backup {
        if had_backup {
            std::fs::rename(&backup, &held).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir(&backup).map_err(|e| e.to_string())?;
    } else {
        std::fs::write(path, b"deliberately corrupt synthetic draft document")
            .map_err(|e| e.to_string())?;
    }
    let pending = format!("{BODY}\n失敗後仍保留");
    set_body(ui, &pending)?;
    // Use the editor's Close action: a failed flush must keep it mounted.
    state.invoke_action(3);
    until(
        || {
            !owner.closing.get()
                && !state.get_status().is_empty()
                && state.get_status().as_str() != pttstella::msg!("draft-saving")
        },
        "storage failure did not reach the actual UI",
    )
    .await?;
    require(
        state.get_open()
            && state.get_selected()
            && !saved(owner)
            && source(owner) == pending
            && !state.get_can_transfer(),
        "failed close discarded or falsely acknowledged the pending document",
    )?;
    let message = state.get_status().to_string();
    require(
        message != pttstella::msg!("draft-saved") && message != pttstella::msg!("draft-unsaved"),
        "storage error was not surfaced",
    )?;
    let rejected = std::fs::read(path).map_err(|e| e.to_string())?;
    require(
        rejected
            == if case == Case::Backup {
                before.clone()
            } else {
                b"deliberately corrupt synthetic draft document".to_vec()
            },
        "failed storage changed the existing document",
    )?;
    capture(ui, output, "storage-error.png")?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    require(
        std::fs::read(path).map_err(|e| e.to_string())? == rejected,
        "failed save silently retried",
    )?;
    if case == Case::Backup {
        std::fs::remove_dir(&backup).map_err(|e| e.to_string())?;
        if had_backup {
            std::fs::rename(&held, &backup).map_err(|e| e.to_string())?;
        }
    } else {
        std::fs::write(path, &before).map_err(|e| e.to_string())?;
    }
    let final_body = format!("{pending}\n修復後保存");
    set_body(ui, &final_body)?;
    save(ui, owner).await?;
    let data = Store::at(path.to_path_buf()).load()?;
    require(
        data.entries.len() == 1 && data.entries[0].body == final_body,
        "retry lost pending source or created a false conflict",
    )?;
    require(
        std::fs::read(&backup).map_err(|e| e.to_string())? == before,
        "successful retry did not back up the original durable document",
    )?;
    write(
        output,
        "storage-error.json",
        serde_json::json!({"status":"passed","notice":message,"failed_editor_close_kept_open":true,"old_file_preserved":true,"pending_source_preserved":true,"retry_saved_latest":true,"fault":format!("{case:?}"),"scope":"filesystem backup rename failure or invalid document, not ENOSPC/all disk faults"}),
    )
}
async fn concurrent(
    ui: &TerminalWindow,
    owner: &Rc<Editor>,
    output: &Path,
    case: Case,
) -> Result<(), String> {
    require(
        source(owner) == BODY && saved(owner),
        "concurrent process did not read the seeded baseline",
    )?;
    write(
        output,
        "ready.json",
        serde_json::json!({"pid":std::process::id(),"baseline_body":source(owner),"status":"waiting_for_parent_release"}),
    )?;
    until(
        || output.join("release").is_file(),
        "parent did not release concurrent editor",
    )
    .await?;
    // The second OS window can have focus. Widget activation is explicit;
    // no actual focus/IME assertion is made by this cross-process test.
    crate::verification::foreground(ui, true);
    let a = matches!(case, Case::SameA | Case::DisjointA);
    let body = if a {
        "中文 A\n\x1b[31m紅\x1b[m"
    } else {
        "中文 B\n\x1b[32m綠\x1b[m"
    };
    set_body(ui, body)?;
    save(ui, owner).await?;
    let conflict =
        ui.global::<EditorState>().get_status().as_str() == pttstella::msg!("draft-conflict-saved");
    require(
        conflict == (case == Case::SameB),
        "wrong cross-process conflict notice",
    )?;
    if case == Case::SameB {
        set_body(ui, &format!("{body}\n副本後續修改"))?;
        save(ui, owner).await?;
        require(
            ui.global::<EditorState>().get_status().as_str() == pttstella::msg!("draft-saved"),
            "second edit did not continue the existing conflict copy",
        )?;
    }
    write(
        output,
        "concurrent.json",
        serde_json::json!({"status":"passed","case":format!("{case:?}"),"first_save_conflict":conflict,"body":source(owner),"pid":std::process::id()}),
    )
}
