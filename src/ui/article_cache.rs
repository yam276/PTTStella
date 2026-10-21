//! Bounded, filtered reading history. Absolute article lines belong to Rust;
//! the renderer only lays out this projection and scrolls within it.
use super::presentation::Paint;
use pttstella::{
    connection::{Phase, View},
    mouse::input::Route,
    navigation::Context,
    screen::{Frame, Glyph, Style, hyperlinks::Hyperlink},
};
use serde::Serialize;
use std::collections::BTreeMap;
use tokio::time::{Duration, Instant};

const MAX_FETCH_STEPS: usize = 14;
const MAX_LINK_BYTES: usize = 512 * 1024;
const MAX_FULL_LINES: usize = 4096;
const MAX_FULL_BYTES: usize = 32 * 1024 * 1024;
const LOAD_RESPONSE_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LoadState {
    Loading,
    Complete,
    Cancelled,
    Stopped,
    Changed,
    Limit,
}
#[derive(Default)]
struct Loader {
    state: Option<LoadState>,
    awaiting: Option<(u64, usize, usize, bool, Instant)>,
    steps: usize,
    at_head: bool,
}

#[derive(Clone, Serialize)]
pub(super) struct Reading {
    pub id: u64,
    pub first_line: usize,
    pub last_line: usize,
    pub end: bool,
    pub load: Option<LoadState>,
}
struct Row {
    reply: Option<pttstella::filters::Push>,
    glyphs: Vec<Glyph>,
    hyperlinks: Vec<Hyperlink>,
}
#[derive(Default)]
pub(super) struct ArticleCache {
    serial: u64,
    scope: Option<(u64, u64)>,
    rows: BTreeMap<usize, Row>,
    end: Option<usize>,
    turning: bool,
    source_range: Option<(usize, usize)>,
    // PTT reports logical lines; wrapped rows need their own stable positions.
    pages: BTreeMap<(usize, usize), (usize, usize)>,
    fetch: Option<Fetch>,
    loader: Loader,
}

struct Fetch {
    target: usize,
    down: bool,
    previous: usize,
    revision: u64,
    steps: usize,
}
pub(super) fn local_intent(intent: pttstella::navigation::Intent) -> bool {
    use pttstella::navigation::Intent::*;
    matches!(
        intent,
        Lines { .. } | PreviousPage | NextPage | First | Last | Forward
    )
}

pub(super) fn reading_input(context: &Context, bytes: &[u8]) -> bool {
    matches!(context, Context::ArticleReading { .. }) && reading_intent(bytes).is_some()
}
pub(super) fn reading_intent(bytes: &[u8]) -> Option<pttstella::navigation::Intent> {
    use pttstella::navigation::Intent;
    Some(match bytes {
        b"\x1b[5~" | b"\x02" => Intent::PreviousPage,
        b"\x1b[6~" | b"\x1b[C" | b" " | b"\x06" => Intent::NextPage,
        b"\x1b[B" => Intent::Lines {
            down: true,
            count: 1,
        },
        b"\x1b[A" => Intent::Lines {
            down: false,
            count: 1,
        },
        b"\x1b[H" => Intent::First,
        b"\x1b[F" => Intent::Last,
        _ if !bytes.is_empty() && bytes.len() <= 100 && bytes.iter().all(|b| *b == b'j') => {
            Intent::Lines {
                down: true,
                count: bytes.len(),
            }
        }
        _ if !bytes.is_empty() && bytes.len() <= 100 && bytes.iter().all(|b| *b == b'k') => {
            Intent::Lines {
                down: false,
                count: bytes.len(),
            }
        }
        _ => return None,
    })
}
impl ArticleCache {
    pub(super) fn loading(&self) -> bool {
        self.loader.state == Some(LoadState::Loading)
    }
    pub(super) fn load_state(&self) -> Option<LoadState> {
        self.loader.state
    }
    pub(super) fn load_deadline(&self) -> Option<Instant> {
        self.loader.awaiting.map(|a| a.4)
    }
    pub(super) fn stop_load(&mut self, state: LoadState) {
        if self.loading() {
            self.loader.state = Some(state);
            self.loader.awaiting = None;
        }
    }
    pub(super) fn load_action(&mut self, id: u64, resume: bool) -> bool {
        if id != self.serial || !self.active() {
            return false;
        }
        if resume {
            if !matches!(
                self.loader.state,
                Some(LoadState::Cancelled | LoadState::Stopped)
            ) {
                return false;
            }
            self.loader.state = Some(LoadState::Loading);
            self.loader.steps = 0;
            self.loader.awaiting = None;
        } else {
            if !self.loading() {
                return false;
            }
            self.stop_load(LoadState::Cancelled);
        }
        true
    }
    pub(super) fn expire_load(&mut self, now: Instant) -> bool {
        if self.load_deadline().is_some_and(|deadline| now >= deadline) {
            self.stop_load(LoadState::Stopped);
            return true;
        }
        false
    }
    pub(super) fn next_full_page(
        &mut self,
        context: &Context,
        revision: u64,
        now: Instant,
    ) -> Option<pttstella::navigation::Intent> {
        use pttstella::navigation::Intent;
        if !self.loading() {
            return None;
        }
        let Context::ArticleReading {
            first_line,
            last_line,
            ..
        } = context
        else {
            return None;
        };
        if self.loader.awaiting.is_some() {
            return None;
        }
        self.rows.first_key_value()?;
        let down = self.loader.at_head;
        if self.loader.steps >= MAX_FULL_LINES * 2 + 2 {
            self.stop_load(LoadState::Limit);
            return None;
        }
        self.loader.steps += 1;
        self.loader.awaiting = Some((
            revision,
            *first_line,
            *last_line,
            down,
            now + LOAD_RESPONSE_TIMEOUT,
        ));
        Some(if down {
            Intent::NextPage
        } else {
            Intent::First
        })
    }
    fn reset_segment(&mut self) {
        let state = self.loader.state.map(|s| {
            if s == LoadState::Loading {
                LoadState::Changed
            } else {
                s
            }
        });
        self.clear();
        self.loader.state = state;
    }
    // Focus is applied after caching and must not discard unmasked history.
    pub(super) fn focus_version(&mut self, version: u64) {
        if let Some((_, policy)) = &mut self.scope {
            *policy = version;
        }
    }
    pub(super) fn active(&self) -> bool {
        !self.rows.is_empty()
    }
    pub(super) fn begin_fetch(
        &mut self,
        intent: pttstella::navigation::Intent,
        context: &Context,
        revision: u64,
    ) {
        self.stop_load(LoadState::Cancelled);
        use pttstella::navigation::Intent;
        let Context::ArticleReading { first_line, .. } = context else {
            return;
        };
        let Some((&first, _)) = self.rows.first_key_value() else {
            return;
        };
        let Some((&last, _)) = self.rows.last_key_value() else {
            return;
        };
        self.fetch = match intent {
            Intent::NextPage if self.end != Some(last) => Some(Fetch {
                target: last.saturating_add(1),
                down: true,
                previous: *first_line,
                revision,
                steps: 1,
            }),
            Intent::PreviousPage if first > 1 => Some(Fetch {
                target: first - 1,
                down: false,
                previous: *first_line,
                revision,
                steps: 1,
            }),
            _ => None,
        };
    }
    pub(super) fn cancel_fetch(&mut self) {
        self.fetch = None;
    }
    pub(super) fn continue_fetch(
        &mut self,
        context: &Context,
        revision: u64,
    ) -> Option<pttstella::navigation::Intent> {
        if matches!(
            self.loader.state,
            Some(LoadState::Changed | LoadState::Limit)
        ) {
            self.fetch = None;
            return None;
        }
        let fetch = self.fetch.as_ref()?;
        if revision == fetch.revision {
            return None;
        }
        let Context::ArticleReading {
            first_line,
            percent,
            ..
        } = context
        else {
            self.fetch = None;
            return None;
        };
        if self.rows.contains_key(&fetch.target)
            || fetch.steps >= MAX_FETCH_STEPS
            || *first_line == fetch.previous
            || fetch.down && *percent == 100
            || !fetch.down && *first_line == 1
        {
            self.fetch = None;
            return None;
        }
        let fetch = self.fetch.as_mut()?;
        fetch.previous = *first_line;
        fetch.revision = revision;
        fetch.steps += 1;
        Some(if fetch.down {
            pttstella::navigation::Intent::NextPage
        } else {
            pttstella::navigation::Intent::PreviousPage
        })
    }
    pub(super) fn clear(&mut self) {
        *self = Self {
            serial: self.serial,
            ..Self::default()
        };
    }
    pub(super) fn input(&mut self, context: &Context, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if reading_input(context, bytes) {
            self.turning = true;
        } else {
            self.clear();
        }
    }
    pub(super) fn observe(&mut self, view: &View) {
        if self
            .scope
            .is_some_and(|(generation, _)| generation != view.generation)
            || view.phase != Phase::Connected
            || !view.site.ptt_features()
            || view.mouse.route != Route::Local
            || !view.input_pending
                && !view.output_pending
                && view.frame.control_complete
                && match view.context() {
                    Context::ArticleReading { .. } => false,
                    Context::Unknown { .. } => !self.turning,
                    _ => true,
                }
        {
            self.clear();
        }
    }
    pub(super) fn project(&mut self, paint: &mut Paint) {
        if self
            .scope
            .is_some_and(|scope| scope != (paint.generation, paint.presentation))
        {
            self.clear();
        }
        if !paint.ptt_features || !paint.connected || paint.mouse.route != Route::Local {
            return;
        }
        let source = paint.frame.clone();
        if !paint.input_pending
            && !paint.output_pending
            && source.control_complete
            && let Context::ArticleReading {
                first_line,
                last_line,
                percent,
            } = paint.navigation
            && first_line > 0
            && last_line >= first_line
            && last_line - first_line < source.rows.saturating_sub(1)
            && !(self.turning && self.source_range == Some((first_line, last_line)))
        {
            'page: {
                if matches!(
                    self.loader.state,
                    Some(LoadState::Limit | LoadState::Changed)
                ) {
                    break 'page;
                }
                if first_line == 1 && self.loader.awaiting.is_some_and(|a| !a.3) {
                    self.rows.clear();
                    self.end = None;
                    self.source_range = None;
                    self.pages.clear();
                    self.loader.awaiting = None;
                }
                if first_line == 1 {
                    self.loader.at_head = true;
                }
                if first_line == 1
                    && (1..=3.min(last_line)).any(|line| {
                        self.rows.get(&line).is_some_and(|known| {
                            let incoming: Vec<_> = source
                                .glyphs
                                .iter()
                                .filter(|g| g.row == line - 1)
                                .cloned()
                                .map(|mut g| {
                                    g.row = 0;
                                    g
                                })
                                .collect();
                            !same_glyphs(&incoming, &known.glyphs)
                        })
                    })
                {
                    // A changed opening header cannot inherit another document's tail.
                    self.loader.state = Some(LoadState::Changed);
                    self.loader.awaiting = None;
                    break 'page;
                }
                if let Some((first, last)) = self.source_range
                    && !self.pages.contains_key(&(first_line, last_line))
                    && (last_line.saturating_add(1) < first || first_line > last.saturating_add(1))
                {
                    // An explicit Home/End or an unobserved range is a new segment;
                    // never fill a gap with guessed text.
                    if self.loading() {
                        self.stop_load(LoadState::Changed);
                        break 'page;
                    }
                    self.reset_segment();
                }
                if self.scope.is_none() {
                    self.serial = self.serial.saturating_add(1);
                }
                self.scope = Some((paint.generation, paint.presentation));
                let mut incoming = Vec::new();
                let body_rows = source
                    .glyphs
                    .iter()
                    .filter(|g| {
                        g.row + 1 < source.rows
                            && (g.text.chars().any(|c| !c.is_whitespace())
                                || g.left.bg != 0
                                || g.right.bg != 0
                                || g.left.inverse
                                || g.right.inverse)
                    })
                    .map(|g| g.row + 1)
                    .max()
                    .unwrap_or(0)
                    .max(last_line - first_line + 1);
                for row in 0..body_rows {
                    let line = first_line + row;
                    let glyphs = source
                        .glyphs
                        .iter()
                        .filter(|g| g.row == row)
                        .cloned()
                        .map(|mut g| {
                            g.row = 0;
                            g
                        })
                        .collect();
                    let hyperlinks = source
                        .hyperlinks
                        .iter()
                        .filter_map(|link| {
                            let spans: Vec<_> = link
                                .spans
                                .iter()
                                .filter(|s| s.row == row)
                                .cloned()
                                .map(|mut s| {
                                    s.row = 0;
                                    s
                                })
                                .collect();
                            (!spans.is_empty()).then(|| Hyperlink {
                                target: link.target.clone(),
                                spans,
                            })
                        })
                        .collect();
                    incoming.push((
                        line,
                        Row {
                            glyphs,
                            hyperlinks,
                            reply: paint.replies.iter().find(|reply| reply.row == row).cloned(),
                        },
                    ));
                }
                {
                    if let Some((sent, old_first, old_last, down, _)) = self.loader.awaiting
                        && source.revision != sent
                        && (first_line, last_line) != (old_first, old_last)
                    {
                        if down && last_line <= old_last || !down && first_line >= old_first {
                            self.loader.state = Some(LoadState::Changed);
                            self.loader.awaiting = None;
                            break 'page;
                        }
                        self.loader.awaiting = None;
                    }
                    if self.loader.state.is_none() {
                        self.loader.state = Some(LoadState::Loading);
                    }
                    let start = if let Some(&(start, count)) =
                        self.pages.get(&(first_line, last_line))
                    {
                        if count != body_rows {
                            self.loader.state = Some(LoadState::Changed);
                            self.loader.awaiting = None;
                            break 'page;
                        }
                        start
                    } else if let Some((previous_first, previous_last)) = self.source_range {
                        if first_line < previous_first {
                            let expected =
                                last_line.saturating_add(1).saturating_sub(previous_first);
                            let before = incoming.iter().map(|(_, row)| row).collect::<Vec<_>>();
                            let after = self.rows.values().collect::<Vec<_>>();
                            let overlap = page_overlap(
                                &before,
                                &after,
                                expected,
                                last_line >= previous_first,
                            );
                            let start = overlap.and_then(|n| {
                                self.rows.first_key_value()?.0.checked_sub(body_rows - n)
                            });
                            let Some(start) = start.filter(|n| *n > 0) else {
                                self.loader.state = Some(LoadState::Changed);
                                self.loader.awaiting = None;
                                self.fetch = None;
                                break 'page;
                            };
                            start
                        } else {
                            let expected =
                                previous_last.saturating_add(1).saturating_sub(first_line);
                            let before = self.rows.values().collect::<Vec<_>>();
                            let after = incoming.iter().map(|(_, row)| row).collect::<Vec<_>>();
                            let matched = page_overlap(
                                &before,
                                &after,
                                expected,
                                first_line <= previous_last,
                            );
                            let Some(overlap) = matched else {
                                self.loader.state = Some(LoadState::Changed);
                                self.loader.awaiting = None;
                                break 'page;
                            };
                            self.rows.last_key_value().map_or(1, |(n, _)| n + 1) - overlap
                        }
                    } else {
                        first_line
                    };
                    incoming = incoming
                        .into_iter()
                        .enumerate()
                        .map(|(i, (_, r))| (start + i, r))
                        .collect();
                    let size = self
                        .rows
                        .iter()
                        .filter(|(line, _)| !incoming.iter().any(|(n, _)| n == *line))
                        .map(|(_, r)| row_bytes(r))
                        .sum::<usize>()
                        + incoming.iter().map(|(_, r)| row_bytes(r)).sum::<usize>();
                    let count = self
                        .rows
                        .keys()
                        .filter(|line| !incoming.iter().any(|(n, _)| n == *line))
                        .count()
                        + incoming.len();
                    if count > MAX_FULL_LINES
                        || self.pages.len() >= MAX_FULL_LINES * 2 + 2
                        || size > MAX_FULL_BYTES
                        || self.link_bytes()
                            + incoming
                                .iter()
                                .flat_map(|(_, r)| &r.hyperlinks)
                                .filter_map(|l| l.target.as_ref())
                                .map(String::len)
                                .sum::<usize>()
                            > MAX_LINK_BYTES
                    {
                        self.stop_load(LoadState::Limit);
                        break 'page;
                    }
                }
                let page_start = incoming.first().map_or(first_line, |(n, _)| *n);
                let page_end = page_start + body_rows - 1;
                self.pages
                    .insert((first_line, last_line), (page_start, body_rows));
                self.rows.extend(incoming);
                if percent == 100 {
                    let end = page_end;
                    self.end = Some(end);
                    self.rows.retain(|line, _| *line <= end);
                    self.pages
                        .retain(|_, (start, count)| *start + *count - 1 <= end);
                } else if self.end.is_some_and(|end| page_end >= end) {
                    self.end = None;
                }
                self.turning = false;
                self.source_range = Some((first_line, last_line));
                if self.loading()
                    && self.loader.at_head
                    && self.rows.first_key_value().is_some_and(|(n, _)| *n == 1)
                    && self
                        .rows
                        .last_key_value()
                        .is_some_and(|(n, _)| self.end == Some(*n))
                {
                    self.loader.state = Some(LoadState::Complete);
                    self.loader.awaiting = None;
                }
            }
        }
        let (Some((&first, _)), Some((&last, _))) =
            (self.rows.first_key_value(), self.rows.last_key_value())
        else {
            return;
        };
        // The original 24-row snapshot remains the source of full-page PNGs.
        paint.source_frame = Some(source.clone());
        let mut frame = Frame {
            rows: (self.rows.len() + 1).max(source.rows),
            cursor_visible: false,
            glyphs: Vec::new(),
            hyperlinks: Vec::new(),
            ..source
        };
        paint.replies.clear();
        for (row, data) in self.rows.values().enumerate() {
            if let Some(mut reply) = data.reply.clone() {
                reply.row = row;
                paint.replies.push(reply);
            }
            frame
                .glyphs
                .extend(data.glyphs.iter().cloned().map(|mut g| {
                    g.row = row;
                    g
                }));
            frame
                .hyperlinks
                .extend(data.hyperlinks.iter().cloned().map(|mut link| {
                    for span in &mut link.spans {
                        span.row = row;
                    }
                    link
                }));
        }
        // A minimum-height article still needs every blank cell. Text consumers
        // reject holes in the grid, even when the renderer paints them black.
        for row in self.rows.len()..frame.rows - 1 {
            frame.glyphs.extend((0..frame.cols).map(|col| Glyph {
                row,
                col,
                width: 1,
                text: " ".into(),
                left: Style::default(),
                right: Style::default(),
                unmapped: false,
            }));
        }
        // One live footer, rather than one duplicate footer per cached page.
        frame.glyphs.extend(
            paint
                .source_frame
                .as_ref()
                .into_iter()
                .flat_map(|f| &f.glyphs)
                .filter(|g| g.row + 1 == paint.source_frame.as_ref().map_or(0, |f| f.rows))
                .cloned()
                .map(|mut g| {
                    g.row = frame.rows - 1;
                    g
                }),
        );
        frame.hyperlinks.extend(
            paint
                .source_frame
                .as_ref()
                .into_iter()
                .flat_map(|f| &f.hyperlinks)
                .filter_map(|link| {
                    let spans: Vec<_> = link
                        .spans
                        .iter()
                        .filter(|span| {
                            span.row + 1 == paint.source_frame.as_ref().map_or(0, |f| f.rows)
                        })
                        .cloned()
                        .map(|mut span| {
                            span.row = frame.rows - 1;
                            span
                        })
                        .collect();
                    (!spans.is_empty()).then(|| Hyperlink {
                        target: link.target.clone(),
                        spans,
                    })
                }),
        );
        frame.cursor = (0, frame.rows - 1);
        paint.article = Some(Reading {
            id: self.serial,
            first_line: first,
            last_line: last,
            end: self.end == Some(last),
            load: self.loader.state,
        });
        paint.frame = frame;
        super::presentation::refresh_links(paint);
    }
    fn link_bytes(&self) -> usize {
        self.rows
            .values()
            .flat_map(|row| &row.hyperlinks)
            .filter_map(|link| link.target.as_ref())
            .map(String::len)
            .sum()
    }
}

// Footer ranges are logical lines, and a final-page clamp may land inside a
// wrapped line. Match physical rows, using the reported overlap to disambiguate
// repeated rows rather than requiring it to be the physical overlap length.
fn page_overlap(before: &[&Row], after: &[&Row], expected: usize, required: bool) -> Option<usize> {
    (usize::from(required)..=before.len().min(after.len()))
        .filter(|n| {
            before[before.len() - n..]
                .iter()
                .zip(after)
                .all(|(a, b)| same_glyphs(&a.glyphs, &b.glyphs))
        })
        .min_by_key(|n| n.abs_diff(expected))
}
fn same_glyphs(a: &[Glyph], b: &[Glyph]) -> bool {
    fn colours(style: Style) -> (u8, u8) {
        let fg = style.fg + u8::from(style.bright && style.fg < 8) * 8;
        if style.inverse {
            (style.bg, fg)
        } else {
            (fg, style.bg)
        }
    }
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            let blank = a.text.chars().all(|c| c == ' ' || c == '\u{3000}');
            a.row == b.row
                && a.col == b.col
                && a.width == b.width
                && a.text == b.text
                && a.unmapped == b.unmapped
                && [(a.left, b.left), (a.right, b.right)].iter().all(|(a, b)| {
                    let a = colours(*a);
                    let b = colours(*b);
                    a.1 == b.1 && (blank || a.0 == b.0)
                })
        })
}

fn row_bytes(row: &Row) -> usize {
    row.glyphs.capacity() * std::mem::size_of::<Glyph>()
        + row.glyphs.iter().map(|g| g.text.capacity()).sum::<usize>()
        + row.reply.as_ref().map_or(0, |reply| reply.id.capacity())
        + row.hyperlinks.capacity() * std::mem::size_of::<Hyperlink>()
        + row
            .hyperlinks
            .iter()
            .map(|l| {
                l.target.as_ref().map_or(0, String::capacity)
                    + l.spans.capacity()
                        * std::mem::size_of::<pttstella::screen::hyperlinks::Span>()
            })
            .sum::<usize>()
}
#[cfg(test)]
#[path = "article_cache_tests.rs"]
mod tests;
