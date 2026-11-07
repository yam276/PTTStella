//! Literal search of confirmed, filtered glyphs. No station bytes or UI access.
use crate::{
    article_cache::{MAX_FULL_BYTES, MAX_FULL_LINES},
    navigation::Context,
    presentation::Paint,
    screen::{Frame, Style},
};
use serde::{Deserialize, Serialize};

pub const MAX_QUERY_UNITS: usize = 256;
pub const MAX_MATCHES: usize = 4096;
pub const NORMAL: Style = Style {
    fg: 0,
    bg: 11,
    bright: false,
    inverse: false,
};
pub const SELECTED: Style = Style {
    fg: 15,
    bg: 4,
    bright: false,
    inverse: false,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Open,
    Pending,
    Edit { query: String },
    Step { back: bool },
    Close,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Source {
    pub generation: u64,
    pub revision: u64,
    pub presentation: u64,
    pub input_epoch: u64,
    pub article: Option<u64>,
}
impl Source {
    pub fn of(p: &Paint) -> Self {
        Self {
            generation: p.generation,
            revision: p.frame.revision,
            presentation: p.presentation,
            input_epoch: p.input_epoch,
            article: p.article.as_ref().map(|a| a.id),
        }
    }
    fn same_scope(self, other: Self) -> bool {
        self.generation == other.generation
            && self.article == other.article
            && (self.article.is_some() || self.revision == other.revision)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Hit {
    pub row: usize,
    pub start: usize,
    pub end: usize,
    pub index: usize,
}
#[derive(Serialize)]
pub struct Reply<'a> {
    pub request: u64,
    pub version: u64,
    pub data_version: u64,
    pub opened: bool,
    pub source: Option<Source>,
    pub active: Option<usize>,
    pub total: usize,
    pub limited: bool,
    pub hits: Option<&'a [Hit]>,
    pub jump: bool,
}
// One compact byte-end/cell-range record per glyph or gap. UTF-8 expansion
// (e.g. dotted I) and combining glyphs still map to the original whole cell.
struct Span {
    end: u32,
    start: u16,
    finish: u16,
}
struct Line {
    row: usize,
    text: String,
    spans: Vec<Span>,
}
#[derive(Default)]
struct Index {
    source: Option<Source>,
    rows: usize,
    lines: Vec<Line>,
}
impl Index {
    fn clear(&mut self) {
        *self = Self::default();
    }
    fn build(&mut self, p: &Paint) -> bool {
        let source = Source::of(p);
        let rows = body_rows(p);
        if self.source == Some(source) && self.rows == rows {
            return true;
        }
        self.clear();
        if p.frame.cols != 80
            || rows > MAX_FULL_LINES
            || p.frame.rows > MAX_FULL_LINES + 1
            || p.frame.glyphs.len() > p.frame.rows.saturating_mul(p.frame.cols)
        {
            return false;
        }
        let mut glyphs = p
            .frame
            .glyphs
            .iter()
            .filter(|g| g.row < rows)
            .collect::<Vec<_>>();
        if glyphs
            .windows(2)
            .any(|p| (p[0].row, p[0].col) > (p[1].row, p[1].col))
        {
            glyphs.sort_by_key(|g| (g.row, g.col));
        }
        let mut bytes = 0usize;
        for g in glyphs {
            if g.width == 0 || g.width > 2 || g.col.saturating_add(g.width) > 80 {
                self.clear();
                return false;
            }
            if self.lines.last().is_none_or(|line| line.row != g.row) {
                self.lines.push(Line {
                    row: g.row,
                    text: String::new(),
                    spans: Vec::new(),
                });
            }
            let Some(line) = self.lines.last_mut() else {
                return false;
            };
            let column = line.spans.last().map_or(0, |s| usize::from(s.finish));
            if g.col < column {
                self.clear();
                return false;
            }
            if g.col > column {
                let gap = g.col - column;
                line.text.extend(std::iter::repeat_n(' ', gap));
                bytes += gap;
                let first = line.text.len() - gap;
                for offset in 0..gap {
                    line.spans.push(Span {
                        end: (first + offset + 1) as u32,
                        start: (column + offset) as u16,
                        finish: (column + offset + 1) as u16,
                    });
                }
            }
            let value = if visible(g.left) && (g.width == 1 || visible(g.right)) {
                g.text.to_lowercase()
            } else {
                "\0".into()
            };
            bytes = bytes.saturating_add(value.len());
            if bytes > MAX_FULL_BYTES {
                self.clear();
                return false;
            }
            line.text.push_str(&value);
            line.spans.push(Span {
                end: line.text.len() as u32,
                start: g.col as u16,
                finish: (g.col + g.width) as u16,
            });
        }
        self.source = Some(source);
        self.rows = rows;
        true
    }
    fn search(&self, query: &str) -> (Vec<Hit>, bool) {
        let query = query.to_lowercase();
        let mut hits = Vec::new();
        if query.is_empty() || query.contains('\0') {
            return (hits, false);
        }
        for line in &self.lines {
            let mut from = 0;
            while let Some(offset) = line.text[from..].find(&query) {
                if hits.len() == MAX_MATCHES {
                    return (hits, true);
                }
                let begin = from + offset;
                let end = begin + query.len();
                let a = line.spans.partition_point(|s| s.end as usize <= begin);
                let b = line.spans.partition_point(|s| (s.end as usize) < end);
                if let (Some(a), Some(b)) = (line.spans.get(a), line.spans.get(b)) {
                    hits.push(Hit {
                        row: line.row,
                        start: usize::from(a.start),
                        end: usize::from(b.finish),
                        index: hits.len(),
                    });
                }
                from = end;
            }
        }
        (hits, false)
    }
}
fn visible(style: Style) -> bool {
    style.fg + u8::from(style.bright && style.fg < 8) * 8 != style.bg
}
fn body_rows(p: &Paint) -> usize {
    p.article
        .as_ref()
        .map_or(p.frame.rows.saturating_sub(1), |a| {
            p.frame
                .rows
                .saturating_sub(1)
                .min(a.last_line.saturating_sub(a.first_line).saturating_add(1))
        })
}
fn allowed(p: &Paint) -> bool {
    p.ptt_features
        && matches!(p.navigation, Context::ArticleReading { .. })
        && !p.output_pending
        && !p.input_pending
        && p.frame.cols == 80
        && (2..=MAX_FULL_LINES + 1).contains(&p.frame.rows)
}
pub fn bounded_query(text: &str) -> String {
    let mut units = 0;
    text.chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= MAX_QUERY_UNITS
        })
        .collect()
}

#[derive(Default)]
pub struct Find {
    opened: bool,
    scope: Option<Source>,
    source: Option<Source>,
    request: u64,
    query: String,
    pending: bool,
    index: Index,
    hits: Vec<Hit>,
    active: Option<usize>,
    anchor: Option<(usize, usize)>,
    limited: bool,
    version: u64,
    data_version: u64,
    builds: u64,
    jump: bool,
}
impl Find {
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn data_version(&self) -> u64 {
        self.data_version
    }
    pub fn opened(&self) -> bool {
        self.opened
    }
    pub fn cancel(&mut self) {
        self.close(self.request);
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn builds(&self) -> u64 {
        self.builds
    }
    pub fn indexed_rows(&self) -> usize {
        self.index.lines.len()
    }
    pub fn active(&self) -> Option<Hit> {
        self.active.and_then(|i| self.hits.get(i)).copied()
    }
    pub fn reply(&self, since: u64) -> Reply<'_> {
        Reply {
            request: self.request,
            version: self.version,
            data_version: self.data_version,
            opened: self.opened,
            source: self.source,
            active: self.active,
            total: self.hits.len(),
            limited: self.limited,
            hits: (since != self.data_version).then_some(self.hits.as_slice()),
            jump: self.jump,
        }
    }
    fn changed(&mut self, data: bool) {
        self.version = self.version.saturating_add(1);
        if data {
            self.data_version = self.data_version.saturating_add(1);
        }
    }
    fn clear_hits(&mut self) {
        if let Some(hit) = self.active() {
            self.anchor = Some((hit.row, hit.start));
        }
        self.hits.clear();
        self.active = None;
        self.limited = false;
    }
    pub fn close(&mut self, request: u64) -> bool {
        if request < self.request {
            return false;
        }
        self.request = request;
        self.opened = false;
        self.scope = None;
        self.source = None;
        self.query.clear();
        self.pending = false;
        self.jump = false;
        self.index.clear();
        self.clear_hits();
        self.anchor = None;
        self.changed(true);
        true
    }
    /// Revoke stale results immediately; retain only the query/anchor until a
    /// current presentation has actually been installed and synchronized.
    pub fn suspend(&mut self) {
        if self.source.take().is_some() || !self.hits.is_empty() {
            self.index.clear();
            self.clear_hits();
            self.changed(true);
            self.jump = false;
        }
    }
    pub fn sync(&mut self, p: &Paint) {
        if !self.opened {
            return;
        }
        let source = Source::of(p);
        if !p.ptt_features
            || !matches!(p.navigation, Context::ArticleReading { .. })
            || self.scope.is_none_or(|scope| !scope.same_scope(source))
        {
            self.close(self.request);
            return;
        }
        if !allowed(p) {
            self.suspend();
            return;
        }
        if self.source == Some(source)
            && (self.query.is_empty() || self.pending || self.index.rows == body_rows(p))
        {
            return;
        }
        self.source = Some(source);
        self.jump = false;
        if self.pending {
            self.index.clear();
            self.clear_hits();
            self.changed(true);
        } else {
            self.search(p, true);
        }
    }
    pub fn action(&mut self, p: &Paint, request: u64, action: Action) -> bool {
        if request <= self.request || !allowed(p) {
            return false;
        }
        if matches!(action, Action::Close) {
            return self.close(request);
        }
        if matches!(action, Action::Open) {
            if !self.opened
                || self
                    .scope
                    .is_none_or(|scope| !scope.same_scope(Source::of(p)))
            {
                self.close(request);
                self.opened = true;
                self.scope = Some(Source::of(p));
                self.source = self.scope;
                self.changed(true);
            }
            self.request = request;
            return true;
        }
        // A native draw/DOM ACK may complete after the open gesture. A later
        // trusted edit can establish the same local search once that source is
        // confirmed; it still cannot read an unconfirmed frame or send bytes.
        if !self.opened && matches!(action, Action::Pending | Action::Edit { .. }) {
            self.opened = true;
            self.scope = Some(Source::of(p));
            self.source = self.scope;
        }
        if !self.opened
            || self
                .scope
                .is_none_or(|scope| !scope.same_scope(Source::of(p)))
        {
            return false;
        }
        self.request = request;
        self.sync(p);
        match action {
            Action::Pending => {
                self.pending = true;
                self.jump = false;
                self.clear_hits();
                self.changed(true);
            }
            Action::Edit { query } => {
                self.query = bounded_query(&query);
                self.pending = false;
                self.anchor = None;
                self.search(p, false);
                self.jump = self.opened;
            }
            Action::Step { back } => {
                if self.pending {
                    return false;
                }
                if let Some(active) = self.active {
                    self.active = Some(if back {
                        (active + self.hits.len() - 1) % self.hits.len()
                    } else {
                        (active + 1) % self.hits.len()
                    });
                }
                self.changed(false);
                self.jump = true;
            }
            Action::Open | Action::Close => {}
        }
        true
    }
    fn search(&mut self, p: &Paint, preserve: bool) {
        let old = if preserve {
            self.active().map(|h| (h.row, h.start)).or(self.anchor)
        } else {
            None
        };
        self.source = Some(Source::of(p));
        self.clear_hits();
        if self.query.is_empty() || self.query.contains('\0') {
            self.index.clear();
            self.anchor = None;
            self.changed(true);
            return;
        }
        let rebuild = self.index.source != self.source || self.index.rows != body_rows(p);
        if !self.index.build(p) {
            self.close(self.request);
            return;
        }
        if rebuild {
            self.builds = self.builds.saturating_add(1);
        }
        (self.hits, self.limited) = self.index.search(&self.query);
        self.active = (!self.hits.is_empty()).then(|| {
            self.hits
                .iter()
                .position(|h| Some((h.row, h.start)) == old)
                .unwrap_or(0)
        });
        self.anchor = None;
        self.changed(true);
    }
    /// Display-only override; callers must never use decorated frames as PNG,
    /// clipboard, link or selection sources.
    pub fn style(&self, row: usize, col: usize, width: usize) -> Option<Style> {
        if !self.opened || self.pending || self.source.is_none() {
            return None;
        }
        let begin = self.hits.partition_point(|h| h.row < row);
        let mut normal = false;
        for hit in self.hits[begin..].iter().take_while(|h| h.row == row) {
            if hit.start < col.saturating_add(width) && hit.end > col {
                if Some(hit.index) == self.active {
                    return Some(SELECTED);
                }
                normal = true;
            }
        }
        normal.then_some(NORMAL)
    }
    pub fn decorate(&self, source: Source, frame: &mut Frame, first_row: usize) {
        if self.source != Some(source) {
            return;
        }
        for glyph in &mut frame.glyphs {
            if let Some(style) = self.style(first_row + glyph.row, glyph.col, glyph.width) {
                glyph.left = style;
                glyph.right = style;
            }
        }
    }
}

#[cfg(test)]
#[path = "article_find_tests.rs"]
mod tests;
