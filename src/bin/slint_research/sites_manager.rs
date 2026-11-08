//! Native station form. The shared model owns validation/baselines; Output owns
//! the only file worker and Connection owns every transport transition.
pub mod probe;
pub mod switch_probe;
use crate::{SitesWindow, TerminalWindow, file_job::ResultSlot, output::Output};
use pttstella::{
    connection::Connection,
    encoding::TextEncoding,
    msg,
    sites::{
        Sites, Target,
        session::{Edit, State},
    },
};
use slint::{ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
enum ResultValue {
    Saved(Sites),
    Reloaded(State),
}
pub struct Manager {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    output: Rc<Output>,
    state: Rc<RefCell<State>>,
    window: RefCell<Option<SitesWindow>>,
    result: RefCell<Option<ResultSlot<ResultValue>>>,
    selected: RefCell<Option<Target>>,
    fields: RefCell<[String; 2]>,
    quick: RefCell<Vec<Target>>,
    closing: Cell<bool>,
    stopped: Cell<bool>,
    probe: bool,
}
pub fn encoding(index: i32) -> Result<TextEncoding, String> {
    match index {
        0 => Ok(TextEncoding::Uao),
        1 => Ok(TextEncoding::Big5),
        2 => Ok(TextEncoding::Gbk),
        _ => Err(msg!("sites-invalid")),
    }
}
fn index(value: TextEncoding) -> i32 {
    match value {
        TextEncoding::Uao => 0,
        TextEncoding::Big5 => 1,
        TextEncoding::Gbk => 2,
    }
}
fn describe(target: &Target) -> String {
    msg!(
        "sites-target",
        address = target.endpoint.as_str(),
        encoding = match target.encoding {
            TextEncoding::Uao => "Big5-UAO",
            TextEncoding::Big5 => "Big5",
            TextEncoding::Gbk => "GBK",
        }
    )
}
impl Manager {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        output: Rc<Output>,
        state: Rc<RefCell<State>>,
        probe: bool,
    ) -> Rc<Self> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            connection,
            output,
            state,
            window: RefCell::new(None),
            result: RefCell::new(None),
            selected: RefCell::new(None),
            fields: RefCell::new([String::new(), String::new()]),
            quick: RefCell::new(Vec::new()),
            closing: Cell::new(false),
            stopped: Cell::new(false),
            probe,
        });
        ui.set_sites_available(true);
        owner.publish_choices(ui);
        ui.on_sites_show({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.open();
                }
            }
        });
        ui.on_sites_updated({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.sync();
                }
            }
        });
        owner
    }
    pub fn refresh_labels(&self) {
        let Some(w) = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::as_weak)
            .and_then(|w| w.upgrade())
        else {
            return;
        };
        w.set_labels(ModelRc::new(VecModel::from(vec![
            msg!("sites-title").into(),
            msg!("sites-favourites").into(),
            msg!("sites-manual").into(),
            msg!("sites-name").into(),
            msg!("sites-address").into(),
            msg!("sites-encoding").into(),
            msg!("sites-save").into(),
            msg!("sites-remove").into(),
            msg!("sites-set-default").into(),
            msg!("sites-connect").into(),
            msg!("sites-ptt-wss").into(),
            msg!("sites-ptt-ssh").into(),
            msg!("close").into(),
            msg!("retry").into(),
            msg!("sites-help").into(),
        ])));
        let state = self.state.borrow();
        let mut entries = vec![msg!("sites-manual").into()];
        entries.extend(
            state
                .sites()
                .favourites
                .iter()
                .map(|f| format!("{} — {}", f.name, describe(&f.target)).into()),
        );
        w.set_favourites(ModelRc::new(VecModel::from(entries)));
        let chosen = self.selected.borrow();
        w.set_choice(
            chosen
                .as_ref()
                .and_then(|t| state.sites().favourites.iter().position(|f| &f.target == t))
                .map_or(0, |i| i as i32 + 1),
        );
        w.set_default_note(
            msg!(
                "sites-default-value",
                target = describe(&state.default_target())
            )
            .into(),
        );
        w.set_current_note(
            msg!(
                "slint-sites-current",
                target = describe(&self.connection.updates.borrow().site)
            )
            .into(),
        );
        w.set_writable(state.warning().is_none());
    }
    fn publish_choices(&self, ui: &TerminalWindow) {
        let previous = usize::try_from(ui.get_station_choice())
            .ok()
            .and_then(|i| self.quick.borrow().get(i).cloned());
        let choices = crate::stations::Stations::choices(self.state.borrow().sites().clone()).0;
        ui.set_stations(ModelRc::new(VecModel::from(
            choices.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>(),
        )));
        ui.set_station_choice(
            previous
                .as_ref()
                .and_then(|p| choices.iter().position(|(_, t)| t == p))
                .unwrap_or(0) as i32,
        );
        self.quick
            .replace(choices.into_iter().map(|(_, t)| t).collect());
        self.refresh_labels();
    }
    fn notice(&self, text: String) {
        if let Some(w) = self.window.borrow().as_ref() {
            w.set_message(text.into());
        }
    }
    fn fill(&self, w: &SitesWindow, target: &Target, name: String) {
        self.fields
            .replace([name.clone(), target.endpoint.as_str().to_owned()]);
        w.set_name(name.into());
        w.set_address(target.endpoint.as_str().into());
        w.set_encoding(index(target.encoding));
        self.security(w);
    }
    fn security_for(address: &str) -> String {
        let target = Target::new(address.to_owned(), TextEncoding::Uao);
        match target.as_ref().map(|t| t.endpoint.as_str()) {
            Ok(a) if a.starts_with("ssh://") => msg!("sites-ssh-encrypted"),
            Ok(a) if a.starts_with("wss://") => msg!("sites-encrypted"),
            _ => msg!("sites-plaintext-disabled"),
        }
    }
    fn security(&self, w: &SitesWindow) {
        w.set_security(Self::security_for(w.get_address().as_str()).into());
    }
    fn choose(&self, choice: i32) {
        let Some(w) = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::as_weak)
            .and_then(|w| w.upgrade())
        else {
            return;
        };
        let favourite = usize::try_from(choice - 1)
            .ok()
            .and_then(|i| self.state.borrow().sites().favourites.get(i).cloned());
        self.selected
            .replace(favourite.as_ref().map(|f| f.target.clone()));
        if let Some(f) = favourite {
            self.fill(&w, &f.target, f.name);
        } else {
            self.fields.replace([String::new(), String::new()]);
            w.set_name("".into());
            w.set_address("".into());
            w.set_encoding(0);
        }
    }
    fn open(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.stopped.get()
            || ui.get_app_closing()
            || self.output.busy()
            || ui.get_filters_open()
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
            match self.create_window() {
                Ok(w) => {
                    self.window.replace(Some(w));
                }
                Err(e) => {
                    ui.set_input_notice(e.into());
                    return;
                }
            }
        }
        let already_open = ui.get_sites_open();
        self.closing.set(false);
        ui.invoke_tools_dismiss(false);
        ui.invoke_context_dismiss(false);
        ui.invoke_reset_keyboard();
        ui.invoke_paste_cancel();
        ui.set_sites_open(true);
        self.refresh_labels();
        if let Some(w) = self.window.borrow().as_ref() {
            if !already_open {
                self.selected.replace(None);
                w.set_choice(0);
                self.fill(w, &self.connection.updates.borrow().site, String::new());
                w.set_message(self.state.borrow().warning().unwrap_or("").into());
            }
            if let Err(e) = w.show() {
                ui.set_sites_open(false);
                ui.set_input_notice(e.to_string().into());
                return;
            }
            w.window().with_winit_window(|w| w.focus_window());
            w.invoke_focus_address();
        }
        self.sync();
    }
    fn create_window(self: &Rc<Self>) -> Result<SitesWindow, String> {
        let w = SitesWindow::new().map_err(|e| e.to_string())?;
        w.on_choose({
            let o = Rc::downgrade(self);
            move |i| {
                if let Some(o) = o.upgrade() {
                    o.choose(i);
                }
            }
        });
        w.on_preset({
            let o = Rc::downgrade(self);
            move |ssh| {
                if let Some(o) = o.upgrade()
                    && let Some(w) = o.window.borrow().as_ref()
                {
                    o.selected.replace(None);
                    w.set_choice(0);
                    let target = Target::new(
                        if ssh {
                            pttstella::sites::PTT_SSH
                        } else {
                            pttstella::sites::PTT
                        }
                        .into(),
                        TextEncoding::Uao,
                    );
                    if let Ok(target) = target {
                        o.fill(
                            w,
                            &target,
                            if ssh {
                                msg!("sites-ptt-ssh")
                            } else {
                                msg!("sites-ptt-wss")
                            },
                        );
                    }
                }
            }
        });
        w.on_field_edit({
            let o = Rc::downgrade(self);
            move |field, text| {
                let Some(o) = o.upgrade() else {
                    return "".into();
                };
                let Ok(i) = usize::try_from(field) else {
                    return "".into();
                };
                if i > 1 {
                    return "".into();
                }
                let max = if i == 0 { 128 } else { 512 };
                if text.len() > max || text.chars().any(char::is_control) {
                    o.notice(msg!("slint-sites-field-size", bytes = max.to_string()));
                    return o.fields.borrow()[i].as_str().into();
                }
                o.fields.borrow_mut()[i] = text.to_string();
                if i == 1
                    && let Some(w) = o.window.borrow().as_ref()
                {
                    w.set_security(Self::security_for(text.as_str()).into());
                }
                text
            }
        });
        w.on_action({
            let o = Rc::downgrade(self);
            move |i| {
                if let Some(o) = o.upgrade() {
                    o.action(i);
                }
            }
        });
        w.on_close_sites({
            let o = Rc::downgrade(self);
            move || {
                if let Some(o) = o.upgrade() {
                    o.request_close();
                }
            }
        });
        w.window().on_close_requested({
            let o = Rc::downgrade(self);
            move || {
                if let Some(o) = o.upgrade() {
                    o.request_close();
                }
                slint::CloseRequestResponse::KeepWindowShown
            }
        });
        if let Some(ui) = self.ui.upgrade() {
            crate::native_text::install(&ui, &w);
        }
        Ok(w)
    }
    fn action(self: &Rc<Self>, code: i32) {
        let Some(ui) = self.ui.upgrade() else { return };
        let Some(w) = self
            .window
            .borrow()
            .as_ref()
            .map(ComponentHandle::as_weak)
            .and_then(|w| w.upgrade())
        else {
            return;
        };
        if self.stopped.get()
            || self.output.busy()
            || self.result.borrow().is_some()
            || w.get_composing()
            || !ui.get_sites_open()
        {
            return;
        }
        if code == 3 {
            let result = encoding(w.get_encoding())
                .and_then(|e| Target::new(w.get_address().to_string(), e))
                .and_then(|target| {
                    target.validate_connection(self.probe)?;
                    if self.probe
                        && url::Url::parse(target.endpoint.as_str())
                            .ok()
                            .and_then(|u| u.host_str().map(str::to_owned))
                            .as_deref()
                            != Some("127.0.0.1")
                    {
                        return Err(msg!("sites-probe-local"));
                    }
                    Ok(target)
                });
            match result {
                Ok(target) => {
                    if ui.invoke_sites_connect(
                        target.endpoint.as_str().into(),
                        index(target.encoding),
                    ) {
                        self.hide();
                    } else {
                        self.notice(ui.get_result().to_string());
                    }
                }
                Err(e) => self.notice(e),
            }
            return;
        }
        let store = self.state.borrow().store();
        let work: Result<Box<dyn FnOnce() -> Result<ResultValue, String> + Send>, String> = if code
            == 4
        {
            store.map(|store| {
                let local = self.probe;
                Box::new(move || Ok(ResultValue::Reloaded(State::load(Ok(store), local))))
                    as Box<dyn FnOnce() -> Result<ResultValue, String> + Send>
            })
        } else {
            let action = match code {
                0 => Edit::Favourite,
                1 => Edit::Remove,
                2 => Edit::Default,
                _ => return,
            };
            let target = if code == 1 {
                self.selected
                    .borrow()
                    .clone()
                    .ok_or_else(|| msg!("sites-invalid"))
            } else {
                encoding(w.get_encoding()).and_then(|e| Target::new(w.get_address().to_string(), e))
            };
            target
                .and_then(|target| {
                    self.state.borrow().edit(
                        action,
                        target.endpoint.as_str().into(),
                        target.encoding,
                        w.get_name().to_string(),
                    )
                })
                .map(|write| {
                    Box::new(move || {
                        write
                            .store
                            .save_from(&write.base, write.draft)
                            .map(ResultValue::Saved)
                    })
                        as Box<dyn FnOnce() -> Result<ResultValue, String> + Send>
                })
        };
        let work = match work {
            Ok(work) => work,
            Err(e) => {
                self.notice(e);
                return;
            }
        };
        let slot = ResultSlot::new();
        self.result.replace(Some(slot.clone()));
        self.notice(if code == 4 {
            msg!("sites-loading")
        } else {
            msg!("saving")
        });
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
            self.notice(ui.get_input_notice().to_string());
        }
        self.sync();
    }
    fn collect(&self) -> Result<(), String> {
        let Some(slot) = self.result.borrow_mut().take() else {
            return Ok(());
        };
        let outcome = slot.take().and_then(|value| match value {
            ResultValue::Saved(sites) => self.state.borrow_mut().complete(Ok(sites)),
            ResultValue::Reloaded(state) => {
                self.state.replace(state);
                Ok(())
            }
        });
        match &outcome {
            Ok(()) => {
                if let Some(ui) = self.ui.upgrade() {
                    self.publish_choices(&ui);
                }
                self.notice(
                    self.state
                        .borrow()
                        .warning()
                        .map_or_else(|| msg!("sites-saved"), str::to_owned),
                );
            }
            Err(e) => self.notice(e.clone()),
        };
        outcome
    }
    fn sync(&self) {
        if let Some(w) = self.window.borrow().as_ref() {
            w.set_busy(
                self.output.busy()
                    || self.result.borrow().is_some()
                    || self.ui.upgrade().is_some_and(|u| u.get_switching_station()),
            );
        }
    }
    fn request_close(&self) {
        if self
            .window
            .borrow()
            .as_ref()
            .is_some_and(|w| w.get_composing())
        {
            return;
        }
        self.closing.set(true);
        if self.result.borrow().is_none() {
            self.hide();
        }
    }
    fn hide(&self) {
        if let Some(w) = self.window.borrow().as_ref()
            && let Err(e) = w.hide()
        {
            self.notice(e.to_string());
            return;
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_sites_open(false);
        }
        self.closing.set(false);
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
