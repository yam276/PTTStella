//! URLs are mapped from already decoded glyphs, without joining screen rows.
use crate::{screen::Frame, text};
use serde::Serialize;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Link {
    pub row: usize,
    pub start: usize,
    pub end: usize,
    pub url: String,
}

pub fn validate(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|_| crate::msg!("link-invalid"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(crate::msg!("link-scheme"));
    }
    Ok(url)
}

fn trim_end(value: &str) -> &str {
    let mut value = value;
    loop {
        let Some(last) = value.chars().next_back() else {
            return value;
        };
        let trim = matches!(
            last,
            '.' | ',' | ';' | '!' | '。' | '，' | '；' | '！' | '」' | '』' | '》' | '】'
        ) || [('(', ')'), ('[', ']'), ('{', '}')]
            .iter()
            .any(|(left, right)| {
                last == *right
                    && value.chars().filter(|ch| ch == right).count()
                        > value.chars().filter(|ch| ch == left).count()
            });
        if !trim {
            return value;
        }
        value = &value[..value.len() - last.len_utf8()];
    }
}

pub fn detect(frame: &Frame) -> Vec<Link> {
    let Some(rows) = text::rows(frame) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    for (row, line) in rows.into_iter().enumerate() {
        let text = line.text;
        let mapping = line.spans;
        let lower = text.to_ascii_lowercase();
        let mut offset = 0;
        while offset < text.len() {
            let Some(found) = [
                lower[offset..].find("https://"),
                lower[offset..].find("http://"),
            ]
            .into_iter()
            .flatten()
            .min() else {
                break;
            };
            let start = offset + found;
            let end = text[start..]
                .char_indices()
                .find(|(_, ch)| ch.is_whitespace() || matches!(ch, '<' | '>' | '"' | '\''))
                .map_or(text.len(), |(at, _)| start + at);
            let candidate = trim_end(&text[start..end]);
            let trimmed_end = start + candidate.len();
            // A URL touching the right edge may be only the start of a wrapped URL.
            let mapped = !mapping
                .iter()
                .any(|s| s.glyph.unmapped && s.bytes.start < end && s.bytes.end > start);
            if trimmed_end < text.len()
                && mapped
                && let Ok(url) = validate(candidate)
            {
                let first = mapping
                    .iter()
                    .find(|s| s.bytes.start <= start && start < s.bytes.end);
                let last = mapping
                    .iter()
                    .find(|s| s.bytes.start < trimmed_end && trimmed_end <= s.bytes.end);
                if let (Some(first), Some(last)) = (first, last) {
                    links.push(Link {
                        row,
                        start: first.glyph.col,
                        end: last.glyph.col + last.glyph.width,
                        url: url.into(),
                    });
                }
            }
            offset = end.max(start + "http://".len());
        }
    }
    links
}
