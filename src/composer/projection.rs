//! Incremental logical rows. Screen remains the only rendition interpreter;
//! atoms only map original UTF-8 boundaries to native editor cells.
use crate::screen::{Screen, SourceRendition, Style};
use std::{ops::Range, sync::Arc};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Color,
    Plain,
    Source,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Affinity {
    Before,
    After,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub row: usize,
    pub col: usize,
}
#[derive(Clone, Debug)]
struct Atom {
    raw: Range<usize>,
    width: usize,
    kind: Kind,
    left: Style,
    right: Style,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Hidden,
    Glyph,
    Tab,
    Escape,
    Unmapped,
}
pub struct Glyph<'a> {
    pub raw: Range<usize>,
    pub col: usize,
    pub width: usize,
    pub text: &'a str,
    pub left: Style,
    pub right: Style,
    pub unmapped: bool,
}
#[derive(Debug)]
pub struct Row {
    raw: String,
    atoms: Vec<Atom>,
    initial: SourceRendition,
    end: SourceRendition,
    terminated: bool,
}
impl Row {
    fn parse(raw: &str, initial: SourceRendition, terminated: bool) -> Result<Self, String> {
        let mut core = Screen::new(2, 1).map_err(str::to_owned)?;
        core.restore_source_rendition(initial);
        let mut atoms = Vec::new();
        let mut from = 0;
        while from < raw.len() {
            let rest = &raw[from..];
            if let Some(sgr) = super::safe_sgr(rest) {
                atoms.push(Atom {
                    raw: from..from + sgr.len(),
                    width: 0,
                    kind: Kind::Hidden,
                    left: Style::default(),
                    right: Style::default(),
                });
                core.feed(sgr.as_bytes());
                from += sgr.len();
                continue;
            }
            let ch = rest
                .chars()
                .next()
                .ok_or_else(|| crate::msg!("draft-invalid"))?;
            let style = core.source_rendition().style;
            let (kind, width, left, right) = if ch == '\t' {
                core.feed(b"\t");
                (Kind::Tab, 0, style, style)
            } else if ch == '\x1b' {
                (Kind::Escape, 1, style, style)
            } else {
                match crate::encoding::encode(ch.encode_utf8(&mut [0; 4])) {
                    Ok(encoded) => match core.project_source_glyph(&encoded) {
                        Some((l, r)) => (Kind::Glyph, encoded.len(), l, r),
                        None => (Kind::Unmapped, 2, style, style),
                    },
                    Err(_) => (Kind::Unmapped, 2, style, style),
                }
            };
            atoms.push(Atom {
                raw: from..from + ch.len_utf8(),
                width,
                kind,
                left,
                right,
            });
            from += ch.len_utf8();
        }
        if terminated {
            core.feed(b"\n");
        }
        Ok(Self {
            raw: raw.into(),
            atoms,
            initial,
            end: core.source_rendition(),
            terminated,
        })
    }
    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub fn glyphs(&self, mode: Mode) -> Glyphs<'_> {
        Glyphs {
            row: self,
            mode,
            index: 0,
            characters: self.raw.char_indices(),
            col: 0,
        }
    }
    pub fn columns(&self, mode: Mode) -> usize {
        self.glyphs(mode).last().map_or(0, |g| g.col + g.width)
    }
    /// A hit in either half of a wide glyph resolves to a complete UTF-8
    /// boundary. Hidden SGR at a boundary uses explicit insertion affinity.
    pub fn offset_at_column(&self, mode: Mode, col: usize, affinity: Affinity) -> usize {
        let mut before = 0;
        let mut end_column = 0;
        for glyph in self.glyphs(mode) {
            if col == glyph.col {
                return if affinity == Affinity::Before {
                    before
                } else {
                    glyph.raw.start
                };
            }
            if col < glyph.col + glyph.width {
                return if affinity == Affinity::Before {
                    glyph.raw.start
                } else {
                    glyph.raw.end
                };
            }
            before = glyph.raw.end;
            end_column = glyph.col + glyph.width;
        }
        if affinity == Affinity::Before && col == end_column {
            before
        } else {
            self.raw.len()
        }
    }
    pub fn column_at_offset(&self, mode: Mode, raw: usize) -> usize {
        let mut col = 0;
        for glyph in self.glyphs(mode) {
            if raw < glyph.raw.end {
                return glyph.col;
            }
            col = glyph.col + glyph.width;
        }
        col
    }
}
pub struct Glyphs<'a> {
    row: &'a Row,
    mode: Mode,
    index: usize,
    characters: std::str::CharIndices<'a>,
    col: usize,
}
impl<'a> Iterator for Glyphs<'a> {
    type Item = Glyph<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let (raw, width, text, left, right, unmapped) = if self.mode == Mode::Source {
            let (from, ch) = self.characters.next()?;
            let text = if ch == '\x1b' {
                "␛"
            } else {
                &self.row.raw[from..from + ch.len_utf8()]
            };
            let width = if ch == '\t' {
                8 - self.col % 8
            } else if ch == '\x1b' || ch.is_ascii() {
                1
            } else {
                crate::encoding::encode(text).map_or(2, |b| b.len())
            };
            (
                from..from + ch.len_utf8(),
                width,
                text,
                Style::default(),
                Style::default(),
                !ch.is_ascii() && crate::encoding::encode(text).is_err(),
            )
        } else {
            let atom = loop {
                let atom = self.row.atoms.get(self.index)?;
                self.index += 1;
                if atom.kind != Kind::Hidden {
                    break atom;
                }
            };
            let text = if atom.kind == Kind::Escape {
                "␛"
            } else {
                &self.row.raw[atom.raw.clone()]
            };
            let width = if atom.kind == Kind::Tab {
                8 - self.col % 8
            } else {
                atom.width
            };
            let (left, right) = if self.mode == Mode::Plain {
                (Style::default(), Style::default())
            } else {
                (atom.left, atom.right)
            };
            (
                atom.raw.clone(),
                width,
                text,
                left,
                right,
                atom.kind == Kind::Unmapped,
            )
        };
        let col = self.col;
        self.col += width;
        Some(Glyph {
            raw,
            col,
            width,
            text,
            left,
            right,
            unmapped,
        })
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Update {
    pub reparsed_rows: usize,
    pub reused_rows: usize,
}
pub(super) struct Cache {
    pub rows: Vec<Arc<Row>>,
    pub starts: Vec<usize>,
}
impl Cache {
    pub fn new(source: &str) -> Result<Self, String> {
        let lines: Vec<_> = source.split('\n').collect();
        let mut rows = Vec::with_capacity(lines.len());
        let mut starts = Vec::with_capacity(lines.len());
        let mut state = SourceRendition::default();
        let mut offset = 0;
        for (i, line) in lines.iter().enumerate() {
            starts.push(offset);
            offset += line.len() + 1;
            let row = Row::parse(line, state, i + 1 < lines.len())?;
            state = row.end;
            rows.push(Arc::new(row));
        }
        Ok(Self { rows, starts })
    }
    pub fn row_at(&self, offset: usize) -> usize {
        self.starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1)
    }
    pub fn update(
        &mut self,
        new: &str,
        old_range: Range<usize>,
        inserted: &str,
    ) -> Result<Update, String> {
        let first = self.row_at(old_range.start);
        let lines: Vec<_> = new.split('\n').collect();
        let new_tail = first + inserted.bytes().filter(|b| *b == b'\n').count() + 1;
        let delta = lines.len() as isize - self.rows.len() as isize;
        let mut rows = self.rows[..first].to_vec();
        let mut state = rows.last().map_or(SourceRendition::default(), |r| r.end);
        let mut update = Update {
            reparsed_rows: 0,
            reused_rows: first,
        };
        for (i, line) in lines.iter().enumerate().skip(first) {
            if i >= new_tail
                && let Some(old) = i.checked_add_signed(-delta).and_then(|i| self.rows.get(i))
                && old.raw == *line
                && old.initial == state
                && old.terminated == (i + 1 < lines.len())
            {
                let start = i
                    .checked_add_signed(-delta)
                    .ok_or_else(|| crate::msg!("draft-invalid"))?;
                update.reused_rows += self.rows.len() - start;
                rows.extend(self.rows[start..].iter().cloned());
                break;
            }
            let row = Row::parse(line, state, i + 1 < lines.len())?;
            state = row.end;
            rows.push(Arc::new(row));
            update.reparsed_rows += 1;
        }
        let mut offset = 0;
        let starts = lines
            .iter()
            .map(|line| {
                let start = offset;
                offset += line.len() + 1;
                start
            })
            .collect();
        self.rows = rows;
        self.starts = starts;
        Ok(update)
    }
    pub fn position(&self, mode: Mode, raw: usize) -> Position {
        let row = self.row_at(raw);
        Position {
            row,
            col: self.rows[row].column_at_offset(mode, raw - self.starts[row]),
        }
    }
    pub fn offset(&self, mode: Mode, p: Position, affinity: Affinity) -> Option<usize> {
        Some(
            self.starts.get(p.row)?
                + self
                    .rows
                    .get(p.row)?
                    .offset_at_column(mode, p.col, affinity),
        )
    }
}
