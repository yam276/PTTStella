//! Native gestures lease the acknowledged presentation; the shared reducer pins
//! original rows and the Session performs the same Refresh/ListArticle protocol.
use super::ArticleSession;
use pttstella::{
    article_cache::{ArchiveSnapshot, LoadState},
    connection::{Phase, View},
    mouse::input::Route,
    navigation::{Context, Stamp},
    navigation_job::Destination,
    settings::ArticleMode,
    surface_state::Identity,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub identity: Identity,
    pub stamp: Stamp,
    pub article: Option<u64>,
    pub cell: Option<(usize, usize)>,
}
impl ArticleSession {
    pub fn bookmark_source(&self, view: &View, cell: Option<(usize, usize)>) -> Option<Source> {
        let ui = self.ui.upgrade()?;
        if self.busy.get()
            || self.closed.get()
            || !ui.get_native_foreground()
            || ui.get_article_station()
            || view.phase != Phase::Connected
            || !view.site.ptt_features()
            || view.input_pending
            || view.output_pending
            || view.login_pending
            || view.mouse.route != Route::Local
        {
            return None;
        }
        let identity = self.output_identity(view)?;
        let core = self.core.borrow();
        let paint = core.current(identity, view)?;
        if paint.article_mode != ArticleMode::Web || !core.navigation_shown(view.stamp()) {
            return None;
        }
        let article = if let Some((row, col)) = cell {
            if !matches!(view.context(), Context::ArticleList { articles, .. }
                if articles.iter().any(|a| a.row == row && a.bookmark && col >= a.title_start && col < view.frame.cols))
            {
                return None;
            }
            pttstella::board_navigation::current_board(&view.frame)?;
            None
        } else {
            let (id, first, _, load) = core.reading_progress()?;
            if first != 1
                || load != Some(LoadState::Complete)
                || !core.cached_reading(view.stamp(), view)
                || core.updates_holding()
                || core
                    .updates_status()
                    .is_none_or(|s| !s.available || s.busy || s.recovery)
            {
                return None;
            }
            Some(id)
        };
        Some(Source {
            identity,
            stamp: view.stamp(),
            article,
            cell,
        })
    }
    pub fn bookmark_begin(
        &self,
        source: Source,
        view: &View,
    ) -> Result<(ArchiveSnapshot, Destination), String> {
        if self.bookmark_source(view, source.cell) != Some(source) {
            return Err(pttstella::msg!("bookmarks-current-unavailable"));
        }
        let id = source
            .article
            .ok_or_else(|| pttstella::msg!("bookmarks-current-unavailable"))?;
        self.core
            .borrow_mut()
            .bookmark_current(source.stamp, id, view)
    }
    pub fn bookmark_list(&self, source: Source, view: &View) -> Result<Destination, String> {
        if self.bookmark_source(view, source.cell) != Some(source) || source.article.is_some() {
            return Err(pttstella::msg!("bookmarks-current-unavailable"));
        }
        Ok(Destination::ListArticle {
            board: pttstella::board_navigation::current_board(&view.frame)
                .ok_or_else(|| pttstella::msg!("bookmarks-current-unavailable"))?,
            row: source
                .cell
                .ok_or_else(|| pttstella::msg!("bookmarks-current-unavailable"))?
                .0,
        })
    }
    pub fn bookmark_observe_list(
        &self,
        capture: &mut pttstella::bookmarks::session::FromList,
        view: &View,
    ) -> Option<Result<(u64, pttstella::article_target::ArticleTarget), String>> {
        let gate = self
            .ui
            .upgrade()
            .filter(|ui| self.allowed(ui))
            .and(self.permit.get());
        let core = self.core.borrow();
        capture.observe(view, core.reading_scope(), core.reading_progress(), gate)
    }
    pub fn bookmark_failed(&self) -> bool {
        self.core
            .borrow()
            .updates_status()
            .is_none_or(|s| s.failed || s.recovery)
    }
    pub fn bookmark_abort(&self, stamp: Stamp, from_list: bool) -> Result<(), String> {
        if from_list {
            self.core.borrow_mut().stop_full_load();
        } else {
            self.core.borrow_mut().fail_updates();
        }
        let result = self.cancel_destination(stamp);
        self.notify.notify_one();
        result
    }
}
