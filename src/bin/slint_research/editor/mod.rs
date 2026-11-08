//! Native local editor. Document owns source/history; the retained draft actor
//! owns disk state; Connection alone captures and replaces the station source.
mod palette;
pub mod probe;
mod storage;
mod viewport;
use crate::{
    DraftItem, EditorState, SelectionRect, TerminalWindow,
    clipboard::Clipboard,
    renderer::{Renderer, Size},
    session::FrameId,
};
use pttstella::{
    composer::document::{Document, Mode, Version},
    connection::Connection,
    drafts::{
        Scope,
        session::{Edit, Reply},
        store::Store,
    },
    editor_color::Color,
};
use slint::{
    ComponentHandle, Image, Model, ModelRc, Timer, VecModel, winit_030::WinitWindowAccessor,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};
use storage::{Command, Storage};
use viewport::{COLUMNS, ROWS, SelectionMode, Viewport};

enum SourceResult {
    Captured(pttstella::editor_source::Snapshot),
    Transferred,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PaintStamp {
    version: Version,
    mode: Mode,
    top: usize,
    left: usize,
    size: Size,
}

pub struct Editor {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    presented: Rc<Cell<Option<FrameId>>>,
    clipboard: Clipboard,
    document: RefCell<Option<Document>>,
    viewport: RefCell<Viewport>,
    renderer: RefCell<Renderer>,
    painted: Cell<Option<PaintStamp>>,
    storage: RefCell<Storage>,
    scope: RefCell<Option<Scope>>,
    template: Cell<bool>,
    saved: Cell<Option<Version>>,
    offered: Cell<Option<(u64, Version)>>,
    sequence: Cell<u64>,
    edited_at: Cell<Option<Instant>>,
    offered_at: Cell<Option<Instant>>,
    debounce: Timer,
    closing: Cell<bool>,
    closed: Cell<bool>,
    replies: RefCell<Option<slint::JoinHandle<()>>>,
    updates: RefCell<Option<slint::JoinHandle<()>>>,
    operation: RefCell<Option<slint::JoinHandle<()>>>,
    source_cancel: RefCell<Option<tokio::sync::oneshot::Sender<()>>>,
    palette: RefCell<Option<palette::Lease>>,
    trace: probe::Trace,
}
impl Editor {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        presented: Rc<Cell<Option<FrameId>>>,
        clipboard: Clipboard,
        renderer: &Renderer,
        store: Result<Store, String>,
    ) -> Result<Rc<Self>, String> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            connection,
            presented,
            clipboard,
            document: RefCell::new(None),
            viewport: RefCell::new(Viewport::default()),
            renderer: RefCell::new(renderer.fork(renderer.frame().clone())),
            painted: Cell::new(None),
            storage: RefCell::new(Storage::start(store)),
            scope: RefCell::new(None),
            template: Cell::new(false),
            saved: Cell::new(None),
            offered: Cell::new(None),
            sequence: Cell::new(0),
            edited_at: Cell::new(None),
            offered_at: Cell::new(None),
            debounce: Timer::default(),
            closing: Cell::new(false),
            closed: Cell::new(false),
            replies: RefCell::new(None),
            updates: RefCell::new(None),
            operation: RefCell::new(None),
            source_cancel: RefCell::new(None),
            palette: RefCell::new(None),
            trace: probe::Trace::new("document"),
        });
        ui.global::<EditorState>().on_action({
            let owner = Rc::downgrade(&owner);
            move |code| {
                if let Some(owner) = owner.upgrade() {
                    owner.action(code);
                }
            }
        });
        ui.global::<EditorState>().on_choose({
            let owner = Rc::downgrade(&owner);
            move |id| {
                if let Some(owner) = owner.upgrade() {
                    owner.choose(if id.is_empty() { None } else { Some(id.into()) });
                }
            }
        });
        ui.global::<EditorState>().on_delete({
            let owner = Rc::downgrade(&owner);
            move |id| {
                if let Some(owner) = owner.upgrade() {
                    owner.command(Command::Delete(id.into()));
                }
            }
        });
        ui.global::<EditorState>().on_insert({
            let owner = Rc::downgrade(&owner);
            move |text| owner.upgrade().is_some_and(|o| o.insert(&text))
        });
        ui.global::<EditorState>().on_title_edit({
            let owner = Rc::downgrade(&owner);
            move |text| {
                if let Some(owner) = owner.upgrade() {
                    owner.edit(|d| d.edit_title(d.version(), text.into()).map(|_| ()));
                }
            }
        });
        ui.global::<EditorState>().on_mode_edit({
            let owner = Rc::downgrade(&owner);
            move |mode| {
                if let Some(owner) = owner.upgrade() {
                    owner.mode(mode);
                }
            }
        });
        ui.global::<EditorState>().on_palette_request({
            let owner = Rc::downgrade(&owner);
            move |x, y| {
                if let Some(owner) = owner.upgrade() {
                    owner.open_palette(x, y);
                }
            }
        });
        ui.global::<EditorState>().on_palette_dismiss({
            let owner = Rc::downgrade(&owner);
            move |restore| {
                if let Some(owner) = owner.upgrade() {
                    owner.dismiss_palette(restore);
                }
            }
        });
        ui.global::<EditorState>().on_palette_choose({
            let owner = Rc::downgrade(&owner);
            move |index| {
                if let Some(owner) = owner.upgrade() {
                    owner.choose_color(index);
                }
            }
        });
        ui.global::<EditorState>().on_selection_edit({
            let owner = Rc::downgrade(&owner);
            move |mode| {
                if let Some(owner) = owner.upgrade() {
                    owner.selection_mode(mode);
                }
            }
        });
        ui.global::<EditorState>().on_layout({
            let owner = Rc::downgrade(&owner);
            move || {
                if let Some(owner) = owner.upgrade() {
                    owner.dismiss_palette(false);
                    owner.paint();
                }
            }
        });
        ui.global::<EditorState>().on_scroll({
            let owner = Rc::downgrade(&owner);
            move |rows, cols| {
                if let Some(owner) = owner.upgrade() {
                    owner.dismiss_palette(false);
                    if let Some(d) = owner.document.borrow().as_ref() {
                        owner.viewport.borrow_mut().scroll(d, rows, cols);
                    }
                    owner.paint();
                }
            }
        });
        ui.global::<EditorState>().on_pointer({
            let owner = Rc::downgrade(&owner);
            move |kind, x, y, extend, rectangle| {
                if let Some(owner) = owner.upgrade() {
                    owner.pointer(kind, x, y, extend, rectangle);
                }
            }
        });
        ui.global::<EditorState>().on_focus_changed({
            let owner = Rc::downgrade(&owner);
            move || {
                if let Some(owner) = owner.upgrade()
                    && let Some(ui) = owner.ui.upgrade()
                {
                    ui.invoke_keyboard_updated();
                    ui.invoke_keep_alive_changed();
                }
            }
        });
        ui.on_editor_key({
            let owner = Rc::downgrade(&owner);
            move |code, extend| {
                if let Some(owner) = owner.upgrade() {
                    owner.key(code, extend);
                }
            }
        });
        ui.on_editor_edit_key({
            let owner = Rc::downgrade(&owner);
            move |code| {
                if let Some(owner) = owner.upgrade() {
                    owner.clipboard_key(code);
                }
            }
        });
        ui.on_editor_composition({
            let owner = Rc::downgrade(&owner);
            move |active| {
                if let Some(owner) = owner.upgrade()
                    && let Some(ui) = owner.ui.upgrade()
                {
                    if active {
                        owner.dismiss_palette(false);
                        if let Some(d) = owner.document.borrow().as_ref()
                            && let Err(error) = owner.viewport.borrow_mut().reveal(d)
                        {
                            owner.notice(error);
                        }
                        owner.paint();
                    }
                    ui.global::<EditorState>().set_composing(active);
                    if !active {
                        owner.schedule();
                    }
                    ui.invoke_keep_alive_changed();
                }
            }
        });
        let mut replies = owner.storage.borrow().replies.clone();
        owner.replies.replace(Some(
            slint::spawn_local({
                let owner = Rc::downgrade(&owner);
                async move {
                    while replies.changed().await.is_ok() {
                        let reply = replies.borrow_and_update().clone();
                        let Some(owner) = owner.upgrade() else { break };
                        if let Some(reply) = reply {
                            owner.reply(&reply);
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?,
        ));
        let mut updates = owner.connection.updates.clone();
        owner.updates.replace(Some(
            slint::spawn_local({
                let owner = Rc::downgrade(&owner);
                async move {
                    loop {
                        updates.borrow_and_update();
                        let Some(owner) = owner.upgrade() else { break };
                        owner.availability();
                        drop(owner);
                        if updates.changed().await.is_err() {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                }
            })
            .map_err(|e| e.to_string())?,
        ));
        let state = ui.global::<EditorState>();
        state.set_palette_labels(crate::palette::labels());
        state.set_foreground_label(pttstella::msg!("editor-color-foreground").into());
        state.set_background_label(pttstella::msg!("editor-color-background").into());
        state.set_selection_labels(ModelRc::new(VecModel::from(vec![
            pttstella::msg!("draft-select-text").into(),
            pttstella::msg!("draft-select-line").into(),
            pttstella::msg!("draft-select-rectangle").into(),
        ])));
        owner.availability();
        Ok(owner)
    }
    fn notice(&self, text: impl Into<slint::SharedString>) {
        if let Some(ui) = self.ui.upgrade() {
            ui.global::<EditorState>().set_status(text.into());
        }
    }
    fn command(&self, command: Command) -> bool {
        match self.storage.borrow().commands.try_send(command) {
            Ok(()) => {
                if let Some(ui) = self.ui.upgrade() {
                    ui.global::<EditorState>().set_busy(true);
                }
                true
            }
            Err(_) => {
                self.notice("草稿工作忙碌，請稍後再試");
                false
            }
        }
    }
    fn shown(&self) -> bool {
        let view = self.connection.updates.borrow();
        !view.input_pending
            && !view.output_pending
            && self.presented.get()
                == Some(FrameId {
                    generation: view.generation,
                    revision: view.frame.revision,
                })
    }
    fn availability(&self) {
        if let Some(ui) = self.ui.upgrade() {
            let state = ui.global::<EditorState>();
            state.set_available(
                self.connection.updates.borrow().draft_post_scope.is_some()
                    || pttstella::drafts::scope(&self.connection.updates.borrow()).is_some()
                    || self.scope.borrow().is_some(),
            );
            let valid_template = self.template.get()
                && self
                    .connection
                    .updates
                    .borrow()
                    .draft_post_scope
                    .as_ref()
                    .is_some_and(|s| {
                        self.scope
                            .borrow()
                            .as_ref()
                            .is_some_and(|current| s.same(current))
                    });
            state.set_can_transfer(
                valid_template
                    && self
                        .document
                        .borrow()
                        .as_ref()
                        .is_some_and(|d| self.saved.get() == Some(d.version())),
            );
        }
    }
    fn open(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        if ui.get_editor_open()
            || ui.get_native_composing()
            || ui.get_find_open()
            || ui.get_media_open()
            || ui.get_paste_open()
            || ui.get_login_open()
            || ui.get_tools_busy()
            || ui.get_context_open()
            || ui.get_output_busy()
            || ui.get_switching_station()
        {
            return;
        }
        ui.invoke_clear_selection();
        ui.invoke_station_mouse_cancel();
        let view = self.connection.updates.borrow().clone();
        let state = ui.global::<EditorState>();
        self.dismiss_palette(false);
        state.set_drafts(ModelRc::default());
        state.set_position("".into());
        state.set_open(true);
        state.set_selected(false);
        state.set_can_create(false);
        state.set_transfer_confirm(false);
        state.set_status("讀取草稿…".into());
        // This is an explicit Open action, including accessibility activation
        // from a background window. Match settings: activate the host as well
        // as its widget, while startup itself continues without taking focus.
        ui.window()
            .with_winit_window(|window| window.focus_window());
        ui.invoke_focus_editor_dialog();
        if view.draft_post_scope.is_some() && pttstella::navigation::article_editor(&view.frame) {
            if !self.shown() {
                self.notice(pttstella::msg!("draft-transfer-stopped"));
                return;
            }
            let reply = match self.connection.request_draft_source(view.stamp(), None) {
                Ok(reply) => reply,
                Err(e) => {
                    self.notice(e);
                    return;
                }
            };
            self.template.set(false);
            let connection = self.connection.clone();
            self.start_source(async move {
                reply.wait().await?;
                let template = connection
                    .updates
                    .borrow()
                    .draft_source
                    .as_deref()
                    .cloned()
                    .ok_or_else(|| pttstella::msg!("draft-template-unavailable"))?;
                Ok(SourceResult::Captured(template))
            });
        } else {
            self.template.set(false);
            let scope = pttstella::drafts::scope(&view).or_else(|| {
                (view.phase != pttstella::connection::Phase::Connected)
                    .then(|| self.scope.borrow().clone())
                    .flatten()
            });
            match scope {
                Some(scope) => {
                    self.command(Command::Open(scope, None));
                }
                None => self.notice(pttstella::msg!("draft-unavailable")),
            }
        }
    }
    fn choose(&self, id: Option<String>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<EditorState>();
        if self.document.borrow().is_some()
            || !state.get_open()
            || state.get_busy()
            || id.is_none() && !state.get_can_create()
        {
            return;
        }
        self.command(Command::Choose(id));
    }
    fn reply(self: &Rc<Self>, reply: &Reply) {
        self.trace.record("reply", || serde_json::json!({"state":match reply {Reply::Saved{..}=>"saved",Reply::Error{..}=>"error",_=>"other"}}));
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<EditorState>();
        match reply {
            Reply::List {
                scope,
                entries,
                can_create,
                ..
            } => {
                self.scope.replace(Some(scope.clone()));
                state.set_scope(format!("{} / {}", scope.account, scope.board.as_str()).into());
                state.set_drafts(ModelRc::new(VecModel::from(
                    entries
                        .iter()
                        .map(|e| DraftItem {
                            id: e.id.clone().into(),
                            title: if e.title.is_empty() {
                                "（無標題）".into()
                            } else {
                                e.title.clone().into()
                            },
                            conflict: e.conflict,
                        })
                        .collect::<Vec<_>>(),
                )));
                state.set_can_create(*can_create);
                state.set_busy(false);
                self.notice("選擇草稿，或從完整站端範本建立");
            }
            Reply::Selected { token, entry } => {
                match Document::new(*token, entry.title.clone(), entry.body.clone()) {
                    Ok(document) => {
                        self.saved
                            .set((entry.revision > 0).then_some(document.version()));
                        self.offered.set(None);
                        self.sequence.set(0);
                        self.closing.set(false);
                        self.document.replace(Some(document));
                        self.viewport.replace(Viewport::default());
                        state.set_selection_mode(0);
                        state.set_selected(true);
                        state.set_busy(false);
                        state.set_status("本機編輯，不會自動送到站端".into());
                        self.paint();
                        ui.invoke_focus_editor();
                        if entry.revision == 0 {
                            self.changed();
                        }
                    }
                    Err(e) => {
                        state.set_busy(false);
                        self.notice(e);
                    }
                }
            }
            Reply::Saved {
                token,
                sequence,
                conflicted,
            } => {
                if let Some((offered, version)) = self.offered.get()
                    && offered == *sequence
                    && version.token == *token
                {
                    self.saved.set(Some(version));
                    let current = self
                        .document
                        .borrow()
                        .as_ref()
                        .is_some_and(|d| d.version() == version);
                    if current {
                        self.edited_at.set(None);
                        self.notice(if *conflicted {
                            "已保存為衝突副本"
                        } else {
                            "已保存"
                        });
                    }
                    if current && self.closing.get() {
                        self.hide();
                    }
                }
            }
            Reply::Error { message } => {
                state.set_busy(false);
                self.closing.set(false);
                self.notice(message.clone());
            }
            Reply::Deleted { id } => {
                let items = state.get_drafts();
                let retained = (0..items.row_count())
                    .filter_map(|i| items.row_data(i))
                    .filter(|e| e.id.as_str() != id)
                    .collect::<Vec<_>>();
                state.set_drafts(ModelRc::new(VecModel::from(retained)));
                state.set_busy(false);
                self.notice("已刪除草稿");
            }
            _ => {}
        }
        self.availability();
    }
    fn edit(self: &Rc<Self>, change: impl FnOnce(&mut Document) -> Result<(), String>) -> bool {
        let Some(ui) = self.ui.upgrade() else {
            return false;
        };
        let state = ui.global::<EditorState>();
        if !state.get_selected() || state.get_busy() || state.get_transfer_confirm() {
            return false;
        }
        let result = {
            let mut document = self.document.borrow_mut();
            let Some(d) = document.as_mut() else {
                return false;
            };
            let before = d.version();
            let before_bytes = d.source().len();
            let result = change(d).map(|()| before != d.version());
            self.trace.record("edit",|| serde_json::json!({"before_bytes":before_bytes,"after_bytes":d.source().len(),"changed":result.as_ref().ok()}));
            result
        };
        match result {
            Ok(changed) => {
                if changed {
                    self.viewport.borrow_mut().reset_motion();
                    if let Some(d) = self.document.borrow().as_ref()
                        && let Err(e) = self.viewport.borrow_mut().reveal(d)
                    {
                        self.notice(e);
                    }
                    self.changed();
                }
                self.paint();
                true
            }
            Err(e) => {
                self.notice(e);
                self.paint();
                false
            }
        }
    }
    fn insert(self: &Rc<Self>, text: &str) -> bool {
        self.trace
            .record("insert", || serde_json::json!({"bytes":text.len()}));
        if text.is_empty() {
            return true;
        }
        self.edit(|d| self.viewport.borrow_mut().replace(d, text))
    }
    fn changed(self: &Rc<Self>) {
        self.dismiss_palette(false);
        self.edited_at.set(Some(Instant::now()));
        self.notice("尚未保存");
        if let Some(ui) = self.ui.upgrade() {
            ui.global::<EditorState>().set_transfer_confirm(false);
        }
        self.availability();
        self.schedule();
    }
    fn schedule(self: &Rc<Self>) {
        self.debounce.stop();
        let Some(at) = self.edited_at.get() else {
            return;
        };
        let Some(ui) = self.ui.upgrade() else { return };
        if ui.get_editor_composing() {
            return;
        }
        let now = Instant::now();
        let deadline = (at + Duration::from_secs(2)).max(
            self.offered_at
                .get()
                .map_or(at, |last| last + Duration::from_secs(5)),
        );
        let owner = Rc::downgrade(self);
        self.debounce.start(
            slint::TimerMode::SingleShot,
            deadline.saturating_duration_since(now),
            move || {
                if let Some(owner) = owner.upgrade() {
                    owner.save(false);
                }
            },
        );
    }
    fn snapshot(&self) -> Option<Edit> {
        let d = self.document.borrow();
        let d = d.as_ref()?;
        if self.saved.get() == Some(d.version()) {
            return None;
        }
        let sequence = self.sequence.get().checked_add(1)?;
        self.sequence.set(sequence);
        self.offered.set(Some((sequence, d.version())));
        Some(Edit {
            token: d.version().token,
            sequence,
            title: d.title().into(),
            body: d.source().into(),
        })
    }
    fn save(&self, explicit: bool) {
        let Some(ui) = self.ui.upgrade() else { return };
        if ui.get_editor_composing() {
            return;
        }
        if let Some(edit) = self.snapshot() {
            self.debounce.stop();
            self.offered_at.set(Some(Instant::now()));
            if self.storage.borrow().edits.send(Some(edit)).is_err() {
                self.notice("草稿保存工作已停止");
                self.closing.set(false);
            } else {
                self.notice("保存中…");
            }
        } else if explicit {
            self.notice("已保存");
        }
    }
    fn mode(&self, mode: i32) {
        self.dismiss_palette(false);
        if let Some(ui) = self.ui.upgrade()
            && !ui.get_editor_composing()
            && let Some(d) = self.document.borrow_mut().as_mut()
        {
            d.set_mode(match mode {
                1 => Mode::Plain,
                2 => Mode::Source,
                _ => Mode::Color,
            });
            self.viewport.borrow_mut().reset_motion();
        }
        self.paint();
    }
    fn selection_mode(&self, mode: i32) {
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<EditorState>();
        if state.get_busy() || state.get_composing() {
            return;
        }
        let mode = match mode {
            0 => SelectionMode::Text,
            1 => SelectionMode::Line,
            2 => SelectionMode::Rectangle,
            _ => return,
        };
        self.dismiss_palette(false);
        let result = {
            let mut document = self.document.borrow_mut();
            document
                .as_mut()
                .map(|d| self.viewport.borrow_mut().set_selection_mode(d, mode))
        };
        if let Some(Err(e)) = result {
            self.notice(e);
        }
        state.set_selection_mode(match mode {
            SelectionMode::Text => 0,
            SelectionMode::Line => 1,
            SelectionMode::Rectangle => 2,
        });
        self.paint();
    }
    fn dismiss_palette(&self, restore: bool) {
        self.palette.take();
        if let Some(ui) = self.ui.upgrade() {
            let state = ui.global::<EditorState>();
            let open = state.get_palette_open();
            state.set_palette_open(false);
            if open && restore {
                ui.invoke_focus_editor();
            }
        }
    }
    fn open_palette(&self, x: f32, y: f32) {
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<EditorState>();
        if !state.get_selected()
            || state.get_busy()
            || state.get_composing()
            || state.get_transfer_confirm()
            || !x.is_finite()
            || !y.is_finite()
        {
            return;
        }
        let lease = {
            let document = self.document.borrow();
            let Some(d) = document.as_ref() else { return };
            palette::Lease::new(d, &self.viewport.borrow())
        };
        match lease {
            Ok(lease) => {
                self.viewport.borrow_mut().stop_gesture();
                self.palette.replace(Some(lease));
                let bounds = ui.get_editor_grid_bounds();
                state.set_palette_x(if x < 0. {
                    bounds.x + state.get_caret_column() * bounds.width / COLUMNS as f32
                } else {
                    x
                });
                state.set_palette_y(if y < 0. {
                    bounds.y + (state.get_caret_row() + 1.) * bounds.height / ROWS as f32
                } else {
                    y
                });
                state.set_palette_selected(state.get_foreground());
                state.set_palette_open(true);
                ui.invoke_focus_editor_palette();
            }
            Err(e) => self.notice(e),
        }
    }
    fn choose_color(self: &Rc<Self>, index: i32) {
        if !(0..24).contains(&index) {
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<EditorState>();
        if !state.get_palette_open() || state.get_busy() || state.get_composing() {
            return;
        }
        let Some(lease) = self.palette.take() else {
            return;
        };
        let color = Color {
            foreground: if index < 16 {
                index as u8
            } else {
                state.get_foreground() as u8
            },
            background: if index >= 16 {
                (index - 16) as u8
            } else {
                state.get_background() as u8
            },
        };
        let accepted = self.edit(|d| lease.apply(d, &self.viewport.borrow(), color));
        if accepted {
            self.paint();
            state.set_foreground(color.foreground.into());
            state.set_background(color.background.into());
            self.dismiss_palette(false);
            ui.invoke_focus_editor();
        } else {
            self.palette.replace(Some(lease));
        }
    }
    pub fn key(self: &Rc<Self>, key: i32, extend: bool) {
        self.trace
            .record("key", || serde_json::json!({"code":key,"extend":extend}));
        if let Some(ui) = self.ui.upgrade()
            && (ui.get_editor_composing() || ui.global::<EditorState>().get_busy())
        {
            return;
        }
        match key {
            0..=9 => {
                let result = {
                    let mut d = self.document.borrow_mut();
                    d.as_mut()
                        .map(|d| self.viewport.borrow_mut().navigate(d, key, extend))
                };
                if let Some(Err(e)) = result {
                    self.notice(e);
                }
                self.paint();
            }
            10 | 11 => {
                self.edit(|d| self.viewport.borrow_mut().delete(d, key == 10));
            }
            12 => {
                self.insert("\n");
            }
            13 => {
                self.insert("\t");
            }
            _ => {}
        }
    }
    fn clipboard_key(self: &Rc<Self>, code: i32) {
        self.trace
            .record("clipboard", || serde_json::json!({"code":code}));
        if let Some(ui) = self.ui.upgrade()
            && (ui.get_editor_composing() || ui.global::<EditorState>().get_busy())
        {
            return;
        }
        match code {
            0 => {
                if let Some(d) = self.document.borrow_mut().as_mut()
                    && let Err(e) = self.viewport.borrow_mut().select_all(d)
                {
                    self.notice(e);
                }
                self.paint();
            }
            1 | 2 => {
                let result = (|| {
                    let document = self.document.borrow();
                    let d = document.as_ref().ok_or("沒有開啟草稿")?;
                    let ranges = self.viewport.borrow().ranges(d)?;
                    let text = ranges
                        .iter()
                        .map(|r| &d.source()[r.clone()])
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.clipboard.write_text(text, || Ok(()))
                })();
                match result {
                    Ok(()) => {
                        if code == 2 {
                            self.edit(|d| self.viewport.borrow_mut().replace(d, ""));
                        }
                    }
                    Err(e) => self.notice(e),
                }
            }
            3 => match self.clipboard.read_text() {
                Ok(text) => {
                    // CRLF normalization can at most halve the raw byte count.
                    // Reject before allocating normalized clipboard copies.
                    if text.len() > 2 * pttstella::drafts::MAX_BODY {
                        self.notice(pttstella::msg!("draft-limit"));
                    } else {
                        self.insert(&text.replace("\r\n", "\n").replace('\r', "\n"));
                    }
                }
                Err(e) => self.notice(e),
            },
            4 => {
                self.edit(|d| d.undo(d.version()).map(|_| ()));
            }
            5 => {
                self.edit(|d| d.redo(d.version()).map(|_| ()));
            }
            _ => {}
        }
    }
    fn pointer(&self, kind: i32, x: f32, y: f32, extend: bool, rectangular: bool) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<EditorState>();
        let (w, h) = (state.get_view_width(), state.get_view_height());
        if w <= 0. || h <= 0. {
            return;
        }
        let (col, row) = (
            (x.max(0.) / w * COLUMNS as f32).floor() as usize,
            (y.max(0.) / h * ROWS as f32).floor() as usize,
        );
        let result = {
            let mut d = self.document.borrow_mut();
            d.as_mut().map(|d| {
                self.viewport.borrow_mut().pointer(
                    d,
                    kind,
                    row.min(ROWS - 1),
                    col.min(COLUMNS),
                    extend,
                    rectangular,
                )
            })
        };
        match result {
            Some(Ok(true)) => {
                self.dismiss_palette(false);
                self.paint();
            }
            Some(Err(e)) => self.notice(e),
            _ => {}
        }
    }
    pub fn copy_display(&self, source: &Renderer) {
        self.renderer.borrow_mut().copy_display(source);
        self.painted.set(None);
        self.paint();
    }

    fn paint(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<EditorState>();
        let document = self.document.borrow();
        let Some(d) = document.as_ref() else { return };
        state.set_title(d.title().into());
        state.set_mode(match d.mode() {
            Mode::Color => 0,
            Mode::Plain => 1,
            Mode::Source => 2,
        });
        state.set_can_undo(d.can_undo());
        state.set_can_redo(d.can_redo());
        let width = state.get_view_width();
        let height = state.get_view_height();
        let dpi = ui.window().scale_factor();
        if !width.is_finite() || !height.is_finite() || width <= 0. || height <= 0. {
            return;
        }
        let size = Size {
            width: (width * dpi).clamp(1., 8192.) as u32,
            height: (height * dpi).clamp(1., 8192.) as u32,
        };
        if u64::from(size.width) * u64::from(size.height) > 16 * 1024 * 1024 {
            self.notice("編輯區尺寸超過繪圖上限");
            return;
        }
        let view = self.viewport.borrow();
        let stamp = PaintStamp {
            version: d.version(),
            mode: d.mode(),
            top: view.top,
            left: view.left,
            size,
        };
        if self.painted.get() != Some(stamp) {
            let mut renderer = self.renderer.borrow_mut();
            renderer.set_frame(view.frame(d));
            match renderer.paint_viewport(size, false, None, view.left) {
                Ok(pixels) => {
                    state.set_pixels(Image::from_rgba8_premultiplied(pixels));
                    self.painted.set(Some(stamp));
                }
                Err(e) => self.notice(e),
            }
        }
        if let Ok(p) = d.position(d.selection().head) {
            state.set_caret_column(p.col.saturating_sub(view.left) as f32);
            state.set_caret_row(p.row.saturating_sub(view.top) as f32);
            state.set_caret_visible(
                p.col >= view.left
                    && p.col < view.left + COLUMNS
                    && p.row >= view.top
                    && p.row < view.top + ROWS,
            );
            state.set_position(
                format!("{}:{} · {} 行", p.row + 1, p.col + 1, d.rows().len()).into(),
            );
        }
        match view.highlights(d) {
            Ok(spans) => state.set_spans(ModelRc::new(VecModel::from(
                spans
                    .into_iter()
                    .map(|(row, col, span)| SelectionRect {
                        x: width * col as f32 / COLUMNS as f32,
                        y: height * row as f32 / ROWS as f32,
                        width: width * span as f32 / COLUMNS as f32,
                        height: height / ROWS as f32,
                    })
                    .collect::<Vec<_>>(),
            ))),
            Err(e) => self.notice(e),
        }
    }
    fn hide(&self) {
        self.dismiss_palette(false);
        self.debounce.stop();
        self.document.replace(None);
        self.painted.set(None);
        self.closing.set(false);
        self.viewport.borrow_mut().stop_gesture();
        if let Some(ui) = self.ui.upgrade() {
            let state = ui.global::<EditorState>();
            state.set_open(false);
            state.set_selected(false);
            state.set_pixels(Image::default());
            state.set_spans(ModelRc::default());
            state.set_drafts(ModelRc::default());
            let mut frame = self.renderer.borrow().frame().clone();
            frame.glyphs.clear();
            self.renderer.borrow_mut().set_frame(frame);
            state.set_body_focused(false);
            state.set_title_focused(false);
            state.set_composing(false);
            ui.invoke_clear_editor_input();
            ui.invoke_focus_terminal();
        }
    }
    fn action(self: &Rc<Self>, code: i32) {
        self.trace
            .record("action", || serde_json::json!({"code":code}));
        match code {
            0 => self.open(),
            1 => self.save(true),
            2 => {
                if let Some(ui) = self.ui.upgrade()
                    && ui.global::<EditorState>().get_can_transfer()
                {
                    ui.global::<EditorState>().set_transfer_confirm(true);
                }
            }
            3 => {
                if self
                    .ui
                    .upgrade()
                    .is_some_and(|ui| ui.global::<EditorState>().get_palette_open())
                {
                    self.dismiss_palette(true);
                    return;
                }
                if let Some(ui) = self.ui.upgrade()
                    && !ui.global::<EditorState>().get_busy()
                    && !ui.get_editor_composing()
                {
                    let dirty = self
                        .document
                        .borrow()
                        .as_ref()
                        .is_some_and(|d| self.saved.get() != Some(d.version()));
                    if dirty {
                        self.closing.set(true);
                        self.save(true);
                    } else {
                        self.hide();
                    }
                }
            }
            4 | 5 => self.clipboard_key(code),
            6 => self.transfer(),
            7 => {
                if let Some(ui) = self.ui.upgrade() {
                    ui.global::<EditorState>().set_transfer_confirm(false);
                }
            }
            8 => {
                self.dismiss_palette(false);
                self.viewport.borrow_mut().stop_gesture();
                self.save(false);
            }
            9 => self.cancel_source(),
            10 => self.open_palette(-1., -1.),
            _ => {}
        }
    }
    fn transfer(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<EditorState>();
        if !state.get_transfer_confirm()
            || !state.get_can_transfer()
            || state.get_busy()
            || ui.get_editor_composing()
            || !self.shown()
        {
            self.notice(pttstella::msg!("draft-transfer-stopped"));
            return;
        }
        let Some(token) = self.document.borrow().as_ref().map(|d| d.version().token) else {
            return;
        };
        let (answer, wait) = tokio::sync::oneshot::channel();
        if !self.command(Command::Replacement(token, answer)) {
            return;
        }
        let stamp = self.connection.updates.borrow().stamp();
        let connection = self.connection.clone();
        let presented = self.presented.clone();
        self.start_source(async move {
            let replacement = wait
                .await
                .map_err(|_| pttstella::msg!("draft-transfer-stopped"))
                .and_then(|r| r)?;
            {
                let view = connection.updates.borrow();
                if view.input_pending
                    || view.output_pending
                    || view.stamp() != stamp
                    || presented.get()
                        != Some(FrameId {
                            generation: view.generation,
                            revision: view.frame.revision,
                        })
                {
                    return Err(pttstella::msg!("draft-transfer-stopped"));
                }
            }
            connection
                .request_draft_source(stamp, Some(replacement))?
                .wait()
                .await?;
            Ok(SourceResult::Transferred)
        });
    }
    fn cancel_source(&self) {
        if let Some(cancel) = self.source_cancel.take() {
            if let Some(ui) = self.ui.upgrade() {
                ui.global::<EditorState>().set_source_cancelling(true);
            }
            let _ = cancel.send(());
        }
    }
    fn start_source(
        self: &Rc<Self>,
        work: impl std::future::Future<Output = Result<SourceResult, String>> + 'static,
    ) {
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<EditorState>();
        self.dismiss_palette(false);
        state.set_busy(true);
        state.set_source_busy(true);
        state.set_source_cancelling(false);
        let (cancel, mut cancelled) = tokio::sync::oneshot::channel();
        self.source_cancel.replace(Some(cancel));
        let owner = Rc::downgrade(self);
        match slint::spawn_local(async move {
            let result = tokio::select! {biased;_= &mut cancelled=>None,result=work=>Some(result)};
            let Some(owner) = owner.upgrade() else { return };
            owner.source_cancel.take();
            owner.operation.take();
            if owner.closed.get() {
                return;
            }
            let Some(ui) = owner.ui.upgrade() else { return };
            let state = ui.global::<EditorState>();
            state.set_source_busy(false);
            state.set_source_cancelling(false);
            state.set_busy(false);
            state.set_transfer_confirm(false);
            match result {
                Some(Ok(SourceResult::Captured(template))) => {
                    owner.template.set(true);
                    owner.command(Command::Open(template.scope.clone(), Some(template)));
                }
                Some(Ok(SourceResult::Transferred)) => {
                    owner.hide();
                    ui.set_result(pttstella::msg!("draft-transferred").into());
                }
                Some(Err(error)) => owner.notice(error),
                None => owner.notice(pttstella::msg!("draft-stop-requested")),
            }
        }) {
            Ok(task) => {
                self.operation.replace(Some(task));
            }
            Err(error) => {
                self.source_cancel.take();
                state.set_source_busy(false);
                state.set_busy(false);
                self.notice(error.to_string());
            }
        }
    }
    pub fn finish(&self) -> Result<(), String> {
        if self.closed.replace(true) {
            return Ok(());
        }
        self.debounce.stop();
        self.cancel_source();
        for job in [&self.replies, &self.updates, &self.operation] {
            if let Some(job) = job.take() {
                job.abort();
            }
        }
        let final_edit = self.snapshot();
        self.storage.borrow_mut().finish(final_edit)
    }
}
