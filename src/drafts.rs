//! Full local source; never reconstructed from a terminal viewport.
use crate::{board_navigation::BoardName, sites::Target};
use serde::{Deserialize, Serialize};
#[cfg(feature = "desktop")]
pub mod session;
#[cfg(feature = "desktop")]
pub mod store;
pub const MAX_BODY: usize = 128 * 1024;
pub const MAX_DRAFTS: usize = 64;
pub const MAX_BYTES: usize = 12 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub site: Target,
    pub account: String,
    #[serde(deserialize_with = "board")]
    pub board: BoardName,
}
impl Scope {
    pub fn valid(&self) -> bool {
        self.site.ptt_features() && crate::filters::valid_id(&self.account)
    }
    pub fn same(&self, other: &Self) -> bool {
        self.site == other.site
            && self.account.eq_ignore_ascii_case(&other.account)
            && self.board.same(&other.board)
    }
}
fn board<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BoardName, D::Error> {
    BoardName::parse(&String::deserialize(d)?).map_err(serde::de::Error::custom)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub scope: Scope,
    pub revision: u64,
    pub title: String,
    pub body: String,
    pub saved_at: u64,
    pub conflict: bool,
}
impl Entry {
    pub fn validate(&self) -> Result<(), String> {
        if !self.scope.valid()
            || self.id.is_empty()
            || self.id.len() > 80
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || self.title.len() > 256
            || self.title.chars().any(char::is_control)
            || self.body.len() > MAX_BODY
            || self.body.split('\n').count() > 4096
        {
            return Err(crate::msg!("draft-invalid"));
        }
        // Incomplete SGR edits are saved faithfully and remain visible in local
        // projections. Transfer requires complete safe SGR; unrelated controls
        // are never accepted.
        if self
            .body
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t' | '\x1b'))
        {
            return Err(crate::msg!("draft-invalid"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drafts {
    version: u8,
    pub entries: Vec<Entry>,
}
impl Default for Drafts {
    fn default() -> Self {
        Self::empty()
    }
}
impl Drafts {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err(crate::msg!("draft-limit"));
        }
        let data: Self = serde_json::from_slice(bytes).map_err(|_| crate::msg!("draft-invalid"))?;
        data.validate()?;
        Ok(data)
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err(crate::msg!("draft-limit"));
        }
        Ok(bytes)
    }
    pub fn empty() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
    fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.entries.len() > MAX_DRAFTS {
            return Err(crate::msg!("draft-limit"));
        }
        for (index, entry) in self.entries.iter().enumerate() {
            entry.validate()?;
            if self.entries[..index].iter().any(|e| e.id == entry.id) {
                return Err(crate::msg!("draft-invalid"));
            }
        }
        Ok(())
    }
}
pub fn transfer(body: &str, encoding: crate::encoding::TextEncoding) -> Result<Vec<u8>, String> {
    if body.len() > MAX_BODY || body.split('\n').count() > 4096 {
        return Err(crate::msg!("draft-limit"));
    }
    for line in body.split('\n') {
        let bytes = encoding.encode(line).map_err(|e| e.to_string())?;
        if bytes.len() > 510 {
            return Err(crate::msg!("draft-long-line"));
        }
    }
    // Do not rely on the remote editor's implicit wrap/split behaviour. Count
    // actual encoded terminal columns after removing validated SGR atoms.
    crate::input::prepare_editor_source(body, encoding, MAX_BODY).map_err(|e| e.to_string())?;
    for line in body.split('\n') {
        let mut rest = line;
        let mut columns = 0;
        while !rest.is_empty() {
            if let Some(sgr) = rest.strip_prefix("\x1b[") {
                let end = sgr.find('m').ok_or(crate::msg!("draft-invalid"))?;
                rest = &sgr[end + 1..];
            } else {
                let ch = rest.chars().next().ok_or(crate::msg!("draft-invalid"))?;
                columns += if ch == '\t' {
                    8 - columns % 8
                } else {
                    encoding
                        .encode(&ch.to_string())
                        .map_err(|e| e.to_string())?
                        .len()
                };
                rest = &rest[ch.len_utf8()..];
            }
            if columns > 79 {
                return Err(crate::msg!("draft-long-line"));
            }
        }
    }
    crate::input::prepare_editor_source(body, encoding, MAX_BODY)
        .map(|p| p.bytes)
        .map_err(|e| e.to_string())
}
/// Visible empty-editor candidate. The source flow must also query document EOF
/// and line End: a literal tilde or blank line can look like an EOF display row.
pub fn empty_editor(frame: &crate::screen::Frame) -> bool {
    crate::navigation::article_editor(frame)
        && frame.cursor == (0, 0)
        && crate::text::rows(frame).is_some_and(|rows| {
            rows[..frame.rows - 1].iter().enumerate().all(|(index, r)| {
                r.mapped() && (r.text.trim().is_empty() || index > 0 && r.text.trim_end() == "~")
            })
        })
}
/// A confirmed connected editor scope; never derived from pending output.
#[cfg(feature = "desktop")]
pub fn scope(view: &crate::connection::View) -> Option<Scope> {
    if view.phase != crate::connection::Phase::Connected
        || view.input_pending
        || view.output_pending
        || !view.site.ptt_features()
    {
        return None;
    }
    Some(Scope {
        site: view.site.clone(),
        account: view.account.clone()?,
        board: crate::board_navigation::current_board(&view.frame)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_chinese_ansi_source_transfers_beyond_clipboard_limit_without_commands() {
        let text = "\x1b[1;31m中文\x1b[m\n".repeat(300);
        let bytes = transfer(&text, crate::encoding::TextEncoding::Uao).unwrap();
        assert!(bytes.len() > 4096);
        assert!(!bytes.contains(&24));
        assert!(!bytes.contains(&27));
        assert_eq!(bytes.iter().filter(|b| **b == 21).count(), 600);
        assert_eq!(bytes.iter().filter(|b| **b == 13).count(), 300);
        for bad in [
            "x\x18",
            "x\x1b[2J",
            "x\x1b]0;title\x07",
            "x\x1b[31",
            &"中".repeat(256),
        ] {
            assert!(transfer(bad, crate::encoding::TextEncoding::Uao).is_err());
        }
        assert!(
            crate::input::prepare_editor_paste(&text, crate::encoding::TextEncoding::Uao).is_err()
        );
    }
    #[test]
    fn local_colour_preserves_chinese_line_boundaries_and_original_tail_styles() {
        let source = "\x1b[32m前中文後\n\n第三行\x1b[m尾";
        let start = source[..source.find('中').unwrap()].encode_utf16().count();
        let end = source[..source.find("行").unwrap() + "行".len()]
            .encode_utf16()
            .count();
        let result = crate::editor_color::local_source(
            source,
            start,
            end,
            crate::editor_color::Color {
                foreground: 9,
                background: 0,
            },
        )
        .unwrap();
        let mut screen = crate::screen::Screen::new(80, 24).unwrap();
        screen.feed(&crate::encoding::encode(&result.replace('\n', "\r\n")).unwrap());
        let frame = screen.frame();
        let glyph = |row, text| {
            frame
                .glyphs
                .iter()
                .find(|g| g.row == row && g.text == text)
                .unwrap()
        };
        assert_eq!(glyph(0, "前").left.fg, 2);
        assert_eq!(glyph(0, "中").left.fg, 1);
        assert!(glyph(0, "中").left.bright);
        assert_eq!(glyph(2, "第").left.fg, 1);
        assert_eq!(glyph(2, "尾").left.fg, 7);
        assert_eq!(result.matches('\n').count(), 2);
        assert!(
            crate::editor_color::local_source(
                source,
                2,
                6,
                crate::editor_color::Color {
                    foreground: 9,
                    background: 0
                }
            )
            .is_err()
        );
    }
}
