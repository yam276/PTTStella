//! Local account presentation. Source frames and station coordinates remain unchanged.
use crate::{
    navigation,
    screen::{Frame, Glyph, Style},
    text,
};
use serde::{Deserialize, Serialize};

pub mod import;
#[cfg(feature = "desktop")]
pub mod store;

pub const MAX_BYTES: usize = 256 * 1024;
pub const MAX_ENTRIES: usize = 512;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    #[default]
    Mask,
    MaskRow,
    Allow,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub note: String,
    #[serde(default)]
    pub policy: Policy,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub version: u8,
    pub entries: Vec<Entry>,
}
impl Default for Rules {
    fn default() -> Self {
        Self {
            version: 3,
            entries: Vec::new(),
        }
    }
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 12
        && id.as_bytes()[0].is_ascii_alphabetic()
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
impl Rules {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.version, 1..=3)
            || self.version == 1 && self.entries.iter().any(|e| e.policy != Policy::Mask)
            || self.version < 3 && self.entries.iter().any(|e| e.policy == Policy::MaskRow)
        {
            return Err(crate::msg!("filters-version"));
        }
        if self.entries.len() > MAX_ENTRIES {
            return Err(crate::msg!("filters-capacity"));
        }
        for (i, entry) in self.entries.iter().enumerate() {
            if !valid_id(&entry.id) {
                return Err(crate::msg!("filters-id-invalid"));
            }
            if entry.note.chars().count() > 128 || entry.note.chars().any(char::is_control) {
                return Err(crate::msg!("filters-note-invalid"));
            }
            if self.entries[..i]
                .iter()
                .any(|e| e.id.eq_ignore_ascii_case(&entry.id))
            {
                return Err(crate::msg!("filters-duplicate"));
            }
        }
        Ok(())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err(crate::msg!("filters-too-large"));
        }
        let mut rules: Self =
            serde_json::from_slice(bytes).map_err(|_| crate::msg!("filters-invalid"))?;
        rules.validate().map_err(|error| error.to_string())?;
        rules.version = 3;
        Ok(rules)
    }
    pub fn edit(&self, id: &str, note: &str, remove: bool) -> Result<Self, String> {
        self.edit_policy(id, note, Policy::Mask, remove)
    }
    pub fn edit_policy(
        &self,
        id: &str,
        note: &str,
        policy: Policy,
        remove: bool,
    ) -> Result<Self, String> {
        if !valid_id(id) {
            return Err(crate::msg!("filters-id-invalid"));
        }
        let mut next = self.clone();
        next.version = 3;
        let existing = next
            .entries
            .iter()
            .position(|e| e.id.eq_ignore_ascii_case(id));
        match (existing, remove) {
            (Some(i), true) => {
                next.entries.remove(i);
            }
            (None, true) => return Err(crate::msg!("filters-id-missing")),
            (Some(i), false) => {
                next.entries[i] = Entry {
                    id: id.into(),
                    note: note.into(),
                    policy,
                }
            }
            (None, false) => next.entries.push(Entry {
                id: id.into(),
                note: note.into(),
                policy,
            }),
        }
        next.validate().map_err(|error| error.to_string())?;
        Ok(next)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Push {
    pub row: usize,
    pub id: String,
    pub start: usize,
    pub end: usize,
}
fn colour(style: Style, fg: u8, bright: bool) -> bool {
    style.fg == fg && style.bg == 0 && style.bright == bright && !style.inverse
}
fn span_colour(row: &text::Row<'_>, start: usize, end: usize, fg: u8, bright: bool) -> bool {
    row.spans
        .iter()
        .filter(|s| s.glyph.col < end && s.glyph.col + s.glyph.width > start)
        .all(|s| colour(s.glyph.left, fg, bright) && colour(s.glyph.right, fg, bright))
}
fn timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 11 || b[2] != b'/' || b[5] != b' ' || b[8] != b':' {
        return false;
    }
    let number = |a: usize| -> Option<u8> {
        (b[a].is_ascii_digit() && b[a + 1].is_ascii_digit())
            .then(|| (b[a] - b'0') * 10 + b[a + 1] - b'0')
    };
    matches!(
        (number(0), number(3), number(6), number(9)),
        (Some(1..=12), Some(1..=31), Some(0..=23), Some(0..=59))
    )
}
/// Normal pmore footer, complete mapping, station push colours, ID delimiter and
/// fixed right-hand timestamp must all agree. Plain quoted "推 ID:" is not enough.
/// Wrapped/truncated/anonymous/legacy formats remain untouched; never infer a
/// continuation's author from the preceding row or a different screen page.
pub fn pushes(frame: &Frame) -> Vec<Push> {
    if !matches!(
        navigation::analyze(frame),
        navigation::Context::ArticleReading { .. }
    ) {
        return Vec::new();
    }
    let Some(rows) = text::rows(frame) else {
        return Vec::new();
    };
    rows.iter()
        .take(23)
        .enumerate()
        .filter_map(|(n, row)| {
            let marker = row.cells(0, 2)?;
            let fg = match marker {
                "推" => 7,
                "噓" | "→" => 1,
                _ => return None,
            };
            if row.cells(2, 3)? != " "
                || !span_colour(row, 0, 3, fg, true)
                || !timestamp(row.cells(67, 78)?)
                || row.cells(78, 80)?.trim() != ""
                || !span_colour(row, 66, 78, 7, false)
            {
                return None;
            }
            let colon = row
                .spans
                .iter()
                .find(|s| s.glyph.col >= 4 && s.glyph.col <= 15 && s.glyph.text == ":")?
                .glyph
                .col;
            let id = row.cells(3, colon)?.trim_end();
            if !valid_id(id)
                || !span_colour(row, 3, colon, 3, true)
                || !span_colour(row, colon, colon + 1, 3, false)
            {
                return None;
            }
            Some(Push {
                row: n,
                id: id.into(),
                start: colon + 1,
                end: 66,
            })
        })
        .collect()
}

pub struct Projection {
    pub frame: Frame,
    pub matches: Vec<Push>,
    pub revealed: bool,
}
struct Target {
    content: Push,
    id_start: usize,
}
fn targets(frame: &Frame) -> Vec<Target> {
    match navigation::analyze(frame) {
        navigation::Context::ArticleList { articles, .. } => {
            let Some(rows) = text::rows(frame) else {
                return Vec::new();
            };
            articles
                .iter()
                .filter_map(|article| {
                    let field = rows.get(article.row)?.cells(17, 30)?;
                    let id = field.trim();
                    if !valid_id(id) {
                        return None;
                    }
                    Some(Target {
                        content: Push {
                            row: article.row,
                            id: id.into(),
                            start: 30,
                            end: 80,
                        },
                        id_start: 17 + field.len() - field.trim_start().len(),
                    })
                })
                .collect()
        }
        navigation::Context::ArticleReading { .. } => pushes(frame)
            .into_iter()
            .map(|content| Target {
                content,
                id_start: 3,
            })
            .collect(),
        _ => Vec::new(),
    }
}
pub fn project(frame: &Frame, rules: &Rules, revealed: bool) -> Projection {
    let mut matches = Vec::new();
    let mut highlights = Vec::new();
    if rules.validate().is_ok() {
        for target in targets(frame) {
            let Some(entry) = rules
                .entries
                .iter()
                .find(|e| e.id.eq_ignore_ascii_case(&target.content.id))
            else {
                continue;
            };
            let mut content = target.content;
            match entry.policy {
                Policy::Allow => {
                    content.start = target.id_start;
                    content.end = target.id_start + content.id.len();
                    highlights.push(content);
                }
                Policy::Mask => matches.push(content),
                Policy::MaskRow => {
                    content.start = 0;
                    content.end = frame.cols;
                    matches.push(content);
                }
            }
        }
    }
    let mut output = frame.clone();
    if !revealed {
        mask_ranges(&mut output, &matches);
    }
    for glyph in &mut output.glyphs {
        if highlights
            .iter()
            .any(|p| p.row == glyph.row && glyph.col >= p.start && glyph.col + glyph.width <= p.end)
        {
            for style in [&mut glyph.left, &mut glyph.right] {
                // Preserve the effective background, including inverse rows.
                let background = if style.inverse { style.fg } else { style.bg };
                style.fg = 6;
                style.bg = background;
                style.bright = true;
                style.inverse = false;
            }
        }
    }
    Projection {
        frame: output,
        matches,
        revealed,
    }
}

/// Shared cell replacement for content masks and repeated reply identities.
fn mask_ranges(frame: &mut Frame, ranges: &[Push]) {
    if ranges.is_empty() {
        return;
    }
    let mut redacted = Vec::new();
    frame.hyperlinks.retain_mut(|link| {
        link.spans.retain(|span| {
            let hidden = ranges
                .iter()
                .any(|push| push.row == span.row && span.start < push.end && span.end > push.start);
            if hidden {
                redacted.push(span.clone());
            }
            !hidden
        });
        !link.spans.is_empty()
    });
    if !redacted.is_empty() {
        frame.hyperlinks.push(crate::screen::hyperlinks::Hyperlink {
            target: None,
            spans: redacted,
        });
    }
    frame.glyphs = frame
        .glyphs
        .iter()
        .flat_map(|g| {
            if ranges
                .iter()
                .any(|p| p.row == g.row && g.col < p.end && g.col + g.width > p.start)
            {
                (g.col..g.col + g.width)
                    .map(|col| Glyph {
                        col,
                        row: g.row,
                        width: 1,
                        text: "█".into(),
                        left: Style {
                            fg: 0,
                            ..Style::default()
                        },
                        right: Style {
                            fg: 0,
                            ..Style::default()
                        },
                        unmapped: false,
                    })
                    .collect()
            } else {
                vec![g.clone()]
            }
        })
        .collect();
}

/// Merge only adjacent, independently recognized replies. The caller keeps
/// identities in Rust so masking or highlighting never becomes parser input.
pub fn merge_push_ids(frame: &mut Frame, replies: &[Push]) {
    let ranges: Vec<_> = replies
        .windows(2)
        .filter_map(|pair| {
            let (previous, current) = (&pair[0], &pair[1]);
            (previous.row + 1 == current.row && previous.id.eq_ignore_ascii_case(&current.id)).then(
                || Push {
                    row: current.row,
                    id: current.id.clone(),
                    start: 3,
                    end: 3 + current.id.len(),
                },
            )
        })
        .collect();
    mask_ranges(frame, &ranges);
}

/// Policy/reveal revision is separate from the immutable source frame revision.
#[derive(Default)]
pub struct State {
    rules: Rules,
    version: u64,
    revealed: Option<(u64, u64)>,
}
impl State {
    pub fn new(rules: Rules) -> Self {
        Self {
            rules,
            ..Self::default()
        }
    }
    pub fn rules(&self) -> &Rules {
        &self.rules
    }
    pub fn version(&self) -> u64 {
        self.version
    }
    pub fn replace(&mut self, rules: Rules) {
        self.rules = rules;
        self.revealed = None;
        self.version = self.version.saturating_add(1);
    }
    pub fn reveal(&mut self, generation: u64, revision: u64, reveal: bool) {
        self.revealed = reveal.then_some((generation, revision));
        self.version = self.version.saturating_add(1);
    }
    pub fn sync(&mut self, generation: u64, revision: u64) {
        if self
            .revealed
            .is_some_and(|old| old != (generation, revision))
        {
            self.revealed = None;
            self.version = self.version.saturating_add(1);
        }
    }
    pub fn project(&self, generation: u64, frame: &Frame) -> Projection {
        project(
            frame,
            &self.rules,
            self.revealed == Some((generation, frame.revision)),
        )
    }
}
