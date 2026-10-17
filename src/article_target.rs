//! Typed article identity. Parsing never sends keys or follows a web redirect.
use crate::board_navigation::BoardName;
use serde::Serialize;
use std::fmt;

const ALPHABET: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-_";
pub const MAX_INPUT: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Aid(String);
impl Aid {
    pub fn parse(value: &str) -> Result<Self, Error> {
        let value = value.strip_prefix('#').unwrap_or(value);
        if value.len() != 8 {
            return Err(Error::Aid);
        }
        let mut raw = 0u64;
        for byte in value.bytes() {
            let digit = ALPHABET
                .iter()
                .position(|known| *known == byte)
                .ok_or(Error::Aid)?;
            raw = (raw << 6) | digit as u64;
        }
        // Only the public raw identity: one type bit, a positive time4_t and
        // twelve suffix bits. Internal index hints never belong in an AID.
        let stamp = (raw >> 12) & 0xffff_ffff;
        if raw >> 45 != 0 || stamp == 0 || stamp > i32::MAX as u64 {
            return Err(Error::Aid);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    fn from_filename(filename: &str) -> Result<Self, Error> {
        let mut parts = filename.split('.');
        let kind = match parts.next() {
            Some("M") => 0,
            Some("G") => 1u64 << 44,
            _ => return Err(Error::Url),
        };
        let timestamp = parts.next().ok_or(Error::Url)?;
        if timestamp.is_empty()
            || timestamp.len() > 10
            || !timestamp.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(Error::Url);
        }
        let timestamp: u32 = timestamp.parse().map_err(|_| Error::Url)?;
        if timestamp == 0 || timestamp > i32::MAX as u32 || parts.next() != Some("A") {
            return Err(Error::Url);
        }
        let suffix = match parts.next() {
            None => 0,
            Some(hex) if hex.len() == 3 && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
                u64::from_str_radix(hex, 16).map_err(|_| Error::Url)?
            }
            _ => return Err(Error::Url),
        };
        if parts.next().is_some() {
            return Err(Error::Url);
        }
        let mut raw = kind | ((timestamp as u64) << 12) | suffix;
        let mut code = [b'0'; 8];
        for digit in code.iter_mut().rev() {
            *digit = ALPHABET[(raw & 63) as usize];
            raw >>= 6;
        }
        let code = std::str::from_utf8(&code).map_err(|_| Error::Url)?;
        Self::parse(code)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ArticleTarget {
    board: BoardName,
    aid: Aid,
}
impl ArticleTarget {
    pub fn new(board: &str, aid: &str) -> Result<Self, Error> {
        if [board, aid].iter().any(|value| {
            value.len() > MAX_INPUT || value.chars().any(|character| character.is_control())
        }) {
            return Err(Error::Target);
        }
        Ok(Self {
            board: BoardName::parse(board).map_err(|_| Error::Board)?,
            aid: Aid::parse(aid.trim())?,
        })
    }
    pub fn board(&self) -> &BoardName {
        &self.board
    }
    pub fn aid(&self) -> &Aid {
        &self.aid
    }
    pub(crate) fn confirm_board(&mut self, board: BoardName) {
        self.board = board;
    }
    /// Supported forms: #AID@Board, or an exact www.ptt.cc /bbs article URL.
    /// Bare AIDs need the separate explicit board input supplied to `new`.
    pub fn parse(value: &str) -> Result<Self, Error> {
        if value.len() > MAX_INPUT || value.chars().any(|c| c.is_control()) {
            return Err(Error::Target);
        }
        let value = value.trim();
        if !value.contains("://") {
            let (aid, board) = value.split_once('@').ok_or(Error::Target)?;
            if aid.contains(char::is_whitespace) || board.contains(char::is_whitespace) {
                return Err(Error::Target);
            }
            return Self::new(board, aid);
        }
        // Reject escaped paths and URL normalization tricks before URL parsing.
        if value.contains(['%', '\\']) || value.chars().any(char::is_whitespace) {
            return Err(Error::Url);
        }
        let (authority, original_path) = value
            .split_once("://")
            .and_then(|(_, rest)| rest.split_once('/'))
            .ok_or(Error::Url)?;
        if !authority.eq_ignore_ascii_case("www.ptt.cc") {
            return Err(Error::Url);
        }
        let url = url::Url::parse(value).map_err(|_| Error::Url)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str() != Some("www.ptt.cc")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::Url);
        }
        if original_path
            .split('/')
            .any(|part| matches!(part, "." | ".."))
        {
            return Err(Error::Url);
        }
        let parts: Vec<_> = url.path().split('/').collect();
        let ["", "bbs", board, article] = parts.as_slice() else {
            return Err(Error::Url);
        };
        let board = BoardName::parse(board).map_err(|_| Error::Board)?;
        let aid = match article.strip_suffix(".html") {
            Some(filename) => Aid::from_filename(filename)?,
            None => Aid::parse(article).map_err(|_| Error::Url)?,
        };
        Ok(Self { board, aid })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Aid,
    Board,
    Url,
    Target,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&match self {
            Self::Aid => crate::msg!("article-aid-invalid"),
            Self::Board => crate::msg!("board-name-invalid"),
            Self::Url => crate::msg!("article-url-invalid"),
            Self::Target => crate::msg!("article-target-invalid"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aid_identity_keeps_case_and_accepts_only_the_defined_raw_bits() {
        for code in ["00000101", "1AbCdE-_", "50000100"] {
            assert_eq!(Aid::parse(&format!("#{code}")).unwrap().as_str(), code);
        }
        for bad in [
            "00000000",
            "00000001",
            "20000000",
            "60000000",
            "80000100",
            "1AbCdE+_",
            "1AbCdE/0",
            "##1AbCdE-_",
            "1AbCdE-",
            "1AbCdE-__",
            "1AbCdE-_ ",
            "1AbCdE-\r",
        ] {
            assert!(Aid::parse(bad).is_err(), "{bad:?}");
        }
        assert_ne!(
            Aid::parse("1AbCdE-_").unwrap(),
            Aid::parse("1ABCDE-_").unwrap()
        );
    }
    #[test]
    fn public_filename_fields_have_independent_expected_codes() {
        for (name, expected) in [
            ("M.1.A.001", "00000101"),
            ("M.1.A.FFF", "000001__"),
            ("M.64.A.040", "00001010"),
            ("G.1.A.001", "40000101"),
            ("M.1.A", "00000100"),
            ("M.2147483647.A.FFF", "1_______"),
        ] {
            assert_eq!(
                Aid::from_filename(name).unwrap().as_str(),
                expected,
                "{name}"
            );
        }
        for bad in [
            "M.0.A.001",
            "M.2147483648.A.001",
            "M.-1.A.001",
            "M.1.B.001",
            "M.1.A.1",
            "M.1.A.000.extra",
            "L.1.A.001",
            "M.1.A.G00",
        ] {
            assert!(Aid::from_filename(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn supported_urls_and_explicit_pairs_normalize_to_one_identity() {
        let expected = ArticleTarget::new("Stella", "00000101").unwrap();
        for value in [
            "#00000101@Stella",
            "00000101@Stella",
            " https://www.ptt.cc/bbs/Stella/M.1.A.001.html ",
            "http://www.ptt.cc/bbs/Stella/M.1.A.001.html",
            "https://www.ptt.cc/bbs/Stella/00000101",
        ] {
            assert_eq!(ArticleTarget::parse(value).unwrap(), expected, "{value}");
        }
        assert_eq!(
            ArticleTarget::new(" stella ", " #00000101 ")
                .unwrap()
                .board()
                .as_str(),
            "stella"
        );
        assert!(ArticleTarget::new("Stella\r", "00000101").is_err());
        assert!(ArticleTarget::new("Stella", "00000101\n").is_err());
        assert!(ArticleTarget::new(&"x".repeat(MAX_INPUT + 1), "00000101").is_err());
        assert_eq!(
            serde_json::to_value(&expected).unwrap(),
            serde_json::json!({"board":"Stella","aid":"00000101"})
        );
    }
    #[test]
    fn unrelated_or_ambiguous_urls_and_key_injection_are_rejected() {
        for bad in [
            "00000101",
            "#00000101@Stella@Other",
            "#00000101@Stella\r",
            "#00000101@Stella more",
            "https://ptt.cc/bbs/Stella/M.1.A.001.html",
            "https://www.ptt.cc.evil.example/bbs/Stella/M.1.A.001.html",
            "https://user@www.ptt.cc/bbs/Stella/M.1.A.001.html",
            "https://@www.ptt.cc/bbs/Stella/M.1.A.001.html",
            "https://www.ptt.cc:443/bbs/Stella/M.1.A.001.html",
            "http://www.ptt.cc:80/bbs/Stella/M.1.A.001.html",
            "https://www.ptt.cc./bbs/Stella/M.1.A.001.html",
            "https://www.ptt.cc:8443/bbs/Stella/M.1.A.001.html",
            "https://www.ptt.cc/bbs/Stella/M.1.A.001.html?q=1",
            "https://www.ptt.cc/bbs/Stella/M.1.A.001.html#tail",
            "https://www.ptt.cc/bbs/Stella/../Other/M.1.A.001.html",
            "https://www.ptt.cc/bbs/%53tella/M.1.A.001.html",
            "https://www.ptt.cc\\bbs\\Stella\\M.1.A.001.html",
            "file:///bbs/Stella/M.1.A.001.html",
            "https://www.ptt.cc/bbs/Stella/index.html",
            "https://www.ptt.cc/bbs/Stella/M.1.A.001.html/extra",
        ] {
            assert!(ArticleTarget::parse(bad).is_err(), "{bad:?}");
        }
        assert!(ArticleTarget::parse(&"x".repeat(MAX_INPUT + 1)).is_err());
    }
}
