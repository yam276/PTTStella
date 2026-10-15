//! A shared projection of complete frame glyphs to UTF-8 byte and cell ranges.
use crate::screen::{Frame, Glyph, Style};
use std::ops::Range;

pub struct Span<'a> {
    pub bytes: Range<usize>,
    pub glyph: &'a Glyph,
}
pub struct Row<'a> {
    pub text: String,
    pub spans: Vec<Span<'a>>,
}
impl Row<'_> {
    pub fn cells(&self, start: usize, end: usize) -> Option<&str> {
        let first = self.spans.iter().find(|s| s.glyph.col == start)?;
        let last = self
            .spans
            .iter()
            .find(|s| s.glyph.col + s.glyph.width == end)?;
        self.text.get(first.bytes.start..last.bytes.end)
    }
    pub fn mapped(&self) -> bool {
        self.spans.iter().all(|s| !s.glyph.unmapped)
    }
    pub fn bar_text(&self, text: &str) -> bool {
        let Some(start) = self.text.find(text) else {
            return false;
        };
        let end = start + text.len();
        self.spans
            .iter()
            .filter(|s| s.bytes.start < end && s.bytes.end > start)
            .all(|s| background(s.glyph.left) != 0 && background(s.glyph.right) != 0)
    }
}
fn background(style: Style) -> u8 {
    if style.inverse { style.fg } else { style.bg }
}

/// Reject malformed projections rather than inventing missing or overlapping cells.
pub fn rows(frame: &Frame) -> Option<Vec<Row<'_>>> {
    if frame.cols == 0 || frame.rows == 0 {
        return None;
    }
    let mut rows: Vec<Row<'_>> = (0..frame.rows)
        .map(|_| Row {
            text: String::new(),
            spans: Vec::new(),
        })
        .collect();
    let (mut row, mut col) = (0, 0);
    for glyph in &frame.glyphs {
        if glyph.row != row
            || glyph.col != col
            || !matches!(glyph.width, 1 | 2)
            || col + glyph.width > frame.cols
        {
            return None;
        }
        let line = rows.get_mut(row)?;
        let start = line.text.len();
        line.text.push_str(&glyph.text);
        line.spans.push(Span {
            bytes: start..line.text.len(),
            glyph,
        });
        col += glyph.width;
        if col == frame.cols {
            row += 1;
            col = 0;
        }
    }
    (row == frame.rows && col == 0).then_some(rows)
}
