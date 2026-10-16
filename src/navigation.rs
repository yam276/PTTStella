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
    BoardHeader,
    BoardRows,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Context {
    Unknown {
        reason: UnknownReason,
    },
    MainMenu {
        first_row: usize,
        last_row: usize,
    },
    BoardList {
        first_row: usize,
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
            Self::MainMenu { .. } => "main_menu",
            Self::BoardList { .. } => "board_list",
            Self::ArticleList { .. } => "article_list",
            Self::ArticleReading { .. } => "article_reading",
        }
    }
    pub fn allows_keepalive(&self) -> bool {
        matches!(self, Self::ArticleList { .. } | Self::ArticleReading { .. })
    }
    pub fn content_rows(&self) -> usize {
        match self {
            Self::MainMenu {
                first_row,
                last_row,
            } => last_row - first_row + 1,
            Self::BoardList { first_row } => 23 - first_row,
            Self::ArticleList { articles, .. } => 23 - articles.first().map_or(3, |a| a.row),
            _ => 23,
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
    let footer = &rows[23];
    if let Some(context) = main_menu(&rows, frame.cursor) {
        return context;
    }
    if let Some(context) = board_list(&rows, frame.cursor) {
        return context;
    }
    if rows.iter().any(|r| !r.mapped()) {
        return unknown(UnknownReason::Unmapped);
    }
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
    // PTT distributes commands across the subheader and footer. The forward
    // command is commonly on row 1, while the caption/return command is below.
    if !commands(&rows, "閱讀") {
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
fn commands(rows: &[Row<'_>], forward: &str) -> bool {
    let bars = format!("{} {}", rows[1].text, rows[23].text);
    bars.contains('←')
        && (bars.contains("離開") || bars.contains("回到上層") || bars.contains("回上層"))
        && bars.contains('→')
        && bars.contains(forward)
        && (rows[23].bar_text("離開")
            || rows[23].bar_text("回到上層")
            || rows[23].bar_text("回上層")
            || rows[23].bar_text("文章選讀")
            || rows[23].bar_text("選擇看板"))
}
fn pointer(line: &Row<'_>, col: usize) -> bool {
    matches!(line.cells(col, col + 2).map(str::trim), Some(">" | "●"))
}
fn main_menu(rows: &[Row<'_>], cursor: (usize, usize)) -> Option<Context> {
    if !compact(&rows[0].text).contains("主功能表")
        || !rows[0].bar_text("主功能表")
        || !rows[23].bar_text("主選單")
        || !rows[23].text.contains("說明")
    {
        return None;
    }
    let mut items = Vec::new();
    for (row, line) in rows.iter().enumerate().take(23).skip(1) {
        let Some(item) = line.cells(22, 80) else {
            continue;
        };
        if item.starts_with('(')
            && item
                .as_bytes()
                .get(1)
                .is_some_and(u8::is_ascii_alphanumeric)
            && item.as_bytes().get(2) == Some(&b')')
            && item.contains('【')
            && item.contains('】')
        {
            items.push(row);
        }
    }
    let (&first_row, &last_row) = (items.first()?, items.last()?);
    if !rows[0].mapped()
        || !rows[23].mapped()
        || rows[first_row..=last_row].iter().any(|r| !r.mapped())
    {
        return Some(unknown(UnknownReason::Unmapped));
    }
    Some(
        if items.len() >= 3
            && items.windows(2).all(|pair| pair[1] == pair[0] + 1)
            && items.contains(&cursor.1)
            && (20..=21).contains(&cursor.0)
            && pointer(&rows[cursor.1], 20)
        {
            Context::MainMenu {
                first_row,
                last_row,
            }
        } else {
            unknown(UnknownReason::InvalidCursor)
        },
    )
}
fn board_list(rows: &[Row<'_>], cursor: (usize, usize)) -> Option<Context> {
    let class_root = rows[0].bar_text("分類看板");
    let first_row = if class_root {
        7
    } else {
        let header = (1..=2).find(|&row| {
            let text = compact(&rows[row].text);
            text.contains("看板") && text.contains("類別")
        })?;
        let text = compact(&rows[header].text);
        // The description caption is presentation, not an input contract.
        // Match stable browser columns plus title/style instead of its wording.
        if !(rows[0].bar_text("看板列表") || rows[0].bar_text("我的最愛"))
            || !(text.contains("編號") || text.contains("總數"))
            || !text.contains("人氣")
            || !text.contains("板主")
            || !rows[header].bar_text("類別")
        {
            return Some(unknown(UnknownReason::BoardHeader));
        }
        header + 1
    };
    if !rows[0].mapped()
        || rows[if class_root { 7 } else { 1 }..]
            .iter()
            .any(|r| !r.mapped())
    {
        return Some(unknown(UnknownReason::Unmapped));
    }
    if !commands(rows, "進入") && !commands(rows, "閱讀") {
        return Some(unknown(UnknownReason::Unrecognized));
    }
    let mut items = Vec::new();
    let mut blank = false;
    for (row, line) in rows.iter().enumerate().take(23).skip(first_row) {
        if line.text.trim().is_empty() {
            blank = true;
            continue;
        }
        let valid = if class_root {
            line.cells(10, 17)
                .and_then(|s| number(s.trim()))
                .is_some_and(|n| n > 0)
                && line.cells(20, 80).is_some_and(|s| !s.trim().is_empty())
        } else {
            // Names occupy a fixed column; folders and separator rows share
            // the browser without interpreting their descriptions.
            let name = line.cells(10, 23).map(str::trim).unwrap_or("");
            !name.is_empty()
                && name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_- .".contains(&c))
        };
        if blank || !valid {
            return Some(unknown(UnknownReason::BoardRows));
        }
        items.push(row);
    }
    Some(
        if cursor.0 <= 2 && items.contains(&cursor.1) && pointer(&rows[cursor.1], 0) {
            Context::BoardList { first_row }
        } else {
            unknown(UnknownReason::InvalidCursor)
        },
    )
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
    // PTT's columns are gutter(1), number(6), flags/recommendations(4),
    // date(6), author(13), title. Recommendations may touch the date.
    // The legacy two-cell pointer covers the first number cell, so only that
    // selected row may have a partial numeric suffix (including "00000").
    let (prefix, partial) = match line.cells(1, 7) {
        Some(prefix) => (prefix.trim(), false),
        None if pointer(line, 0) => (line.cells(2, 7)?.trim(), true),
        None => return None,
    };
    if prefix != "★" && !number(prefix).is_some_and(|n| n > 0 || partial) {
        return None;
    }
    let (month, day) = line.cells(11, 17)?.trim().split_once('/')?;
    if !number(month.trim()).is_some_and(|m| (1..=12).contains(&m))
        || !number(day.trim()).is_some_and(|d| (1..=31).contains(&d))
    {
        return None;
    }
    let author = line.cells(17, 30)?.trim();
    if author.is_empty() {
        return None;
    }
    let title = line.cells(30, 80)?.trim();
    let (mark, content) = title.split_once(char::is_whitespace)?;
    if content.trim().is_empty() {
        return None;
    }
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
            Context::MainMenu { .. } | Context::BoardList { .. } | Context::ArticleList { .. } => {
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
        (
            Context::MainMenu { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. }
            | Context::ArticleReading { .. },
            Intent::First,
        ) => b"\x1b[H".as_slice(),
        (
            Context::MainMenu { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. }
            | Context::ArticleReading { .. },
            Intent::Last,
        ) => b"\x1b[F".as_slice(),
        (
            Context::MainMenu { .. } | Context::BoardList { .. } | Context::ArticleList { .. },
            Intent::Forward,
        ) => b"\r".as_slice(),
        (Context::ArticleReading { .. }, Intent::Forward) => b"\x1b[6~".as_slice(),
        (
            Context::MainMenu { .. } | Context::BoardList { .. } | Context::ArticleList { .. },
            Intent::Back,
        ) => b"\x1b[D".as_slice(),
        (
            Context::MainMenu { .. } | Context::BoardList { .. } | Context::ArticleList { .. },
            Intent::PreviousPage,
        ) => b"\x1b[5~".as_slice(),
        (
            Context::MainMenu { .. } | Context::BoardList { .. } | Context::ArticleList { .. },
            Intent::NextPage,
        ) => b"\x1b[6~".as_slice(),
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
        (
            Context::MainMenu { .. } | Context::BoardList { .. } | Context::ArticleList { .. },
            Intent::PreviousItem,
        ) => b"\x1b[A".as_slice(),
        (
            Context::MainMenu { .. } | Context::BoardList { .. } | Context::ArticleList { .. },
            Intent::NextItem,
        ) => b"\x1b[B".as_slice(),
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
