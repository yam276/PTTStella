//! Bookmark form, explicit online acquisition and local archive reader.
//! Offline rendering has no connection, media downloader or keyboard route.
mod capture;
pub mod capture_probe;
pub mod probe;
use crate::{
    TerminalWindow,
    article::session::ArticleSession,
    file_job::ResultSlot,
    output::Output,
    renderer::{Renderer, Size},
};
use pttstella::{
    article_target::ArticleTarget,
    bookmarks::{
        Bookmarks,
        session::{LocalArticle, State, Summary},
        store::Store,
    },
    msg,
};
use slint::{ComponentHandle, Image, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crate::BookmarksState;
enum Value {
    Loaded(State),
    Saved(Bookmarks),
    Read(LocalArticle),
}
#[derive(Clone, Copy, PartialEq)]
enum Operation {
    Load,
    Save,
    Read,
}
struct Flight {
    slot: ResultSlot<Value>,
    operation: Operation,
    epoch: u64,
}
type Work = Box<dyn FnOnce() -> Result<Value, String> + Send>;
struct Offline {
    article: LocalArticle,
    top: usize,
    renderer: Option<Renderer>,
    painted: Option<(usize, Size)>,
}
pub struct Manager {
    ui: slint::Weak<TerminalWindow>,
    output: Rc<Output>,
    article: Rc<ArticleSession>,
    connection: Rc<pttstella::connection::Connection>,
    capture: RefCell<Option<capture::Capture>>,
    capture_timer: slint::Timer,
    capture_serial: Cell<u64>,
    capture_observing: Cell<bool>,
    capture_trace_left: Cell<u8>,
    store: Result<Store, String>,
    state: RefCell<Option<State>>,
    window: RefCell<Option<TerminalWindow>>,
    rows: RefCell<Vec<Summary>>,
    selected: RefCell<Option<ArticleTarget>>,
    fields: RefCell<[String; 2]>,
    offline: RefCell<Option<Offline>>,
    result: RefCell<Option<Flight>>,
    epoch: Cell<u64>,
    closing: Cell<bool>,
    painting: Cell<bool>,
    stopped: Cell<bool>,
}
fn target_text(t: &ArticleTarget) -> String {
    format!("#{}@{}", t.aid().as_str(), t.board().as_str())
}
fn same(a: &ArticleTarget, b: &ArticleTarget) -> bool {
    a.board().same(b.board()) && a.aid() == b.aid()
}
fn strings(items: impl IntoIterator<Item = String>) -> ModelRc<slint::SharedString> {
    ModelRc::new(VecModel::from(
        items.into_iter().map(Into::into).collect::<Vec<_>>(),
    ))
}
impl Manager {
    pub fn mount(
        ui: &TerminalWindow,
        output: Rc<Output>,
        article: Rc<ArticleSession>,
        connection: Rc<pttstella::connection::Connection>,
        store: Result<Store, String>,
    ) -> Rc<Self> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            output,
            article,
            connection,
            capture: RefCell::new(None),
            capture_timer: slint::Timer::default(),
            capture_serial: Cell::new(0),
            capture_observing: Cell::new(false),
            capture_trace_left: Cell::new(
                if std::env::var_os("PTTSTELLA_SLINT_CAPTURE_PROBE").is_some()
                    && std::env::var_os("PTTSTELLA_SLINT_CAPTURE_TRACE").is_some()
                {
                    64
                } else {
                    0
                },
            ),
            store,
            state: RefCell::new(None),
            window: RefCell::new(None),
            rows: RefCell::new(Vec::new()),
            selected: RefCell::new(None),
            fields: RefCell::new(Default::default()),
            offline: RefCell::new(None),
            result: RefCell::new(None),
            epoch: Cell::new(0),
            closing: Cell::new(false),
            painting: Cell::new(false),
            stopped: Cell::new(false),
        });
        ui.set_bookmarks_available(true);
        ui.on_bookmarks_show({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.open();
                }
            }
        });
        ui.on_bookmarks_updated({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.sync();
                    o.capture_tick();
                }
            }
        });
        ui.on_bookmarks_capture_updated({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.capture_tick();
                }
            }
        });
        ui.on_bookmarks_capture_current({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.capture_current();
                }
            }
        });
        ui.on_bookmarks_capture_cancel({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.cancel_capture();
                }
            }
        });
        owner
    }
    pub(crate) fn window(&self) -> Option<TerminalWindow> {
        self.window
            .borrow()
            .as_ref()
            .and_then(|w| w.as_weak().upgrade())
    }
    fn notice(&self, text: String) {
        if let Some(w) = self.window() {
            w.global::<BookmarksState>().set_message(text.into());
        }
    }
    pub fn refresh_labels(&self) {
        let Some(w) = self.window() else { return };
        let loader = pttstella::i18n::application().loader();
        w.global::<BookmarksState>().set_labels(strings(
            [
                "bookmarks-title",
                "bookmarks-description",
                "bookmarks-list",
                "bookmarks-new",
                "bookmarks-target",
                "bookmarks-label",
                "bookmarks-save",
                "bookmarks-remove",
                "bookmarks-read",
                "retry",
                "close",
                "bookmarks-archive-readonly",
                "context-previous",
                "context-next",
                "bookmarks-go",
            ]
            .map(|key| loader.get(key)),
        ));
        self.render();
        if let Err(e) = self.paint() {
            self.notice(e);
        }
    }
    fn clear_draft(&self) {
        self.selected.borrow_mut().take();
        *self.fields.borrow_mut() = Default::default();
        if let Some(w) = self.window() {
            w.global::<BookmarksState>().set_target("".into());
            w.global::<BookmarksState>().set_label("".into());
            w.global::<BookmarksState>().set_choice(0);
        }
        self.sync();
    }
    fn sync(&self) {
        if let Some(w) = self.window() {
            w.global::<BookmarksState>()
                .set_busy(self.output.busy() || self.result.borrow().is_some());
            w.global::<BookmarksState>().set_writable(
                self.state
                    .borrow()
                    .as_ref()
                    .is_some_and(|s| s.check_write().is_ok()),
            );
            w.global::<BookmarksState>()
                .set_selected(self.selected.borrow().is_some());
            w.global::<BookmarksState>().set_go_ready(
                self.ui
                    .upgrade()
                    .is_some_and(|ui| ui.get_destination_available()),
            );
            w.global::<BookmarksState>()
                .set_has_copy(self.selected.borrow().as_ref().is_some_and(|t| {
                    self.rows
                        .borrow()
                        .iter()
                        .any(|r| same(r.target(), t) && r.cached_at().is_some())
                }));
        }
    }
    fn render(&self) {
        let Some(w) = self.window() else { return };
        let snapshot = self.state.borrow().as_ref().map(State::snapshot);
        let rows = snapshot.map(|s| s.entries).unwrap_or_default();
        w.global::<BookmarksState>().set_entries(strings(
            std::iter::once(msg!("bookmarks-new")).chain(rows.iter().map(|r| {
                let copy = if r.cached_at().is_some() {
                    format!(" · {}", msg!("bookmarks-local-copy"))
                } else {
                    String::new()
                };
                format!(
                    "{}{}{}{}",
                    r.label().unwrap_or(""),
                    if r.label().is_some() { "  " } else { "" },
                    target_text(r.target()),
                    copy
                )
            })),
        ));
        let choice = self
            .selected
            .borrow()
            .as_ref()
            .and_then(|t| rows.iter().position(|r| same(r.target(), t)));
        if choice.is_none() {
            self.selected.borrow_mut().take();
        }
        w.global::<BookmarksState>()
            .set_choice(choice.map_or(0, |i| (i + 1) as i32));
        self.rows.replace(rows);
        self.sync();
    }
    fn choose(&self, index: i32) {
        if self.output.busy() {
            return;
        }
        if index == 0 {
            self.clear_draft();
            return;
        }
        let rows = self.rows.borrow();
        let Some(row) = usize::try_from(index)
            .ok()
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| rows.get(i))
        else {
            return;
        };
        self.selected.replace(Some(row.target().clone()));
        let fields = [
            target_text(row.target()),
            row.label().unwrap_or("").to_owned(),
        ];
        if let Some(w) = self.window() {
            w.global::<BookmarksState>()
                .set_target(fields[0].clone().into());
            w.global::<BookmarksState>()
                .set_label(fields[1].clone().into());
            w.global::<BookmarksState>().set_choice(index);
        }
        self.fields.replace(fields);
        drop(rows);
        self.sync();
    }
    fn open(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.stopped.get()
            || self.capture.borrow().is_some()
            || ui.get_app_closing()
            || self.output.busy()
            || ui.get_destination_open()
            || ui.get_sites_open()
            || ui.get_filters_open()
            || ui.get_settings_open()
            || ui.get_editor_open()
            || ui.get_station_color_open()
            || ui.get_media_open()
            || ui.get_login_open()
            || ui.get_paste_open()
            || ui.get_native_composing()
            || ui.get_find_composing()
        {
            return;
        }
        if self.window.borrow().is_none() {
            match self.create_window(&ui) {
                Ok(w) => {
                    self.window.replace(Some(w));
                }
                Err(e) => {
                    crate::notice::error(&ui, e);
                    return;
                }
            }
        }
        if !ui.get_bookmarks_open() {
            self.clear_draft();
        }
        self.closing.set(false);
        ui.invoke_tools_dismiss(false);
        ui.invoke_context_dismiss(false);
        ui.invoke_reset_keyboard();
        ui.set_bookmarks_open(true);
        self.refresh_labels();
        if let Some(state) = self.state.borrow().as_ref() {
            self.notice(
                state
                    .snapshot()
                    .warning
                    .unwrap_or_else(|| msg!("bookmarks-hint")),
            );
        }
        if let Some(w) = self.window() {
            w.global::<BookmarksState>().invoke_focus_target();
        }
        if self.state.borrow().is_none() {
            self.reload();
        }
    }
    fn create_window(self: &Rc<Self>, _ui: &TerminalWindow) -> Result<TerminalWindow, String> {
        let w = self.ui.upgrade().ok_or("terminal window closed")?;
        w.global::<BookmarksState>().on_choose({
            let o = Rc::downgrade(self);
            move |i| {
                if let Some(o) = o.upgrade()
                    && o.admit()
                {
                    o.choose(i);
                }
            }
        });
        w.global::<BookmarksState>().on_new_entry({
            let o = Rc::downgrade(self);
            move || {
                if let Some(o) = o.upgrade()
                    && o.admit()
                {
                    o.clear_draft();
                }
            }
        });
        w.global::<BookmarksState>().on_field_edit({
            let o = Rc::downgrade(self);
            move |field, text| {
                let Some(o) = o.upgrade() else {
                    return "".into();
                };
                let Ok(field) = usize::try_from(field) else {
                    return "".into();
                };
                if field > 1 {
                    return "".into();
                }
                if o.stopped.get()
                    || o.ui
                        .upgrade()
                        .is_none_or(|ui| !ui.get_bookmarks_open() || ui.get_app_closing())
                {
                    return o.fields.borrow()[field].clone().into();
                }
                let text = text.to_string();
                let limit = if field == 0 { 256 } else { 512 };
                if o.output.busy()
                    || text.len() > limit
                    || text.chars().any(char::is_control)
                    || field == 1 && text.chars().count() > 128
                {
                    o.notice(if field == 0 {
                        msg!("destination-input-invalid")
                    } else {
                        msg!("bookmarks-label-invalid")
                    });
                    return o.fields.borrow()[field].clone().into();
                }
                o.fields.borrow_mut()[field] = text.clone();
                text.into()
            }
        });
        w.global::<BookmarksState>().on_action({
            let o = Rc::downgrade(self);
            move |code| {
                if let Some(o) = o.upgrade()
                    && o.admit()
                {
                    o.action(code);
                }
            }
        });
        w.global::<BookmarksState>().on_close_bookmarks({
            let o = Rc::downgrade(self);
            move || {
                if let Some(o) = o.upgrade() {
                    o.close();
                }
            }
        });
        w.global::<BookmarksState>().on_page_key({
            let o = Rc::downgrade(self);
            move |code| {
                if let Some(o) = o.upgrade() {
                    o.page(code);
                }
            }
        });
        w.global::<BookmarksState>().on_reader_resized({
            let o = Rc::downgrade(self);
            move || {
                if let Some(o) = o.upgrade()
                    && let Err(e) = o.paint()
                {
                    o.notice(e);
                }
            }
        });
        Ok(w)
    }
    fn admit(&self) -> bool {
        !self.stopped.get()
            && !self.output.busy()
            && self.result.borrow().is_none()
            && self
                .ui
                .upgrade()
                .is_some_and(|ui| ui.get_bookmarks_open() && !ui.get_app_closing())
            && self.window().is_some_and(|w| {
                !w.global::<BookmarksState>().get_composing()
                    && !w.global::<BookmarksState>().get_offline()
            })
    }
    fn reload(self: &Rc<Self>) {
        let store = self.store.clone();
        self.start(
            Box::new(move || Ok(Value::Loaded(State::with_store(store)))),
            Operation::Load,
        );
    }
    fn action(self: &Rc<Self>, code: i32) {
        if code == 4 {
            if let Some(ui) = self.ui.upgrade()
                && ui.get_destination_available()
            {
                let target = self.selected.borrow().as_ref().map(target_text);
                if let Some(target) = target {
                    self.hide();
                    ui.invoke_destination_fill(target.into());
                }
            }
            return;
        }
        if code == 3 {
            self.reload();
            return;
        }
        let work = (|| {
            let state = self.state.borrow();
            let state = state.as_ref().ok_or_else(|| msg!("bookmarks-loading"))?;
            if code == 2 {
                let target = self
                    .selected
                    .borrow()
                    .as_ref()
                    .map(target_text)
                    .ok_or_else(|| msg!("bookmarks-not-found"))?;
                let settings = self.article.settings();
                let read = state.read(&target, self.article.rules(), settings)?;
                return Ok((
                    Box::new(move || Ok(Value::Read(read.run()))) as Work,
                    Operation::Read,
                ));
            }
            if !matches!(code, 0 | 1) {
                return Err(msg!("bookmarks-not-found"));
            }
            let fields = self.fields.borrow();
            let target = if code == 1 {
                self.selected
                    .borrow()
                    .as_ref()
                    .map(target_text)
                    .ok_or_else(|| msg!("bookmarks-not-found"))?
            } else {
                fields[0].clone()
            };
            let write = state.edit(&target, &fields[1], code == 1)?;
            Ok((
                Box::new(move || {
                    write
                        .store
                        .save_from(&write.base, write.draft)
                        .map(Value::Saved)
                }) as Work,
                Operation::Save,
            ))
        })();
        match work {
            Ok((work, kind)) => self.start(work, kind),
            Err(e) => self.notice(e),
        }
    }
    fn start(self: &Rc<Self>, work: Work, operation: Operation) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.output.busy() || self.result.borrow().is_some() {
            return;
        }
        let slot = ResultSlot::new();
        self.result.replace(Some(Flight {
            slot: slot.clone(),
            operation,
            epoch: self.epoch.get(),
        }));
        self.notice(if operation == Operation::Save {
            msg!("saving")
        } else {
            msg!("bookmarks-loading")
        });
        let o = Rc::downgrade(self);
        self.output.work(
            &ui,
            move || slot.publish(work()),
            move |_, _, _| {
                if let Some(o) = o.upgrade() {
                    let success = o.collect().is_ok();
                    if o.closing.get() && success {
                        o.hide();
                    } else if !success {
                        o.closing.set(false);
                    }
                    o.sync();
                }
            },
        );
        if !ui.get_output_busy() {
            self.result.borrow_mut().take();
            self.notice(ui.get_input_notice().to_string());
        }
        self.sync();
    }
    fn collect(&self) -> Result<(), String> {
        let Some(flight) = self.result.borrow_mut().take() else {
            return Ok(());
        };
        let value = flight.slot.take();
        if flight.operation != Operation::Save && flight.epoch != self.epoch.get() {
            return Ok(());
        }
        let result = value.and_then(|v| match v {
            Value::Loaded(state) => {
                let warning = state.snapshot().warning;
                self.state.replace(Some(state));
                self.capture_trace("store-loaded", None);
                self.render();
                self.notice(warning.unwrap_or_else(|| msg!("bookmarks-hint")));
                self.capture_trace("store-rendered", None);
                Ok(())
            }
            Value::Saved(saved) => {
                self.state
                    .borrow_mut()
                    .as_mut()
                    .ok_or_else(|| msg!("bookmarks-loading"))?
                    .complete(Ok(saved))?;
                self.render();
                self.notice(msg!("bookmarks-saved"));
                if let Some(ui) = self.ui.upgrade() {
                    crate::notice::success(&ui, msg!("bookmarks-saved"));
                }
                Ok(())
            }
            Value::Read(article) => {
                self.offline.replace(Some(Offline {
                    article,
                    top: 0,
                    renderer: None,
                    painted: None,
                }));
                if let Some(w) = self.window() {
                    w.global::<BookmarksState>().set_offline(true);
                    self.paint()?;
                    w.global::<BookmarksState>().invoke_focus_reader();
                }
                self.notice(msg!("bookmarks-hint"));
                Ok(())
            }
        });
        if let Err(e) = &result {
            self.notice(e.clone());
            if let Some(ui) = self.ui.upgrade() {
                crate::notice::error(&ui, e.clone());
            }
        }
        self.sync();
        result
    }
    fn page(&self, code: i32) {
        if self.stopped.get()
            || self.window().is_none_or(|w| {
                !w.global::<BookmarksState>().get_offline()
                    || w.global::<BookmarksState>().get_composing()
            })
        {
            return;
        }
        if code == 6 {
            self.offline.borrow_mut().take();
            if let Some(w) = self.window() {
                w.global::<BookmarksState>().set_offline(false);
                w.global::<BookmarksState>().set_page(Image::default());
                w.global::<BookmarksState>().invoke_focus_target();
            }
            return;
        }
        if let Some(o) = self.offline.borrow_mut().as_mut() {
            let max = o.article.frame.rows.saturating_sub(23);
            o.top = match code {
                0 => o.top.saturating_sub(1),
                1 => o.top.saturating_add(1).min(max),
                2 => o.top.saturating_sub(22),
                3 => o.top.saturating_add(22).min(max),
                4 => 0,
                5 => max,
                _ => return,
            };
        }
        if let Err(e) = self.paint() {
            self.notice(e);
        }
    }
    fn paint(&self) -> Result<(), String> {
        if self.painting.replace(true) {
            return Ok(());
        }
        let result = self.paint_inner();
        self.painting.set(false);
        result
    }
    fn paint_inner(&self) -> Result<(), String> {
        let Some(w) = self.window() else {
            return Ok(());
        };
        let mut offline = self.offline.borrow_mut();
        let Some(o) = offline.as_mut() else {
            return Ok(());
        };
        let Some(mut size) = Size::fit(
            w.global::<BookmarksState>().get_reader_width(),
            w.global::<BookmarksState>().get_reader_height() * 24. / 23.,
            w.window().scale_factor(),
            1.,
        ) else {
            return Ok(());
        };
        size.height = (size.height * 23 / 24).max(1);
        if o.painted == Some((o.top, size)) {
            return Ok(());
        }
        let end = (o.top + 23).min(o.article.frame.rows);
        let mut frame = crate::output::crop_rows(&o.article.frame, o.top..end)?;
        frame.rows = 23;
        match &mut o.renderer {
            Some(renderer) => renderer.set_frame(frame),
            None => {
                o.renderer = Some(Renderer::new(frame, &o.article.settings).map_err(str::to_owned)?)
            }
        }
        let renderer = o.renderer.as_ref().ok_or("offline renderer missing")?;
        w.global::<BookmarksState>()
            .set_page(Image::from_rgba8_premultiplied(
                renderer.paint(size, false).map_err(str::to_owned)?,
            ));
        w.global::<BookmarksState>().set_progress(
            format!(
                "{} · {}",
                target_text(&o.article.target),
                msg!(
                    "bookmarks-lines",
                    first = (o.top + 1),
                    last = end,
                    total = o.article.frame.rows
                )
            )
            .into(),
        );
        o.painted = Some((o.top, size));
        Ok(())
    }
    fn close(&self) {
        if self
            .window()
            .is_some_and(|w| w.global::<BookmarksState>().get_composing())
        {
            return;
        }
        if self
            .result
            .borrow()
            .as_ref()
            .is_some_and(|f| f.operation == Operation::Save)
        {
            self.closing.set(true);
        } else {
            self.hide();
        }
    }
    fn hide(&self) {
        if let Some(w) = self.window() {
            w.global::<BookmarksState>().set_offline(false);
            w.global::<BookmarksState>().set_page(Image::default());
        }
        self.epoch.set(self.epoch.get().wrapping_add(1));
        self.offline.borrow_mut().take();
        self.clear_draft();
        self.closing.set(false);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_bookmarks_open(false);
        }
    }
    pub fn prepare_app_close(&self) {
        self.cancel_capture();
        self.hide();
    }
    pub fn finish(&self) -> Result<(), String> {
        self.cancel_capture();
        self.stopped.set(true);
        let joined = self.output.finish();
        let collected = self.collect();
        self.hide();
        self.window.borrow_mut().take();
        joined?;
        collected
    }
}
