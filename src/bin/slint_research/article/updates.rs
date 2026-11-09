//! Native presentation of the common opt-in refresh owner. The article loop owns
//! the only deadline and command receipt; this module never starts a task/timer.
use super::ArticleSession;
use crate::TerminalWindow;
use pttstella::{article_updates::Status, connection::View};
use std::rc::Rc;
use tokio::time::Instant;

type Controls = (Option<Status>, bool, usize, bool);
#[derive(Default)]
pub(super) struct Presentation {
    scope: Option<(u64, u64)>,
    total: usize,
    seen: usize,
    tracking: bool,
    shown: Option<Controls>,
}
impl Presentation {
    pub(super) fn stopped(&self) -> bool {
        self.scope.is_none() && !self.tracking && self.shown.is_none()
    }
    fn observe(&mut self, scope: (u64, u64), count: usize, enabled: bool) {
        if self.scope != Some(scope) {
            *self = Self {
                scope: Some(scope),
                total: count,
                seen: count,
                tracking: enabled,
                shown: None,
            };
        } else {
            self.total = count;
            self.seen = if self.tracking {
                self.seen.min(count)
            } else {
                count
            };
            self.tracking |= enabled;
        }
    }
}
impl ArticleSession {
    pub(super) fn install_updates(self: &Rc<Self>, ui: &TerminalWindow) {
        let weak = Rc::downgrade(self);
        ui.on_article_updates_action(move |action| {
            if let Some(owner) = weak.upgrade() {
                owner.updates_action(action);
            }
        });
        let weak = Rc::downgrade(self);
        ui.on_article_updates_scrolled(move || {
            if let Some(owner) = weak.upgrade() {
                owner.notify.notify_one();
            }
        });
    }
    fn updates_action(self: &Rc<Self>, action: i32) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.closed.get() || ui.get_article_updates_blocked() {
            return;
        }
        let view = self.connection.updates.borrow().clone();
        let Some(status) = self.core.borrow().updates_status() else {
            return;
        };
        if !ui.get_article_active() || view.generation != status.generation {
            return;
        }
        match action {
            0 => {
                // Stopping a retained operation must remain possible while its
                // station screen no longer matches the local projection.
                if !status.enabled && self.output_identity(&view).is_none() {
                    return;
                }
                if !self.core.borrow_mut().updates_action(
                    status.generation,
                    status.article,
                    !status.enabled,
                ) {
                    return;
                }
                if status.enabled {
                    self.core.borrow_mut().interrupt_full_load();
                    if self.destination_wait.borrow().is_none() {
                        self.cancel_command();
                    }
                    self.core.borrow_mut().update(&view);
                } else {
                    self.live_updates.borrow_mut().tracking = true;
                }
            }
            1 => {
                if self.output_identity(&view).is_none() {
                    return;
                }
                if let Some(native) = self.view.borrow().as_ref() {
                    native.navigate(pttstella::navigation::Intent::Last);
                }
                let mut state = self.live_updates.borrow_mut();
                state.seen = state.total;
            }
            2 => {
                if !self
                    .core
                    .borrow_mut()
                    .restore_station(status.generation, status.article)
                {
                    return;
                }
                self.focus_after_paint.set(true);
                self.terminal_recovery.set(Some(view.stamp()));
                self.presentation_changed(&view);
            }
            _ => return,
        }
        if action != 2 && ui.get_context_focus_restorable() {
            ui.invoke_focus_terminal();
        }
        self.updates_sync_ui();
        self.notify.notify_one();
    }
    pub(super) fn updates_step(self: &Rc<Self>, view: &View, ui: &TerminalWindow) {
        let now = Instant::now();
        // Bookmark acquisition uses the same common refresh reducer but has its
        // own environment/cancellation owner. Only opted-in updates pause here.
        let enabled = self
            .core
            .borrow()
            .updates_status()
            .is_some_and(|s| s.enabled);
        if enabled && !self.allowed(ui) {
            if std::env::var_os("PTTSTELLA_SLINT_UPDATES_PROBE").is_some()
                && (self.core.borrow().updates_holding()
                    || self
                        .live_updates
                        .borrow()
                        .shown
                        .as_ref()
                        .is_none_or(|(_, paused, _, _)| !paused))
            {
                eprintln!(
                    "native updates paused: ready={},focused={},foreground={},permit={:?},scope={:?},busy={},status={:?}",
                    ui.get_article_reading_ready(),
                    ui.get_article_focused(),
                    ui.get_native_foreground(),
                    self.permit.get(),
                    self.core.borrow().reading_scope(),
                    self.busy.get(),
                    self.core.borrow().updates_status()
                );
            }
            self.core.borrow_mut().pause_updates();
        }
        let timed_out = {
            let core = self.core.borrow();
            core.updates_holding() && core.updates_deadline(false).is_some_and(|at| at <= now)
        };
        if timed_out {
            self.core.borrow_mut().fail_updates();
            self.core.borrow_mut().update(view);
        }
        let cancelled = self.core.borrow_mut().cancelled_update();
        if let Some(stamp) = cancelled
            && let Err(error) = self.cancel_destination(stamp)
        {
            self.notice(error);
        }
        if !self.busy.get() && self.allowed(ui) {
            let destination = self.core.borrow_mut().next_update(view);
            if let Some(destination) = destination {
                let weak = Rc::downgrade(self);
                if let Err(error) = self.go_to(view.stamp(), destination, move |result| {
                    if let Some(owner) = weak.upgrade()
                        && let Err(error) = result
                    {
                        owner.core.borrow_mut().fail_updates();
                        owner.notice(error);
                        owner.notify.notify_one();
                    }
                }) {
                    self.core.borrow_mut().fail_updates();
                    self.notice(error);
                }
            }
        }
    }
    pub(super) fn updates_sync_ui(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.closed.get() {
            return;
        }
        let view = self.connection.updates.borrow();
        let status = self.core.borrow().updates_status();
        let source = self.output_identity(&view).and_then(|id| {
            let core = self.core.borrow();
            let p = core.current(id, &view)?;
            let a = p.article.as_ref()?;
            Some(((p.generation, a.id), a.reply_count))
        });
        let connected = view.phase == pttstella::connection::Phase::Connected;
        drop(view);
        let paused = !self.allowed(&ui);
        let mut state = self.live_updates.borrow_mut();
        if let Some((scope, count)) = source {
            let enabled = status.as_ref().is_some_and(|s| s.enabled);
            state.observe(scope, count, enabled);
            if ui.get_article_document_height()
                - ui.get_article_body_height()
                - ui.get_article_offset_y()
                <= 2.
            {
                state.seen = state.total;
            }
        } else if state.scope.is_some()
            && status
                .as_ref()
                .is_none_or(|s| state.scope != Some((s.generation, s.article)))
        {
            *state = Presentation::default();
        }
        let same = status
            .as_ref()
            .is_some_and(|s| state.scope == Some((s.generation, s.article)));
        let status = status.filter(|_| same && ui.get_article_active() && connected);
        let unread = if status.is_some() {
            state.total.saturating_sub(state.seen)
        } else {
            0
        };
        let available = status.as_ref().is_some_and(|s| s.available);
        let controls = (status.clone(), paused, unread, available);
        if state.shown.as_ref() == Some(&controls) {
            return;
        }
        state.shown = Some(controls);
        drop(state);
        let enabled = status.as_ref().is_some_and(|s| s.enabled);
        let recovery = status.as_ref().is_some_and(|s| s.recovery);
        ui.set_article_updates_available(available);
        ui.set_article_updates_visible(
            enabled || recovery || status.as_ref().is_some_and(|s| s.failed) || unread > 0,
        );
        ui.set_article_updates_enabled(enabled);
        ui.set_article_updates_recovery(recovery);
        ui.set_article_updates_unread(unread as i32);
        ui.set_article_updates_toggle_label(
            if enabled {
                pttstella::msg!("article-updates-pause")
            } else {
                pttstella::msg!("article-updates-start")
            }
            .into(),
        );
        ui.set_article_updates_new_label(
            pttstella::msg!("article-updates-new", count = (unread as i64)).into(),
        );
        let label = if recovery {
            Some("article-updates-retained")
        } else if status.as_ref().is_some_and(|s| s.failed) {
            Some("article-updates-stopped")
        } else if !enabled {
            None
        } else if status.as_ref().is_some_and(|s| s.busy) {
            Some("article-updates-refreshing")
        } else if paused {
            Some("article-updates-paused")
        } else {
            Some("article-updates-waiting")
        };
        ui.set_article_updates_status(
            label
                .map_or_else(String::new, |id| match id {
                    "article-updates-retained" => pttstella::msg!("article-updates-retained"),
                    "article-updates-stopped" => pttstella::msg!("article-updates-stopped"),
                    "article-updates-refreshing" => pttstella::msg!("article-updates-refreshing"),
                    "article-updates-paused" => pttstella::msg!("article-updates-paused"),
                    _ => pttstella::msg!("article-updates-waiting"),
                })
                .into(),
        );
    }
    pub(super) fn updates_language_changed(&self) {
        self.live_updates.borrow_mut().shown = None;
        self.updates_sync_ui();
    }
    pub(super) fn updates_finish(&self) {
        self.live_updates.replace(Presentation::default());
        self.core.borrow_mut().fail_updates();
        let cancelled = self.core.borrow_mut().cancelled_update();
        if let Some(stamp) = cancelled
            && let Err(error) = self.connection.cancel_navigation(stamp)
        {
            self.notice(error);
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_article_updates_visible(false);
            ui.set_article_updates_available(false);
            ui.set_article_updates_enabled(false);
            ui.set_article_updates_recovery(false);
            ui.set_article_updates_unread(0);
            ui.set_article_updates_status("".into());
        }
    }
}
