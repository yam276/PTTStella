//! Bounded, filtered reading history. Absolute article lines belong to Rust;
//! the renderer only lays out this projection and scrolls within it.
use super::presentation::Paint;
use pttstella::{
    connection::{Phase, View},
    mouse::input::Route,
    navigation::Context,
    screen::{Frame, Glyph, hyperlinks::Hyperlink},
};
use serde::Serialize;
use std::collections::BTreeMap;

pub(super) const MAX_LINES: usize = 256;
const MAX_LINK_BYTES: usize = 512 * 1024;

#[derive(Clone, Serialize)]
pub(super) struct Reading {
    pub id: u64,
    pub first_line: usize,
    pub last_line: usize,
    pub end: bool,
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
    fetch: Option<Fetch>,
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
            || fetch.steps >= MAX_LINES.div_ceil(22) + 2
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
            let previous = self.rows.first_key_value().map(|(line, _)| *line);
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
                        incoming != known.glyphs
                    })
                })
            {
                // A changed opening header cannot inherit another document's tail.
                self.clear();
            }
            if let (Some((&first, _)), Some((&last, _))) =
                (self.rows.first_key_value(), self.rows.last_key_value())
                && (last_line.saturating_add(1) < first || first_line > last.saturating_add(1))
            {
                // An explicit Home/End or an unobserved range is a new segment;
                // never fill a gap with guessed text.
                self.clear();
            }
            if self.scope.is_none() {
                self.serial = self.serial.saturating_add(1);
            }
            self.scope = Some((paint.generation, paint.presentation));
            for line in first_line..=last_line {
                let row = line - first_line;
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
                self.rows.insert(
                    line,
                    Row {
                        glyphs,
                        hyperlinks,
                        reply: paint.replies.iter().find(|reply| reply.row == row).cloned(),
                    },
                );
            }
            if percent == 100 {
                self.end = Some(last_line);
                self.rows.retain(|line, _| *line <= last_line);
            } else if self.end.is_some_and(|end| last_line >= end) {
                self.end = None;
            }
            let forward = previous.is_none_or(|first| first_line >= first);
            while self.rows.len() > MAX_LINES || self.link_bytes() > MAX_LINK_BYTES {
                if forward {
                    self.rows.pop_first();
                } else {
                    self.rows.pop_last();
                }
            }
            self.turning = false;
            self.source_range = Some((first_line, last_line));
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
        frame.cursor = (0, frame.rows - 1);
        paint.article = Some(Reading {
            id: self.serial,
            first_line: first,
            last_line: last,
            end: self.end == Some(last),
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

#[cfg(test)]
#[path = "article_cache_tests.rs"]
mod tests;
