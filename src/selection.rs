//! Selection consumes mapped glyphs; it never decodes terminal bytes again.
use crate::screen::Frame;
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
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
    // Screen and article projections are row ordered. Visit each selected row
    // slice once instead of rescanning the entire document for every line.
    // Keep the public Frame API's prior order semantics for hand-built frames.
    let ordered = frame
        .glyphs
        .windows(2)
        .all(|pair| pair[0].row <= pair[1].row);
    let mut begin = if ordered {
        frame.glyphs.partition_point(|g| g.row < start.row)
    } else {
        0
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
        let finish = if ordered {
            frame.glyphs.partition_point(|g| g.row <= row)
        } else {
            frame.glyphs.len()
        };
        for glyph in frame.glyphs[begin..finish]
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
        if ordered {
            begin = finish;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn workload(rows: usize, cols: usize) -> Frame {
        let mut frame = crate::screen::Screen::new(cols, 24).unwrap().frame();
        let glyph = frame.glyphs[0].clone();
        frame.rows = rows;
        frame.glyphs = (0..rows)
            .flat_map(|row| {
                (0..cols).map({
                    let glyph = glyph.clone();
                    move |col| {
                        let mut g = glyph.clone();
                        g.row = row;
                        g.col = col;
                        g.text = "A".into();
                        g
                    }
                })
            })
            .collect();
        frame
    }
    #[test]
    fn long_reverse_selection_preserves_endpoints_and_complete_middle_rows() {
        let frame = workload(837, 80);
        let selected = extract(
            &frame,
            &Selection::Range {
                anchor: Position { row: 602, col: 1 },
                focus: Position { row: 600, col: 7 },
            },
        )
        .unwrap();
        assert_eq!(
            selected.text,
            format!("{}\n{}\nAA", "A".repeat(73), "A".repeat(80))
        );
        assert_eq!(selected.rows, 600..603);
        assert_eq!(
            selected.spans,
            vec![
                Span {
                    row: 600,
                    start: 7,
                    end: 80
                },
                Span {
                    row: 601,
                    start: 0,
                    end: 80
                },
                Span {
                    row: 602,
                    start: 0,
                    end: 2
                }
            ]
        );
        let all = extract(&frame, &Selection::All).unwrap();
        assert_eq!(
            all.text,
            ("A".repeat(80) + "\n").repeat(836) + &"A".repeat(80)
        );
        assert_eq!(all.spans.len(), 837);
    }
    #[test]
    fn sparse_ordered_rows_keep_empty_lines_without_consuming_a_future_row() {
        let mut frame = workload(3, 4);
        frame.glyphs.retain(|g| g.row != 1);
        let selected = extract(&frame, &Selection::All).unwrap();
        assert_eq!(selected.text, "AAAA\n\nAAAA");
        assert_eq!(
            selected.spans,
            vec![
                Span {
                    row: 0,
                    start: 0,
                    end: 4
                },
                Span {
                    row: 2,
                    start: 0,
                    end: 4
                }
            ]
        );
    }
    #[test]
    fn unordered_public_frames_keep_the_original_per_row_glyph_order() {
        let mut frame = workload(2, 3);
        for (g, text) in frame.glyphs.iter_mut().zip(["A", "B", "C", "D", "E", "F"]) {
            g.text = text.into();
        }
        let original = frame.glyphs.clone();
        frame.glyphs = [4, 2, 3, 0, 5, 1].map(|i| original[i].clone()).to_vec();
        let selected = extract(&frame, &Selection::All).unwrap();
        assert_eq!(selected.text, "CAB\nEDF");
        assert_eq!(
            selected.spans,
            vec![
                Span {
                    row: 0,
                    start: 0,
                    end: 3
                },
                Span {
                    row: 1,
                    start: 0,
                    end: 3
                }
            ]
        );
    }
    #[test]
    #[ignore = "manual fixed release workload; no wall-time threshold in ordinary tests"]
    fn measure_long_article_selection() {
        let frame = workload(837, 80);
        let expected = ("A".repeat(80) + "\n").repeat(836) + &"A".repeat(80);
        let mut samples = Vec::new();
        for _ in 0..3 {
            assert_eq!(extract(&frame, &Selection::All).unwrap().text, expected);
        }
        for _ in 0..7 {
            let now = std::time::Instant::now();
            for _ in 0..15 {
                let text = extract(std::hint::black_box(&frame), &Selection::All).unwrap();
                assert_eq!(text.text.len(), expected.len());
                assert_eq!(text.spans.len(), 837);
                std::hint::black_box(text);
            }
            samples.push(now.elapsed().as_nanos() / 15);
        }
        println!(
            "selection_workload rows=837 cols=80 warmup=3 batches=7 iterations=15 ns_per_selection={samples:?}"
        );
    }
}
