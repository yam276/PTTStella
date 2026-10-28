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
    Link { index: usize },
    Range { anchor: Position, focus: Position },
    Line { row: usize },
    All,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Span {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

/// Serialize only the acknowledged selection's glyphs, never hidden source text.
/// PTT's SGR 66 latches the first byte's style inside one SGR, preserving a
/// two-colour Big5 glyph without splitting Unicode in the system clipboard.
pub fn ansi(frame: &Frame, spans: &[Span], rows: Range<usize>) -> String {
    use crate::screen::Style;
    use std::fmt::Write;
    fn parameters(style: Style) -> String {
        let fg = if style.fg < 8 {
            30 + style.fg
        } else {
            90 + style.fg - 8
        };
        let bg = if style.bg < 8 {
            40 + style.bg
        } else {
            100 + style.bg - 8
        };
        format!(
            "0;{};{};{fg};{bg}",
            if style.bright { 1 } else { 22 },
            if style.inverse { 7 } else { 27 }
        )
    }
    let mut output = String::new();
    let mut glyphs = frame.glyphs.iter().peekable();
    let mut spans = spans.iter().peekable();
    for row in rows.clone() {
        if row != rows.start {
            output.push('\n');
        }
        output.push_str("\x1b[0m");
        let mut current = Style::default();
        while glyphs.peek().is_some_and(|g| g.row < row) {
            glyphs.next();
        }
        while spans.peek().is_some_and(|s| s.row < row) {
            spans.next();
        }
        while let Some(glyph) = glyphs.next_if(|g| g.row == row) {
            if !spans.peek().is_some_and(|s| {
                s.row == row && glyph.col >= s.start && glyph.col + glyph.width <= s.end
            }) {
                continue;
            }
            if glyph.width == 2 && glyph.left != glyph.right {
                let _ = write!(
                    output,
                    "\x1b[{};66;{}m",
                    parameters(glyph.left),
                    parameters(glyph.right)
                );
                current = glyph.right;
            } else if glyph.left != current {
                let _ = write!(output, "\x1b[{}m", parameters(glyph.left));
                current = glyph.left;
            }
            output.push_str(&glyph.text);
        }
        output.push_str("\x1b[0m");
    }
    output
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
        Selection::Link { index } => {
            let link = crate::links::detect(frame)
                .into_iter()
                .nth(index)
                .ok_or_else(|| crate::msg!("selection-link-missing"))?;
            return Ok(SelectedText {
                text: link.url,
                spans: vec![Span {
                    row: link.row,
                    start: link.start,
                    end: link.end,
                }],
                rows: link.row..link.row + 1,
            });
        }
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
