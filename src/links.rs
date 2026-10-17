//! One validation and projection for explicit cell metadata and plain-text URLs.
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

pub const MAX_LINKS: usize = 512;
pub const MAX_PROJECTED_URL_BYTES: usize = 256 * 1024;

pub fn validate(value: &str) -> Result<Url, String> {
    if value.len() > crate::screen::hyperlinks::MAX_URL_BYTES
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(crate::msg!("link-invalid"));
    }
    let url = Url::parse(value).map_err(|_| crate::msg!("link-invalid"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || url.as_str().len() > crate::screen::hyperlinks::MAX_URL_BYTES
    {
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
    if !frame.control_complete {
        return Vec::new();
    }
    let Some(rows) = text::rows(frame) else {
        return Vec::new();
    };
    let mut links = Vec::new();
    let mut bytes = 0;
    for explicit in &frame.hyperlinks {
        let Some(url) = explicit
            .target
            .as_deref()
            .and_then(|value| validate(value).ok())
        else {
            continue;
        };
        // Admit a complete target together, so a long wrapped link is never partial.
        let cost = url.as_str().len().saturating_mul(explicit.spans.len());
        if links.len() + explicit.spans.len() > MAX_LINKS || cost > MAX_PROJECTED_URL_BYTES - bytes
        {
            continue;
        }
        for span in &explicit.spans {
            if span.row >= frame.rows || span.start >= span.end || span.end > frame.cols {
                continue;
            }
            links.push(Link {
                row: span.row,
                start: span.start,
                end: span.end,
                url: url.as_str().to_owned(),
            });
        }
        bytes += cost;
    }
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
                    let (left, right) = (first.glyph.col, last.glyph.col + last.glyph.width);
                    if frame.hyperlinks.iter().any(|link| {
                        link.spans
                            .iter()
                            .any(|span| span.row == row && span.start < right && span.end > left)
                    }) {
                        offset = end.max(start + "http://".len());
                        continue;
                    }
                    if links.len() == MAX_LINKS
                        || url.as_str().len() > MAX_PROJECTED_URL_BYTES - bytes
                    {
                        return links;
                    }
                    bytes += url.as_str().len();
                    links.push(Link {
                        row,
                        start: left,
                        end: right,
                        url: url.into(),
                    });
                }
            }
            offset = end.max(start + "http://".len());
        }
    }
    links
}
