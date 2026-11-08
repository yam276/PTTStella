use super::super::Editor;
use super::{BODY, TITLE, set_body, source};
use crate::{
    EditorState, TerminalWindow,
    verification::{capture, require},
};
use pttstella::{
    connection::{DraftFailure, Phase},
    drafts::store::Store,
};
use slint::ComponentHandle;
use std::{
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
async fn until(mut test: impl FnMut() -> bool, message: &str) -> Result<(), String> {
    for _ in 0..900 {
        if test() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(message.into())
}
pub(super) async fn wait_template(ui: &TerminalWindow) -> Result<(), String> {
    until(
        || {
            let s = ui.global::<EditorState>();
            s.get_open() && s.get_can_create() && !s.get_busy()
        },
        "deliberately delayed source capture did not finish",
    )
    .await
}
fn directory() -> Result<PathBuf, String> {
    std::env::var_os("PTTSTELLA_SLINT_DRAFTS_PROBE_DIR")
        .map(PathBuf::from)
        .ok_or("failure cases require an explicit isolated draft directory".into())
}
fn saved_body() -> Result<String, String> {
    let data = Store::at(directory()?.join("probe-drafts.json")).load()?;
    if data.entries.len() != 1 || data.entries[0].title != TITLE {
        return Err("saved failure fixture title/count differs".into());
    }
    Ok(data.entries[0].body.clone())
}
pub(super) async fn close_busy(
    ui: &TerminalWindow,
    owner: &Editor,
    output: &Path,
    before: u64,
    stage: &str,
) -> Result<(), String> {
    until(
        || owner.connection.updates.borrow().frame.revision > before,
        "close case never entered a source step",
    )
    .await?;
    require(
        ui.global::<EditorState>().get_source_busy(),
        "close case no longer has an active source task",
    )?;
    capture(ui, output, "close-busy.png")?;
    std::fs::write(output.join("drafts.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"exit_requested","source_busy":true,"stage":stage,"scope":"synthetic event-loop exit through normal main cleanup; actual window-manager close is separate"})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
pub(super) async fn verify(
    ui: &TerminalWindow,
    owner: &Rc<Editor>,
    output: &Path,
    case: DraftFailure,
) -> Result<(), String> {
    set_body(ui, BODY)?;
    let state = ui.global::<EditorState>();
    state.invoke_title_edit(TITLE.into());
    state.invoke_action(1);
    until(
        || {
            owner
                .document
                .borrow()
                .as_ref()
                .is_some_and(|d| owner.saved.get() == Some(d.version()))
        },
        "failure fixture local draft did not save",
    )
    .await?;
    require(
        saved_body()? == BODY,
        "saved baseline differs before transfer failure",
    )?;
    require(
        state.get_can_transfer(),
        "failure fixture cannot offer transfer",
    )?;
    let before = owner.connection.updates.borrow().frame.revision;
    state.invoke_action(2);
    require(
        state.get_transfer_confirm(),
        "failure case skipped explicit confirmation",
    )?;
    state.invoke_action(6);
    require(
        state.get_source_busy(),
        "failure case did not start source operation",
    )?;
    if case == DraftFailure::CloseTransfer {
        return close_busy(ui, owner, output, before, "transfer").await;
    }
    until(
        || !state.get_source_busy(),
        "source failure did not stop within step deadline",
    )
    .await?;
    require(
        state.get_open() && state.get_selected() && !state.get_busy(),
        "source failure discarded or blocked the local editor",
    )?;
    require(
        source(owner) == BODY && saved_body()? == BODY,
        "source failure changed local saved text",
    )?;
    let message = state.get_status().to_string();
    let expected = if case == DraftFailure::ChangedTemplate {
        pttstella::msg!("draft-template-changed")
    } else {
        pttstella::msg!("draft-transfer-cancelled")
    };
    require(
        message == expected,
        &format!("wrong draft failure category: {message}"),
    )?;
    require(
        owner.operation.borrow().is_none() && owner.source_cancel.borrow().is_none(),
        "source failure retained local task/cancel owner",
    )?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let settled = owner.connection.updates.borrow().received;
    tokio::time::sleep(Duration::from_millis(500)).await;
    require(
        owner.connection.updates.borrow().received == settled,
        "source failure retried or continued",
    )?;
    capture(ui, output, "failed-local-kept.png")?;
    let offline = case == DraftFailure::Disconnect;
    if offline {
        require(
            matches!(
                owner.connection.updates.borrow().phase,
                Phase::Error | Phase::Disconnected
            ),
            "transport failure did not disconnect",
        )?;
        ui.invoke_focus_editor();
        set_body(ui, &format!("{BODY}\n離線保留"))?;
        state.invoke_action(1);
        until(
            || {
                owner
                    .document
                    .borrow()
                    .as_ref()
                    .is_some_and(|d| owner.saved.get() == Some(d.version()))
            },
            "offline edit did not save",
        )
        .await?;
        require(
            saved_body()? == format!("{BODY}\n離線保留"),
            "offline editor lost subsequent changes",
        )?;
    }
    std::fs::write(output.join("drafts.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","failure_case":format!("{case:?}"),"notice":message,"saved_local_body_preserved":true,"no_retry":true,"source_task_retired":true,"offline_edit_saved":offline,"scope":"full main-window owners; strict peer independently checks title/body/publish boundary at close; OS input is separate"})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
pub fn audit_closed(owner: &Editor, output: &Path, success: bool) -> Result<(), String> {
    let case = crate::session::draft_failure()?;
    if !matches!(
        case,
        Some(DraftFailure::CloseCapture | DraftFailure::CloseTransfer)
    ) {
        return Ok(());
    }
    require(success, "editor cleanup failed during busy close")?;
    require(
        owner.closed.get()
            && owner.storage.borrow().stopped()
            && owner.operation.borrow().is_none()
            && owner.source_cancel.borrow().is_none()
            && owner.replies.borrow().is_none()
            && owner.updates.borrow().is_none(),
        "busy close did not retire and join editor owners",
    )?;
    if case == Some(DraftFailure::CloseTransfer) {
        require(
            saved_body()? == BODY,
            "busy transfer close lost the saved draft",
        )?;
    } else {
        require(
            !directory()?.join("probe-drafts.json").exists(),
            "cancelled capture created a phantom local draft",
        )?;
    }
    std::fs::write(output.join("drafts-close.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","editor_cleanup_joined":true,"ui_task_handles_retired":true,"saved_source_checked":true,"scope":"after actual main cleanup; peer Close result remains separate"})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
}
