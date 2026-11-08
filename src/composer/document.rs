//! Native editor's exact raw document, monotonic revision and bounded history.
//! Views and cell geometry never replace the original source or its SGR bytes.
use super::projection::Cache;
pub use super::projection::{Affinity, Glyph, Mode, Position, Row, Update};
use crate::{drafts::MAX_BODY, editor_color::Color};
use std::{collections::VecDeque, ops::Range, sync::Arc};

const HISTORY_BYTES: usize = 8 * MAX_BODY;
const HISTORY_STEPS: usize = 256;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}
impl Selection {
    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }
}
/// Semantic selection belongs to the document transaction, not the viewport.
/// Rectangle columns retain the projection in which the gesture was made, so
/// history can restore exact glyph ranges even after switching display modes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionShape {
    #[default]
    Text,
    All,
    Lines,
    Rectangle {
        anchor: Position,
        head: Position,
        mode: Mode,
    },
}
#[derive(Clone, Copy)]
struct SelectionState {
    primary: Selection,
    shape: SelectionShape,
}
struct Transaction {
    from: usize,
    removed: String,
    inserted: String,
    title_before: String,
    title_after: String,
    selection_before: SelectionState,
    selection_after: SelectionState,
}
impl Transaction {
    fn bytes(&self) -> usize {
        self.removed.len() + self.inserted.len() + self.title_before.len() + self.title_after.len()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version {
    pub token: u64,
    pub revision: u64,
}
pub struct Document {
    token: u64,
    title: String,
    source: String,
    revision: u64,
    mode: Mode,
    selection: Selection,
    shape: SelectionShape,
    cache: Cache,
    undo: VecDeque<Transaction>,
    redo: Vec<Transaction>,
    history_bytes: usize,
}
fn invalid() -> String {
    crate::msg!("draft-invalid")
}
fn validate(title: &str, body: &str) -> Result<(), String> {
    if title.len() > 256
        || title.chars().any(char::is_control)
        || body.len() > MAX_BODY
        || body.bytes().filter(|b| *b == b'\n').count() >= 4096
    {
        return Err(crate::msg!("draft-limit"));
    }
    if body
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\x1b'))
    {
        return Err(invalid());
    }
    Ok(())
}
impl Document {
    pub fn new(token: u64, title: String, source: String) -> Result<Self, String> {
        if token == 0 {
            return Err(invalid());
        }
        validate(&title, &source)?;
        let cache = Cache::new(&source)?;
        Ok(Self {
            token,
            title,
            source,
            revision: 0,
            mode: Mode::Color,
            selection: Selection::default(),
            shape: SelectionShape::Text,
            cache,
            undo: VecDeque::new(),
            redo: Vec::new(),
            history_bytes: 0,
        })
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn version(&self) -> Version {
        Version {
            token: self.token,
            revision: self.revision,
        }
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn set_mode(&mut self, mode: Mode) {
        if mode != self.mode && matches!(self.shape, SelectionShape::Rectangle { .. }) {
            self.shape = SelectionShape::Text;
        }
        self.mode = mode;
    }
    pub fn selection(&self) -> Selection {
        self.selection
    }
    pub fn selection_shape(&self) -> SelectionShape {
        self.shape
    }
    fn selection_state(&self) -> SelectionState {
        SelectionState {
            primary: self.selection,
            shape: self.shape,
        }
    }
    pub fn rows(&self) -> &[Arc<Row>] {
        &self.cache.rows
    }
    pub fn row_start(&self, row: usize) -> Option<usize> {
        self.cache.starts.get(row).copied()
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn history_bytes(&self) -> usize {
        self.history_bytes
    }
    fn offset_valid(&self, p: usize) -> bool {
        p <= self.source.len() && self.source.is_char_boundary(p)
    }
    fn owns(&self, version: Version) -> Result<(), String> {
        if version == self.version() {
            Ok(())
        } else {
            Err(crate::msg!("editor-color-stale"))
        }
    }
    pub fn select(&mut self, version: Version, selection: Selection) -> Result<(), String> {
        self.select_with_shape(version, selection, SelectionShape::Text)
    }
    pub fn select_with_shape(
        &mut self,
        version: Version,
        selection: Selection,
        shape: SelectionShape,
    ) -> Result<(), String> {
        self.owns(version)?;
        if !self.offset_valid(selection.anchor) || !self.offset_valid(selection.head) {
            return Err(invalid());
        }
        match shape {
            SelectionShape::Rectangle { anchor, head, mode }
                if anchor.row >= self.rows().len()
                    || head.row >= self.rows().len()
                    || mode != self.mode =>
            {
                return Err(invalid());
            }
            SelectionShape::All if selection.range() != (0..self.source.len()) => {
                return Err(invalid());
            }
            SelectionShape::Lines
                if [selection.anchor, selection.head].into_iter().any(|p| {
                    p != self.source.len() && self.row_start(self.cache.row_at(p)) != Some(p)
                }) =>
            {
                return Err(invalid());
            }
            _ => {}
        }
        self.selection = selection;
        self.shape = shape;
        Ok(())
    }
    pub fn selected_ranges(&self) -> Result<Vec<Range<usize>>, String> {
        match self.shape {
            SelectionShape::All | SelectionShape::Lines => {
                Ok(std::iter::once(self.selection.range()).collect())
            }
            SelectionShape::Rectangle { anchor, head, mode } if anchor.col == head.col => {
                (anchor.row.min(head.row)..=anchor.row.max(head.row))
                    .map(|row| {
                        let at = self
                            .cache
                            .offset(
                                mode,
                                Position {
                                    row,
                                    col: anchor.col,
                                },
                                Affinity::After,
                            )
                            .ok_or_else(invalid)?;
                        Ok(at..at)
                    })
                    .collect()
            }
            SelectionShape::Rectangle { anchor, head, mode } => self.rectangle_in(
                mode,
                anchor.row.min(head.row)..anchor.row.max(head.row) + 1,
                anchor.col.min(head.col)..anchor.col.max(head.col),
            ),
            SelectionShape::Text => Ok(std::iter::once(self.selected_range()?).collect()),
        }
    }
    pub fn position(&self, raw: usize) -> Result<Position, String> {
        if !self.offset_valid(raw) {
            return Err(invalid());
        }
        Ok(self.cache.position(self.mode, raw))
    }
    pub fn offset(&self, position: Position, affinity: Affinity) -> Result<usize, String> {
        self.cache
            .offset(self.mode, position, affinity)
            .ok_or_else(invalid)
    }
    /// Rectangles are computed from cell columns on each logical row, not by
    /// cutting UTF-8 or the two byte halves of a Chinese character.
    pub fn rectangle(
        &self,
        rows: Range<usize>,
        columns: Range<usize>,
    ) -> Result<Vec<Range<usize>>, String> {
        self.rectangle_in(self.mode, rows, columns)
    }
    fn rectangle_in(
        &self,
        mode: Mode,
        rows: Range<usize>,
        columns: Range<usize>,
    ) -> Result<Vec<Range<usize>>, String> {
        if rows.start > rows.end || rows.end > self.cache.rows.len() || columns.start > columns.end
        {
            return Err(invalid());
        }
        let mut result = Vec::new();
        if rows.is_empty() || columns.is_empty() {
            return Ok(result);
        }
        for row in rows {
            let a = self
                .cache
                .offset(
                    mode,
                    Position {
                        row,
                        col: columns.start,
                    },
                    Affinity::Before,
                )
                .ok_or_else(invalid)?;
            let b = self
                .cache
                .offset(
                    mode,
                    Position {
                        row,
                        col: columns.end,
                    },
                    Affinity::After,
                )
                .ok_or_else(invalid)?;
            if a < b {
                result.push(a..b);
            }
        }
        Ok(result)
    }
    /// Hidden controls at the start/end stay outside a text replacement;
    /// controls inside a selected span are part of that single raw edit.
    pub fn selected_range(&self) -> Result<Range<usize>, String> {
        let range = self.selection.range();
        if self.mode == Mode::Source {
            return Ok(range);
        }
        let start = self.cache.position(self.mode, range.start);
        let end = self.cache.position(self.mode, range.end);
        let from = self.offset(start, Affinity::After)?;
        let to = if range.is_empty() {
            from
        } else {
            self.offset(end, Affinity::Before)?
        };
        Ok(from..to.max(from))
    }
    pub fn replace_selection(&mut self, revision: Version, insert: &str) -> Result<Update, String> {
        self.owns(revision)?;
        if matches!(self.shape, SelectionShape::Rectangle { .. }) {
            let ranges = self.selected_ranges()?;
            if let (Some(first), Some(last)) = (ranges.first(), ranges.last()) {
                let removed: usize = ranges.iter().map(|r| r.len()).sum();
                let inserted = insert
                    .len()
                    .checked_mul(ranges.len())
                    .ok_or_else(|| crate::msg!("draft-limit"))?;
                if inserted > MAX_BODY || self.source.len() - removed > MAX_BODY - inserted {
                    return Err(crate::msg!("draft-limit"));
                }
                let mut replacement = String::new();
                let mut at = first.start;
                for range in &ranges {
                    replacement.push_str(&self.source[at..range.start]);
                    replacement.push_str(insert);
                    at = range.end;
                }
                return self.replace(revision, first.start..last.end, &replacement);
            }
        }
        let range = if matches!(self.shape, SelectionShape::All | SelectionShape::Lines) {
            self.selection.range()
        } else {
            self.selected_range()?
        };
        self.replace(revision, range, insert)
    }
    pub fn replace(
        &mut self,
        revision: Version,
        range: Range<usize>,
        insert: &str,
    ) -> Result<Update, String> {
        self.apply(revision, range, insert, self.title.clone())
    }
    pub fn edit_title(&mut self, revision: Version, title: String) -> Result<Update, String> {
        self.apply(revision, 0..0, "", title)
    }
    fn apply(
        &mut self,
        revision: Version,
        range: Range<usize>,
        insert: &str,
        title: String,
    ) -> Result<Update, String> {
        self.owns(revision)?;
        if range.start > range.end
            || !self.offset_valid(range.start)
            || !self.offset_valid(range.end)
        {
            return Err(invalid());
        }
        if self.source[range.clone()] == *insert && title == self.title {
            return Ok(Update {
                reparsed_rows: 0,
                reused_rows: self.cache.rows.len(),
            });
        }
        let next = self
            .revision
            .checked_add(1)
            .ok_or_else(|| crate::msg!("draft-limit"))?;
        // Check the source budget before allocating/copying a pasted payload.
        if self
            .source
            .len()
            .saturating_sub(range.len())
            .checked_add(insert.len())
            .is_none_or(|bytes| bytes > MAX_BODY)
        {
            return Err(crate::msg!("draft-limit"));
        }
        let mut source = self.source.clone();
        source.replace_range(range.clone(), insert);
        validate(&title, &source)?;
        let selection = if source == self.source {
            self.selection_state()
        } else {
            SelectionState {
                primary: Selection {
                    anchor: range.start + insert.len(),
                    head: range.start + insert.len(),
                },
                shape: SelectionShape::Text,
            }
        };
        let transaction = Transaction {
            from: range.start,
            removed: self.source[range.clone()].into(),
            inserted: insert.into(),
            title_before: self.title.clone(),
            title_after: title.clone(),
            selection_before: self.selection_state(),
            selection_after: selection,
        };
        let update = if source == self.source {
            Update {
                reparsed_rows: 0,
                reused_rows: self.cache.rows.len(),
            }
        } else {
            self.cache.update(&source, range, insert)?
        };
        self.source = source;
        self.title = title;
        self.revision = next;
        self.selection = selection.primary;
        self.shape = selection.shape;
        self.history_bytes -= self.redo.iter().map(Transaction::bytes).sum::<usize>();
        self.redo.clear();
        self.history_bytes += transaction.bytes();
        self.undo.push_back(transaction);
        while self.history_bytes > HISTORY_BYTES || self.undo.len() > HISTORY_STEPS {
            if let Some(old) = self.undo.pop_front() {
                self.history_bytes -= old.bytes();
            } else {
                break;
            }
        }
        Ok(update)
    }
    pub fn undo(&mut self, revision: Version) -> Result<bool, String> {
        self.owns(revision)?;
        let Some(tx) = self.undo.back() else {
            return Ok(false);
        };
        let next = self
            .revision
            .checked_add(1)
            .ok_or_else(|| crate::msg!("draft-limit"))?;
        let range = tx.from..tx.from + tx.inserted.len();
        if self.source.get(range.clone()) != Some(tx.inserted.as_str()) {
            return Err(invalid());
        }
        let mut source = self.source.clone();
        source.replace_range(range.clone(), &tx.removed);
        if source != self.source {
            self.cache.update(&source, range, &tx.removed)?;
        }
        self.source = source;
        self.title = tx.title_before.clone();
        self.selection = tx.selection_before.primary;
        self.shape = tx.selection_before.shape;
        self.revision = next;
        if let Some(tx) = self.undo.pop_back() {
            self.redo.push(tx);
        }
        Ok(true)
    }
    pub fn redo(&mut self, revision: Version) -> Result<bool, String> {
        self.owns(revision)?;
        let Some(tx) = self.redo.last() else {
            return Ok(false);
        };
        let next = self
            .revision
            .checked_add(1)
            .ok_or_else(|| crate::msg!("draft-limit"))?;
        let range = tx.from..tx.from + tx.removed.len();
        if self.source.get(range.clone()) != Some(tx.removed.as_str()) {
            return Err(invalid());
        }
        let mut source = self.source.clone();
        source.replace_range(range.clone(), &tx.inserted);
        if source != self.source {
            self.cache.update(&source, range, &tx.inserted)?;
        }
        self.source = source;
        self.title = tx.title_after.clone();
        self.selection = tx.selection_after.primary;
        self.shape = tx.selection_after.shape;
        self.revision = next;
        if let Some(tx) = self.redo.pop() {
            self.undo.push_back(tx);
        }
        Ok(true)
    }
    pub fn color(
        &mut self,
        revision: Version,
        ranges: &[Range<usize>],
        color: Color,
    ) -> Result<Update, String> {
        self.owns(revision)?;
        if ranges.is_empty()
            || ranges.len() > 4096
            || ranges.iter().any(|r| r.start >= r.end)
            || ranges.windows(2).any(|r| r[0].end > r[1].start)
        {
            return Err(crate::msg!("editor-color-empty"));
        }
        let mut converted = Vec::with_capacity(ranges.len());
        let (mut byte, mut units) = (0, 0);
        for range in ranges {
            if range.start > range.end
                || !self.offset_valid(range.start)
                || !self.offset_valid(range.end)
            {
                return Err(invalid());
            }
            // Rectangular ranges are ordered. Advance one UTF-8/UTF-16 cursor
            // over the document instead of re-counting the prefix per row.
            while byte < range.start {
                let ch = self.source[byte..].chars().next().ok_or_else(invalid)?;
                byte += ch.len_utf8();
                units += ch.len_utf16();
            }
            let from = units;
            while byte < range.end {
                let ch = self.source[byte..].chars().next().ok_or_else(invalid)?;
                byte += ch.len_utf8();
                units += ch.len_utf16();
            }
            converted.push(super::Range { from, to: units });
        }
        let changes = super::color(&self.source, &converted, color)?;
        let Some(change) = changes.first() else {
            return Ok(Update {
                reparsed_rows: 0,
                reused_rows: self.cache.rows.len(),
            });
        };
        if changes.len() != 1 {
            return Err(invalid());
        }
        let from = utf16_byte(&self.source, change.from)?;
        let to = utf16_byte(&self.source, change.to)?;
        let before = self.selection;
        let shape = self.shape;
        let selection_mode = match shape {
            SelectionShape::Rectangle { mode, .. } => mode,
            _ => self.mode,
        };
        let (anchor, head) = (
            self.cache.position(selection_mode, self.selection.anchor),
            self.cache.position(selection_mode, self.selection.head),
        );
        let update = self.replace(revision, from..to, &change.insert)?;
        self.selection = if shape == SelectionShape::All {
            if before.anchor <= before.head {
                Selection {
                    anchor: 0,
                    head: self.source.len(),
                }
            } else {
                Selection {
                    anchor: self.source.len(),
                    head: 0,
                }
            }
        } else if shape == SelectionShape::Lines {
            let boundary = |p: Position| {
                if p.col == 0 {
                    self.row_start(p.row).unwrap_or(self.source.len())
                } else {
                    self.source.len()
                }
            };
            Selection {
                anchor: boundary(anchor),
                head: boundary(head),
            }
        } else if selection_mode == Mode::Source {
            // The source view includes inserted SGR columns: the old physical
            // columns would select the new escape syntax instead of the text.
            let map = |position: usize, after: bool| {
                if position < from {
                    position
                } else if position > to || position == to && to > from {
                    from + change.insert.len() + (position - to)
                } else if after {
                    from + change.insert.len()
                } else {
                    from
                }
            };
            Selection {
                anchor: map(before.anchor, before.anchor >= before.head),
                head: map(before.head, before.head >= before.anchor),
            }
        } else {
            Selection {
                anchor: self
                    .cache
                    .offset(selection_mode, anchor, Affinity::After)
                    .ok_or_else(invalid)?,
                head: self
                    .cache
                    .offset(selection_mode, head, Affinity::After)
                    .ok_or_else(invalid)?,
            }
        };
        self.shape = match shape {
            SelectionShape::Rectangle {
                mode: Mode::Source, ..
            } => SelectionShape::Text,
            _ => shape,
        };
        let after = self.selection_state();
        if let Some(tx) = self.undo.back_mut() {
            tx.selection_after = after;
        }
        Ok(update)
    }
}
fn utf16_byte(text: &str, offset: usize) -> Result<usize, String> {
    let mut position = 0;
    for (byte, ch) in text.char_indices() {
        if position == offset {
            return Ok(byte);
        }
        position += ch.len_utf16();
    }
    if position == offset {
        Ok(text.len())
    } else {
        Err(invalid())
    }
}

#[cfg(test)]
#[path = "document_tests.rs"]
mod tests;
