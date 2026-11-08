//! Native focus/debounce adapter. The shared surface owns all searchable text.
use super::ArticleSession;
use crate::{
    TerminalWindow,
    renderer::{Renderer, Size},
};
use pttstella::{
    article_find::{self, Action, Source},
    navigation::Context,
    surface_state::Identity,
};
use slint::{Rgba8Pixel, SharedPixelBuffer, TimerMode};
use std::{rc::Rc, time::Duration};

impl ArticleSession {
    pub(super) fn install_find(self: &Rc<Self>, ui: &TerminalWindow) {
        ui.set_find_title(pttstella::msg!("article-find-title").into());
        ui.set_find_placeholder(pttstella::msg!("article-find-placeholder").into());
        ui.set_find_hint(pttstella::msg!("article-find-hint").into());
        ui.set_find_previous_label(pttstella::msg!("article-find-previous").into());
        ui.set_find_next_label(pttstella::msg!("article-find-next").into());
        ui.set_find_close_label(pttstella::msg!("article-find-close").into());
        ui.on_find_action({
            let owner = Rc::downgrade(self);
            move |action| {
                if let Some(owner) = owner.upgrade() {
                    owner.find_action(action);
                }
            }
        });
        ui.on_find_edit({
            let owner = Rc::downgrade(self);
            move |text| {
                if let Some(owner) = owner.upgrade() {
                    owner.find_edit(text.as_str());
                }
            }
        });
        ui.on_find_composition_changed({
            let owner = Rc::downgrade(self);
            move || {
                if let Some(owner) = owner.upgrade() {
                    owner.find_pause_changed();
                }
            }
        });
        ui.on_find_state_changed({
            let owner = Rc::downgrade(self);
            move || {
                if let Some(owner) = owner.upgrade() {
                    owner.find_pause_changed();
                }
            }
        });
    }
    fn find_pause_changed(self: &Rc<Self>) {
        self.find_timer.stop();
        if self.find_draft.borrow().is_some()
            && self.ui.upgrade().is_some_and(|ui| {
                ui.get_find_open() && !ui.get_find_blocked() && !ui.get_find_composing()
            })
        {
            self.find_schedule();
        }
    }
    fn find_command(&self, action: Action) -> bool {
        if self.closed.get() {
            return false;
        }
        let view = self.connection.updates.borrow().clone();
        let mut core = self.core.borrow_mut();
        let Some(identity) = core.last_displayed().map(Identity::of) else {
            return false;
        };
        let request = self.find_serial.get().saturating_add(1);
        self.find_serial.set(request);
        core.find_action(identity, view.stamp(), request, action, &view)
    }
    fn find_action(self: &Rc<Self>, action: i32) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if self.closed.get() || ui.get_find_composing() || ui.get_find_blocked() {
            return;
        }
        match action {
            0 if ui.get_find_available() => {
                ui.invoke_context_dismiss(false);
                ui.invoke_tools_dismiss(false);
                ui.invoke_paste_cancel();
                ui.invoke_clear_selection();
                ui.invoke_reset_keyboard();
                if self.find_command(Action::Open) {
                    self.find_sync_ui();
                    ui.invoke_focus_find(true);
                }
            }
            1 | 2 if ui.get_find_open() => {
                if self.find_draft.borrow().is_some() {
                    self.find_timer.stop();
                    self.find_apply_draft();
                } else {
                    self.find_command(Action::Step { back: action == 1 });
                    self.find_sync_ui();
                }
                ui.invoke_focus_find(false);
            }
            3 if ui.get_find_open() => {
                self.find_timer.stop();
                self.find_draft.take();
                self.core.borrow_mut().find_finish();
                self.find_sync_ui();
                ui.invoke_focus_terminal();
            }
            _ => {}
        }
    }
    fn find_edit(self: &Rc<Self>, text: &str) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if self.closed.get() || !ui.get_find_open() || ui.get_find_blocked() {
            return;
        }
        let query = article_find::bounded_query(text);
        if query != text {
            ui.set_find_query(query.as_str().into());
        }
        self.find_draft.replace(Some(query));
        self.find_timer.stop();
        self.find_command(Action::Pending);
        self.find_sync_ui();
        if !ui.get_find_composing() {
            self.find_schedule();
        }
    }
    fn find_schedule(self: &Rc<Self>) {
        let owner = Rc::downgrade(self);
        self.find_timer.start(
            TimerMode::SingleShot,
            Duration::from_millis(120),
            move || {
                if let Some(owner) = owner.upgrade() {
                    owner.find_apply_draft();
                }
            },
        );
    }
    pub(super) fn find_retry_draft(&self) {
        // A debounce may expire while a metadata/source paint awaits ACK.
        // Keep the bounded draft and apply once that same source is confirmed.
        if !self.find_timer.running() && self.find_draft.borrow().is_some() {
            self.find_apply_draft();
        }
    }
    fn find_apply_draft(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if !ui.get_find_open() || ui.get_find_composing() || ui.get_find_blocked() {
            return;
        }
        let Some(query) = self.find_draft.borrow().clone() else {
            return;
        };
        if self.find_command(Action::Edit { query }) {
            self.find_draft.take();
            self.find_sync_ui();
        }
    }
    pub(super) fn find_sync_ui(&self) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        let (available, version, opened, query, status, any, jump) = {
            let core = self.core.borrow();
            let view = self.connection.updates.borrow();
            let available = core.last_displayed().is_some_and(|p| {
                p.generation == view.generation
                    && p.ptt_features
                    && matches!(p.navigation, Context::ArticleReading { .. })
            }) && view.phase == pttstella::connection::Phase::Connected;
            let find = core.find();
            let reply = find.reply(find.data_version());
            let status = if find.query().is_empty() && self.find_draft.borrow().is_none() {
                pttstella::msg!("article-find-empty")
            } else if reply.total == 0 {
                pttstella::msg!("article-find-none")
            } else if reply.limited {
                pttstella::msg!(
                    "article-find-count-limited",
                    current = (reply.active.map_or(0, |n| n + 1) as i64),
                    total = (reply.total as i64)
                )
            } else {
                pttstella::msg!(
                    "article-find-count",
                    current = (reply.active.map_or(0, |n| n + 1) as i64),
                    total = (reply.total as i64)
                )
            };
            (
                available,
                find.version(),
                find.opened(),
                find.query().to_owned(),
                status,
                reply.total != 0,
                reply.jump.then(|| find.active()).flatten(),
            )
        };
        ui.set_find_available(available);
        if self.find_version.replace(version) == version {
            return;
        }
        let was_open = ui.get_find_open();
        let had_focus = ui.get_find_focused();
        ui.set_find_open(opened);
        ui.set_find_status(status.into());
        ui.set_find_has_matches(any);
        if !opened {
            self.find_timer.stop();
            self.find_draft.take();
            ui.set_find_query("".into());
            ui.set_find_composing(false);
        } else if self.find_draft.borrow().is_none() {
            ui.set_find_query(query.into());
        }
        // No core/view borrow may span callbacks: layout and ACK can re-enter.
        let article = self.view.borrow().clone();
        if let Some(article) = article.filter(|_| ui.get_article_active()) {
            article.find_changed();
            if let Some(hit) = jump {
                article.find_reveal(hit);
            }
        } else {
            ui.invoke_find_redraw();
            if let Some(hit) = jump {
                ui.invoke_find_terminal_reveal(hit.row as i32, hit.start as i32);
            }
        }
        if was_open && !opened && had_focus && ui.get_native_foreground() {
            ui.invoke_focus_terminal();
        }
        ui.invoke_article_session_updated();
        ui.invoke_keep_alive_changed();
    }
    pub fn paint_terminal_find(
        &self,
        renderer: &Renderer,
        size: Size,
        cursor: bool,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, &'static str> {
        let view = self.connection.updates.borrow();
        let core = self.core.borrow();
        let valid = core.last_displayed().is_some_and(|p| {
            p.article.is_none()
                && p.frame.revision == renderer.frame().revision
                && core.current(Identity::of(p), &view).is_some()
                && p.input_epoch == view.input_epoch
                && core.find().reply(0).source == Some(Source::of(p))
        });
        renderer.paint_with_find(size, cursor, valid.then_some(core.find()))
    }
}
