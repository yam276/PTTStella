//! UI projection ownership: one paint in flight, one acknowledged snapshot and
//! interactions that can only consume a current acknowledged presentation.
#[cfg(test)]
#[path = "surface_state_tests.rs"]
mod tests;
use super::presentation::{Paint, current_presentation, paint};
use pttstella::{
    connection::{Phase, View},
    export, filters, links,
    media::hover::Lease,
    navigation::{Context, Stamp},
    selection::{self, SelectedText, Selection},
    settings::{ArticleMode, DisplaySettings},
};
use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// A snapshot may be saved only while its UI policy and source still match.
#[derive(Clone)]
pub(super) struct OutputLease {
    identity: Identity,
    revoked: Arc<AtomicBool>,
    input_epoch: Option<u64>,
}
impl OutputLease {
    pub(super) fn check(&self, view: &View) -> Result<(), String> {
        if view.output_pending
            || self
                .input_epoch
                .is_some_and(|epoch| epoch != view.input_epoch)
            || self.revoked.load(Ordering::Acquire)
            || (view.generation, view.frame.revision)
                != (self.identity.generation, self.identity.revision)
        {
            Err(pttstella::msg!("export-stale"))
        } else {
            Ok(())
        }
    }
}
struct ActiveOutput {
    lease: OutputLease,
    selection: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Identity {
    pub presentation: u64,
    pub generation: u64,
    pub revision: u64,
}
impl Identity {
    fn of(paint: &Paint) -> Self {
        Self {
            presentation: paint.presentation,
            generation: paint.generation,
            revision: paint.frame.revision,
        }
    }
}
struct SelectedRows {
    identity: Identity,
    request: u64,
    rows: Range<usize>,
    text: String,
    spans: Vec<selection::Span>,
    input_epoch: u64,
}
pub(super) struct SurfaceState {
    article: super::article_cache::ArticleCache,
    captured: Option<(Stamp, u64)>,
    raw_reading: bool,
    reading_reset: Option<Stamp>,
    reply_focus: Option<(u64, Option<u64>, String)>,
    article_mode: ArticleMode,
    article_boundary_navigation: bool,
    filters: filters::State,
    in_flight: Option<Paint>,
    displayed: Option<Paint>,
    dirty: bool,
    selected: Option<SelectedRows>,
    hover: Option<Lease>,
    output: Option<ActiveOutput>,
}
impl SurfaceState {
    pub(super) fn new(rules: filters::Rules) -> Self {
        Self {
            article: super::article_cache::ArticleCache::default(),
            captured: None,
            raw_reading: false,
            reading_reset: None,
            reply_focus: None,
            article_mode: ArticleMode::default(),
            article_boundary_navigation: false,
            filters: filters::State::new(rules),
            in_flight: None,
            displayed: None,
            dirty: true,
            selected: None,
            hover: None,
            output: None,
        }
    }
    pub(super) fn presentation(&self) -> u64 {
        self.filters.version()
    }
    pub(super) fn rules(&self) -> &filters::Rules {
        self.filters.rules()
    }
    pub(super) fn has_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }
    pub(super) fn dirty(&self) -> bool {
        self.dirty
    }
    pub(super) fn display_changed(&mut self) {
        self.revoke_output();
        self.dirty = true;
    }
    pub(super) fn queue(
        &mut self,
        view: &View,
        probe: bool,
        settings: &DisplaySettings,
    ) -> Option<&Paint> {
        if !self.dirty || self.in_flight.is_some() {
            return None;
        }
        self.capture_reading(view);
        let mut packet = paint(view, probe, settings, &self.filters);
        packet.article_mode = self.article_mode;
        packet.article_boundary_navigation = self.article_boundary_navigation;
        let source_replies = packet.replies.clone();
        if !self.raw_reading && self.article_mode == ArticleMode::Web {
            self.article.render(&mut packet);
        }
        if self
            .reply_focus
            .as_ref()
            .is_some_and(|(generation, article, _)| {
                *generation != view.generation || *article != packet.article.as_ref().map(|a| a.id)
            })
        {
            self.reply_focus = None;
            self.focus_changed();
            packet.presentation = self.filters.version();
        }
        super::presentation::merge_replies(&mut packet, &source_replies);
        let visible_reply_rows: std::collections::HashSet<_> = packet
            .frame
            .glyphs
            .iter()
            .filter(|g| g.text != "█" && g.text != " ")
            .map(|g| g.row)
            .collect();
        packet.reply_rows = packet
            .replies
            .iter()
            .filter(|p| visible_reply_rows.contains(&p.row))
            .map(|p| p.row)
            .collect();
        if let Some((_, _, id)) = &self.reply_focus {
            filters::focus_pushes(&mut packet.frame, &packet.replies, id);
            if let Some(source) = &mut packet.source_frame {
                filters::focus_pushes(source, &source_replies, id);
            }
            packet.reply_focus = Some(id.clone());
        }
        if view.site.ptt_features() {
            packet.account_words = match packet.navigation {
                Context::ArticleReading { .. } => filters::account_words(&packet.frame),
                Context::ArticleList { .. } => filters::article_authors(&view.frame, &packet.frame),
                _ => Vec::new(),
            };
        }
        let authors: std::collections::HashMap<_, _> = packet
            .replies
            .iter()
            .map(|reply| (reply.row, reply.id.as_str()))
            .collect();
        packet.reply_author_words = packet
            .account_words
            .iter()
            .enumerate()
            .filter(|(_, word)| {
                word.start == 3
                    && authors
                        .get(&word.row)
                        .is_some_and(|id| id.eq_ignore_ascii_case(&word.id))
            })
            .map(|(index, _)| index)
            .collect();
        for word in &mut packet.account_words {
            word.policy = self
                .rules()
                .entries
                .iter()
                .find(|entry| entry.id.eq_ignore_ascii_case(&word.id))
                .map(|entry| entry.policy);
        }
        super::presentation::refresh_links(&mut packet);
        packet.hover = self
            .hover
            .as_ref()
            .filter(|lease| lease.stamp == view.stamp())
            .cloned();
        self.in_flight = Some(packet);
        self.dirty = false;
        self.in_flight.as_ref()
    }
    fn focus_changed(&mut self) {
        self.filters.touch();
        self.article.focus_version(self.filters.version());
        self.revoke_output();
        self.selected = None;
        self.hover = None;
        self.dirty = true;
    }
    fn account_paint(&self, identity: Identity, stamp: Stamp, view: &View) -> Option<&Paint> {
        self.current(identity, view).filter(|p| {
            view.site.ptt_features()
                && view.phase == Phase::Connected
                && !view.input_pending
                && !view.login_pending
                && stamp == view.stamp()
                && p.input_epoch == stamp.input_epoch
                && view.mouse.route == pttstella::mouse::input::Route::Local
                && matches!(
                    p.navigation,
                    Context::ArticleReading { .. } | Context::ArticleList { .. }
                )
        })
    }
    pub(super) fn account_rule(
        &self,
        identity: Identity,
        stamp: Stamp,
        cell: (usize, usize),
        policy: filters::Policy,
        remove: bool,
        view: &View,
    ) -> Option<Result<filters::Rules, String>> {
        let (row, col) = cell;
        let word = self
            .account_paint(identity, stamp, view)?
            .account_words
            .iter()
            .find(|w| w.row == row && w.start <= col && col < w.end)?;
        let note = self
            .rules()
            .entries
            .iter()
            .find(|e| e.id.eq_ignore_ascii_case(&word.id))
            .map_or("", |e| e.note.as_str());
        if remove && word.policy != Some(policy) {
            return None;
        }
        Some(self.rules().edit_policy(&word.id, note, policy, remove))
    }
    pub(super) fn focus_reply(
        &mut self,
        identity: Identity,
        stamp: Stamp,
        row: Option<usize>,
        view: &View,
    ) -> bool {
        let Some(paint) = self
            .account_paint(identity, stamp, view)
            .filter(|p| matches!(p.navigation, Context::ArticleReading { .. }))
        else {
            return false;
        };
        let next = if let Some(row) = row {
            if !paint.reply_rows.contains(&row) {
                return false;
            }
            let Some(reply) = paint.replies.iter().find(|p| p.row == row) else {
                return false;
            };
            if self
                .reply_focus
                .as_ref()
                .is_some_and(|(_, _, id)| id.eq_ignore_ascii_case(&reply.id))
            {
                None
            } else {
                Some((
                    view.generation,
                    paint.article.as_ref().map(|a| a.id),
                    reply.id.clone(),
                ))
            }
        } else {
            None
        };
        self.reply_focus = next;
        self.focus_changed();
        true
    }
    pub(super) fn acknowledge(&mut self, identity: Identity) -> Result<(), String> {
        if self.in_flight.as_ref().map(Identity::of) != Some(identity) {
            return Err(pttstella::msg!("paint-unknown-version"));
        }
        self.displayed = self.in_flight.take();
        Ok(())
    }
    pub(super) fn painted(&self, view: &View) -> bool {
        !view.output_pending
            && self.displayed.as_ref().is_some_and(|p| {
                (p.generation, p.frame.revision) == (view.generation, view.frame.revision)
            })
    }
    pub(super) fn current(&self, identity: Identity, view: &View) -> Option<&Paint> {
        self.displayed.as_ref().filter(|p| {
            p.article_mode == self.article_mode
                && current_presentation(
                    p,
                    identity.presentation,
                    identity.generation,
                    identity.revision,
                    &self.filters,
                    view,
                )
        })
    }
    pub(super) fn inline_page(
        &self,
        identity: Identity,
        stamp: Stamp,
        view: &View,
    ) -> Option<&Paint> {
        self.current(identity, view).filter(|p| {
            self.inline_identity(view) == Some(identity)
                && self.navigation_shown(stamp)
                && stamp == view.stamp()
                && p.mouse.route == pttstella::mouse::input::Route::Local
        })
    }
    pub(super) fn inline_identity(&self, view: &View) -> Option<Identity> {
        if self.article_mode == ArticleMode::Terminal {
            return None;
        }
        let p = self.displayed.as_ref()?;
        self.current(Identity::of(p), view)
            .filter(|_| {
                view.phase == Phase::Connected
                    && !view.input_pending
                    && !view.login_pending
                    && view.mouse.route == pttstella::mouse::input::Route::Local
                    && view.frame.control_complete
            })
            .map(Identity::of)
    }
    pub(super) fn navigation_shown(&self, stamp: Stamp) -> bool {
        self.displayed.as_ref().is_some_and(|p| {
            p.article_mode == self.article_mode
                && p.presentation == self.filters.version()
                && !p.output_pending
                && p.generation == stamp.generation
                && p.frame.revision == stamp.revision
                && p.input_epoch == stamp.input_epoch
        })
    }
    pub(super) fn mouse_shown(&self, event: &pttstella::mouse::input::Event) -> bool {
        use pttstella::mouse::{Action, input::Route};
        event.report.action == Action::Release
            || self.navigation_shown(event.lease.stamp)
                && self.displayed.as_ref().is_some_and(|p| {
                    p.mouse.route == Route::Station
                        && p.mouse.modes.epoch() == event.lease.mode_epoch
                        && p.mouse.policy_epoch == event.lease.policy_epoch
                })
    }
    pub(super) fn mouse_sample_shown(&self, sample: &pttstella::mouse::input::Sample) -> bool {
        self.displayed.as_ref().is_some_and(|p| {
            !p.output_pending
                && p.mouse.route == pttstella::mouse::input::Route::Station
                && p.generation == sample.lease.stamp.generation
                && p.mouse.modes.epoch() == sample.lease.mode_epoch
                && p.mouse.policy_epoch == sample.lease.policy_epoch
                && (self.navigation_shown(sample.lease.stamp)
                    || p.mouse.active == Some(sample.lease))
        })
    }
    /// Vim may advance its logical cursor before the next terminal redraw.
    pub(super) fn browse_shown(&self, generation: u64, context: &Context) -> bool {
        self.displayed.as_ref().is_some_and(|p| {
            !p.output_pending && p.generation == generation && p.navigation.kind() == context.kind()
        })
    }
    pub(super) fn set_article_boundary_navigation(&mut self, enabled: bool) {
        if self.article_boundary_navigation != enabled {
            self.article_boundary_navigation = enabled;
            self.dirty = true;
        }
    }
    pub(super) fn adjacent_article_allowed(&self, stamp: Stamp, down: bool, view: &View) -> bool {
        self.article_boundary_navigation
            && stamp == view.stamp()
            && self.cached_reading(stamp, view)
            && !view.input_pending
            && !view.output_pending
            && self
                .displayed
                .as_ref()
                .and_then(|p| p.article.as_ref())
                .is_some_and(|article| {
                    article.load != Some(super::article_cache::LoadState::Loading)
                        && if down {
                            article.end
                        } else {
                            article.first_line == 1
                        }
                })
    }
    pub(super) fn set_article_mode(&mut self, mode: ArticleMode) -> bool {
        if self.article_mode == mode {
            return false;
        }
        self.article_mode = mode;
        self.replace_rules(self.filters.rules().clone());
        true
    }
    pub(super) fn set_raw_reading(&mut self, raw: bool) -> bool {
        if self.raw_reading == raw {
            return false;
        }
        self.raw_reading = raw;
        self.filters.touch();
        self.policy_changed();
        true
    }
    pub(super) fn reading_context(&self, view: &View) -> Option<Context> {
        if self.article_mode == ArticleMode::Terminal {
            return None;
        }
        let paint = self.displayed.as_ref()?;
        let article = paint.article.as_ref()?;
        (view.phase == Phase::Connected
            && view.mouse.route == pttstella::mouse::input::Route::Local
            && paint.generation == view.generation
            && paint.presentation == self.filters.version()
            && self.article.active())
        .then_some(Context::ArticleReading {
            first_line: article.first_line,
            last_line: article.last_line,
            percent: if article.end { 100 } else { 0 },
        })
    }
    pub(super) fn reading_move(
        &self,
        view: &View,
        intent: pttstella::navigation::Intent,
    ) -> Option<super::outbound::Packet<'static>> {
        self.reading_context(view)?;
        if !super::article_cache::local_intent(intent) {
            return None;
        }
        let article = self.displayed.as_ref()?.article.as_ref()?;
        Some(super::outbound::Packet::ReadingMove {
            generation: view.generation,
            article: article.id,
            intent,
        })
    }
    pub(super) fn cached_reading(&self, stamp: Stamp, view: &View) -> bool {
        self.navigation_shown(stamp)
            && self.inline_identity(view).is_some()
            && self.displayed.as_ref().is_some_and(|p| p.article.is_some())
    }
    pub(super) fn article_navigation(
        &mut self,
        intent: pttstella::navigation::Intent,
        view: &View,
    ) {
        let context = view.context();
        if let Ok(bytes) = pttstella::navigation::plan(&context, intent) {
            self.article_input(view, &bytes);
        }
        self.article
            .begin_fetch(intent, &context, view.frame.revision);
    }
    pub(super) fn cancel_article_fetch(&mut self) {
        self.article.cancel_fetch();
    }
    pub(super) fn reading_progress(
        &self,
    ) -> Option<(u64, usize, usize, Option<super::article_cache::LoadState>)> {
        self.article.progress()
    }
    pub(super) fn full_load_active(&self) -> bool {
        self.article.loading()
    }
    pub(super) fn full_load_deadline(&self) -> Option<tokio::time::Instant> {
        self.article.load_deadline()
    }
    pub(super) fn expire_full_load(&mut self, now: tokio::time::Instant) {
        self.dirty |= self.article.expire_load(now);
    }
    pub(super) fn stop_full_load(&mut self) {
        let old = self.article.load_state();
        self.article
            .stop_load(super::article_cache::LoadState::Stopped);
        self.dirty |= old != self.article.load_state();
    }
    pub(super) fn interrupt_full_load(&mut self) {
        let old = self.article.load_state();
        self.article
            .stop_load(super::article_cache::LoadState::Cancelled);
        self.dirty |= old != self.article.load_state();
    }
    pub(super) fn full_load_action(
        &mut self,
        generation: u64,
        id: u64,
        resume: bool,
        view: &View,
    ) -> bool {
        if generation != view.generation
            || self.article_mode != ArticleMode::Web
            || self.raw_reading
            || resume
                && (view.phase != Phase::Connected
                    || !matches!(view.context(), Context::ArticleReading { .. }))
        {
            return false;
        }
        let changed = self.article.load_action(id, resume);
        self.dirty |= changed;
        changed
    }
    fn capture_reading(&mut self, view: &View) {
        if let Some(previous) = self.reading_reset {
            if previous.generation == view.generation
                && (view.input_epoch <= previous.input_epoch
                    || view.frame.revision <= previous.revision)
            {
                return;
            }
            self.reading_reset = None;
        }
        if self.article_mode != ArticleMode::Web
            || self.raw_reading
            || view.input_pending
            || view.output_pending
            || view.login_pending
            || matches!(
                view.destination,
                pttstella::navigation_job::Progress::Active { .. }
            )
            || !view.frame.control_complete
        {
            return;
        }
        let identity = (view.stamp(), self.filters.version());
        if self.captured == Some(identity) {
            return;
        }
        let packet = paint(view, false, &DisplaySettings::default(), &self.filters);
        self.article.capture(&packet);
        self.captured = Some(identity);
    }
    pub(super) fn reading_scope(&self) -> Option<(u64, u64, u64)> {
        if self.article_mode != ArticleMode::Web || self.raw_reading {
            return None;
        }
        let (article, _, _, _) = self.article.progress()?;
        Some((self.captured?.0.generation, article, self.filters.version()))
    }
    pub(super) fn authorize_reading(&self, stamp: Stamp, article: u64) -> Option<(u64, u64, u64)> {
        let scope = self.reading_scope()?;
        if (scope.0, scope.1) != (stamp.generation, article) {
            return None;
        }
        self.displayed
            .iter()
            .chain(self.in_flight.iter())
            .any(|p| {
                p.presentation == scope.2
                    && p.generation == stamp.generation
                    && p.frame.revision == stamp.revision
                    && p.input_epoch == stamp.input_epoch
                    && p.article.as_ref().is_some_and(|a| a.id == article)
            })
            .then_some(scope)
    }
    pub(super) fn next_full_page(&mut self, view: &View) -> Option<pttstella::navigation::Intent> {
        if self.article_mode != ArticleMode::Web
            || self.raw_reading
            || view.phase != Phase::Connected
            || view.input_pending
            || view.output_pending
            || self.captured != Some((view.stamp(), self.filters.version()))
        {
            return None;
        }
        let old = self.article.load_state();
        let intent = self.article.next_full_page(
            &view.context(),
            view.frame.revision,
            tokio::time::Instant::now(),
        );
        self.dirty |= old != self.article.load_state();
        intent
    }
    pub(super) fn continue_article_fetch(
        &mut self,
        view: &View,
    ) -> Option<pttstella::navigation::Intent> {
        if !self.cached_reading(view.stamp(), view) || self.has_in_flight() || self.dirty {
            return None;
        }
        self.article
            .continue_fetch(&view.context(), view.frame.revision)
    }
    pub(super) fn clear_article(&mut self) {
        if self.article.active() || self.reply_focus.is_some() {
            self.filters.touch();
            self.policy_changed();
        } else {
            self.article.clear();
            self.captured = None;
        }
    }
    pub(super) fn article_input(&mut self, view: &View, bytes: &[u8]) {
        let context = view.context();
        if !bytes.is_empty() && !super::article_cache::reading_input(&context, bytes) {
            self.clear_article();
            // Enqueueing input is asynchronous. Never seed the new cache from
            // the unchanged view before the session owner publishes that input.
            self.reading_reset = Some(view.stamp());
        }
        self.article.input(&context, bytes);
    }
    fn policy_changed(&mut self) {
        self.reply_focus = None;
        self.article.clear();
        self.captured = None;
        self.revoke_output();
        self.selected = None;
        self.hover = None;
        self.dirty = true;
    }
    pub(super) fn replace_rules(&mut self, rules: filters::Rules) {
        self.filters.replace(rules);
        self.policy_changed();
    }
    pub(super) fn reveal(&mut self, identity: Identity, reveal: bool, view: &View) -> bool {
        if view.output_pending
            || self.filters.version() != identity.presentation
            || (view.generation, view.frame.revision) != (identity.generation, identity.revision)
        {
            return false;
        }
        self.filters
            .reveal(identity.generation, identity.revision, reveal);
        self.policy_changed();
        true
    }
    /// Even an old in-flight ACK must be drained before queuing the newest view.
    /// Its snapshot stays ineligible for current interactions after this update.
    pub(super) fn update(&mut self, view: &View) -> bool {
        self.article.observe(view);
        if (self
            .reply_focus
            .as_ref()
            .is_some_and(|(generation, _, _)| *generation != view.generation)
            || view.phase != Phase::Connected
            || !view.site.ptt_features()
            || (!view.output_pending
                && !view.input_pending
                && view.frame.control_complete
                && !matches!(view.context(), Context::ArticleReading { .. })))
            && self.reply_focus.take().is_some()
        {
            self.focus_changed();
        }
        let version = self.filters.version();
        self.filters.sync(view.generation, view.frame.revision);
        if self.output.as_ref().is_some_and(|active| {
            active.lease.identity.presentation != self.filters.version()
                || active.lease.check(view).is_err()
        }) {
            self.revoke_output();
        }
        if view.output_pending
            || self.filters.version() != version
            || self.selected.as_ref().is_some_and(|s| {
                s.input_epoch != view.input_epoch
                    || (s.identity.generation, s.identity.revision)
                        != (view.generation, view.frame.revision)
            })
        {
            self.selected = None;
        }
        let mut filtered = self.filters.project(view.generation, &view.frame);
        if view.site.ptt_features() {
            filters::merge_push_ids(&mut filtered.frame, &filters::pushes(&view.frame));
        }
        if let Some((_, _, id)) = &self.reply_focus {
            filters::focus_pushes(&mut filtered.frame, &filters::pushes(&view.frame), id);
        }
        let revoked = self.hover.as_mut().is_some_and(|lease| {
            !lease.renew(
                view.stamp(),
                view.phase == Phase::Connected
                    && view.mouse.route == pttstella::mouse::input::Route::Local
                    && !view.output_pending
                    && view.frame.control_complete,
                &links::detect(&filtered.frame),
            )
        });
        if revoked {
            self.hover = None;
        }
        self.capture_reading(view);
        self.dirty = true;
        revoked
    }
    pub(super) fn select(
        &mut self,
        identity: Identity,
        request: u64,
        selection: &Selection,
        view: &View,
    ) -> Option<SelectedText> {
        let selected = self
            .current(identity, view)
            .and_then(|p| selection::extract(&p.frame, selection).ok());
        if self.output.as_ref().is_some_and(|active| {
            active.selection.is_some_and(|old| {
                old != request || active.lease.identity != identity || selected.is_none()
            })
        }) {
            self.revoke_output();
        }
        self.selected = selected.as_ref().map(|s| SelectedRows {
            identity,
            request,
            rows: s.rows.clone(),
            text: s.text.clone(),
            spans: s.spans.clone(),
            input_epoch: view.input_epoch,
        });
        selected
    }
    pub(super) fn copied(&self, identity: Identity, request: u64, view: &View) -> bool {
        self.selected.as_ref().is_some_and(|s| {
            s.identity == identity && s.request == request && s.input_epoch == view.input_epoch
        }) && self.current(identity, view).is_some()
    }
    pub(super) fn editor_selection(
        &self,
        identity: Identity,
        request: u64,
        view: &View,
    ) -> Option<selection::Selection> {
        if !self.copied(identity, request, view) || !self.current(identity, view)?.editor {
            return None;
        }
        let spans = &self.selected.as_ref()?.spans;
        let first = spans.first()?;
        let last = spans.last()?;
        Some(selection::Selection::Range {
            anchor: selection::Position {
                row: first.row,
                col: first.start,
            },
            focus: selection::Position {
                row: last.row,
                col: last.end.checked_sub(1)?,
            },
        })
    }
    pub(super) fn copy_text(
        &self,
        identity: Identity,
        target: &super::protocol::CopyTarget,
        view: &View,
    ) -> Option<String> {
        let paint = self.current(identity, view)?;
        match target {
            super::protocol::CopyTarget::SelectionAnsi { request } => self
                .selected
                .as_ref()
                .filter(|_| self.copied(identity, *request, view))
                .map(|selected| {
                    selection::ansi(&paint.frame, &selected.spans, selected.rows.clone())
                }),
            super::protocol::CopyTarget::Selection { request } => self
                .selected
                .as_ref()
                .filter(|_| self.copied(identity, *request, view))
                .map(|selected| selected.text.clone()),
            super::protocol::CopyTarget::Link { index } => {
                paint.links.get(*index).map(|link| link.url.clone())
            }
        }
    }
    pub(super) fn export_rows(
        &self,
        identity: Identity,
        request: Option<u64>,
        view: &View,
    ) -> Result<Range<usize>, String> {
        let shown = self
            .current(identity, view)
            .ok_or(pttstella::msg!("export-screen-changed"))?;
        if (
            shown.frame.cols,
            shown.source_frame.as_ref().unwrap_or(&shown.frame).rows,
        ) != (80, export::ROWS)
        {
            return Err(pttstella::msg!("export-grid-unsupported"));
        }
        match request {
            None => Ok(0..export::ROWS),
            Some(request) => self
                .selected
                .as_ref()
                .filter(|s| {
                    s.identity == identity
                        && s.request == request
                        && s.input_epoch == view.input_epoch
                })
                .filter(|s| s.rows.len() <= export::ROWS)
                .map(|s| s.rows.clone())
                .ok_or_else(|| pttstella::msg!("export-selection-changed")),
        }
    }
    pub(super) fn clear_selection(&mut self, identity: Identity, request: u64) {
        if self
            .selected
            .as_ref()
            .is_some_and(|s| s.identity == identity && s.request == request)
        {
            self.selected = None;
            if self.output.as_ref().is_some_and(|active| {
                active.selection == Some(request) && active.lease.identity == identity
            }) {
                self.revoke_output();
            }
        }
    }
    pub(super) fn begin_hover(&mut self, lease: Lease) {
        self.hover = Some(lease);
    }
    pub(super) fn revoke_output(&mut self) {
        if let Some(active) = self.output.take() {
            active.lease.revoked.store(true, Ordering::Release);
        }
    }
    /// Called inside Files' admitted factory, so a busy rejection never revokes
    /// the accepted output. Callers validate export rows or copy targets first.
    pub(super) fn lease_output(
        &mut self,
        identity: Identity,
        selection: Option<u64>,
        input_epoch: u64,
    ) -> OutputLease {
        self.revoke_output();
        let lease = OutputLease {
            identity,
            revoked: Arc::new(AtomicBool::new(false)),
            input_epoch: selection.map(|_| input_epoch),
        };
        self.output = Some(ActiveOutput {
            lease: lease.clone(),
            selection,
        });
        lease
    }
    pub(super) fn lease_picture(&mut self, identity: Identity, epoch: u64) -> OutputLease {
        let mut lease = self.lease_output(identity, None, epoch);
        lease.input_epoch = Some(epoch);
        lease
    }
    pub(super) fn clear_hover(&mut self) -> bool {
        self.hover.take().is_some()
    }
    pub(super) fn cancel_hover(&mut self, sequence: u64) -> bool {
        if self
            .hover
            .as_ref()
            .is_some_and(|lease| lease.sequence == sequence)
        {
            self.hover = None;
            true
        } else {
            false
        }
    }
}
impl Drop for SurfaceState {
    fn drop(&mut self) {
        self.revoke_output();
    }
}
