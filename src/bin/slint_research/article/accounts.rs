//! Account actions consume the acknowledged common reducer. Only a reply
//! background change may rebase the existing article/inline presentation.
use super::ArticleSession;
use pttstella::{connection::View, filters, navigation::Stamp, surface_state::Identity};
impl ArticleSession {
    pub fn account_rule(
        &self,
        identity: Identity,
        stamp: Stamp,
        cell: (usize, usize),
        policy: filters::Policy,
        remove: bool,
    ) -> Option<Result<filters::Rules, String>> {
        let view = self.connection.updates.borrow().clone();
        (self.output_identity(&view) == Some(identity))
            .then(|| {
                self.core
                    .borrow()
                    .account_rule(identity, stamp, cell, policy, remove, &view)
            })
            .flatten()
    }
    pub fn focus_reply(&self, identity: Identity, stamp: Stamp, row: Option<usize>) -> bool {
        if self
            .ui
            .upgrade()
            .is_none_or(|ui| ui.get_context_source_blocked() || ui.get_app_closing())
        {
            return false;
        }
        let view = self.connection.updates.borrow().clone();
        if self.output_identity(&view) != Some(identity) {
            return false;
        }
        let old = self.core.borrow().reading_scope();
        if !self
            .core
            .borrow_mut()
            .focus_reply(identity, stamp, row, &view)
        {
            return false;
        }
        let new = self.core.borrow().reading_scope();
        if let (Some(old), Some(new)) = (old, new)
            && (old.0, old.1) == (new.0, new.1)
        {
            if let Some(native) = self.view.borrow().as_ref() {
                native.rebase_reply_focus(old, new);
            }
            if self.shown.get() == Some(old) {
                self.shown.set(Some(new));
            }
            if self.permit.get() == Some(old) {
                self.permit.set(Some(new));
            }
            if let Some(media) = self.presentation_media.borrow().upgrade() {
                media.rebase_reply_focus(old, new);
            }
        }
        self.presentation_changed(&view);
        true
    }
    pub fn toggle_reveal(&self) -> bool {
        let Some(ui) = self.ui.upgrade() else {
            return false;
        };
        if ui.get_context_source_blocked() || ui.get_app_closing() {
            return false;
        }
        let view = self.connection.updates.borrow().clone();
        let Some(identity) = self.output_identity(&view) else {
            return false;
        };
        let reveal = {
            let core = self.core.borrow();
            let Some(p) = core.current(identity, &view) else {
                return false;
            };
            if !p.ptt_features || p.masked_pushes == 0 || view.input_pending || view.login_pending {
                return false;
            }
            !p.revealed
        };
        if !self.core.borrow_mut().reveal(identity, reveal, &view) {
            return false;
        }
        self.presentation_changed(&view);
        true
    }
    pub(super) fn presentation_changed(&self, view: &View) {
        self.core.borrow_mut().update(view);
        self.retire_stale_projection(view);
        self.next_paint.set(tokio::time::Instant::now());
        if let Some(ui) = self.ui.upgrade() {
            ui.invoke_context_dismiss(false);
            ui.invoke_output_updated();
            ui.invoke_paste_updated();
            ui.invoke_keep_alive_changed();
            ui.invoke_inline_updated();
        }
        self.notify.notify_one();
    }
}
