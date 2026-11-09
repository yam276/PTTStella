//! Local destination drafts; the existing article owner owns the sole native
//! command receipt and the Connection owns every station workflow and byte.
pub mod probe;
mod recent;
use crate::{DestinationWindow, TerminalWindow, article::session::ArticleSession, output::Output};
use pttstella::{
    connection::{Connection, Phase},
    msg,
    navigation::Stamp,
    navigation_job::{Destination, Progress},
    recent_boards::store::Store,
};
use slint::{ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub struct Manager {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    article: Rc<ArticleSession>,
    output: Rc<Output>,
    history: Rc<recent::Recent>,
    window: RefCell<Option<DestinationWindow>>,
    draft: RefCell<String>,
    serial: Cell<u64>,
    pending: Cell<Option<(u64, Stamp)>>,
    active: Cell<Option<Stamp>>,
    generation: Cell<u64>,
    stopping: Cell<bool>,
}
fn strings(values: impl IntoIterator<Item = String>) -> ModelRc<slint::SharedString> {
    ModelRc::new(VecModel::from(
        values.into_iter().map(Into::into).collect::<Vec<_>>(),
    ))
}
impl Manager {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        article: Rc<ArticleSession>,
        output: Rc<Output>,
        store: Result<Store, String>,
    ) -> Rc<Self> {
        let generation = connection.updates.borrow().generation;
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            connection,
            article,
            output,
            history: recent::Recent::mount(ui, store),
            window: RefCell::new(None),
            draft: RefCell::new(String::new()),
            serial: Cell::new(0),
            pending: Cell::new(None),
            active: Cell::new(None),
            generation: Cell::new(generation),
            stopping: Cell::new(false),
        });
        ui.set_destination_available(true);
        ui.on_destination_show({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.open(None);
                }
            }
        });
        ui.on_destination_fill({
            let o = Rc::downgrade(&owner);
            move |value| {
                if let Some(o) = o.upgrade() {
                    o.open(Some(value.to_string()));
                }
            }
        });
        ui.on_destination_stop({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade()
                    && let Err(e) = o.stop()
                {
                    o.notice(e);
                }
            }
        });
        ui.on_destination_updated({
            let o = Rc::downgrade(&owner);
            move || {
                if let Some(o) = o.upgrade() {
                    o.update();
                }
            }
        });
        owner.update();
        owner
    }
    fn window(&self) -> Option<DestinationWindow> {
        self.window
            .borrow()
            .as_ref()
            .and_then(|w| w.as_weak().upgrade())
    }
    fn notice(&self, value: String) {
        self.inline_notice(value.clone());
        if let Some(ui) = self.ui.upgrade() {
            crate::notice::error(&ui, value);
        }
    }
    fn inline_notice(&self, value: String) {
        if let Some(w) = self.window() {
            w.set_message(value.into());
        }
    }
    pub fn refresh_labels(&self) {
        let Some(w) = self.window() else { return };
        let loader = pttstella::i18n::application().loader();
        w.set_labels(strings(
            [
                "destination-title",
                "destination-name",
                "destination-dialog-hint",
                "recent-title",
                "recent-clear",
                "destination-go",
                "destination-stop",
                "cancel",
            ]
            .map(|key| loader.get(key)),
        ));
        self.render();
    }
    fn render(&self) {
        if self
            .ui
            .upgrade()
            .is_none_or(|ui| !ui.get_destination_open())
        {
            return;
        }
        let Some(w) = self.window() else { return };
        let history = self.history.snapshot();
        w.set_boards(strings(
            std::iter::once(msg!("recent-title"))
                .chain(history.boards.iter().map(|b| b.as_str().to_owned())),
        ));
        w.set_history_message(
            history
                .notice
                .unwrap_or_else(|| {
                    if history.boards.is_empty() {
                        msg!("recent-empty")
                    } else {
                        String::new()
                    }
                })
                .into(),
        );
        w.set_busy(self.pending.get().is_some());
        w.set_active(self.active.get().is_some() || self.pending.get().is_some());
    }
    fn create_window(self: &Rc<Self>, ui: &TerminalWindow) -> Result<DestinationWindow, String> {
        let w = DestinationWindow::new().map_err(|e| e.to_string())?;
        w.on_field_edit({
            let o = Rc::downgrade(self);
            move |value| {
                let Some(o) = o.upgrade() else {
                    return "".into();
                };
                if o.stopping.get() || o.pending.get().is_some() {
                    return o.draft.borrow().clone().into();
                }
                let value = value.to_string();
                if value.len() > pttstella::article_target::MAX_INPUT
                    || value.chars().any(char::is_control)
                {
                    o.notice(msg!("destination-input-invalid"));
                    return o.draft.borrow().clone().into();
                }
                o.draft.replace(value.clone());
                value.into()
            }
        });
        w.on_choose({
            let o = Rc::downgrade(self);
            move |index| {
                if let Some(o) = o.upgrade()
                    && o.admit()
                {
                    let history = o.history.snapshot();
                    if let Some(board) = usize::try_from(index)
                        .ok()
                        .and_then(|i| i.checked_sub(1))
                        .and_then(|i| history.boards.get(i))
                    {
                        o.draft.replace(board.as_str().to_owned());
                        if let Some(w) = o.window() {
                            w.set_draft(board.as_str().into());
                            w.invoke_focus_draft();
                        }
                    }
                }
            }
        });
        w.on_action({
            let o = Rc::downgrade(self);
            move |code| {
                if let Some(o) = o.upgrade()
                    && o.admit()
                {
                    match code {
                        0 => o.submit(),
                        1 => {
                            if let Err(e) = o.stop() {
                                o.notice(e);
                            }
                        }
                        2 => o.history.clear(),
                        _ => {}
                    }
                }
            }
        });
        w.on_close_destination({
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
    fn admit(&self) -> bool {
        !self.stopping.get()
            && self
                .ui
                .upgrade()
                .is_some_and(|ui| ui.get_destination_open() && !ui.get_app_closing())
            && self.window().is_some_and(|w| !w.get_composing())
    }
    fn open(self: &Rc<Self>, value: Option<String>) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.stopping.get()
            || ui.get_app_closing()
            || self.output.busy()
            || ui.get_bookmarks_open()
            || ui.get_sites_open()
            || ui.get_filters_open()
            || ui.get_settings_open()
            || ui.get_editor_open()
            || ui.get_station_color_open()
            || ui.get_media_open()
            || ui.get_find_open()
            || ui.get_login_open()
            || ui.get_paste_open()
            || ui.get_native_composing()
        {
            return;
        }
        if self.window.borrow().is_none() {
            match self.create_window(&ui) {
                Ok(w) => {
                    self.window.replace(Some(w));
                }
                Err(e) => {
                    self.notice(e);
                    return;
                }
            }
        }
        if let Some(value) = value {
            // Saved-bookmark handoffs are still drafts, never navigation.
            if Destination::parse(&value).is_err() {
                self.notice(msg!("destination-input-invalid"));
                return;
            }
            if self.pending.get().is_some() {
                return;
            }
            self.draft.replace(value);
        }
        ui.invoke_tools_dismiss(false);
        ui.invoke_context_dismiss(false);
        ui.invoke_reset_keyboard();
        if !ui.get_destination_open() && self.pending.get().is_none() {
            self.inline_notice(String::new());
        }
        ui.set_destination_open(true);
        self.refresh_labels();
        if let Some(w) = self.window() {
            w.set_draft(self.draft.borrow().clone().into());
            w.set_choice(0);
            if let Err(e) = w.show() {
                ui.set_destination_open(false);
                self.notice(e.to_string());
                return;
            }
            w.window().with_winit_window(|w| w.focus_window());
            w.invoke_focus_draft();
        }
        if self.connection.updates.borrow().phase != Phase::Connected {
            self.notice(msg!("destination-dialog-disconnected"));
        }
    }
    fn submit(self: &Rc<Self>) {
        if self.pending.get().is_some() || self.active.get().is_some() || self.output.busy() {
            self.notice(msg!("command-busy"));
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let result = (|| {
            let destination = Destination::parse(&self.draft.borrow())?;
            let stamp = self
                .output
                .current(&ui)
                .ok_or_else(|| msg!("destination-stale"))?;
            let view = self.connection.updates.borrow();
            if view.phase != Phase::Connected {
                return Err(msg!("destination-dialog-disconnected"));
            }
            if !view.site.ptt_features() {
                return Err(msg!("site-ptt-only"));
            }
            if view.input_pending || view.output_pending || view.login_pending {
                return Err(msg!("command-busy"));
            }
            drop(view);
            let request = self.serial.get().wrapping_add(1);
            self.serial.set(request);
            self.pending.set(Some((request, stamp)));
            let weak = Rc::downgrade(self);
            if let Err(e) = self.article.go_to(stamp, destination, move |result| {
                if let Some(o) = weak.upgrade() {
                    o.started(request, stamp, result);
                }
            }) {
                self.pending.set(None);
                return Err(e);
            }
            Ok(())
        })();
        match result {
            Ok(()) => self.inline_notice(msg!("destination-dialog-starting")),
            Err(e) => self.notice(e),
        }
        self.render();
    }
    fn started(self: &Rc<Self>, request: u64, stamp: Stamp, result: Result<(), String>) {
        if self.pending.get() != Some((request, stamp)) {
            return;
        }
        self.pending.set(None);
        match result {
            Ok(()) => {
                self.active.set(Some(stamp));
                self.hide();
            }
            Err(e) => self.notice(e),
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_destination_active(self.active.get().is_some());
        }
        self.update();
    }
    fn stop(&self) -> Result<(), String> {
        let pending = self.pending.take().map(|(_, stamp)| stamp);
        let active = self.active.take();
        self.serial.set(self.serial.get().wrapping_add(1));
        let result = pending
            .or(active)
            .map_or(Ok(()), |stamp| self.article.cancel_destination(stamp));
        if let Some(ui) = self.ui.upgrade() {
            ui.set_destination_active(false);
        }
        self.render();
        result
    }
    fn close(&self) {
        if self.window().is_some_and(|w| w.get_composing()) {
            return;
        }
        if let Err(e) = self.stop() {
            self.notice(e);
        }
        self.hide();
    }
    fn hide(&self) {
        if let Some(w) = self.window() {
            if let Err(e) = w.hide() {
                self.notice(e.to_string());
                return;
            }
            w.set_draft("".into());
            w.set_choice(0);
        }
        self.draft.borrow_mut().clear();
        if let Some(ui) = self.ui.upgrade() {
            ui.set_destination_open(false);
        }
    }
    fn update(self: &Rc<Self>) {
        if self.stopping.get() {
            return;
        }
        let (generation, stamp, progress, visit, message) = {
            let v = self.connection.updates.borrow();
            (
                v.generation,
                v.stamp(),
                v.destination.clone(),
                v.board_visit.clone(),
                v.message.clone(),
            )
        };
        self.history.observe(visit.as_ref());
        if self.generation.replace(generation) != generation
            && let Err(e) = self.stop()
        {
            self.notice(e);
        }
        if let Some(operation) = self.active.get() {
            let ended = match progress {
                Progress::Reached {
                    operation: done, ..
                } => done == operation,
                Progress::Active {
                    operation: other, ..
                } => other != operation,
                Progress::Idle => stamp != operation,
            };
            if ended {
                self.active.set(None);
                if !message.is_empty() {
                    self.inline_notice(message);
                }
            }
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_destination_active(self.active.get().is_some() || self.pending.get().is_some());
        }
        self.render();
    }
    pub async fn shutdown(self: &Rc<Self>) {
        self.stopping.set(true);
        if let Err(e) = self.stop() {
            self.notice(e);
        }
        self.hide();
        self.history.shutdown().await;
    }
    pub fn finish(&self) -> Result<(), String> {
        self.stopping.set(true);
        let stopped = self.stop();
        self.hide();
        self.window.borrow_mut().take();
        let saved = self.history.finish();
        match (stopped, saved) {
            (Err(a), Err(b)) => Err(format!("{a}; recent: {b}")),
            (Err(e), _) | (_, Err(e)) => Err(e),
            _ => Ok(()),
        }
    }
}
