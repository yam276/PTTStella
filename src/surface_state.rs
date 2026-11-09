//! UI projection ownership: one paint in flight, one acknowledged snapshot and
//! interactions that can only consume a current acknowledged presentation.
use super::presentation::{Paint, current_presentation, paint};
use crate::{
    connection::{Phase, View},
    export, filters, links,
    media::hover::Lease,
    navigation::{Context, Stamp},
    selection::{self, SelectedText, Selection},
    settings::{ArticleMode, DisplaySettings},
};
use std::ops::Range;

/// A snapshot may be saved only while its UI policy and source still match.
#[derive(Clone)]
pub struct OutputLease {
    identity: Identity,
    source: export::Lease,
}
impl OutputLease {
    pub fn check(&self, view: &View) -> Result<(), String> {
        self.source.check(view)
    }
}
struct ActiveOutput {
    lease: OutputLease,
    selection: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub presentation: u64,
    pub generation: u64,
    pub revision: u64,
}
impl Identity {
    pub fn of(paint: &Paint) -> Self {
        Self {
            presentation: paint.presentation,
            generation: paint.generation,
            revision: paint.frame.revision,
        }
    }
}
/// Local reading intent; each host adapts it to its own drawing protocol.
#[derive(Clone, Copy, Debug)]
pub struct ReadingMove {
    pub generation: u64,
    pub article: u64,
    pub intent: crate::navigation::Intent,
}
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CopyTarget {
    Selection { request: u64 },
    SelectionAnsi { request: u64 },
    Link { index: usize },
}

struct SelectedRows {
    identity: Identity,
    request: u64,
    rows: Range<usize>,
    text: String,
    spans: Vec<selection::Span>,
    input_epoch: u64,
}
pub struct SurfaceState {
    article: super::article_cache::ArticleCache,
    updates: super::article_updates::Updates,
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
    find: crate::article_find::Find,
}
impl SurfaceState {
    pub fn new(rules: filters::Rules) -> Self {
        Self {
            article: super::article_cache::ArticleCache::default(),
            updates: super::article_updates::Updates::default(),
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
            find: Default::default(),
        }
    }
    /// Read-only retained projection; actions still require current()/leases.
    pub fn last_displayed(&self) -> Option<&Paint> {
        self.displayed.as_ref()
    }
    pub fn find(&self) -> &crate::article_find::Find {
        &self.find
    }
    pub fn find_finish(&mut self) {
        self.find.cancel();
    }
    pub fn find_action(
        &mut self,
        identity: Identity,
        stamp: Stamp,
        request: u64,
        action: crate::article_find::Action,
        view: &View,
    ) -> bool {
        if matches!(action, crate::article_find::Action::Close) {
            return identity.generation == view.generation
                && stamp.generation == view.generation
                && self.find.close(request);
        }
        if view.phase != Phase::Connected
            || !view.site.ptt_features()
            || stamp != view.stamp()
            || !self
                .current(identity, view)
                .is_some_and(|p| p.input_epoch == view.input_epoch)
            || view.input_pending
            || view.login_pending
        {
            return false;
        }
        let Some(paint) = self.displayed.as_ref() else {
            return false;
        };
        self.find.action(paint, request, action)
    }
    pub fn find_sync(&mut self, view: &View) {
        let valid = self.displayed.as_ref().is_some_and(|p| {
            self.current(Identity::of(p), view).is_some()
                && p.input_epoch == view.input_epoch
                && !view.input_pending
                && !view.login_pending
        });
        if valid {
            if let Some(paint) = &self.displayed {
                self.find.sync(paint);
            }
        } else {
            self.find.suspend();
        }
    }
    pub fn find_decorate(
        &self,
        source: &Paint,
        frame: &mut crate::screen::Frame,
        first_row: usize,
    ) {
        if let Some(p) = &self.displayed
            && (
                p.generation,
                p.frame.revision,
                p.presentation,
                p.input_epoch,
            ) == (
                source.generation,
                source.frame.revision,
                source.presentation,
                source.input_epoch,
            )
        {
            self.find
                .decorate(crate::article_find::Source::of(p), frame, first_row);
        }
    }
    pub fn pending_identity(&self) -> Option<Identity> {
        self.in_flight.as_ref().map(Identity::of)
    }
    /// A confirmed source page for the native row rasterizer, without cloning
    /// the full article projection retained for search, selection and exports.
    pub fn article_source(&self, view: &View, settings: &DisplaySettings) -> Option<Paint> {
        if self.captured != Some((view.stamp(), self.filters.version()))
            || self.article_mode != ArticleMode::Web
            || self.raw_reading
        {
            return None;
        }
        let full = self.in_flight.as_ref().or(self.displayed.as_ref())?;
        if (full.generation, full.frame.revision, full.presentation)
            != (view.generation, view.frame.revision, self.filters.version())
        {
            return None;
        }
        let mut source = paint(view, false, settings, &self.filters);
        source.frame = full.source_frame.as_ref()?.clone();
        super::presentation::refresh_links(&mut source);
        Some(source)
    }
    /// Native raster tiles consume the final acknowledged/in-flight projection,
    /// including merged IDs and reply focus. They cannot bypass a mask through
    /// the cache's earlier, pre-decoration rows.
    pub fn article_window(
        &self,
        source: &Paint,
        rows: Range<usize>,
    ) -> Option<crate::article_cache::Window> {
        let full = self.in_flight.as_ref().or(self.displayed.as_ref())?;
        if (full.generation, full.frame.revision, full.presentation)
            != (
                source.generation,
                source.frame.revision,
                source.presentation,
            )
            || full.article.is_none()
        {
            return None;
        }
        let total = full.frame.rows.saturating_sub(1);
        if rows.is_empty() || rows.start >= total {
            return None;
        }
        let rows = rows.start..rows.end.min(total);
        let mut window = self.article.window(source, rows.clone())?;
        let footer = full.frame.rows - 1;
        window.paint.frame.glyphs = full
            .frame
            .glyphs
            .iter()
            .filter(|g| rows.contains(&g.row) || g.row == footer)
            .cloned()
            .map(|mut g| {
                g.row = if g.row == footer {
                    rows.len()
                } else {
                    g.row - rows.start
                };
                g
            })
            .collect();
        window.paint.frame.hyperlinks = full
            .frame
            .hyperlinks
            .iter()
            .filter_map(|link| {
                let spans = link
                    .spans
                    .iter()
                    .filter(|span| rows.contains(&span.row) || span.row == footer)
                    .cloned()
                    .map(|mut span| {
                        span.row = if span.row == footer {
                            rows.len()
                        } else {
                            span.row - rows.start
                        };
                        span
                    })
                    .collect::<Vec<_>>();
                (!spans.is_empty()).then(|| crate::screen::hyperlinks::Hyperlink {
                    target: link.target.clone(),
                    spans,
                })
            })
            .collect();
        window.paint.replies = full
            .replies
            .iter()
            .filter(|p| rows.contains(&p.row))
            .cloned()
            .map(|mut p| {
                p.row -= rows.start;
                p
            })
            .collect();
        window.paint.article = full.article.clone();
        window.body_rows = total;
        super::presentation::refresh_links(&mut window.paint);
        Some(window)
    }
    pub fn presentation(&self) -> u64 {
        self.filters.version()
    }
    pub fn rules(&self) -> &filters::Rules {
        self.filters.rules()
    }
    pub fn has_in_flight(&self) -> bool {
        self.in_flight.is_some()
    }
    pub fn dirty(&self) -> bool {
        self.dirty
    }
    pub fn display_changed(&mut self) {
        self.revoke_output();
        self.dirty = true;
    }
    pub fn queue(
        &mut self,
        view: &View,
        probe: bool,
        settings: &DisplaySettings,
    ) -> Option<&Paint> {
        if !self.dirty || self.in_flight.is_some() || self.updates.holding() {
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
        if packet.article.is_some() && view.site.ptt_features() {
            packet.reply_navigation = crate::reply_navigation::index(
                &packet.replies,
                self.rules(),
                packet.revealed,
                packet
                    .article
                    .as_ref()
                    .filter(|a| a.first_line == 1)
                    .and_then(|_| crate::reply_navigation::author(&packet.frame)),
                view.account.as_deref(),
                packet.reply_focus.as_deref(),
            );
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
                && view.mouse.route == crate::mouse::input::Route::Local
                && matches!(
                    p.navigation,
                    Context::ArticleReading { .. } | Context::ArticleList { .. }
                )
        })
    }
    pub fn account_rule(
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
    pub fn focus_reply(
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
    pub fn acknowledge(&mut self, identity: Identity) -> Result<(), String> {
        if self.in_flight.as_ref().map(Identity::of) != Some(identity) {
            return Err(crate::msg!("paint-unknown-version"));
        }
        self.displayed = self.in_flight.take();
        Ok(())
    }
    pub fn painted(&self, view: &View) -> bool {
        !view.output_pending
            && self.displayed.as_ref().is_some_and(|p| {
                (p.generation, p.frame.revision) == (view.generation, view.frame.revision)
            })
    }
    pub fn current(&self, identity: Identity, view: &View) -> Option<&Paint> {
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
    /// Frozen metadata for local scrolling during owned refresh/recovery.
    /// This does not authorize input, selection, exports or media gestures;
    /// those consumers must continue to use current() and their output leases.
    pub fn retained_reading_identity(&self, view: &View) -> Option<Identity> {
        if !self.updates.holding()
            || view.phase != Phase::Connected
            || !view.site.ptt_features()
            || view.login_pending
            || view.mouse.route != crate::mouse::input::Route::Local
        {
            return None;
        }
        let scope = self.reading_scope()?;
        self.displayed
            .as_ref()
            .filter(|p| {
                p.connected
                    && p.ptt_features
                    && !p.output_pending
                    && p.article_mode == self.article_mode
                    && p.mouse.route == crate::mouse::input::Route::Local
                    && (
                        p.generation,
                        p.article.as_ref().map(|a| a.id),
                        p.presentation,
                    ) == (view.generation, Some(scope.1), scope.2)
                    && p.generation == scope.0
            })
            .map(Identity::of)
    }
    pub fn inline_page(&self, identity: Identity, stamp: Stamp, view: &View) -> Option<&Paint> {
        self.current(identity, view).filter(|p| {
            self.inline_identity(view) == Some(identity)
                && self.navigation_shown(stamp)
                && stamp == view.stamp()
                && p.mouse.route == crate::mouse::input::Route::Local
        })
    }
    pub fn inline_identity(&self, view: &View) -> Option<Identity> {
        if self.article_mode == ArticleMode::Terminal {
            return None;
        }
        let p = self.displayed.as_ref()?;
        self.current(Identity::of(p), view)
            .filter(|_| {
                view.phase == Phase::Connected
                    && !view.input_pending
                    && !view.login_pending
                    && view.mouse.route == crate::mouse::input::Route::Local
                    && view.frame.control_complete
            })
            .map(Identity::of)
    }
    pub fn navigation_shown(&self, stamp: Stamp) -> bool {
        !self.updates.holding()
            && self.displayed.as_ref().is_some_and(|p| {
                p.article_mode == self.article_mode
                    && p.presentation == self.filters.version()
                    && !p.output_pending
                    && p.generation == stamp.generation
                    && p.frame.revision == stamp.revision
                    && p.input_epoch == stamp.input_epoch
            })
    }
    pub fn mouse_shown(&self, event: &crate::mouse::input::Event) -> bool {
        use crate::mouse::{Action, input::Route};
        event.report.action == Action::Release
            || self.navigation_shown(event.lease.stamp)
                && self.displayed.as_ref().is_some_and(|p| {
                    p.mouse.route == Route::Station
                        && p.mouse.modes.epoch() == event.lease.mode_epoch
                        && p.mouse.policy_epoch == event.lease.policy_epoch
                })
    }
    pub fn mouse_sample_shown(&self, sample: &crate::mouse::input::Sample) -> bool {
        self.displayed.as_ref().is_some_and(|p| {
            !p.output_pending
                && p.mouse.route == crate::mouse::input::Route::Station
                && p.generation == sample.lease.stamp.generation
                && p.mouse.modes.epoch() == sample.lease.mode_epoch
                && p.mouse.policy_epoch == sample.lease.policy_epoch
                && (self.navigation_shown(sample.lease.stamp)
                    || p.mouse.active == Some(sample.lease))
        })
    }
    /// Vim may advance its logical cursor before the next terminal redraw.
    pub fn browse_shown(&self, generation: u64, context: &Context) -> bool {
        !self.updates.holding()
            && self.displayed.as_ref().is_some_and(|p| {
                !p.output_pending
                    && p.generation == generation
                    && p.navigation.kind() == context.kind()
            })
    }
    pub fn set_article_boundary_navigation(&mut self, enabled: bool) {
        if self.article_boundary_navigation != enabled {
            self.article_boundary_navigation = enabled;
            self.dirty = true;
        }
    }
    pub fn adjacent_article_allowed(&self, stamp: Stamp, down: bool, view: &View) -> bool {
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
    pub fn set_article_mode(&mut self, mode: ArticleMode) -> bool {
        if self.article_mode == mode {
            return false;
        }
        self.article_mode = mode;
        self.replace_rules(self.filters.rules().clone());
        true
    }
    pub fn set_raw_reading(&mut self, raw: bool) -> bool {
        if self.raw_reading == raw {
            return false;
        }
        self.raw_reading = raw;
        self.filters.touch();
        self.policy_changed();
        true
    }
    pub fn reading_context(&self, view: &View) -> Option<Context> {
        if self.article_mode == ArticleMode::Terminal {
            return None;
        }
        let paint = self.displayed.as_ref()?;
        let article = paint.article.as_ref()?;
        (view.phase == Phase::Connected
            && view.mouse.route == crate::mouse::input::Route::Local
            && paint.generation == view.generation
            && paint.presentation == self.filters.version()
            && self.article.active())
        .then_some(Context::ArticleReading {
            first_line: article.first_line,
            last_line: article.last_line,
            percent: if article.end { 100 } else { 0 },
        })
    }
    pub fn reading_move(
        &self,
        view: &View,
        intent: crate::navigation::Intent,
    ) -> Option<ReadingMove> {
        self.reading_context(view)?;
        if !super::article_cache::local_intent(intent) {
            return None;
        }
        let article = self.displayed.as_ref()?.article.as_ref()?;
        Some(ReadingMove {
            generation: view.generation,
            article: article.id,
            intent,
        })
    }
    pub fn cached_reading(&self, stamp: Stamp, view: &View) -> bool {
        self.navigation_shown(stamp)
            && self.inline_identity(view).is_some()
            && self.displayed.as_ref().is_some_and(|p| p.article.is_some())
    }
    pub fn article_navigation(&mut self, intent: crate::navigation::Intent, view: &View) {
        let context = view.context();
        if let Ok(bytes) = crate::navigation::plan(&context, intent) {
            self.article_input(view, &bytes);
        }
        self.article
            .begin_fetch(intent, &context, view.frame.revision);
    }
    pub fn cancel_article_fetch(&mut self) {
        self.article.cancel_fetch();
    }
    pub fn reading_progress(
        &self,
    ) -> Option<(u64, usize, usize, Option<super::article_cache::LoadState>)> {
        self.article.progress()
    }
    pub fn full_load_active(&self) -> bool {
        self.article.loading()
    }
    pub fn updates_status(&self) -> Option<super::article_updates::Status> {
        self.updates.status().map(|mut status| {
            status.available = !status.recovery && self.article.refresh_destination().is_some();
            status
        })
    }
    pub fn retained_relative_key(&self, key: &crate::vim::Key) -> bool {
        self.updates.holding()
            && !key.ctrl
            && !key.alt
            && matches!(
                key.key.as_str(),
                "ArrowLeft" | "ArrowRight" | "Enter" | "Backspace"
            )
    }
    pub fn updates_holding(&self) -> bool {
        self.updates.holding()
    }
    pub fn update_ready(&self, view: &View) -> bool {
        self.cached_reading(view.stamp(), view) && !view.input_pending && !view.output_pending
    }
    pub fn updates_deadline(&self, allowed: bool) -> Option<tokio::time::Instant> {
        self.updates.deadline(allowed)
    }
    pub fn cancelled_update(&mut self) -> Option<Stamp> {
        self.updates.take_cancelled()
    }
    pub fn updates_action(&mut self, generation: u64, id: u64, enabled: bool) -> bool {
        self.updates.action(
            generation,
            id,
            enabled,
            self.article.refresh_destination().is_some(),
            tokio::time::Instant::now(),
        )
    }
    pub fn restore_station(&mut self, generation: u64, id: u64) -> bool {
        if !self
            .updates
            .status()
            .is_some_and(|s| s.generation == generation && s.article == id && s.recovery)
        {
            return false;
        }
        self.clear_article();
        true
    }
    pub fn pause_updates(&mut self) {
        if self.updates.holding() {
            self.updates.stop(true);
            self.dirty = true;
        } else {
            self.updates.defer(tokio::time::Instant::now());
        }
    }
    pub fn fail_updates(&mut self) {
        self.updates.stop(true);
        self.dirty = true;
    }
    pub fn bookmark_current(
        &mut self,
        stamp: Stamp,
        id: u64,
        view: &View,
    ) -> Result<
        (
            super::article_cache::ArchiveSnapshot,
            crate::navigation_job::Destination,
        ),
        String,
    > {
        if self.authorize_reading(stamp, id).is_none()
            || !self.cached_reading(stamp, view)
            || stamp != view.stamp()
            || view.input_pending
            || view.output_pending
            || self.updates.holding()
            || self.updates.status().is_some_and(|s| s.busy)
        {
            return Err(crate::msg!("bookmarks-current-unavailable"));
        }
        let destination = self
            .article
            .refresh_destination()
            .ok_or(crate::msg!("bookmarks-current-unavailable"))?;
        let frame = self
            .article
            .archive_snapshot(&view.frame)
            .ok_or(crate::msg!("bookmarks-current-unavailable"))?;
        self.updates.begin(stamp, tokio::time::Instant::now());
        Ok((frame, destination))
    }
    pub fn next_update(&mut self, view: &View) -> Option<crate::navigation_job::Destination> {
        let now = tokio::time::Instant::now();
        if self.find.opened()
            || !self.updates.due(now)
            || !self.cached_reading(view.stamp(), view)
            || view.input_pending
            || view.output_pending
            || self.has_in_flight()
            || self.dirty
        {
            return None;
        }
        let destination = self.article.refresh_destination()?;
        self.updates.begin(view.stamp(), now);
        Some(destination)
    }
    pub fn full_load_deadline(&self) -> Option<tokio::time::Instant> {
        self.article.load_deadline()
    }
    pub fn expire_full_load(&mut self, now: tokio::time::Instant) {
        if self.article.expire_load(now) {
            self.updates.stop(true);
            self.dirty = true;
        }
    }
    pub fn stop_full_load(&mut self) {
        if self.updates.fetching() {
            self.updates.stop(true);
        }
        let old = self.article.load_state();
        self.article
            .stop_load(super::article_cache::LoadState::Stopped);
        self.dirty |= old != self.article.load_state();
    }
    pub fn interrupt_full_load(&mut self) {
        if self.updates.fetching() {
            self.updates.stop(false);
        }
        let old = self.article.load_state();
        self.article
            .stop_load(super::article_cache::LoadState::Cancelled);
        self.dirty |= old != self.article.load_state();
    }
    pub fn full_load_action(
        &mut self,
        generation: u64,
        id: u64,
        resume: bool,
        view: &View,
    ) -> bool {
        if generation != view.generation
            || self.updates.holding()
            || self.article_mode != ArticleMode::Web
            || self.raw_reading
            || resume
                && (view.phase != Phase::Connected
                    || !matches!(view.context(), Context::ArticleReading { .. }))
        {
            return false;
        }
        if resume
            && !view.input_pending
            && !view.output_pending
            && !view.login_pending
            && view.frame.control_complete
            && !matches!(
                view.destination,
                crate::navigation_job::Progress::Active { .. }
            )
            && self.article.advance_segment(
                id,
                view.generation,
                &view.context(),
                view.frame.revision,
            )
        {
            self.focus_changed();
            self.captured = None;
            self.capture_reading(view);
            return true;
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
                crate::navigation_job::Progress::Active { .. }
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
        self.article.capture_source(&packet, Some(&view.frame));
        self.captured = Some(identity);
        self.updates.sync(
            self.article
                .progress()
                .map(|(id, _, _, _)| (view.generation, id)),
        );
        if self.updates.fetching() {
            match self.article.load_state() {
                Some(super::article_cache::LoadState::Complete) => {
                    self.updates.finish(tokio::time::Instant::now())
                }
                Some(
                    super::article_cache::LoadState::Changed
                    | super::article_cache::LoadState::Limit
                    | super::article_cache::LoadState::Stopped
                    | super::article_cache::LoadState::Cancelled,
                ) => self.updates.stop(true),
                _ => {}
            }
        }
    }
    pub fn reading_scope(&self) -> Option<(u64, u64, u64)> {
        if self.article_mode != ArticleMode::Web || self.raw_reading {
            return None;
        }
        let (article, _, _, _) = self.article.progress()?;
        Some((self.captured?.0.generation, article, self.filters.version()))
    }
    pub fn authorize_reading(&self, stamp: Stamp, article: u64) -> Option<(u64, u64, u64)> {
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
    pub fn next_full_page(&mut self, view: &View) -> Option<crate::navigation::Intent> {
        if self.find.opened()
            || self.article_mode != ArticleMode::Web
            || self.raw_reading
            || self.updates.holding()
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
    pub fn continue_article_fetch(&mut self, view: &View) -> Option<crate::navigation::Intent> {
        if !self.cached_reading(view.stamp(), view) || self.has_in_flight() || self.dirty {
            return None;
        }
        self.article
            .continue_fetch(&view.context(), view.frame.revision)
    }
    pub fn clear_article(&mut self) {
        self.updates.sync(None);
        if self.article.active() || self.reply_focus.is_some() {
            self.filters.touch();
            self.policy_changed();
        } else {
            self.article.clear();
            self.captured = None;
        }
    }
    pub fn article_input(&mut self, view: &View, bytes: &[u8]) {
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
        self.find.suspend();
        self.updates.sync(None);
        self.reply_focus = None;
        self.article.clear();
        self.captured = None;
        self.revoke_output();
        self.selected = None;
        self.hover = None;
        self.dirty = true;
    }
    pub fn replace_rules(&mut self, rules: filters::Rules) {
        self.filters.replace(rules);
        self.policy_changed();
    }
    pub fn reveal(&mut self, identity: Identity, reveal: bool, view: &View) -> bool {
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
    pub fn update(&mut self, view: &View) -> bool {
        if self.find.opened()
            && (view.phase != Phase::Connected
                || !view.site.ptt_features()
                || self
                    .displayed
                    .as_ref()
                    .is_some_and(|p| p.generation != view.generation)
                || (!view.output_pending
                    && !view.input_pending
                    && view.frame.control_complete
                    && !matches!(view.context(), Context::ArticleReading { .. })))
        {
            self.find.cancel();
        } else if self.find.reply(0).source.is_some_and(|source| {
            (source.generation, source.revision, source.input_epoch)
                != (view.generation, view.frame.revision, view.input_epoch)
        }) || view.output_pending
            || view.input_pending
            || view.login_pending
        {
            self.find.suspend();
        }
        let holding = self.updates.holding();
        if self.updates.observe(view, tokio::time::Instant::now()) {
            return false;
        }
        if holding && self.updates.fetching() {
            self.article.begin_refresh();
        }
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
                && !matches!(view.context(), Context::ArticleReading { .. })
                // The cache already decides whether an intermediate screen is
                // part of an owned page turn. Keep focus on that same article
                // while a split encoded response is temporarily unrecognizable.
                && !self.reply_focus.as_ref().is_some_and(|(generation, article, _)| {
                    self.reading_scope().is_some_and(|scope| {
                        scope.0 == *generation && Some(scope.1) == *article
                    })
                })))
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
                    && view.mouse.route == crate::mouse::input::Route::Local
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
    pub fn select(
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
    pub fn copied(&self, identity: Identity, request: u64, view: &View) -> bool {
        self.selected.as_ref().is_some_and(|s| {
            s.identity == identity && s.request == request && s.input_epoch == view.input_epoch
        }) && self.current(identity, view).is_some()
    }
    pub fn editor_selection(
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
    pub fn copy_text(
        &self,
        identity: Identity,
        target: &CopyTarget,
        view: &View,
    ) -> Option<String> {
        let paint = self.current(identity, view)?;
        match target {
            CopyTarget::SelectionAnsi { request } => self
                .selected
                .as_ref()
                .filter(|_| self.copied(identity, *request, view))
                .map(|selected| {
                    selection::ansi(&paint.frame, &selected.spans, selected.rows.clone())
                }),
            CopyTarget::Selection { request } => self
                .selected
                .as_ref()
                .filter(|_| self.copied(identity, *request, view))
                .map(|selected| selected.text.clone()),
            CopyTarget::Link { index } => paint.links.get(*index).map(|link| link.url.clone()),
        }
    }
    pub fn export_rows(
        &self,
        identity: Identity,
        request: Option<u64>,
        view: &View,
    ) -> Result<Range<usize>, String> {
        let shown = self
            .current(identity, view)
            .ok_or(crate::msg!("export-screen-changed"))?;
        if (
            shown.frame.cols,
            shown.source_frame.as_ref().unwrap_or(&shown.frame).rows,
        ) != (80, export::ROWS)
        {
            return Err(crate::msg!("export-grid-unsupported"));
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
                .ok_or_else(|| crate::msg!("export-selection-changed")),
        }
    }
    pub fn clear_selection(&mut self, identity: Identity, request: u64) {
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
    pub fn begin_hover(&mut self, lease: Lease) {
        self.hover = Some(lease);
    }
    pub fn revoke_output(&mut self) {
        if let Some(active) = self.output.take() {
            active.lease.source.revoke();
        }
    }
    /// Called inside Files' admitted factory, so a busy rejection never revokes
    /// the accepted output. Callers validate export rows or copy targets first.
    pub fn lease_output(
        &mut self,
        identity: Identity,
        selection: Option<u64>,
        input_epoch: u64,
    ) -> OutputLease {
        self.revoke_output();
        let lease = OutputLease {
            identity,
            source: export::Lease::new(
                Stamp {
                    generation: identity.generation,
                    revision: identity.revision,
                    input_epoch,
                },
                selection.is_some(),
            ),
        };
        self.output = Some(ActiveOutput {
            lease: lease.clone(),
            selection,
        });
        lease
    }
    pub fn lease_picture(&mut self, identity: Identity, epoch: u64) -> OutputLease {
        let mut lease = self.lease_output(identity, None, epoch);
        lease.source = lease.source.require_input_epoch(epoch);
        lease
    }
    pub fn clear_hover(&mut self) -> bool {
        self.hover.take().is_some()
    }
    pub fn cancel_hover(&mut self, sequence: u64) -> bool {
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
