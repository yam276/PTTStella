//! One native form over the article owner's effective rules. Explicit writes
//! use the shared Store and Output's joined worker; previews never project.
pub mod probe;
use crate::{
    FiltersWindow, TerminalWindow, article::session::ArticleSession, file_job::ResultSlot,
    output::Output,
};
use pttstella::{
    filters::{Entry, Policy, Rules, import::Parsed, preview::Import, store::Store},
    msg,
};
use slint::{ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

const COLOR_KEYS: [&str; 15] = [
    "filters-color-red",
    "filters-color-green",
    "filters-color-yellow",
    "filters-color-blue",
    "filters-color-magenta",
    "filters-color-cyan",
    "filters-color-gray",
    "filters-color-dark-gray",
    "filters-color-bright-red",
    "filters-color-bright-green",
    "filters-color-bright-yellow",
    "filters-color-bright-blue",
    "filters-color-bright-magenta",
    "filters-color-bright-cyan",
    "filters-color-white",
];
enum Value {
    Saved(Rules),
    Imported(u64, Option<Parsed>),
    Reloaded(Rules),
}
#[derive(Clone, Copy)]
enum Operation {
    Save,
    Import(u64),
    Reload,
}
struct Flight {
    slot: ResultSlot<Value>,
    operation: Operation,
}
type Work = Box<dyn FnOnce() -> Result<Value, String> + Send>;
pub struct Manager {
    ui: slint::Weak<TerminalWindow>,
    output: Rc<Output>,
    article: Rc<ArticleSession>,
    store: Result<Store, String>,
    warning: RefCell<Option<String>>,
    window: RefCell<Option<FiltersWindow>>,
    result: RefCell<Option<Flight>>,
    import: RefCell<Import>,
    request: Cell<Option<u64>>,
    serial: Cell<u64>,
    preview: RefCell<Option<Rules>>,
    rows: RefCell<Vec<Entry>>,
    fields: RefCell<[String; 2]>,
    closing: Cell<bool>,
    stopped: Cell<bool>,
}
fn strings(values: impl IntoIterator<Item = String>) -> ModelRc<slint::SharedString> {
    ModelRc::new(VecModel::from(
        values.into_iter().map(Into::into).collect::<Vec<_>>(),
    ))
}
impl Manager {
    pub fn mount(
        ui: &TerminalWindow,
        output: Rc<Output>,
        article: Rc<ArticleSession>,
        store: Result<Store, String>,
    ) -> Rc<Self> {
        let loaded = store.as_ref().map_err(Clone::clone).and_then(Store::load);
        let warning = loaded.as_ref().err().cloned();
        if let Ok(rules) = loaded {
            article.replace_rules(rules);
        }
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            output,
            article,
            store,
            warning: RefCell::new(warning),
            window: RefCell::new(None),
            result: RefCell::new(None),
            import: RefCell::new(Import::default()),
            request: Cell::new(None),
            serial: Cell::new(0),
            preview: RefCell::new(None),
            rows: RefCell::new(Vec::new()),
            fields: RefCell::new(Default::default()),
            closing: Cell::new(false),
            stopped: Cell::new(false),
        });
        ui.set_filters_available(true);
        ui.on_filters_show({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.open();
                }
            }
        });
        ui.on_filters_updated({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.sync();
                }
            }
        });
        owner
    }
    fn rules(&self) -> Rules {
        self.article.rules()
    }
    fn window(&self) -> Option<FiltersWindow> {
        self.window
            .borrow()
            .as_ref()
            .and_then(|w| w.as_weak().upgrade())
    }
    fn notice(&self, text: String) {
        if let Some(w) = self.window() {
            w.set_message(text.into());
        }
    }
    pub fn refresh_labels(&self) {
        let Some(w) = self.window() else { return };
        let loader = pttstella::i18n::application().loader();
        w.set_labels(strings(
            [
                "filters-title",
                "filters-tab-block",
                "filters-tab-allow",
                "filters-new",
                "filters-sort-id",
                "filters-sort-category",
                "filters-account-id",
                "filters-policy",
                "filters-policy-mask",
                "filters-id-color",
                "filters-policy-mask-row",
                "filters-note",
                "filters-save",
                "filters-remove",
                "filters-default-color",
                "filters-default-color-save",
                "filters-import",
                "filters-import-apply",
                "cancel",
                "close",
                "filters-description",
                "filters-limitations",
                "retry",
            ]
            .map(|key| loader.get(key)),
        ));
        let colors = COLOR_KEYS.map(|key| loader.get(key));
        w.set_colors(strings(colors.clone()));
        w.set_individual_colors(strings(
            std::iter::once(msg!("filters-color-shared")).chain(colors),
        ));
        self.render();
        self.sync();
    }
    fn render(&self) {
        let Some(w) = self.window() else { return };
        let rules = self
            .preview
            .borrow()
            .clone()
            .unwrap_or_else(|| self.rules());
        let mut entries: Vec<_> = rules
            .entries
            .into_iter()
            .filter(|e| (e.policy == Policy::Allow) == w.get_allow())
            .collect();
        entries.sort_by_key(|e| {
            (
                if w.get_category_sort() {
                    if w.get_allow() {
                        e.color.unwrap_or(rules.allow_color)
                    } else {
                        u8::from(e.policy == Policy::MaskRow)
                    }
                } else {
                    0
                },
                e.id.to_ascii_lowercase(),
            )
        });
        w.set_choice(
            entries
                .iter()
                .position(|e| e.id.eq_ignore_ascii_case(w.get_account().as_str()))
                .map_or(-1, |i| i as i32),
        );
        w.set_entries(strings(entries.iter().map(|e| {
            let detail = match e.policy {
                Policy::Mask => msg!("filters-policy-mask"),
                Policy::MaskRow => msg!("filters-policy-mask-row"),
                Policy::Allow => format!(
                    "{}{}",
                    pttstella::i18n::application()
                        .loader()
                        .get(COLOR_KEYS[usize::from(e.color.unwrap_or(rules.allow_color) - 1)]),
                    if e.color.is_none() {
                        format!(" · {}", msg!("filters-color-shared"))
                    } else {
                        String::new()
                    }
                ),
            };
            format!(
                "{} · {}{}",
                e.id,
                detail,
                if e.note.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", e.note)
                }
            )
        })));
        w.set_empty_message(
            if w.get_allow() {
                msg!("filters-empty-allow")
            } else {
                msg!("filters-empty-block")
            }
            .into(),
        );
        self.rows.replace(entries);
    }
    fn clear_draft(&self) {
        self.fields.replace(Default::default());
        if let Some(w) = self.window() {
            w.set_account("".into());
            w.set_note("".into());
            w.set_policy(0);
            w.set_entry_color(0);
            w.set_choice(-1);
            w.set_shared_color(i32::from(self.rules().allow_color) - 1);
        }
    }
    fn sync(&self) {
        if let Some(w) = self.window() {
            w.set_busy(self.output.busy() || self.result.borrow().is_some());
            w.set_writable(self.warning.borrow().is_none());
            w.set_importing(self.request.get().is_some());
            w.set_preview(self.preview.borrow().is_some());
        }
    }
    fn open(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.stopped.get()
            || ui.get_app_closing()
            || self.output.busy()
            || ui.get_sites_open()
            || ui.get_settings_open()
            || ui.get_editor_open()
            || ui.get_station_color_open()
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
                    ui.set_input_notice(e.into());
                    return;
                }
            }
        }
        self.closing.set(false);
        if !ui.get_filters_open() {
            self.clear_draft();
            self.notice(
                self.warning
                    .borrow()
                    .clone()
                    .unwrap_or_else(|| msg!("filters-save-help")),
            );
        }
        ui.invoke_tools_dismiss(false);
        ui.invoke_context_dismiss(false);
        ui.invoke_reset_keyboard();
        ui.set_filters_open(true);
        self.refresh_labels();
        if let Some(w) = self.window() {
            if let Err(e) = w.show() {
                ui.set_filters_open(false);
                ui.set_input_notice(e.to_string().into());
                return;
            }
            w.window().with_winit_window(|w| w.focus_window());
            w.invoke_focus_account();
        }
    }
    fn create_window(self: &Rc<Self>, ui: &TerminalWindow) -> Result<FiltersWindow, String> {
        let w = FiltersWindow::new().map_err(|e| e.to_string())?;
        w.on_tab({
            let o = Rc::downgrade(self);
            move |allow| {
                if let Some(o) = o.upgrade()
                    && let Some(w) = o.window()
                    && !w.get_composing()
                    && w.get_allow() != allow
                {
                    w.set_allow(allow);
                    o.clear_draft();
                    o.render();
                }
            }
        });
        w.on_sort({
            let o = Rc::downgrade(self);
            move || {
                if let Some(o) = o.upgrade()
                    && let Some(w) = o.window()
                    && !w.get_composing()
                {
                    w.set_category_sort(!w.get_category_sort());
                    o.render();
                }
            }
        });
        w.on_new_entry({
            let o = Rc::downgrade(self);
            move || {
                if let Some(o) = o.upgrade()
                    && o.editable()
                {
                    o.clear_draft();
                    if let Some(w) = o.window() {
                        w.invoke_focus_account();
                    }
                }
            }
        });
        w.on_choose({
            let o = Rc::downgrade(self);
            move |index| {
                if let Some(o) = o.upgrade()
                    && let Some(w) = o.window()
                    && !o.output.busy()
                    && !w.get_composing()
                {
                    let entry = usize::try_from(index)
                        .ok()
                        .and_then(|i| o.rows.borrow().get(i).cloned());
                    if let Some(e) = entry {
                        o.fields.replace([e.id.clone(), e.note.clone()]);
                        w.set_account(e.id.into());
                        w.set_note(e.note.into());
                        w.set_policy(i32::from(e.policy == Policy::MaskRow));
                        w.set_entry_color(e.color.map_or(0, i32::from));
                        w.set_choice(index);
                    }
                }
            }
        });
        w.on_field_edit({
            let o = Rc::downgrade(self);
            move |index, text| {
                let Some(o) = o.upgrade() else {
                    return "".into();
                };
                let Ok(i) = usize::try_from(index) else {
                    return "".into();
                };
                if i > 1 {
                    return "".into();
                }
                if !o.editable()
                    || text.chars().count() > if i == 0 { 12 } else { 128 }
                    || text.chars().any(char::is_control)
                {
                    return o.fields.borrow()[i].as_str().into();
                }
                o.fields.borrow_mut()[i] = text.to_string();
                text
            }
        });
        w.on_action({
            let o = Rc::downgrade(self);
            move |code| {
                if let Some(o) = o.upgrade() {
                    o.action(code);
                }
            }
        });
        w.on_close_filters({
            let o = Rc::downgrade(self);
            move || {
                if let Some(o) = o.upgrade() {
                    o.close();
                }
            }
        });
        w.window().on_close_requested({
            let o = Rc::downgrade(self);
            move || {
                if let Some(o) = o.upgrade() {
                    o.close();
                }
                slint::CloseRequestResponse::KeepWindowShown
            }
        });
        crate::native_text::install(ui, &w);
        Ok(w)
    }
    fn editable(&self) -> bool {
        !self.stopped.get()
            && self.warning.borrow().is_none()
            && self.request.get().is_none()
            && self.result.borrow().is_none()
            && !self.output.busy()
            && self
                .ui
                .upgrade()
                .is_some_and(|u| u.get_filters_open() && !u.get_app_closing())
            && self.window().is_some_and(|w| !w.get_composing())
    }
    fn cancel_import(&self) {
        self.request.set(None);
        self.import.borrow_mut().clear();
        self.preview.borrow_mut().take();
        self.clear_draft();
        self.render();
        self.sync();
    }
    fn begin_import(&self) -> Result<u64, String> {
        let request = self
            .serial
            .get()
            .checked_add(1)
            .ok_or_else(|| msg!("filters-import-stale"))?;
        self.serial.set(request);
        self.request.set(Some(request));
        self.import.borrow_mut().begin(request);
        Ok(request)
    }
    fn import_work(request: u64) -> Work {
        Box::new(move || {
            let path = rfd::FileDialog::new()
                .set_title(msg!("filters-import-title"))
                .add_filter(msg!("filters-import-file"), &["txt"])
                .pick_file();
            let parsed = path
                .map(|path| {
                    let bytes =
                        pttstella::storage::read_bounded(&path, pttstella::filters::MAX_BYTES)
                            .map_err(|error| match error {
                                pttstella::storage::ReadError::Io(e) => {
                                    msg!("filters-read-failed", e = e.to_string())
                                }
                                pttstella::storage::ReadError::TooLarge => {
                                    msg!("filters-too-large")
                                }
                            })?;
                    let bytes = bytes
                        .ok_or_else(|| msg!("filters-read-failed", e = "file no longer exists"))?;
                    pttstella::filters::import::parse(&bytes)
                })
                .transpose()?;
            Ok(Value::Imported(request, parsed))
        })
    }
    fn action(self: &Rc<Self>, code: i32) {
        let Some(ui) = self.ui.upgrade() else { return };
        let Some(w) = self.window() else { return };
        if self.stopped.get() || ui.get_app_closing() || !ui.get_filters_open() || w.get_composing()
        {
            return;
        }
        if code == 5 && self.request.get().is_some() {
            self.cancel_import();
            self.notice(msg!("filters-import-cancelled"));
            return;
        }
        if self.output.busy() || self.result.borrow().is_some() {
            return;
        }
        if code != 6 && self.warning.borrow().is_some() {
            return;
        }
        if self.request.get().is_some() && code != 4 {
            return;
        }
        let work = (|| {
            let store = self.store.clone()?;
            if code == 6 {
                return Ok::<(Work, Operation), String>((
                    Box::new(move || store.load().map(Value::Reloaded)),
                    Operation::Reload,
                ));
            }
            if code == 3 {
                let request = self.begin_import()?;
                return Ok((Self::import_work(request), Operation::Import(request)));
            }
            let base = self.rules();
            let next = match code {
                0 | 1 => base.edit_color(
                    w.get_account().as_str(),
                    w.get_note().as_str(),
                    if w.get_allow() {
                        Policy::Allow
                    } else if w.get_policy() == 1 {
                        Policy::MaskRow
                    } else {
                        Policy::Mask
                    },
                    if w.get_allow() && w.get_entry_color() > 0 {
                        Some(
                            u8::try_from(w.get_entry_color())
                                .map_err(|_| msg!("filters-color-invalid"))?,
                        )
                    } else {
                        None
                    },
                    code == 1,
                )?,
                2 => base.with_allow_color(
                    u8::try_from(w.get_shared_color() + 1)
                        .map_err(|_| msg!("filters-color-invalid"))?,
                )?,
                4 => {
                    let request = self
                        .request
                        .get()
                        .ok_or_else(|| msg!("filters-import-stale"))?;
                    let next = self.import.borrow_mut().apply(request, &base)?;
                    self.request.set(None);
                    self.preview.borrow_mut().take();
                    next
                }
                _ => return Err(msg!("filters-import-stale")),
            };
            Ok((
                Box::new(move || store.save_from(&base, next).map(Value::Saved)) as Work,
                Operation::Save,
            ))
        })();
        match work {
            Ok((work, operation)) => self.start(work, operation),
            Err(e) => {
                if code == 4 {
                    self.cancel_import();
                }
                self.notice(e);
            }
        }
    }
    fn start(self: &Rc<Self>, work: Work, operation: Operation) {
        let Some(ui) = self.ui.upgrade() else { return };
        let slot = ResultSlot::new();
        self.result.replace(Some(Flight {
            slot: slot.clone(),
            operation,
        }));
        self.notice(msg!("saving"));
        let o = Rc::downgrade(self);
        self.output.work(
            &ui,
            move || slot.publish(work()),
            move |_, _, _| {
                if let Some(o) = o.upgrade() {
                    let success = o.collect().is_ok();
                    o.sync();
                    if o.closing.get() && success {
                        o.hide();
                    } else if !success {
                        o.closing.set(false);
                    }
                }
            },
        );
        if !ui.get_output_busy() {
            self.result.borrow_mut().take();
            if matches!(operation, Operation::Import(_)) {
                self.cancel_import();
            }
            self.notice(ui.get_input_notice().to_string());
        }
        self.sync();
    }
    fn collect(&self) -> Result<(), String> {
        let Some(flight) = self.result.borrow_mut().take() else {
            return Ok(());
        };
        let value = flight.slot.take();
        // A cancelled/closed import must not publish even an old error.
        if let Operation::Import(request) = flight.operation
            && !self.import.borrow().accepts(request)
        {
            return Ok(());
        }
        let result = value.and_then(|value| match value {
            Value::Saved(rules) | Value::Reloaded(rules) => {
                self.article.replace_rules(rules);
                self.warning.borrow_mut().take();
                if let Some(w) = self.window() {
                    w.set_shared_color(i32::from(self.rules().allow_color) - 1);
                }
                self.render();
                self.notice(msg!("filters-saved"));
                Ok(())
            }
            Value::Imported(request, parsed) => {
                if !self.import.borrow().accepts(request) {
                    return Ok(());
                }
                match parsed {
                    Some(parsed) => {
                        let rules = self.import.borrow_mut().preview(
                            request,
                            &self.rules(),
                            &parsed.entries,
                        )?;
                        self.preview.replace(Some(rules));
                        self.clear_draft();
                        self.render();
                        self.notice(msg!(
                            "filters-import-preview",
                            count = parsed.entries.len(),
                            skipped = parsed.skipped,
                            adjusted = parsed.adjusted
                        ));
                    }
                    None => {
                        self.cancel_import();
                        self.notice(msg!("filters-import-cancelled"));
                    }
                }
                Ok(())
            }
        });
        if let Err(e) = &result {
            if matches!(flight.operation, Operation::Import(_)) {
                self.cancel_import();
            }
            if matches!(flight.operation, Operation::Reload) {
                self.warning.replace(Some(e.clone()));
            }
            self.render();
            self.notice(e.clone());
        }
        self.sync();
        result
    }
    fn close(&self) {
        if self.window().is_some_and(|w| w.get_composing()) {
            return;
        }
        self.closing.set(true);
        if self.request.get().is_some() {
            self.cancel_import();
            self.hide();
        } else if self.result.borrow().is_none() {
            self.hide();
        }
    }
    fn hide(&self) {
        if let Some(w) = self.window()
            && let Err(e) = w.hide()
        {
            self.notice(e.to_string());
            return;
        }
        self.cancel_import();
        self.clear_draft();
        self.closing.set(false);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_filters_open(false);
        }
    }
    pub fn prepare_app_close(&self) {
        self.hide();
    }
    pub fn finish(&self) -> Result<(), String> {
        self.stopped.set(true);
        let joined = self.output.finish();
        let collected = self.collect();
        self.hide();
        self.window.borrow_mut().take();
        joined?;
        collected
    }
}
