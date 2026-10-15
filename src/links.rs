//! URLs are mapped from already decoded glyphs, without joining screen rows.
use crate::screen::Frame;
use serde::Serialize;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Link {
    pub row: usize,
    pub start: usize,
    pub end: usize,
    pub url: String,
}

pub fn validate(value: &str) -> Result<Url, &'static str> {
    let url = Url::parse(value).map_err(|_| "網址格式不正確")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("只支援 HTTP／HTTPS 網址");
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
    let mut rows = vec![Vec::new(); frame.rows];
    for glyph in &frame.glyphs {
        rows[glyph.row].push(glyph);
    }
    let mut links = Vec::new();
    for (row, glyphs) in rows.into_iter().enumerate() {
        let mut text = String::new();
        let mut mapping = Vec::new();
        for glyph in glyphs {
            let start = text.len();
            text.push_str(&glyph.text);
            mapping.push((
                start,
                text.len(),
                glyph.col,
                glyph.col + glyph.width,
                glyph.unmapped,
            ));
        }
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
                .any(|(a, b, _, _, unmapped)| *unmapped && *a < end && *b > start);
            if trimmed_end < text.len()
                && mapped
                && let Ok(url) = validate(candidate)
            {
                let first = mapping
                    .iter()
                    .find(|(a, b, _, _, _)| *a <= start && start < *b);
                let last = mapping
                    .iter()
                    .find(|(a, b, _, _, _)| *a < trimmed_end && trimmed_end <= *b);
                if let (Some((_, _, left, _, _)), Some((_, _, _, right, _))) = (first, last) {
                    links.push(Link {
                        row,
                        start: *left,
                        end: *right,
                        url: url.into(),
                    });
                }
            }
            offset = end.max(start + "http://".len());
        }
    }
    links
}
