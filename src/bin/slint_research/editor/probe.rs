//! Complete main-window editor conformance. Strict loopback, synthetic toolkit
//! input only; native IME/clipboard and accessibility require separate OS proof.
use super::Editor;
use crate::{
    EditorState, TerminalWindow,
    verification::{Verification, capture, click, foreground, require, wait},
};
use slint::{
    ComponentHandle, Model,
    platform::{PointerEventButton as Button, WindowEvent as Event},
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
    ui.global::<EditorState>().invoke_action(0);
    wait(
        || {
            let s = ui.global::<EditorState>();
            s.get_open() && s.get_can_create() && !s.get_busy()
        },
        "template capture/list did not finish",
    )
    .await?;
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
        ui.global::<EditorState>().invoke_color();
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
        ui.global::<EditorState>().set_foreground(10);
        ui.global::<EditorState>().invoke_color();
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
