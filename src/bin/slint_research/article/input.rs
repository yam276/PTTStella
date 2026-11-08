//! Native input consumes the shared keyboard result before the station writer.
use super::ArticleSession;
use pttstella::{
    article_cache,
    connection::View,
    navigation::{Context, Intent},
};
use tokio::time::Instant;

impl ArticleSession {
    pub fn input_context(&self, view: &View) -> Option<Context> {
        let ui = self.ui.upgrade()?;
        if self.closed.get()
            || !ui.get_article_active()
            || self.permit.get() != self.core.borrow().reading_scope()
        {
            return None;
        }
        self.core.borrow().reading_context(view)
    }

    pub fn local_intent(&self, intent: Intent) -> bool {
        let Some(ui) = self.ui.upgrade() else {
            return false;
        };
        if !ui.get_article_active() || !article_cache::local_intent(intent) {
            return false;
        }
        // A local key remains consumed while its projection is being replaced.
        // It must never become a station PageDown because its old scope expired.
        if !self.closed.get()
            && !ui.get_article_blocked()
            && self
                .input_context(&self.connection.updates.borrow())
                .is_some()
            && let Some(view) = self.view.borrow().as_ref()
        {
            view.navigate(intent);
        }
        true
    }

    /// Returns true for cache-only navigation. Otherwise invalidates the old
    /// article before the caller enqueues the actual user bytes exactly once.
    pub fn before_input(&self, view: &View, bytes: &[u8]) -> bool {
        if let Some(intent) = article_cache::reading_intent(bytes)
            && self.local_intent(intent)
        {
            return true;
        }
        self.station_handoff();
        self.core.borrow_mut().article_input(view, bytes);
        self.retire_stale_projection(view);
        if let Some(ui) = self.ui.upgrade() {
            ui.invoke_focus_terminal();
        }
        self.notify.notify_one();
        false
    }

    pub fn route_input(&self, generation: u64, bytes: Vec<u8>) -> Result<(), String> {
        let view = self.connection.updates.borrow().clone();
        if let Some(left) = self.input_trace_left.get().checked_sub(1) {
            self.input_trace_left.set(left);
            eprintln!(
                "native input bytes={} local={:?} active={} ready={}",
                bytes.len(),
                article_cache::reading_intent(&bytes),
                self.ui.upgrade().is_some_and(|ui| ui.get_article_active()),
                self.input_context(&view).is_some()
            );
        }
        if self.closed.get() || generation != view.generation {
            return Err(pttstella::msg!("navigation-stale"));
        }
        if self.before_input(&view, &bytes) {
            Ok(())
        } else {
            self.connection.input(generation, bytes)
        }
    }

    pub fn station_handoff(&self) {
        self.raw_generation
            .set(Some(self.connection.updates.borrow().generation));
        self.core.borrow_mut().interrupt_full_load();
        self.cancel_command();
        self.core.borrow_mut().set_raw_reading(true);
        if let Some(view) = self.view.borrow().as_ref() {
            view.hide();
        }
        self.shown.set(None);
        self.permit.set(None);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_article_station(true);
            ui.invoke_geometry_changed();
            ui.invoke_focus_terminal();
        }
        self.notify.notify_one();
    }

    pub fn resume_reading(&self) {
        if self.closed.get() {
            return;
        }
        self.composition_handoff.set(None);
        self.raw_generation.set(None);
        self.focus_after_paint.set(true);
        let view = self.connection.updates.borrow().clone();
        let mut core = self.core.borrow_mut();
        core.set_raw_reading(false);
        core.update(&view);
        drop(core);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_article_station(false);
        }
        self.next_paint.set(Instant::now());
        self.notify.notify_one();
    }

    /// The article's real TextInput owns its first preedit and candidate anchor.
    /// A committed raw input switches surfaces; cancellation stays in reading.
    pub fn begin_composition(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        if !ui.get_article_active() || self.closed.get() {
            return;
        }
        self.composition_handoff
            .set(Some(self.connection.updates.borrow().generation));
        self.notify.notify_one();
    }

    pub fn end_composition(&self) {
        if self.composition_handoff.replace(None).is_some() && !self.closed.get() {
            self.focus_after_paint.set(true);
            self.core.borrow_mut().display_changed();
            self.next_paint.set(Instant::now());
            self.notify.notify_one();
        }
    }
}
