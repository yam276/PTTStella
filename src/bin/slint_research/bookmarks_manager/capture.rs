//! Online acquisition ends at ArchiveWrite; the existing joined local worker
//! owns persistence. An offline reader never receives a station route.
use super::*;
use crate::article::session::bookmarks::Source;
use pttstella::{
    bookmarks::session::{Acquisition, FromList},
    navigation::Stamp,
};
use std::time::Duration;

pub(super) enum Capture {
    Loading(Source),
    Current {
        stamp: Stamp,
        acquisition: Acquisition,
    },
    List(FromList),
}
impl Manager {
    pub(super) fn capture_trace(&self, at: &str, source: Option<Source>) {
        let remaining = self.capture_trace_left.get();
        if remaining == 0 {
            return;
        }
        self.capture_trace_left.set(remaining - 1);
        let stage = match self.capture.borrow().as_ref() {
            Some(Capture::Loading(_)) => "loading",
            Some(Capture::Current { .. }) => "current",
            Some(Capture::List(_)) => "list",
            None => "none",
        };
        let current = source.and_then(|source| {
            self.article
                .bookmark_source(&self.connection.updates.borrow(), source.cell)
        });
        eprintln!(
            "{}",
            serde_json::json!({"bookmark_capture_trace":{
                "at":at,"serial":self.capture_serial.get(),"stage":stage,
                "source":format!("{source:?}"),"current_source":format!("{current:?}"),
                "state_loaded":self.state.borrow().is_some(),
                "file_result_pending":self.result.borrow().is_some(),"output_busy":self.output.busy(),
                "ui_output_busy":self.ui.upgrade().map(|ui|ui.get_output_busy()),
                "article":self.article.diagnostics()
            }})
        );
    }
    fn capture_environment(&self, ui: &TerminalWindow) -> bool {
        !self.stopped.get()
            && !ui.get_app_closing()
            && ui.get_native_foreground()
            && !ui.get_article_station()
            && !ui.get_manager_open()
            && !ui.get_settings_open()
            && !ui.get_editor_open()
            && !ui.get_station_color_open()
            && !ui.get_media_open()
            && !ui.get_login_open()
            && !ui.get_paste_open()
            && !ui.get_native_composing()
            && !ui.get_find_composing()
            && !ui.get_find_open()
            && !ui.get_selection_active()
            && !ui.get_selection_dragging()
            && !ui.get_switching_station()
            && !ui.get_station_mouse_held()
    }
    fn capture_notice(&self, text: String, kind: pttstella::notification::Kind) {
        if let Some(ui) = self.ui.upgrade() {
            crate::notice::show(&ui, text.clone(), kind);
        }
        self.notice(text);
    }
    pub fn list_source(
        &self,
        identity: pttstella::surface_state::Identity,
        stamp: Stamp,
        cell: (usize, usize),
    ) -> Option<Source> {
        let ui = self.ui.upgrade()?;
        if !self.capture_environment(&ui)
            || self.capture.borrow().is_some()
            || self.output.busy()
            || self.result.borrow().is_some()
        {
            return None;
        }
        let source = self
            .article
            .bookmark_source(&self.connection.updates.borrow(), Some(cell))?;
        (source.identity == identity && source.stamp == stamp).then_some(source)
    }
    pub fn current_available(&self) -> bool {
        self.ui
            .upgrade()
            .is_some_and(|ui| self.capture_environment(&ui))
            && self.capture.borrow().is_none()
            && !self.output.busy()
            && self.result.borrow().is_none()
            && self
                .article
                .bookmark_source(&self.connection.updates.borrow(), None)
                .is_some()
    }
    pub fn capture_current(self: &Rc<Self>) {
        if !self.current_available() {
            return;
        }
        let source = self
            .article
            .bookmark_source(&self.connection.updates.borrow(), None);
        if let Some(source) = source {
            self.capture_source(source);
        }
    }
    pub fn capture_source(self: &Rc<Self>, source: Source) {
        self.capture_trace("source", Some(source));
        let Some(ui) = self.ui.upgrade() else { return };
        if !self.capture_environment(&ui)
            || self.output.busy()
            || self.result.borrow().is_some()
            || self.capture.borrow().is_some()
        {
            return;
        }
        if self
            .article
            .bookmark_source(&self.connection.updates.borrow(), source.cell)
            != Some(source)
        {
            self.capture_notice(
                msg!("bookmarks-current-unavailable"),
                pttstella::notification::Kind::Error,
            );
            return;
        }
        self.capture_serial
            .set(self.capture_serial.get().wrapping_add(1));
        self.capture.replace(Some(Capture::Loading(source)));
        // Loading is part of this owned capture, including a failed or stalled
        // local file job. It must not wait forever before acquisition starts.
        self.capture_timer(Duration::from_secs(30));
        ui.invoke_tools_dismiss(false);
        ui.invoke_context_dismiss(false);
        ui.invoke_reset_keyboard();
        if self.state.borrow().is_none() {
            self.reload();
            self.capture_trace("cold-load-started", Some(source));
        }
        self.capture_tick();
    }
    fn capture_timer(self: &Rc<Self>, duration: Duration) {
        let weak = Rc::downgrade(self);
        self.capture_timer
            .start(slint::TimerMode::SingleShot, duration, move || {
                if let Some(o) = weak.upgrade() {
                    o.cancel_capture_with(msg!("bookmarks-capture-failed"));
                }
            });
    }
    fn capture_go(
        self: &Rc<Self>,
        stamp: Stamp,
        target: pttstella::navigation_job::Destination,
    ) -> Result<(), String> {
        let request = self.capture_serial.get();
        let weak = Rc::downgrade(self);
        self.article.go_to(stamp, target, move |result| {
            if let Some(o) = weak.upgrade()
                && o.capture_serial.get() == request
                && let Err(e) = result
            {
                o.cancel_capture_with(e);
            }
        })
    }
    fn begin_current(
        self: &Rc<Self>,
        source: Source,
        expected: Option<ArticleTarget>,
    ) -> Result<(), String> {
        self.capture_trace("begin-current", Some(source));
        let (snapshot, destination) = self
            .article
            .bookmark_begin(source, &self.connection.updates.borrow())
            .inspect_err(|_| self.capture_trace("begin-current-rejected", Some(source)))?;
        let mut acquisition = Acquisition::new(self.capture_serial.get(), source.stamp, snapshot);
        if let Some(expected) = expected {
            acquisition = acquisition.expect(expected);
        }
        self.capture.replace(Some(Capture::Current {
            stamp: source.stamp,
            acquisition,
        }));
        self.capture_timer(Duration::from_secs(30));
        self.capture_notice(
            msg!("bookmarks-capturing"),
            pttstella::notification::Kind::Persistent,
        );
        self.capture_go(source.stamp, destination)
    }
    pub fn capture_tick(self: &Rc<Self>) {
        if self.capture_observing.replace(true) {
            return;
        }
        self.capture_tick_inner();
        self.capture_observing.set(false);
        if let Some(ui) = self.ui.upgrade() {
            let active = self.capture.borrow().is_some();
            let available = self.current_available();
            if ui.get_bookmarks_capturing() != active
                || ui.get_bookmarks_current_available() != available
            {
                ui.set_bookmarks_capturing(active);
                ui.set_bookmarks_current_available(available);
                ui.invoke_tools_updated();
            }
        }
    }
    fn capture_tick_inner(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.capture.borrow().is_none() {
            return;
        }
        if !self.capture_environment(&ui) {
            self.cancel_capture_with(msg!("bookmarks-capture-cancelled"));
            return;
        }
        if ui.get_output_busy() && !matches!(*self.capture.borrow(), Some(Capture::Loading(_))) {
            self.cancel_capture_with(msg!("file-busy"));
            return;
        }
        let stage = self.capture.borrow_mut().take();
        let Some(stage) = stage else { return };
        let result = match stage {
            Capture::Loading(source) => {
                if self.output.busy() || self.result.borrow().is_some() || ui.get_output_busy() {
                    self.capture.replace(Some(Capture::Loading(source)));
                    return;
                }
                self.capture_trace("loading-ready", Some(source));
                let checked = self
                    .state
                    .borrow()
                    .as_ref()
                    .ok_or_else(|| msg!("bookmarks-loading"))
                    .and_then(State::check_write);
                checked.and_then(|()| {
                    if source.article.is_some() {
                        self.begin_current(source, None)
                    } else {
                        let target = self
                            .article
                            .bookmark_list(source, &self.connection.updates.borrow())?;
                        self.capture.replace(Some(Capture::List(FromList::new(
                            self.capture_serial.get(),
                            source.stamp,
                        ))));
                        self.capture_timer(Duration::from_secs(35));
                        self.capture_notice(
                            msg!("bookmarks-list-loading"),
                            pttstella::notification::Kind::Persistent,
                        );
                        self.capture_go(source.stamp, target)
                    }
                })
            }
            Capture::Current { stamp, acquisition } => {
                let resolved = acquisition.resolve(
                    &self.connection.updates.borrow(),
                    self.article.bookmark_failed(),
                );
                match resolved {
                    None => {
                        self.capture
                            .replace(Some(Capture::Current { stamp, acquisition }));
                        Ok(())
                    }
                    Some(Err(e)) => {
                        self.capture
                            .replace(Some(Capture::Current { stamp, acquisition }));
                        Err(e)
                    }
                    Some(Ok(target)) => {
                        let write = self
                            .state
                            .borrow()
                            .as_ref()
                            .ok_or_else(|| msg!("bookmarks-loading"))
                            .and_then(|state| acquisition.into_write(state, target));
                        self.capture_timer.stop();
                        write.map(|write| {
                            self.start(
                                Box::new(move || write.run().map(Value::Saved)),
                                Operation::Save,
                            )
                        })
                    }
                }
            }
            Capture::List(mut capture) => {
                let observed = self
                    .article
                    .bookmark_observe_list(&mut capture, &self.connection.updates.borrow());
                if capture.deadline().is_none() {
                    self.capture_timer.stop();
                }
                match observed {
                    None => {
                        self.capture.replace(Some(Capture::List(capture)));
                        Ok(())
                    }
                    Some(Err(e)) => {
                        self.capture.replace(Some(Capture::List(capture)));
                        Err(e)
                    }
                    Some(Ok((_, target))) => {
                        let source = self
                            .article
                            .bookmark_source(&self.connection.updates.borrow(), None);
                        if let Some(source) = source {
                            self.begin_current(source, Some(target))
                        } else {
                            self.capture.replace(Some(Capture::List(capture)));
                            Ok(())
                        }
                    }
                }
            }
        };
        if let Err(e) = result {
            if self.capture.borrow().is_some() {
                self.cancel_capture_with(e);
            } else {
                self.capture_notice(e, pttstella::notification::Kind::Error);
            }
        }
    }
    pub fn cancel_capture(&self) {
        self.cancel_capture_with(msg!("bookmarks-capture-cancelled"));
    }
    fn cancel_capture_with(&self, message: String) {
        let capture = self.capture.borrow_mut().take();
        let Some(capture) = capture else { return };
        // Retire before the synchronous receipt cancellation callback.
        self.capture_serial
            .set(self.capture_serial.get().wrapping_add(1));
        self.capture_timer.stop();
        let result = match capture {
            Capture::Loading(_) => Ok(()),
            Capture::Current { stamp, .. } => self.article.bookmark_abort(stamp, false),
            Capture::List(capture) => self.article.bookmark_abort(capture.stamp, true),
        };
        match result {
            Ok(()) => self.capture_notice(message, pttstella::notification::Kind::Persistent),
            Err(error) => self.capture_notice(error, pttstella::notification::Kind::Error),
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_bookmarks_capturing(false);
            ui.invoke_tools_updated();
        }
    }
}
