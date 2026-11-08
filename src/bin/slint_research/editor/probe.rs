//! Complete main-window editor conformance. Strict loopback, synthetic toolkit
//! input only; native IME/clipboard and accessibility require separate OS proof.
mod failure;
use super::Editor;
use crate::{
    EditorState, TerminalWindow,
    verification::{Verification, capture, click, foreground, require, wait},
};
pub use failure::audit_closed;
use slint::{
    ComponentHandle, Model,
    platform::{Key, PointerEventButton as Button, WindowEvent as Event},
};
use std::{
    error::Error,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};
const BODY: &str = "第一行中文\n\x1b[1;31m亮紅中文\x1b[m\n第三行";
const TITLE: &str = "本機草稿測試";
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<Editor>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(ui, directory, "drafts", move |ui, directory| async move {
        verify(&ui, &owner, &directory).await
    })
}
fn source(owner: &Editor) -> String {
    owner
        .document
        .borrow()
        .as_ref()
        .map(|d| d.source().to_owned())
        .unwrap_or_default()
}
fn set_body(ui: &TerminalWindow, body: &str) -> Result<(), String> {
    ui.invoke_editor_edit_key(0);
    require(
        ui.global::<EditorState>().invoke_insert(body.into()),
        "body edit rejected",
    )
}
fn point(ui: &TerminalWindow, col: f32, row: f32) -> slint::LogicalPosition {
    let b = ui.get_editor_grid_bounds();
    slint::LogicalPosition::new(b.x + b.width * col / 79., b.y + b.height * row / 23.)
}
async fn verify(ui: &TerminalWindow, owner: &Rc<Editor>, directory: &Path) -> Result<(), String> {
    wait(
        || {
            foreground(ui, true);
            let view = owner.connection.updates.borrow();
            pttstella::drafts::scope(&view).is_some()
                && owner.shown()
                && !ui.get_frame_paint_pending()
        },
        "strict draft peer did not present its synthetic board",
    )
    .await?;
    let generation = owner.connection.updates.borrow().generation;
    owner.connection.input(generation, vec![16])?;
    wait(
        || owner.connection.updates.borrow().draft_post_scope.is_some() && owner.shown(),
        "native posting fixture not ready",
    )
    .await?;
    // Let initial window manager resize/focus settle before toolkit activation.
    tokio::time::sleep(Duration::from_millis(300)).await;
    foreground(ui, true);
    require(
        ui.global::<EditorState>().get_available(),
        "draft entry button disappeared in station editor",
    )?;
    let cancel_case = std::env::var("PTTSTELLA_DRAFTS_CANCEL").ok();
    if cancel_case
        .as_deref()
        .is_some_and(|c| !matches!(c, "capture" | "capture-live" | "transfer"))
    {
        return Err("unknown drafts cancellation case".into());
    }
    let before_capture = owner.connection.updates.borrow().frame.revision;
    ui.global::<EditorState>().invoke_action(0);
    let failure_case = crate::session::draft_failure()?;
    if failure_case == Some(pttstella::connection::DraftFailure::CloseCapture) {
        return failure::close_busy(ui, owner, directory, before_capture, "capture").await;
    }
    if matches!(cancel_case.as_deref(), Some("capture" | "capture-live")) {
        if cancel_case.as_deref() == Some("capture-live") {
            wait(
                || owner.connection.updates.borrow().frame.revision > before_capture,
                "capture never entered a station step",
            )
            .await?;
        }
        return cancel(
            ui,
            owner,
            directory,
            cancel_case.as_deref().unwrap_or("capture"),
        )
        .await;
    }
    if failure_case.is_some() {
        failure::wait_template(ui).await?;
    } else {
        wait(
            || {
                let s = ui.global::<EditorState>();
                s.get_open() && s.get_can_create() && !s.get_busy()
            },
            "template capture/list did not finish",
        )
        .await?;
    }
    let restored = std::env::var_os("PTTSTELLA_SLINT_DRAFTS_RESTORE").is_some();
    if restored {
        let drafts = ui.global::<EditorState>().get_drafts();
        require(
            drafts.row_count() == 1,
            "restart did not find exactly one isolated draft",
        )?;
        let item = drafts.row_data(0).ok_or("restart draft missing")?;
        require(item.title.as_str() == TITLE, "restart title differs")?;
        ui.global::<EditorState>().invoke_choose(item.id);
    } else {
        ui.global::<EditorState>().invoke_choose("".into());
    }
    wait(
        || ui.global::<EditorState>().get_selected() && ui.get_editor_grid_bounds().width > 0.,
        "native editor did not mount",
    )
    .await?;
    foreground(ui, true);
    ui.invoke_focus_editor();
    wait(
        || ui.get_editor_body_focused(),
        "native transient input not focused",
    )
    .await?;
    require(
        !ui.get_keep_alive_surface_ready(),
        "editor did not suspend BBS keepalive",
    )?;
    if let Some(case) = failure_case {
        return failure::verify(ui, owner, directory, case).await;
    }
    require(
        ui.get_terminal_pointer_blocked() && !ui.get_vim_available(),
        "editor did not block terminal tools/input",
    )?;
    if !restored {
        require(
            source(owner) == "看板預設格式\n\x1b[32m請依格式填寫\x1b[m\n",
            "template lost ANSI or trailing empty line",
        )?;
        ui.window()
            .dispatch_event(Event::KeyPressed { text: "Z".into() });
        ui.window()
            .dispatch_event(Event::KeyReleased { text: "Z".into() });
        require(
            source(owner).starts_with("Z看板"),
            "native body field did not commit once",
        )?;
        ui.invoke_editor_edit_key(4);
        require(
            source(owner).starts_with("看板"),
            "body native commit did not share Rust undo",
        )?;
        // Half-colour graphic fixture uses the production grid painter.
        set_body(ui, "\x1b[31;66;34m田▁▂▃▄▅▆▇█\n\x1b[0mA\t中\n")?;
        capture(ui, directory, "dual-graphic.png")?;
        let exact = source(owner);
        for mode in [1, 2, 0] {
            ui.global::<EditorState>().invoke_mode_edit(mode);
            require(source(owner) == exact, "mode switch modified raw source")?;
        }
        ui.invoke_editor_edit_key(0);
        ui.global::<EditorState>().invoke_palette_request(-1., -1.);
        ui.global::<EditorState>().invoke_palette_choose(15);
        ui.global::<EditorState>().invoke_palette_dismiss(true);
        require(
            source(owner) == exact,
            "ambiguous SGR66 colour must reject atomically",
        )?;
        set_body(ui, BODY)?;
        require(
            source(owner) == BODY,
            "whole-document replacement retained boundary SGR",
        )?;
        ui.global::<EditorState>().invoke_title_edit(TITLE.into());
        require(
            owner
                .document
                .borrow()
                .as_ref()
                .is_some_and(|d| d.title() == TITLE),
            "title does not share Document",
        )?;
        ui.invoke_editor_edit_key(4);
        require(
            owner
                .document
                .borrow()
                .as_ref()
                .is_some_and(|d| d.title() != TITLE),
            "title not in shared undo",
        )?;
        ui.invoke_editor_edit_key(5);
        // The real body TouchArea resolves either half of a Chinese character.
        let before_selection_image = ui.global::<EditorState>().get_pixels();
        let a = point(ui, 0.1, 0.5);
        let b = point(ui, 7.9, 0.5);
        ui.window().dispatch_event(Event::PointerPressed {
            position: a,
            button: Button::Left,
        });
        ui.window()
            .dispatch_event(Event::PointerMoved { position: b });
        ui.window().dispatch_event(Event::PointerReleased {
            position: b,
            button: Button::Left,
        });
        require(
            ui.global::<EditorState>().get_pixels() == before_selection_image,
            "selection rerasterized unchanged source",
        )?;
        let selected = {
            let doc = owner.document.borrow();
            let d = doc.as_ref().ok_or("missing document")?;
            d.source()[d.selected_range()?].to_owned()
        };
        require(
            selected == "第一行中",
            "drag split or missed the whole Chinese glyph",
        )?;
        require(
            ui.global::<EditorState>().get_spans().row_count() > 0,
            "selection highlight did not mount",
        )?;
        capture(ui, directory, "selection.png")?;
        let geometry = ui.get_editor_grid_bounds();
        click(ui, b, Button::Right);
        require(
            ui.global::<EditorState>().get_palette_open(),
            "right-click did not open the leased palette",
        )?;
        require(
            ui.get_editor_grid_bounds() == geometry,
            "floating palette changed the document layout",
        )?;
        require(
            ui.global::<EditorState>().get_pixels() == before_selection_image,
            "palette rebuilt the document raster",
        )?;
        capture(ui, directory, "palette.png")?;
        crate::verification::key(ui, Key::Home);
        for _ in 0..10 {
            crate::verification::key(ui, Key::RightArrow);
        }
        crate::verification::key(ui, Key::Return);
        require(
            !ui.global::<EditorState>().get_palette_open(),
            "color choice did not close palette",
        )?;
        wait(
            || ui.get_editor_body_focused(),
            "successful palette did not return body focus for native undo",
        )
        .await?;
        require(
            source(owner) != BODY,
            "ordinary native colour action did not edit source",
        )?;
        ui.invoke_editor_edit_key(4);
        require(
            source(owner) == BODY,
            &format!(
                "colour undo lost raw bytes: source {:?}, composing {}, busy {}, undo {}",
                source(owner),
                ui.get_editor_composing(),
                ui.global::<EditorState>().get_busy(),
                ui.global::<EditorState>().get_can_undo()
            ),
        )?;
        ui.global::<EditorState>().invoke_palette_request(-1., -1.);
        require(
            ui.global::<EditorState>().get_palette_open(),
            "palette failed to reopen",
        )?;
        crate::verification::key(ui, Key::Escape);
        require(
            ui.get_editor_open() && !ui.global::<EditorState>().get_palette_open(),
            "palette Escape closed the local editor",
        )?;
        ui.global::<EditorState>().invoke_palette_request(-1., -1.);
        click(ui, slint::LogicalPosition::new(2., 2.), Button::Left);
        require(
            !ui.global::<EditorState>().get_palette_open() && source(owner) == BODY,
            "outside dismiss leaked a document edit",
        )?;
        ui.global::<EditorState>().invoke_selection_edit(1);
        let p = point(ui, 1.2, 0.5);
        click(ui, p, Button::Left);
        require(
            owner
                .document
                .borrow()
                .as_ref()
                .is_some_and(|d| d.selection().range().is_empty()),
            "line-mode single click selected a character/line",
        )?;
        let end = point(ui, 2.2, 1.5);
        ui.window().dispatch_event(Event::PointerPressed {
            position: p,
            button: Button::Left,
        });
        ui.window()
            .dispatch_event(Event::PointerMoved { position: end });
        ui.window().dispatch_event(Event::PointerReleased {
            position: end,
            button: Button::Left,
        });
        let lines = {
            let document = owner.document.borrow();
            let d = document.as_ref().ok_or("missing document")?;
            owner
                .viewport
                .borrow()
                .ranges(d)?
                .into_iter()
                .map(|r| d.source()[r].to_owned())
                .collect::<Vec<_>>()
        };
        require(
            lines == ["第一行中文\n\x1b[1;31m亮紅中文\x1b[m\n"],
            "whole-line drag lost raw style or selected an extra line",
        )?;
        capture(ui, directory, "whole-lines.png")?;
        ui.global::<EditorState>().invoke_selection_edit(0);
        set_body(ui, "A中B\nC文D\n")?;
        ui.global::<EditorState>().invoke_selection_edit(2);
        let a = point(ui, 2.2, 0.5);
        let b = point(ui, 3.1, 1.5);
        ui.window().dispatch_event(Event::PointerPressed {
            position: a,
            button: Button::Left,
        });
        ui.window()
            .dispatch_event(Event::PointerMoved { position: b });
        ui.window().dispatch_event(Event::PointerReleased {
            position: b,
            button: Button::Left,
        });
        let parts = {
            let document = owner.document.borrow();
            let d = document.as_ref().ok_or("missing document")?;
            owner
                .viewport
                .borrow()
                .ranges(d)?
                .into_iter()
                .map(|r| d.source()[r].to_owned())
                .collect::<Vec<_>>()
        };
        require(
            parts == ["中", "文"],
            "explicit rectangle selector did not preserve whole Chinese glyphs",
        )?;
        click(ui, b, Button::Right);
        ui.global::<EditorState>().invoke_palette_choose(20);
        require(
            !ui.global::<EditorState>().get_palette_open(),
            "rectangle color did not complete",
        )?;
        {
            let document = owner.document.borrow();
            let d = document.as_ref().ok_or("missing document")?;
            for row in d.rows().iter().take(2) {
                let glyphs = row.glyphs(d.mode()).collect::<Vec<_>>();
                require(
                    glyphs[0].left.bg == 0
                        && glyphs[1].left.bg == 4
                        && glyphs[1].right.bg == 4
                        && glyphs[2].left.bg == 0,
                    "rectangle color changed an unselected cell",
                )?;
            }
            require(
                owner.viewport.borrow().ranges(d)?.len() == 2,
                "successful rectangle color discarded separate selections",
            )?;
        }
        capture(ui, directory, "rectangle.png")?;
        let colored_rectangle = source(owner);
        ui.invoke_editor_edit_key(4);
        require(
            source(owner) == "A中B\nC文D\n",
            "rectangle color was not one undo",
        )?;
        {
            let document = owner.document.borrow();
            let d = document.as_ref().ok_or("missing document")?;
            require(
                owner.viewport.borrow().ranges(d)? == [1..4, 7..10],
                "undo converted the rectangle to one destructive text range",
            )?;
            require(
                owner.viewport.borrow().highlights(d)? == [(0, 1, 2), (1, 1, 2)],
                "undo did not restore both visible rectangle spans",
            )?;
        }
        ui.invoke_editor_edit_key(5);
        require(
            source(owner) == colored_rectangle,
            "rectangle redo changed raw source",
        )?;
        ui.invoke_editor_edit_key(4);
        ui.window()
            .dispatch_event(Event::KeyPressed { text: "X".into() });
        ui.window()
            .dispatch_event(Event::KeyReleased { text: "X".into() });
        require(
            source(owner) == "AXB\nCXD\n",
            "typing after native undo deleted unselected exterior/intervening text",
        )?;
        ui.invoke_editor_edit_key(4);
        capture(ui, directory, "rectangle-history.png")?;
        ui.global::<EditorState>().invoke_selection_edit(0);
        set_body(ui, BODY)?;
        // IME preedit stays outside the document and autosave snapshot.
        ui.invoke_editor_composition(true);
        require(
            source(owner) == BODY,
            "preedit changed committed raw source",
        )?;
        ui.invoke_editor_composition(false);
        // Scroll and clipped rendering allocate only the fixed 79x23 visible frame.
        let long = (0..90)
            .map(|n| format!("第{n:02}行 中{}", "AB中".repeat(30)))
            .collect::<Vec<_>>()
            .join("\n");
        set_body(ui, &long)?;
        ui.global::<EditorState>().invoke_scroll(35, 7);
        require(
            owner.renderer.borrow().frame().cols == 79
                && owner.renderer.borrow().frame().rows == 23,
            "editor allocated the full document surface",
        )?;
        require(
            owner.renderer.borrow().frame().glyphs.len() <= 79 * 23 + 23,
            "visible glyph cap exceeded",
        )?;
        capture(ui, directory, "scrolled.png")?;
        ui.invoke_editor_edit_key(4);
        require(source(owner) == BODY, "long edit undo did not restore body")?;
        ui.global::<EditorState>().invoke_action(1);
        wait(
            || {
                let d = owner.document.borrow();
                d.as_ref()
                    .is_some_and(|d| owner.saved.get() == Some(d.version()))
            },
            "explicit save did not finish",
        )
        .await?;
        require(
            ui.global::<EditorState>().get_can_transfer(),
            "saved matching template cannot transfer",
        )?;
        capture(ui, directory, "saved.png")?;
        if cancel_case.as_deref() == Some("transfer") {
            ui.global::<EditorState>().invoke_action(2);
            ui.global::<EditorState>().invoke_action(6);
            return cancel(ui, owner, directory, "transfer").await;
        }
        // Close and restore in this process, retaining the exact same disk owner.
        ui.global::<EditorState>().invoke_action(3);
        require(
            !ui.get_editor_open(),
            "saved close did not restore terminal",
        )?;
        require(
            ui.global::<EditorState>().get_pixels().size().width == 0
                && owner.renderer.borrow().frame().glyphs.is_empty(),
            "closed editor retained document pixels/glyphs",
        )?;
        ui.global::<EditorState>().invoke_action(0);
        wait(
            || {
                !ui.global::<EditorState>().get_busy()
                    && ui.global::<EditorState>().get_can_create()
            },
            "reopen capture did not finish",
        )
        .await?;
        let item = ui
            .global::<EditorState>()
            .get_drafts()
            .row_data(0)
            .ok_or("saved draft not listed")?;
        ui.global::<EditorState>().invoke_choose(item.id);
        wait(
            || ui.global::<EditorState>().get_selected(),
            "saved draft not restored",
        )
        .await?;
    }
    require(source(owner) == BODY, "restore body mismatch")?;
    require(
        owner
            .document
            .borrow()
            .as_ref()
            .is_some_and(|d| d.title() == TITLE),
        "restore title mismatch",
    )?;
    // Escape/confirmation cancel leaves the station unchanged. Transfer is two clicks.
    ui.global::<EditorState>().invoke_action(2);
    require(
        ui.global::<EditorState>().get_transfer_confirm(),
        "missing explicit transfer confirmation",
    )?;
    ui.global::<EditorState>().invoke_action(7);
    require(
        !ui.global::<EditorState>().get_transfer_confirm(),
        "confirmation did not cancel",
    )?;
    let transferred = restored;
    if transferred {
        ui.global::<EditorState>().invoke_action(2);
        ui.global::<EditorState>().invoke_action(6);
        wait(
            || !ui.get_editor_open(),
            "confirmed transfer did not finish",
        )
        .await?;
        let snapshot = owner
            .connection
            .updates
            .borrow()
            .draft_source
            .clone()
            .ok_or("missing final station source")?;
        require(
            snapshot.body == BODY && snapshot.title == TITLE,
            "station bytes/title differ from saved source",
        )?;
        require(
            pttstella::navigation::article_editor(&owner.connection.updates.borrow().frame),
            "transfer did not stop in station editor",
        )?;
    }
    // Focus/keyboard behaviour here is synthetic toolkit evidence only.
    if !transferred {
        let p = point(ui, 1., 0.5);
        click(ui, p, Button::Left);
    }
    let report = serde_json::json!({"status":"passed","restored_new_process":restored,"transferred":transferred,"body":BODY,"title":TITLE,"template_preserved":true,"toolkit_body_commit":!restored,"scope":"main Slint window and strict loopback; OS IME, clipboard and accessibility remain separate"});
    std::fs::write(
        directory.join("drafts.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn cancel(
    ui: &TerminalWindow,
    owner: &Editor,
    directory: &Path,
    case: &str,
) -> Result<(), String> {
    require(
        ui.global::<EditorState>().get_source_busy(),
        "cancellation case did not have a live source task",
    )?;
    let before = source(owner);
    let received_before = owner.connection.updates.borrow().received;
    ui.global::<EditorState>().invoke_action(9);
    wait(
        || !ui.global::<EditorState>().get_source_busy(),
        "cancel did not retire the source task",
    )
    .await?;
    require(
        ui.get_editor_open() && source(owner) == before,
        "cancellation discarded the local draft",
    )?;
    require(
        owner.operation.borrow().is_none() && owner.source_cancel.borrow().is_none(),
        "source task/cancel owner survived cancellation",
    )?;
    require(
        ui.global::<EditorState>().get_status().as_str() == pttstella::msg!("draft-stop-requested"),
        "cancellation did not warn about retained local/partial station state",
    )?;
    tokio::time::sleep(Duration::from_millis(300)).await;
    let settled = owner.connection.updates.borrow().received;
    tokio::time::sleep(Duration::from_millis(300)).await;
    require(
        owner.connection.updates.borrow().received == settled,
        "source work continued sending after cancellation settled",
    )?;
    if case == "transfer" {
        require(
            received_before == settled,
            "queued transfer wrote to the station after cancel",
        )?;
    } else {
        require(
            !ui.global::<EditorState>().get_can_create()
                && !ui.global::<EditorState>().get_selected(),
            "late capture opened a cancelled draft",
        )?;
    }
    capture(ui, directory, "cancelled.png")?;
    let report = serde_json::json!({"status":"passed","cancel_case":case,"source_task_retired":true,"no_late_ui_open":true,"received_before":received_before,"received_settled":settled,"local_source_preserved":true,"scope":"explicit native-widget cancellation; partial-clear/transport error cases remain separate"});
    std::fs::write(
        directory.join("drafts.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
