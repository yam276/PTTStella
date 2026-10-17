//! Selection consumes mapped glyphs; it never decodes terminal bytes again.
use crate::screen::Frame;
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Position {
    pub col: usize,
    pub row: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Selection {
    Range { anchor: Position, focus: Position },
    Line { row: usize },
    All,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Span {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct SelectedText {
    pub text: String,
    pub spans: Vec<Span>,
    /// Complete rows covered by the selection, including blank cells.
    pub rows: Range<usize>,
}

/// Inclusive cell endpoints expand to whole glyphs. Text retains all selected spaces.
pub fn extract(frame: &Frame, selection: &Selection) -> Result<SelectedText, String> {
    let (anchor, focus) = match *selection {
        Selection::Range { anchor, focus } => (anchor, focus),
        Selection::Line { row } => (
            Position { col: 0, row },
            Position {
                col: frame.cols.saturating_sub(1),
                row,
            },
        ),
        Selection::All => (
            Position { col: 0, row: 0 },
            Position {
                col: frame.cols.saturating_sub(1),
                row: frame.rows.saturating_sub(1),
            },
        ),
    };
    if frame.cols == 0
        || frame.rows == 0
        || [anchor, focus]
            .iter()
            .any(|p| p.col >= frame.cols || p.row >= frame.rows)
    {
        return Err(crate::msg!("selection-outside"));
    }
    let (start, end) = if (anchor.row, anchor.col) <= (focus.row, focus.col) {
        (anchor, focus)
    } else {
        (focus, anchor)
    };
    let mut result = SelectedText {
        text: String::new(),
        spans: Vec::new(),
        rows: start.row..end.row + 1,
    };
    for row in start.row..=end.row {
        if row != start.row {
            result.text.push('\n');
        }
        let left = if row == start.row { start.col } else { 0 };
        let right = if row == end.row {
            end.col + 1
        } else {
            frame.cols
        };
        let mut span = Span {
            row,
            start: right,
            end: left,
        };
        for glyph in frame
            .glyphs
            .iter()
            .filter(|g| g.row == row && g.col < right && g.col + g.width > left)
        {
            result.text.push_str(&glyph.text);
            span.start = span.start.min(glyph.col);
            span.end = span.end.max(glyph.col + glyph.width);
        }
        if span.start < span.end {
            result.spans.push(span);
        }
    }
    Ok(result)
}
