//! Cell-owned OSC 8 attributes. IDs never escape the screen's lifetime.
use super::{Cell, Glyph};

pub const MAX_URL_BYTES: usize = 4096;
pub const MAX_PARAMETERS: usize = 256;
pub const MAX_TARGETS: usize = 128;
pub const MAX_TARGET_BYTES: usize = 64 * 1024;
pub(super) const MAX_OSC_BYTES: usize = 3 + MAX_PARAMETERS + MAX_URL_BYTES;
pub(super) const BLOCKED: u8 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}

/// `None` marks explicit but unusable metadata, suppressing plain-text guessing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hyperlink {
    pub target: Option<String>,
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Hyperlinks {
    targets: Vec<Option<String>>,
    pub active: u8,
}
impl Hyperlinks {
    pub fn finish(&mut self, data: &[u8], overflow: bool, cells: &[Cell]) {
        let Some(body) = data.strip_prefix(b"8;") else {
            return;
        };
        // An invalid replacement must not accidentally retain the previous target.
        self.active = BLOCKED;
        if overflow {
            return;
        }
        let Some(separator) = body.iter().position(|b| *b == b';') else {
            return;
        };
        let (parameters, uri) = (&body[..separator], &body[separator + 1..]);
        if parameters.len() > MAX_PARAMETERS || !parameters.iter().all(|b| (32..=126).contains(b)) {
            return;
        }
        if uri.is_empty() {
            self.active = 0;
            return;
        }
        if uri.len() > MAX_URL_BYTES || !uri.iter().all(|b| (33..=126).contains(b)) {
            return;
        }
        let Ok(uri) = std::str::from_utf8(uri) else {
            return;
        };
        // Collect only when opening a target, after all cell edits have applied.
        // A slot cannot be reused while any old cell still refers to it.
        let mut used = [false; MAX_TARGETS];
        for cell in cells {
            if cell.hyperlink >= 2 {
                used[usize::from(cell.hyperlink) - 2] = true;
            }
        }
        for (slot, used) in self.targets.iter_mut().zip(used) {
            if !used {
                *slot = None;
            }
        }
        if let Some(slot) = self
            .targets
            .iter()
            .position(|target| target.as_deref() == Some(uri))
        {
            self.active = (slot + 2) as u8;
            return;
        }
        let bytes: usize = self.targets.iter().flatten().map(String::len).sum();
        if bytes + uri.len() > MAX_TARGET_BYTES {
            return;
        }
        let slot = if let Some(slot) = self.targets.iter().position(Option::is_none) {
            slot
        } else if self.targets.len() < MAX_TARGETS {
            self.targets.push(None);
            self.targets.len() - 1
        } else {
            return;
        };
        self.targets[slot] = Some(uri.to_owned());
        self.active = (slot + 2) as u8;
    }

    pub fn project(&self, glyphs: &[Glyph], cells: &[Cell], cols: usize) -> Vec<Hyperlink> {
        let mut output: Vec<Hyperlink> = Vec::new();
        let mut groups = [None; MAX_TARGETS + 2];
        for glyph in glyphs {
            let at = glyph.row * cols + glyph.col;
            let mut id = cells[at].hyperlink;
            if glyph.unmapped || (glyph.width == 2 && cells[at + 1].hyperlink != id) {
                id = if id != 0 || (glyph.width == 2 && cells[at + 1].hyperlink != 0) {
                    BLOCKED
                } else {
                    0
                };
            }
            if id == 0 {
                continue;
            }
            let group = *groups[usize::from(id)].get_or_insert_with(|| {
                output.push(Hyperlink {
                    target: id
                        .checked_sub(2)
                        .and_then(|slot| self.targets.get(usize::from(slot)))
                        .and_then(Clone::clone),
                    spans: Vec::new(),
                });
                output.len() - 1
            });
            let spans = &mut output[group].spans;
            if let Some(last) = spans
                .last_mut()
                .filter(|last| last.row == glyph.row && last.end == glyph.col)
            {
                last.end += glyph.width;
            } else {
                spans.push(Span {
                    row: glyph.row,
                    start: glyph.col,
                    end: glyph.col + glyph.width,
                });
            }
        }
        output
    }
}
