//! View-local article status. Station cells remain the source of copy and PNG output.
use pttstella::{
    article_cache::Reading,
    article_flow::Flow,
    screen::{Frame, Style},
};
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Percent {
    Unknown,
    Known(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Display {
    pub percent: Option<Percent>,
    page: Option<usize>,
    lines: Option<(usize, usize)>,
}

pub(super) fn display(
    reading: &Reading,
    flow: &Flow,
    row_height: u32,
    top: f64,
    bottom: f64,
    whole_rows: Option<Range<usize>>,
) -> Display {
    let rows = reading
        .last_line
        .saturating_sub(reading.first_line)
        .saturating_add(1);
    let content = flow.top(rows) as f64;
    let percent = reading.percent_cells.as_ref().map(|_| {
        if !reading.end {
            return Percent::Unknown;
        }
        // Match Dioxus: include inline media spacing but exclude the footer
        // and minimum-height terminal padding from the article length.
        let earlier = reading.first_line.saturating_sub(1) as f64 * f64::from(row_height);
        let total = (earlier + content).max(1.);
        let seen = earlier + bottom;
        Percent::Known(if seen >= total - 1. {
            100
        } else {
            (seen / total * 100.).floor().clamp(0., 99.) as u8
        })
    });
    let first = reading
        .first_line
        .saturating_add(flow.anchor(top).row.min(rows - 1));
    let last = reading.first_line.saturating_add(
        flow.anchor((bottom.min(content) - 0.01).max(top))
            .row
            .min(rows - 1),
    );
    let page = reading
        .pages
        .iter()
        .enumerate()
        .min_by_key(|(_, page)| page.line.abs_diff(first))
        .map(|(index, _)| index);
    let (first, last) =
        whole_rows
            .filter(|range| !range.is_empty())
            .map_or((first, last), |range| {
                (
                    reading.first_line + range.start.min(rows - 1),
                    reading.first_line + (range.end - 1).min(rows - 1),
                )
            });
    Display {
        percent,
        page,
        lines: reading
            .range_cells
            .as_ref()
            .map(|_| (first, last.max(first))),
    }
}

pub(super) fn changed(previous: &Reading, current: &Reading) -> bool {
    previous.first_line != current.first_line
        || previous.last_line != current.last_line
        || previous.end != current.end
        || previous.percent_cells != current.percent_cells
        || previous.page_cells != current.page_cells
        || previous.range_cells != current.range_cells
        || previous.pages != current.pages
}

fn field(cells: &Range<usize>, value: &str) -> (usize, String) {
    let width = cells.len();
    let text = if value.len() > width {
        "-".repeat(width)
    } else {
        format!("{value:>width$}")
    };
    (cells.start, text)
}

pub(super) fn frame(source: &Frame, reading: &Reading, display: Display) -> Frame {
    let mut fields = Vec::with_capacity(4);
    if let Some((cells, percent)) = reading.percent_cells.as_ref().zip(display.percent) {
        let text = match percent {
            Percent::Unknown => "-".repeat(cells.len().min(2)),
            Percent::Known(percent) => percent.to_string(),
        };
        fields.push(field(cells, &text));
    }
    let page = display.page.and_then(|index| reading.pages.get(index));
    if let Some((cells, page)) = reading.page_cells.as_ref().zip(page) {
        let text = page.number.to_string();
        if text.len() <= cells.len() {
            fields.push(field(cells, &text));
        }
    }
    if let Some((cells, (first, last))) = reading.range_cells.as_ref().zip(display.lines) {
        fields.push(field(&cells.first, &first.to_string()));
        fields.push(field(&cells.last, &last.to_string()));
    }
    let styles: Option<&[Style]> = page
        .map(|page| page.styles.as_slice())
        .filter(|styles| styles.len() == source.cols);
    let glyphs = source
        .glyphs
        .iter()
        .filter(|glyph| glyph.row + 1 == source.rows)
        .map(|glyph| {
            let mut glyph = glyph.clone();
            glyph.row = 0;
            if let Some(styles) = styles
                && let (Some(&left), Some(&right)) = (
                    styles.get(glyph.col),
                    styles.get(glyph.col + glyph.width - 1),
                )
            {
                glyph.left = left;
                glyph.right = right;
            }
            if glyph.width == 1
                && let Some(byte) = fields.iter().find_map(|(start, text)| {
                    glyph
                        .col
                        .checked_sub(*start)
                        .and_then(|index| text.as_bytes().get(index))
                        .copied()
                })
            {
                glyph.text = char::from(byte).to_string();
            }
            glyph
        })
        .collect();
    Frame {
        control_complete: source.control_complete,
        cols: source.cols,
        rows: 1,
        cursor: (0, 0),
        cursor_visible: false,
        revision: source.revision,
        glyphs,
        hyperlinks: Vec::new(),
        unsupported: source.unsupported,
    }
}
