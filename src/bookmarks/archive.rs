//! Versioned local article data. This is a validated grid snapshot, never a replay/session.
use crate::{
    filters,
    screen::{
        Frame, Glyph, Style,
        hyperlinks::{Hyperlink, Span},
    },
};
use serde::{Deserialize, Serialize};

pub const MAX_ROWS: usize = 4096;
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
// Compact tuples retain both halves without serializing a field name per cell.
type StoredGlyph = (usize, usize, String, u32, u32, bool);
type StoredLink = (Option<String>, Vec<(usize, usize, usize)>);
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    version: u8,
    saved_at: u64,
    #[serde(deserialize_with = "read_rows")]
    rows: Vec<Vec<StoredGlyph>>,
    #[serde(deserialize_with = "read_glyphs")]
    footer: Vec<StoredGlyph>,
    #[serde(deserialize_with = "read_links")]
    links: Vec<StoredLink>,
}
pub(super) fn limited<'de, D, T, const N: usize>(d: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Visitor<T, const N: usize>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>, const N: usize> serde::de::Visitor<'de> for Visitor<T, N> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "at most {N} items")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(N));
            while let Some(item) = seq.next_element()? {
                if items.len() == N {
                    return Err(serde::de::Error::custom("too many items"));
                }
                items.push(item);
            }
            Ok(items)
        }
    }
    d.deserialize_seq(Visitor::<T, N>(std::marker::PhantomData))
}
fn read_glyphs<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<StoredGlyph>, D::Error> {
    limited::<D, StoredGlyph, 80>(d)
}
fn read_rows<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Vec<StoredGlyph>>, D::Error> {
    #[derive(Deserialize)]
    struct Row(#[serde(deserialize_with = "read_glyphs")] Vec<StoredGlyph>);
    limited::<D, Row, MAX_ROWS>(d).map(|rows| rows.into_iter().map(|r| r.0).collect())
}
fn read_spans<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<(usize, usize, usize)>, D::Error> {
    limited::<D, (usize, usize, usize), 80>(d)
}
fn read_links<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<StoredLink>, D::Error> {
    #[derive(Deserialize)]
    struct Link(
        Option<String>,
        #[serde(deserialize_with = "read_spans")] Vec<(usize, usize, usize)>,
    );
    limited::<D, Link, { MAX_ROWS * 80 }>(d)
        .map(|links| links.into_iter().map(|l| (l.0, l.1)).collect())
}
fn pack(s: Style) -> u32 {
    u32::from(s.fg) | u32::from(s.bg) << 8 | u32::from(s.bright) << 16 | u32::from(s.inverse) << 17
}
fn unpack(s: u32) -> Style {
    Style {
        fg: (s & 255) as u8,
        bg: ((s >> 8) & 255) as u8,
        bright: s & 65536 != 0,
        inverse: s & 131072 != 0,
    }
}
fn store(g: &Glyph) -> StoredGlyph {
    (
        g.col,
        g.width,
        g.text.clone(),
        pack(g.left),
        pack(g.right),
        g.unmapped,
    )
}
fn glyph(g: &StoredGlyph, row: usize) -> Glyph {
    Glyph {
        col: g.0,
        row,
        width: g.1,
        text: g.2.clone(),
        left: unpack(g.3),
        right: unpack(g.4),
        unmapped: g.5,
    }
}
fn invalid() -> String {
    crate::msg!("bookmarks-archive-invalid")
}
impl Archive {
    pub fn new(frame: &Frame, saved_at: u64) -> Result<Self, String> {
        if frame.cols != 80 || frame.rows < 2 || frame.rows > MAX_ROWS + 1 {
            return Err(invalid());
        }
        let mut rows = vec![Vec::new(); frame.rows - 1];
        let mut footer = Vec::new();
        for g in &frame.glyphs {
            if g.row == frame.rows - 1 {
                footer.push(store(g));
            } else {
                rows.get_mut(g.row).ok_or_else(invalid)?.push(store(g));
            }
        }
        let archive = Self {
            version: 1,
            saved_at,
            rows,
            footer,
            links: frame
                .hyperlinks
                .iter()
                .map(|l| {
                    (
                        l.target.clone(),
                        l.spans
                            .iter()
                            .filter(|s| s.row + 1 < frame.rows)
                            .map(|s| (s.row, s.start, s.end))
                            .collect(),
                    )
                })
                .filter(|(_, s): &StoredLink| !s.is_empty())
                .collect(),
        };
        archive.validate()?;
        Ok(archive)
    }
    pub fn saved_at(&self) -> u64 {
        self.saved_at
    }
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || !(1..=super::MAX_CREATED_AT).contains(&self.saved_at)
            || self.rows.is_empty()
            || self.rows.len() > MAX_ROWS
        {
            return Err(invalid());
        }
        for row in self.rows.iter().chain(std::iter::once(&self.footer)) {
            let mut end = 0;
            for (col, width, text, a, b, unmapped) in row {
                if !matches!(width, 1 | 2)
                    || *col != end
                    || col.checked_add(*width).is_none_or(|n| n > 80)
                    || *unmapped
                    || text.is_empty()
                    || text.len() > 8
                    || text.chars().count() > 2
                    || text.chars().any(char::is_control)
                {
                    return Err(invalid());
                }
                for style in [a, b] {
                    if *style > 262143 || (*style & 255) > 15 || ((*style >> 8) & 255) > 15 {
                        return Err(invalid());
                    }
                }
                end = col + width;
            }
            if end != 80 {
                return Err(invalid());
            }
        }
        if self.links.len() > MAX_ROWS * 80 {
            return Err(invalid());
        }
        let mut link_bytes = 0usize;
        let mut spans = 0usize;
        for (target, regions) in &self.links {
            if let Some(url) = target {
                if url.len() > 4096 || !url.bytes().all(|b| (33..=126).contains(&b)) {
                    return Err(invalid());
                }
                link_bytes = link_bytes.saturating_add(url.len());
            }
            spans = spans.saturating_add(regions.len());
            if regions.is_empty()
                || regions.len() > 80
                || regions
                    .iter()
                    .any(|(row, start, end)| *row >= self.rows.len() || start >= end || *end > 80)
            {
                return Err(invalid());
            }
        }
        if link_bytes > 512 * 1024
            || spans > MAX_ROWS * 80
            || serde_json::to_vec(self).map_err(|_| invalid())?.len() > MAX_BYTES
        {
            return Err(invalid());
        }
        let mut check = Frame {
            control_complete: true,
            cols: 80,
            rows: 24,
            cursor: (0, 23),
            cursor_visible: false,
            revision: 0,
            glyphs: Vec::new(),
            hyperlinks: Vec::new(),
            unsupported: 0,
        };
        for row in 0..23 {
            if let Some(data) = self.rows.get(row) {
                check.glyphs.extend(data.iter().map(|g| glyph(g, row)));
            } else {
                for col in 0..80 {
                    check.glyphs.push(Glyph {
                        col,
                        row,
                        width: 1,
                        text: " ".into(),
                        left: Style::default(),
                        right: Style::default(),
                        unmapped: false,
                    });
                }
            }
        }
        check
            .glyphs
            .extend(self.footer.iter().map(|g| glyph(g, 23)));
        if !matches!(
            crate::navigation::analyze(&check),
            crate::navigation::Context::ArticleReading { .. }
        ) {
            return Err(invalid());
        }
        Ok(())
    }
    /// Apply the current policy to normal 24-row terminal projections before
    /// assembling the read-only local frame. No connection stamp is fabricated.
    pub fn filtered(&self, rules: &filters::Rules) -> Frame {
        let mut full = Frame {
            control_complete: true,
            cols: 80,
            rows: self.rows.len(),
            cursor: (0, 0),
            cursor_visible: false,
            revision: 0,
            glyphs: Vec::new(),
            hyperlinks: Vec::new(),
            unsupported: 0,
        };
        let mut page_links: Vec<Vec<Hyperlink>> = vec![Vec::new(); self.rows.len().div_ceil(23)];
        for (target, spans) in &self.links {
            let mut grouped = std::collections::BTreeMap::<usize, Vec<Span>>::new();
            for (row, start, end) in spans {
                grouped.entry(row / 23).or_default().push(Span {
                    row: row % 23,
                    start: *start,
                    end: *end,
                });
            }
            for (page, spans) in grouped {
                page_links[page].push(Hyperlink {
                    target: target.clone(),
                    spans,
                });
            }
        }
        for start in (0..self.rows.len()).step_by(23) {
            let end = (start + 23).min(self.rows.len());
            let mut page = Frame {
                control_complete: true,
                cols: 80,
                rows: 24,
                cursor: (0, 23),
                cursor_visible: false,
                revision: 0,
                glyphs: Vec::new(),
                hyperlinks: Vec::new(),
                unsupported: 0,
            };
            for (row, data) in self.rows[start..end].iter().enumerate() {
                page.glyphs.extend(data.iter().map(|g| glyph(g, row)));
            }
            for row in end - start..23 {
                for col in 0..80 {
                    page.glyphs.push(Glyph {
                        col,
                        row,
                        width: 1,
                        text: " ".into(),
                        left: Style::default(),
                        right: Style::default(),
                        unmapped: false,
                    });
                }
            }
            page.glyphs.extend(self.footer.iter().map(|g| glyph(g, 23)));
            page.hyperlinks = std::mem::take(&mut page_links[start / 23]);
            let projected = filters::project(&page, rules, false).frame;
            full.glyphs.extend(
                projected
                    .glyphs
                    .into_iter()
                    .filter(|g| g.row < end - start)
                    .map(|mut g| {
                        g.row += start;
                        g
                    }),
            );
            full.hyperlinks
                .extend(projected.hyperlinks.into_iter().map(|mut l| {
                    for s in &mut l.spans {
                        s.row += start;
                    }
                    l
                }));
        }
        full
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> Frame {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/filters.json")).unwrap();
        let mut screen = crate::screen::Screen::new(80, 24).unwrap();
        screen.feed(&crate::encoding::encode(fixture["reading"].as_str().unwrap()).unwrap());
        screen.frame()
    }
    #[test]
    fn original_chinese_two_halves_and_current_policy_survive_restart() {
        let mut raw = source();
        let chinese = raw
            .glyphs
            .iter_mut()
            .find(|g| g.row == 1 && g.width == 2)
            .unwrap();
        chinese.left.fg = 1;
        chinese.right.fg = 4;
        let archive = Archive::new(&raw, 100).unwrap();
        let bytes = serde_json::to_vec(&archive).unwrap();
        let reopened: Archive = serde_json::from_slice(&bytes).unwrap();
        reopened.validate().unwrap();
        let unmasked = reopened.filtered(&filters::Rules::default());
        assert_eq!(
            unmasked.glyphs,
            raw.glyphs
                .iter()
                .filter(|g| g.row < 23)
                .cloned()
                .collect::<Vec<_>>()
        );
        let mask = filters::Rules::default()
            .edit_policy("Alice", "", filters::Policy::Mask, false)
            .unwrap();
        let hidden = reopened.filtered(&mask);
        assert!(
            hidden
                .glyphs
                .iter()
                .filter(|g| g.row == 5 && g.col > 8 && g.col < 66)
                .all(|g| g.left.fg == 0 && g.left.bg == 0 && g.right.fg == 0 && g.right.bg == 0)
        );
        assert_eq!(
            hidden
                .glyphs
                .iter()
                .find(|g| g.row == 5 && g.col == 3)
                .unwrap()
                .left
                .fg,
            1
        );
        assert!(unmasked.glyphs.iter().any(|g| g.row == 5 && g.text == "機"));
        let full = filters::Rules::default()
            .edit_policy("Bob", "", filters::Policy::MaskRow, false)
            .unwrap();
        assert!(
            reopened
                .filtered(&full)
                .glyphs
                .iter()
                .filter(|g| g.row == 8)
                .all(|g| g.left.fg == 0 && g.left.bg == 0 && g.right.fg == 0 && g.right.bg == 0)
        );
        let allow = filters::Rules::default()
            .edit_color("Alice2", "", filters::Policy::Allow, Some(13), false)
            .unwrap();
        let colored = reopened.filtered(&allow);
        let id = colored
            .glyphs
            .iter()
            .find(|g| g.row == 7 && g.col == 3)
            .unwrap();
        assert_eq!(id.left.fg, 5);
        assert!(id.left.bright);
        assert!(
            reopened
                .filtered(&filters::Rules::default())
                .glyphs
                .iter()
                .any(|g| g.row == 5 && g.text == "機")
        );
    }
    #[test]
    fn short_final_chunk_still_applies_policy_and_does_not_store_synthetic_padding() {
        let mut raw = source();
        // Six real rows followed by the original source footer, rebased to row 6.
        raw.glyphs.retain(|g| g.row < 6 || g.row == 23);
        for g in &mut raw.glyphs {
            if g.row == 23 {
                g.row = 6;
            }
        }
        raw.rows = 7;
        let archive = Archive::new(&raw, 100).unwrap();
        assert_eq!(archive.row_count(), 6);
        let mask = filters::Rules::default().edit("Alice", "", false).unwrap();
        let filtered = archive.filtered(&mask);
        assert_eq!(filtered.rows, 6);
        assert!(
            filtered
                .glyphs
                .iter()
                .filter(|g| g.row == 5 && g.col > 8 && g.col < 66)
                .all(|g| g.left.fg == 0 && g.left.bg == 0)
        );
        let mut invalid = source();
        invalid
            .glyphs
            .iter_mut()
            .find(|g| g.row == 23 && g.col == 0)
            .unwrap()
            .text = "X".into();
        assert!(Archive::new(&invalid, 100).is_err());
        invalid = source();
        invalid.glyphs[0].unmapped = true;
        assert!(Archive::new(&invalid, 100).is_err());
    }
    #[test]
    fn invalid_geometry_styles_text_versions_and_rows_are_rejected() {
        let value = serde_json::to_value(Archive::new(&source(), 100).unwrap()).unwrap();
        for (key, changed) in [
            ("version", serde_json::json!(2)),
            ("saved_at", serde_json::json!(0)),
            ("rows", serde_json::json!([])),
        ] {
            let mut bad = value.clone();
            bad[key] = changed;
            assert!(
                serde_json::from_value::<Archive>(bad)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        for (index, changed) in [
            (0, serde_json::json!(80)),
            (1, serde_json::json!(3)),
            (2, serde_json::json!("\u{001b}[31m")),
            (3, serde_json::json!(16)),
            (4, serde_json::json!(262144)),
        ] {
            let mut bad = value.clone();
            bad["rows"][0][0][index] = changed;
            assert!(
                serde_json::from_value::<Archive>(bad)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        let mut bad = value;
        bad["rows"] = serde_json::json!(vec![Vec::<u8>::new(); MAX_ROWS + 1]);
        assert!(serde_json::from_value::<Archive>(bad).is_err());
    }
}
