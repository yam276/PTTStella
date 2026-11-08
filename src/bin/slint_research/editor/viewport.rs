//! Local editor coordinates consume Document's projection, never ANSI bytes.
use pttstella::{
    composer::document::{Affinity, Document, Position, Selection},
    screen::{Frame, Glyph},
};
use std::ops::Range;

pub const COLUMNS: usize = 79;
pub const ROWS: usize = 23;
#[derive(Default)]
pub struct Viewport {
    pub top: usize,
    pub left: usize,
    preferred: Option<usize>,
    pub rectangle: Option<(Position, Position)>,
    dragging: bool,
    all: bool,
}
impl Viewport {
    pub fn reveal(&mut self, document: &Document) -> Result<(), String> {
        let p = document.position(document.selection().head)?;
        if p.row < self.top {
            self.top = p.row;
        }
        if p.row >= self.top + ROWS {
            self.top = p.row + 1 - ROWS;
        }
        if p.col < self.left {
            self.left = p.col;
        }
        if p.col >= self.left + COLUMNS {
            self.left = p.col + 1 - COLUMNS;
        }
        self.top = self.top.min(document.rows().len().saturating_sub(1));
        Ok(())
    }
    pub fn scroll(&mut self, document: &Document, rows: i32, columns: i32) {
        self.top = self
            .top
            .saturating_add_signed(rows as isize)
            .min(document.rows().len().saturating_sub(1));
        let longest = document
            .rows()
            .iter()
            .map(|r| r.columns(document.mode()))
            .max()
            .unwrap_or(0);
        self.left = self
            .left
            .saturating_add_signed(columns as isize)
            .min(longest.saturating_sub(1));
    }
    pub fn ranges(&self, document: &Document) -> Result<Vec<Range<usize>>, String> {
        if self.all {
            return Ok(std::iter::once(0..document.source().len()).collect());
        }
        match self.rectangle {
            Some((a, b)) => document.rectangle(
                a.row.min(b.row)..a.row.max(b.row) + 1,
                a.col.min(b.col)..a.col.max(b.col),
            ),
            None => Ok(vec![document.selected_range()?]),
        }
    }
    pub fn pointer(
        &mut self,
        document: &mut Document,
        kind: i32,
        row: usize,
        col: usize,
        extend: bool,
        rectangular: bool,
    ) -> Result<bool, String> {
        if kind == 1 && !self.dragging {
            return Ok(false);
        }
        let before = (document.selection(), self.rectangle);
        self.all = false;
        let position = Position {
            row: (self.top + row).min(document.rows().len() - 1),
            col: self.left + col,
        };
        let raw = document.offset(position, Affinity::Before)?;
        match kind {
            0 => {
                self.dragging = true;
                self.preferred = None;
                self.rectangle = rectangular.then_some((position, position));
                let anchor = if extend {
                    document.selection().anchor
                } else {
                    raw
                };
                document.select(document.version(), Selection { anchor, head: raw })?;
            }
            1 | 2 if self.dragging => {
                if let Some((anchor, _)) = self.rectangle {
                    self.rectangle = Some((anchor, position));
                }
                let anchor = document.position(document.selection().anchor)?;
                let forward = (position.row, position.col) >= (anchor.row, anchor.col);
                let head = document.offset(
                    position,
                    if forward {
                        Affinity::After
                    } else {
                        Affinity::Before
                    },
                )?;
                document.select(
                    document.version(),
                    Selection {
                        head,
                        ..document.selection()
                    },
                )?;
                if kind == 2 {
                    self.dragging = false;
                }
            }
            3 => self.dragging = false,
            4 => {
                self.rectangle = None;
                let from = document
                    .row_start(position.row)
                    .ok_or("invalid editor row")?;
                let to = document
                    .row_start(position.row + 1)
                    .unwrap_or(document.source().len());
                document.select(
                    document.version(),
                    Selection {
                        anchor: from,
                        head: to,
                    },
                )?;
            }
            _ => {}
        }
        Ok(before != (document.selection(), self.rectangle))
    }
    pub fn stop_gesture(&mut self) {
        self.dragging = false;
    }
    pub fn clear_rectangle(&mut self) {
        self.rectangle = None;
        self.preferred = None;
        self.all = false;
    }
    pub fn select_all(&mut self, document: &mut Document) -> Result<(), String> {
        self.clear_rectangle();
        document.select(
            document.version(),
            Selection {
                anchor: 0,
                head: document.source().len(),
            },
        )?;
        self.all = true;
        Ok(())
    }
    /// 0 left, 1 right, 2 up, 3 down, 4 home, 5 end, 6 page-up,
    /// 7 page-down, 8 document-start, 9 document-end.
    pub fn navigate(
        &mut self,
        document: &mut Document,
        key: i32,
        extend: bool,
    ) -> Result<(), String> {
        self.all = false;
        let current = document.selection();
        let mut p = document.position(current.head)?;
        if matches!(key, 0 | 1) && !extend && !current.range().is_empty() {
            let head = if key == 0 {
                current.range().start
            } else {
                current.range().end
            };
            document.select(document.version(), Selection { anchor: head, head })?;
            self.clear_rectangle();
            return self.reveal(document);
        }
        if !matches!(key, 2 | 3 | 6 | 7) {
            self.preferred = None;
        }
        let target = match key {
            0 => {
                if p.col == 0 && p.row > 0 {
                    p.row -= 1;
                    p.col = document.rows()[p.row].columns(document.mode());
                } else {
                    p.col = p.col.saturating_sub(1);
                }
                document.offset(p, Affinity::Before)?
            }
            1 => {
                if p.col >= document.rows()[p.row].columns(document.mode())
                    && p.row + 1 < document.rows().len()
                {
                    p.row += 1;
                    p.col = 0;
                } else {
                    p.col += 1;
                }
                document.offset(p, Affinity::After)?
            }
            2 | 3 | 6 | 7 => {
                p.col = *self.preferred.get_or_insert(p.col);
                let amount = if key >= 6 { ROWS } else { 1 };
                p.row = if matches!(key, 2 | 6) {
                    p.row.saturating_sub(amount)
                } else {
                    (p.row + amount).min(document.rows().len() - 1)
                };
                document.offset(p, Affinity::Before)?
            }
            4 => document.offset(Position { col: 0, ..p }, Affinity::After)?,
            5 => document.offset(
                Position {
                    col: usize::MAX,
                    ..p
                },
                Affinity::Before,
            )?,
            8 => 0,
            9 => document.source().len(),
            _ => return Ok(()),
        };
        self.rectangle = None;
        document.select(
            document.version(),
            Selection {
                anchor: if extend { current.anchor } else { target },
                head: target,
            },
        )?;
        self.reveal(document)
    }
    pub fn replace(&mut self, document: &mut Document, text: &str) -> Result<(), String> {
        if self.all {
            document.replace(document.version(), 0..document.source().len(), text)?;
        } else if self.rectangle.is_none() {
            document.replace_selection(document.version(), text)?;
        } else {
            let ranges = self.ranges(document)?;
            if let (Some(first), Some(last)) = (ranges.first(), ranges.last()) {
                let removed: usize = ranges.iter().map(|r| r.len()).sum();
                let inserted = text
                    .len()
                    .checked_mul(ranges.len())
                    .ok_or_else(|| pttstella::msg!("draft-limit"))?;
                if inserted > pttstella::drafts::MAX_BODY
                    || document.source().len() - removed > pttstella::drafts::MAX_BODY - inserted
                {
                    return Err(pttstella::msg!("draft-limit"));
                }
                let mut replacement = String::new();
                let mut at = first.start;
                for range in &ranges {
                    replacement.push_str(&document.source()[at..range.start]);
                    replacement.push_str(text);
                    at = range.end;
                }
                document.replace(document.version(), first.start..last.end, &replacement)?;
            }
        }
        self.clear_rectangle();
        self.reveal(document)
    }
    pub fn delete(&mut self, document: &mut Document, backward: bool) -> Result<(), String> {
        if self.all || self.rectangle.is_some() {
            return self.replace(document, "");
        }
        let selection = document.selection();
        if selection.range().is_empty() {
            self.navigate(document, if backward { 0 } else { 1 }, true)?;
        }
        document.replace_selection(document.version(), "")?;
        self.clear_rectangle();
        self.reveal(document)
    }
    pub fn frame(&self, document: &Document) -> Frame {
        let mut glyphs = Vec::new();
        for (row_index, row) in document.rows().iter().enumerate().skip(self.top).take(ROWS) {
            for glyph in row.glyphs(document.mode()) {
                if glyph.col >= self.left + COLUMNS {
                    break;
                }
                if glyph.col + glyph.width <= self.left {
                    continue;
                }
                if glyph.text == "\t" {
                    // Tabs have an arbitrary cell span; the painter consumes 1/2-cell glyphs.
                    for col in
                        glyph.col.max(self.left)..(glyph.col + glyph.width).min(self.left + COLUMNS)
                    {
                        glyphs.push(Glyph {
                            col,
                            row: row_index - self.top,
                            width: 1,
                            text: " ".into(),
                            left: glyph.left,
                            right: glyph.left,
                            unmapped: false,
                        });
                    }
                } else {
                    glyphs.push(Glyph {
                        col: glyph.col,
                        row: row_index - self.top,
                        width: glyph.width,
                        text: glyph.text.into(),
                        left: glyph.left,
                        right: glyph.right,
                        unmapped: glyph.unmapped,
                    });
                }
            }
        }
        Frame {
            cols: COLUMNS,
            rows: ROWS,
            cursor: (0, 0),
            cursor_visible: false,
            revision: document.revision(),
            glyphs,
            control_complete: true,
            hyperlinks: Vec::new(),
            unsupported: 0,
        }
    }
    /// Visible selections in cell coordinates. Wide glyph boundaries come from Document.
    pub fn highlights(&self, document: &Document) -> Result<Vec<(usize, usize, usize)>, String> {
        let mut spans = Vec::new();
        for range in self.ranges(document)? {
            if range.is_empty() {
                continue;
            }
            let start = document.position(range.start)?;
            let end = document.position(range.end)?;
            for row in start.row.max(self.top)..=end.row.min(self.top + ROWS - 1) {
                let from = if row == start.row { start.col } else { 0 };
                let to = if row == end.row {
                    end.col
                } else {
                    document.rows()[row].columns(document.mode()) + 1
                };
                let left = from.max(self.left);
                let right = to.min(self.left + COLUMNS);
                if left < right {
                    spans.push((row - self.top, left - self.left, right - left));
                }
            }
        }
        Ok(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::composer::document::Mode;
    fn doc(source: &str) -> Document {
        Document::new(1, "title".into(), source.into()).unwrap()
    }
    #[test]
    fn viewport_retains_left_half_clip_and_expands_tabs_without_losing_styles() {
        let mut d = doc("\x1b[41;66;44m中\tA\n末\n");
        let mut v = Viewport {
            left: 1,
            ..Default::default()
        };
        let frame = v.frame(&d);
        assert_eq!(frame.glyphs[0].text, "中");
        assert_eq!(frame.glyphs[0].col, 0);
        assert_eq!(frame.glyphs[0].left.bg, 1);
        assert_eq!(frame.glyphs[0].right.bg, 4);
        assert!(frame.glyphs.iter().all(|g| (1..=2).contains(&g.width)));
        assert_eq!(
            frame
                .glyphs
                .iter()
                .filter(|g| g.row == 0 && g.text == " ")
                .count(),
            6
        );
        let raw = d.source().to_owned();
        d.set_mode(Mode::Source);
        v.scroll(&d, 1, 1);
        assert_eq!(d.source(), raw);
        assert_eq!(d.rows().len(), 3);
    }
    #[test]
    fn explicit_select_all_replaces_outer_sgr_and_rectangular_edit_is_one_undo() {
        let mut d = doc("\x1b[31m中\x1b[m");
        let mut v = Viewport::default();
        v.select_all(&mut d).unwrap();
        v.replace(&mut d, "new").unwrap();
        assert_eq!(d.source(), "new");
        d.undo(d.version()).unwrap();
        assert_eq!(d.source(), "\x1b[31m中\x1b[m");
        let mut d = doc("A中B\nC文D\n");
        v.pointer(&mut d, 0, 0, 2, false, true).unwrap();
        v.pointer(&mut d, 2, 1, 3, false, true).unwrap();
        v.replace(&mut d, "X").unwrap();
        assert_eq!(d.source(), "AXB\nCXD\n");
        d.undo(d.version()).unwrap();
        assert_eq!(d.source(), "A中B\nC文D\n");
    }
    #[test]
    fn movement_and_delete_preserve_sgr_and_wide_glyphs_and_cross_newlines() {
        let mut d = doc("\x1b[31m中A\x1b[m\n二\n");
        let mut v = Viewport::default();
        v.navigate(&mut d, 1, false).unwrap();
        assert_eq!(
            d.position(d.selection().head).unwrap(),
            Position { row: 0, col: 2 }
        );
        v.delete(&mut d, true).unwrap();
        assert_eq!(d.source(), "\x1b[31mA\x1b[m\n二\n");
        d.undo(d.version()).unwrap();
        v.navigate(&mut d, 5, false).unwrap();
        v.delete(&mut d, false).unwrap();
        assert_eq!(d.source(), "\x1b[31m中A\x1b[m二\n");
    }
    #[test]
    fn rectangle_uses_whole_chinese_glyphs_and_highlights_only_visible_rows() {
        let mut d = doc("A中B\nC文D\n");
        let mut v = Viewport::default();
        v.pointer(&mut d, 0, 0, 2, false, true).unwrap();
        v.pointer(&mut d, 2, 1, 3, false, true).unwrap();
        let selected: Vec<_> = v
            .ranges(&d)
            .unwrap()
            .into_iter()
            .map(|r| &d.source()[r])
            .collect();
        assert_eq!(selected, ["中", "文"]);
        assert_eq!(v.highlights(&d).unwrap(), [(0, 1, 2), (1, 1, 2)]);
    }
}
