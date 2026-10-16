//! Conservative screen recognition and typed browsing intents, independent of UI/network.
use crate::{
    screen::Frame,
    text::{self, Row},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Stamp {
    pub generation: u64,
    pub revision: u64,
    pub input_epoch: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Article {
    pub row: usize,
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownReason {
    UnsupportedGrid,
    Incomplete,
    Unmapped,
    Unrecognized,
    InvalidCursor,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Context {
    Unknown {
        reason: UnknownReason,
    },
    ArticleList {
        articles: Vec<Article>,
        cursor_row: usize,
    },
    ArticleReading {
        first_line: usize,
        last_line: usize,
        percent: u8,
    },
}
impl Context {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Unknown { .. } => "unknown",
            Self::ArticleList { .. } => "article_list",
            Self::ArticleReading { .. } => "article_reading",
        }
    }
}
fn unknown(reason: UnknownReason) -> Context {
    Context::Unknown { reason }
}

pub fn analyze(frame: &Frame) -> Context {
    if (frame.cols, frame.rows) != (80, 24) {
        return unknown(UnknownReason::UnsupportedGrid);
    }
    if !frame.control_complete {
        return unknown(UnknownReason::Incomplete);
    }
    let Some(rows) = text::rows(frame) else {
        return unknown(UnknownReason::Incomplete);
    };
    if rows.iter().any(|r| !r.mapped()) {
        return unknown(UnknownReason::Unmapped);
    }
    let footer = &rows[23];
    // Only the normal pmore range report is admitted in this first profile.
    if frame.cursor.1 == 23
        && footer.bar_text("瀏覽")
        && footer.bar_text("目前顯示")
        && footer.text.contains('←')
        && footer.text.contains("離開")
        && let Some((first_line, last_line, percent)) = reading(&footer.text)
    {
        return Context::ArticleReading {
            first_line,
            last_line,
            percent,
        };
    }
    let header = (1..=2).find(|row| {
        let compact = compact(&rows[*row].text);
        compact.contains("編號")
            && compact.contains("日期")
            && compact.contains("作者")
            && compact.contains("標題")
            && rows[*row].bar_text("編號")
    });
    let Some(header) = header else {
        return unknown(UnknownReason::Unrecognized);
    };
    if !footer.text.contains("離開")
        || !footer.text.contains("閱讀")
        || !footer.text.contains('←')
        || !footer.text.contains('→')
    {
        return unknown(UnknownReason::Unrecognized);
    }
    let mut articles = Vec::new();
    let mut blank = false;
    for (row, line) in rows.iter().enumerate().take(23).skip(header + 1) {
        if line.text.trim().is_empty() {
            blank = true;
            continue;
        }
        if blank {
            return unknown(UnknownReason::Incomplete);
        }
        let Some(article) = article(line, row) else {
            return unknown(UnknownReason::Unrecognized);
        };
        articles.push(article);
    }
    if !articles.iter().any(|a| a.row == frame.cursor.1) || frame.cursor.0 > 2 {
        return unknown(UnknownReason::InvalidCursor);
    }
    let marker = rows[frame.cursor.1].cells(0, 2).unwrap_or("").trim();
    if !matches!(marker, ">" | "●") {
        return unknown(UnknownReason::InvalidCursor);
    }
    Context::ArticleList {
        articles,
        cursor_row: frame.cursor.1,
    }
}
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
fn number(text: &str) -> Option<usize> {
    (!text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()))
        .then(|| text.parse().ok())
        .flatten()
}
fn article(line: &Row<'_>, row: usize) -> Option<Article> {
    let prefix = line.cells(2, 8)?.trim();
    if prefix != "★" && number(prefix).is_none_or(|n| n == 0) {
        return None;
    }
    // Date, author and a non-empty title independently constrain an item row.
    let rest = line.cells(8, 80)?;
    let tokens: Vec<_> = rest.split_whitespace().collect();
    // PTT's five-column date pads a single-digit day: " 1/ 9".
    // Whitespace tokenization splits that date, but not its following author.
    let (date, date_tokens) = tokens.iter().take(3).enumerate().find_map(|(i, s)| {
        let (month, day) = s.split_once('/')?;
        let padded = day.is_empty();
        let day = if padded { *tokens.get(i + 1)? } else { day };
        (number(month).is_some_and(|m| (1..=12).contains(&m))
            && number(day).is_some_and(|d| (1..=31).contains(&d)))
        .then_some((i, if padded { 2 } else { 1 }))
    })?;
    let author_index = date + date_tokens;
    if tokens.len() < author_index + 3 {
        return None;
    }
    let author = tokens[author_index];
    let mark = tokens[author_index + 1];
    let deleted = author == "-" && matches!(mark, "╳" | "□");
    if !deleted
        && (author.len() > 16
            || !author
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_'))
    {
        return None;
    }
    if !matches!(mark, "□" | "◇" | "R:" | "轉" | "Re:" | "鎖" | "ˇ") && !deleted {
        return None;
    }
    Some(Article {
        row,
        start: 2,
        end: 8,
    })
}
fn reading(text: &str) -> Option<(usize, usize, u8)> {
    let text = compact(text);
    let rest = text.strip_prefix("瀏覽第")?;
    let (pages, rest) = rest.split_once("頁(")?;
    if let Some((page, total)) = pages.split_once('/') {
        let (page, total) = (number(page)?, number(total)?);
        if page == 0 || page > total {
            return None;
        }
    } else if number(pages)? == 0 {
        return None;
    }
    let (percent, rest) = rest.split_once("%)")?;
    let percent = number(percent)?;
    if percent > 100 {
        return None;
    }
    let rest = rest.strip_prefix("目前顯示:第")?;
    let (first, rest) = rest.split_once('~')?;
    let (last, _) = rest.split_once('行')?;
    let (first, last) = (number(first)?, number(last)?);
    if first == 0 || last < first || last - first >= 23 {
        return None;
    }
    Some((first, last, percent as u8))
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Intent {
    Open { row: usize },
    PreviousPage,
    NextPage,
    Back,
    PreviousItem,
    NextItem,
    First,
    Last,
    Forward,
    Lines { down: bool, count: usize },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unavailable,
    Stale,
    AwaitingScreen,
    Disconnected,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "此畫面或位置未提供導覽",
            Self::Stale => "畫面或輸入已變更；導覽未送出",
            Self::AwaitingScreen => "等待站端更新畫面後再導覽",
            Self::Disconnected => "連線已變更；導覽未送出",
        })
    }
}
pub fn plan(context: &Context, intent: Intent) -> Result<Vec<u8>, Error> {
    if let Intent::Lines { down, count } = intent {
        if count == 0 || count > crate::input::MAX_INPUT_BYTES / 3 {
            return Err(Error::Unavailable);
        }
        return match context {
            Context::ArticleList { .. } => {
                Ok(if down { b"\x1b[B" } else { b"\x1b[A" }.repeat(count))
            }
            Context::ArticleReading {
                first_line,
                percent,
                ..
            } => {
                let count = if down {
                    if *percent == 100 { 0 } else { count }
                } else {
                    count.min(first_line - 1)
                };
                Ok(if down { b"j" } else { b"k" }.repeat(count))
            }
            _ => Err(Error::Unavailable),
        };
    }
    let bytes = match (context, intent) {
        (Context::ArticleReading { first_line: 1, .. }, Intent::First)
        | (Context::ArticleReading { percent: 100, .. }, Intent::Last)
        | (Context::ArticleReading { percent: 100, .. }, Intent::Forward) => b"".as_slice(),
        (Context::ArticleList { .. } | Context::ArticleReading { .. }, Intent::First) => {
            b"\x1b[H".as_slice()
        }
        (Context::ArticleList { .. } | Context::ArticleReading { .. }, Intent::Last) => {
            b"\x1b[F".as_slice()
        }
        (Context::ArticleList { .. }, Intent::Forward) => b"\r".as_slice(),
        (Context::ArticleReading { .. }, Intent::Forward) => b"\x1b[6~".as_slice(),
        (Context::ArticleList { .. }, Intent::Back) => b"\x1b[D".as_slice(),
        (Context::ArticleList { .. }, Intent::PreviousPage) => b"\x1b[5~".as_slice(),
        (Context::ArticleList { .. }, Intent::NextPage) => b"\x1b[6~".as_slice(),
        (
            Context::ArticleList {
                articles,
                cursor_row,
            },
            Intent::Open { row },
        ) if row < 23 && *cursor_row < 23 && articles.iter().any(|a| a.row == row) => {
            let mut bytes = if row < *cursor_row {
                b"\x1b[A".repeat(cursor_row - row)
            } else {
                b"\x1b[B".repeat(row - cursor_row)
            };
            bytes.push(13);
            return Ok(bytes);
        }
        (Context::ArticleList { .. }, Intent::PreviousItem) => b"\x1b[A".as_slice(),
        (Context::ArticleList { .. }, Intent::NextItem) => b"\x1b[B".as_slice(),
        (Context::ArticleReading { first_line, .. }, Intent::PreviousPage) if *first_line > 1 => {
            b"\x1b[5~".as_slice()
        }
        (Context::ArticleReading { percent, .. }, Intent::NextPage) if *percent < 100 => {
            b"\x1b[6~".as_slice()
        }
        (Context::ArticleReading { .. }, Intent::Back) => b"\x1b[D".as_slice(),
        _ => return Err(Error::Unavailable),
    };
    Ok(bytes.to_vec())
}
