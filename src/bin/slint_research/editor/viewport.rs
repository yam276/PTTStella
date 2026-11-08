//! Local editor coordinates consume Document's projection, never ANSI bytes.
use pttstella::{
    composer::document::{Affinity, Document, Position, Selection, SelectionShape},
    screen::{Frame, Glyph},
};
use std::ops::Range;

pub const COLUMNS: usize = 79;
pub const ROWS: usize = 23;
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum SelectionMode {
    #[default]
    Text,
    Line,
    Rectangle,
}

#[derive(Default)]
pub struct Viewport {
    pub top: usize,
    pub left: usize,
    preferred: Option<usize>,
    dragging: Option<Position>,
    pub selection_mode: SelectionMode,
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
        document.selected_ranges()
    }
    fn rectangle(document: &Document) -> Option<(Position, Position)> {
        match document.selection_shape() {
            SelectionShape::Rectangle { anchor, head, .. } => Some((anchor, head)),
            _ => None,
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
        if kind == 1 && self.dragging.is_none() {
            return Ok(false);
        }
        let before = (document.selection(), document.selection_shape());
        let position = Position {
            row: (self.top + row).min(document.rows().len() - 1),
            col: self.left + col,
        };
        let raw = document.offset(position, Affinity::Before)?;
        match kind {
            0 => {
                self.dragging = Some(
                    if extend && self.selection_mode == SelectionMode::Line && !rectangular {
                        document.position(document.selection().anchor)?
                    } else {
                        position
                    },
                );
                self.preferred = None;
                let anchor = if extend {
                    document.selection().anchor
                } else {
                    raw
                };
                document.select_with_shape(
                    document.version(),
                    Selection { anchor, head: raw },
                    if rectangular || self.selection_mode == SelectionMode::Rectangle {
                        SelectionShape::Rectangle {
                            anchor: position,
                            head: position,
                            mode: document.mode(),
                        }
                    } else {
                        SelectionShape::Text
                    },
                )?;
            }
            1 | 2 if self.dragging.is_some() => {
                let rectangle = Self::rectangle(document);
                if self.selection_mode == SelectionMode::Line && rectangle.is_none() {
                    let origin = self.dragging.ok_or("editor gesture missing")?;
                    let first = origin.row.min(position.row);
                    let last = origin.row.max(position.row);
                    let from = document.row_start(first).ok_or("invalid editor row")?;
                    let to = document
                        .row_start(last + 1)
                        .unwrap_or(document.source().len());
                    let selection = if (position.row, position.col) < (origin.row, origin.col) {
                        Selection {
                            anchor: to,
                            head: from,
                        }
                    } else {
                        Selection {
                            anchor: from,
                            head: to,
                        }
                    };
                    document.select_with_shape(
                        document.version(),
                        selection,
                        SelectionShape::Lines,
                    )?;
                    if kind == 2 {
                        self.dragging = None;
                    }
                    return Ok(before != (document.selection(), document.selection_shape()));
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
                document.select_with_shape(
                    document.version(),
                    Selection {
                        head,
                        ..document.selection()
                    },
                    rectangle.map_or(SelectionShape::Text, |(anchor, _)| {
                        SelectionShape::Rectangle {
                            anchor,
                            head: position,
                            mode: document.mode(),
                        }
                    }),
                )?;
                if kind == 2 {
                    self.dragging = None;
                }
            }
            3 => self.dragging = None,
            6 => {
                self.dragging = None;
                // A tap in rectangle mode is still an ordinary insertion caret.
                if Self::rectangle(document).is_some() {
                    document.select(document.version(), document.selection())?;
                }
            }
            4 => {
                let from = document
                    .row_start(position.row)
                    .ok_or("invalid editor row")?;
                let to = document
                    .row_start(position.row + 1)
                    .unwrap_or(document.source().len());
                document.select_with_shape(
                    document.version(),
                    Selection {
                        anchor: from,
                        head: to,
                    },
                    SelectionShape::Lines,
                )?;
            }
            _ => {}
        }
        if kind == 0 && extend && Self::rectangle(document).is_none() {
            self.normalize_lines(document)?;
        }
        Ok(before != (document.selection(), document.selection_shape()))
    }
    pub fn set_selection_mode(
        &mut self,
        document: &mut Document,
        mode: SelectionMode,
    ) -> Result<(), String> {
        self.stop_gesture();
        self.reset_motion();
        document.select(document.version(), document.selection())?;
        self.selection_mode = mode;
        self.normalize_lines(document)
    }
    fn normalize_lines(&self, document: &mut Document) -> Result<(), String> {
        let selection = document.selection();
        let range = selection.range();
        if self.selection_mode != SelectionMode::Line || range.is_empty() {
            return Ok(());
        }
        let a = document.position(range.start)?;
        let mut b = document.position(range.end)?;
        if b.col == 0 && b.row > a.row {
            b.row -= 1;
        }
        let from = document.row_start(a.row).ok_or("invalid editor row")?;
        let to = document
            .row_start(b.row + 1)
            .unwrap_or(document.source().len());
        document.select_with_shape(
            document.version(),
            if selection.anchor <= selection.head {
                Selection {
                    anchor: from,
                    head: to,
                }
            } else {
                Selection {
                    anchor: to,
                    head: from,
                }
            },
            SelectionShape::Lines,
        )
    }
    pub fn stop_gesture(&mut self) {
        self.dragging = None;
    }
    pub fn reset_motion(&mut self) {
        self.stop_gesture();
        self.preferred = None;
    }
    pub fn select_all(&mut self, document: &mut Document) -> Result<(), String> {
        self.reset_motion();
        document.select_with_shape(
            document.version(),
            Selection {
                anchor: 0,
                head: document.source().len(),
            },
            SelectionShape::All,
        )?;
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
        self.navigate_inner(document, key, extend, true)
    }
    fn navigate_inner(
        &mut self,
        document: &mut Document,
        key: i32,
        extend: bool,
        whole_lines: bool,
    ) -> Result<(), String> {
        let current = document.selection();
        let mut p = document.position(current.head)?;
        if matches!(key, 0 | 1) && !extend && !current.range().is_empty() {
            let head = if key == 0 {
                current.range().start
            } else {
                current.range().end
            };
            document.select(document.version(), Selection { anchor: head, head })?;
            self.reset_motion();
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
        document.select(
            document.version(),
            Selection {
                anchor: if extend { current.anchor } else { target },
                head: target,
            },
        )?;
        if whole_lines {
            self.normalize_lines(document)?;
        }
        self.reveal(document)
    }
    pub fn replace(&mut self, document: &mut Document, text: &str) -> Result<(), String> {
        document.replace_selection(document.version(), text)?;
        self.reset_motion();
        self.reveal(document)
    }
    pub fn delete(&mut self, document: &mut Document, backward: bool) -> Result<(), String> {
        if document.selection_shape() != SelectionShape::Text {
            return self.replace(document, "");
        }
        let selection = document.selection();
        if selection.range().is_empty() {
            self.navigate_inner(document, if backward { 0 } else { 1 }, true, false)?;
        }
        document.replace_selection(document.version(), "")?;
        self.reset_motion();
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
    fn line_mode_click_keeps_caret_and_drag_includes_raw_sgr_and_newlines() {
        let original = "\x1b[31mABC\x1b[m\n中文\n末\n";
        let mut d = doc(original);
        let mut view = Viewport::default();
        view.set_selection_mode(&mut d, SelectionMode::Line)
            .unwrap();
        view.pointer(&mut d, 0, 0, 1, false, false).unwrap();
        view.pointer(&mut d, 6, 0, 1, false, false).unwrap();
        assert!(d.selection().range().is_empty());
        assert_eq!(
            d.position(d.selection().head).unwrap(),
            Position { row: 0, col: 1 }
        );
        view.pointer(&mut d, 0, 0, 1, false, false).unwrap();
        view.pointer(&mut d, 2, 1, 1, false, false).unwrap();
        let ranges = view.ranges(&d).unwrap();
        assert_eq!(&d.source()[ranges[0].clone()], "\x1b[31mABC\x1b[m\n中文\n");
        view.replace(&mut d, "X\n").unwrap();
        assert_eq!(d.source(), "X\n末\n");
        d.undo(d.version()).unwrap();
        assert_eq!(d.source(), original);
    }
    #[test]
    fn line_mode_keyboard_boundary_and_caret_backspace_do_not_take_an_extra_row() {
        let mut d = doc("A\nB\nC");
        let mut view = Viewport::default();
        d.select(d.version(), Selection { anchor: 0, head: 2 })
            .unwrap();
        view.set_selection_mode(&mut d, SelectionMode::Line)
            .unwrap();
        let ranges = view.ranges(&d).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], 0..2);
        d.select(d.version(), Selection { anchor: 3, head: 3 })
            .unwrap();
        view.delete(&mut d, true).unwrap();
        assert_eq!(d.source(), "A\n\nC");
        let mut shift = doc("A\nB\nC\nD");
        view.set_selection_mode(&mut shift, SelectionMode::Line)
            .unwrap();
        view.pointer(&mut shift, 0, 1, 0, true, false).unwrap();
        view.pointer(&mut shift, 6, 1, 0, true, false).unwrap();
        let ranges = view.ranges(&shift).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(&shift.source()[ranges[0].clone()], "A\n");
        view.pointer(&mut shift, 0, 1, 1, true, false).unwrap();
        view.pointer(&mut shift, 6, 1, 1, true, false).unwrap();
        let ranges = view.ranges(&shift).unwrap();
        assert_eq!(&shift.source()[ranges[0].clone()], "A\nB\n");
        let mut d = doc("中A\n文B");
        view.set_selection_mode(&mut d, SelectionMode::Rectangle)
            .unwrap();
        view.pointer(&mut d, 0, 0, 1, false, false).unwrap();
        view.pointer(&mut d, 2, 1, 2, false, false).unwrap();
        assert_eq!(
            view.ranges(&d)
                .unwrap()
                .iter()
                .map(|r| &d.source()[r.clone()])
                .collect::<Vec<_>>(),
            ["中", "文"]
        );
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
    #[test]
    fn rectangle_history_restores_disjoint_ranges_without_replacing_intervening_text() {
        let mut d = doc("A中B\nC文D\n");
        let mut v = Viewport::default();
        v.pointer(&mut d, 0, 0, 2, false, true).unwrap();
        v.pointer(&mut d, 2, 1, 3, false, true).unwrap();
        v.replace(&mut d, "X").unwrap();
        assert_eq!(d.source(), "AXB\nCXD\n");
        d.undo(d.version()).unwrap();
        assert_eq!(v.ranges(&d).unwrap(), [1..4, 7..10]);
        assert_eq!(v.highlights(&d).unwrap(), [(0, 1, 2), (1, 1, 2)]);
        v.replace(&mut d, "Y").unwrap();
        assert_eq!(d.source(), "AYB\nCYD\n");
        assert!(!d.can_redo());
    }
    #[test]
    fn rectangle_tap_is_an_insertion_caret_and_line_mode_does_not_reinterpret_restored_text() {
        let mut d = doc("A中B\nC文D\n");
        let mut v = Viewport::default();
        v.set_selection_mode(&mut d, SelectionMode::Rectangle)
            .unwrap();
        v.pointer(&mut d, 0, 0, 1, false, false).unwrap();
        v.pointer(&mut d, 6, 0, 1, false, false).unwrap();
        v.replace(&mut d, "X").unwrap();
        assert_eq!(d.source(), "AX中B\nC文D\n");
        d.undo(d.version()).unwrap();
        d.select(d.version(), Selection { anchor: 1, head: 4 })
            .unwrap();
        v.replace(&mut d, "Y").unwrap();
        v.set_selection_mode(&mut d, SelectionMode::Line).unwrap();
        d.undo(d.version()).unwrap();
        let ranges = v.ranges(&d).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], 1..4);
        v.replace(&mut d, "Z").unwrap();
        assert_eq!(d.source(), "AZB\nC文D\n");
    }
}
