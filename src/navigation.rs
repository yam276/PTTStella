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

/// Pointer navigation competes with station reports; buttons and keys are
/// explicit commands. Only keys may continue a movement without a redraw.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Pointer,
    Button,
    Keyboard,
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
    DifferentSite,
    UnsupportedGrid,
    Incomplete,
    Unmapped,
    Unrecognized,
    InvalidCursor,
    BoardHeader,
    BoardRows,
    UnsupportedLayout,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Mail,
    Archive,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Context {
    Unknown {
        reason: UnknownReason,
    },
    ContinuePrompt,
    EmptyList,
    CommandHelp {
        first_row: usize,
        last_row: usize,
    },
    MainMenu {
        first_row: usize,
        last_row: usize,
        cursor_row: usize,
    },
    SystemMenu {
        first_row: usize,
        last_row: usize,
        cursor_row: usize,
    },
    DocumentList {
        category: DocumentKind,
        first_row: usize,
        last_row: usize,
        cursor_row: usize,
    },
    BoardList {
        first_row: usize,
        last_row: usize,
        cursor_row: usize,
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
            Self::ContinuePrompt => "continue_prompt",
            Self::EmptyList => "empty_list",
            Self::CommandHelp { .. } => "command_help",
            Self::MainMenu { .. } => "main_menu",
            Self::SystemMenu { .. } => "system_menu",
            Self::BoardList { .. } => "board_list",
            Self::DocumentList { .. } => "document_list",
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
                ..
            }
            | Self::SystemMenu {
                first_row,
                last_row,
                ..
            }
            | Self::DocumentList {
                first_row,
                last_row,
                ..
            }
            | Self::CommandHelp {
                first_row,
                last_row,
                ..
            } => last_row - first_row + 1,
            Self::BoardList { first_row, .. } => 23 - first_row,
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
    // PTT's default vshowmsg(NULL) footer is a standalone blue pause band,
    // padded with spaces/half blocks, followed by its safe final column.
    // Art above it need not be mapped: only this footer authorizes one key.
    if frame.cursor == (79, 23)
        && footer.mapped()
        && footer
            .text
            .trim_matches(|c: char| c.is_whitespace() || c == '▄')
            == "請按任意鍵繼續"
        && footer.bar_text("請按任意鍵繼續")
    {
        return Context::ContinuePrompt;
    }
    if let Some(context) = command_help(&rows, frame.cursor) {
        return context;
    }
    if let Some(context) = main_menu(&rows, frame.cursor) {
        return context;
    }
    if let Some(context) = board_list(&rows, frame.cursor) {
        return context;
    }
    if let Some(context) = document_list(&rows, frame.cursor) {
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
    if empty_list(&rows, frame.cursor, header + 1, false) {
        return Context::EmptyList;
    }
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
    if !pointer(&rows[frame.cursor.1], 0) {
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
            || rows[23].bar_text("選擇看板")
            || rows[23].bar_text("看板列表")
            || rows[23].bar_text("我的最愛")
            || rows[23].bar_text("分類看板")
            || rows[23].bar_text("郵件選單")
            || rows[23].bar_text("精華列表")
            || rows[23].bar_text("精華管理"))
}

fn command_help(rows: &[Row<'_>], cursor: (usize, usize)) -> Option<Context> {
    if !rows[0].bar_text("操作說明 (Help)") {
        return None;
    }
    let header = compact(&rows[1].text);
    let footer = compact(&rows[23].text);
    if rows.iter().any(|row| !row.mapped())
        || !header.contains("按鍵/標籤")
        || !rows[1].bar_text("功能說明")
        || !rows[23].bar_text("操作說明")
        || !footer.contains("(Enter)執行")
        || !footer.contains("(q/←)離開")
    {
        return Some(unknown(UnknownReason::Unrecognized));
    }
    let mut last = None;
    let mut blank = false;
    for (row, line) in rows.iter().enumerate().take(23).skip(2) {
        if line.text.trim().is_empty() {
            blank = true;
            continue;
        }
        let Some(item) = line.cells(2, 80) else {
            return Some(unknown(UnknownReason::Unmapped));
        };
        let item = item.trim_end();
        let valid = item
            .strip_prefix('(')
            .and_then(|item| item.split_once(')'))
            .is_some_and(|(key, description)| {
                !key.is_empty() && key.len() <= 64 && !description.trim().is_empty()
            });
        if blank || !valid {
            return Some(unknown(UnknownReason::Unrecognized));
        }
        last = Some(row);
    }
    Some(match last {
        Some(last_row)
            if cursor.0 <= 2
                && (2..=last_row).contains(&cursor.1)
                && pointer(&rows[cursor.1], 0) =>
        {
            Context::CommandHelp {
                first_row: 2,
                last_row,
            }
        }
        _ => unknown(UnknownReason::InvalidCursor),
    })
}

fn empty_list(rows: &[Row<'_>], cursor: (usize, usize), first_row: usize, board: bool) -> bool {
    if cursor != (79, 23)
        || first_row != 3
        || rows.iter().any(|r| !r.mapped())
        || !rows[1].text.trim().starts_with('[')
        || ["請輸入", "密碼", "是否"]
            .iter()
            .any(|prompt| rows[1].text.contains(prompt))
    {
        return false;
    }
    let footer = &rows[23];
    let caption = if board {
        ["看板列表", "我的最愛"]
            .iter()
            .any(|caption| footer.bar_text(caption))
    } else {
        rows[0].bar_text("看板") && footer.bar_text("文章列表")
    };
    let message = rows[3].text.trim();
    let message = if board {
        matches!(
            message,
            "--- 本目錄目前尚無看板 ---"
                | "--- 空目錄，請按 a 新增或用 y 列出全部看板後按 z 增刪 ---"
                | "--- 註冊的使用者才能新增看板喔 (可按 s 手動選取) ---"
        )
    } else {
        message == "沒有文章..."
    };
    caption
        && message
        && footer.text.contains('←')
        && (footer.bar_text("回上層") || footer.bar_text("離開"))
        && rows[4..23].iter().all(|row| row.text.trim().is_empty())
}
fn pointer(line: &Row<'_>, col: usize) -> bool {
    // The one-cell ASCII pointer can touch a six-digit article number. The
    // legacy circle occupies two cells and may cover the first number digit.
    line.cells(col, col + 1) == Some(">") || line.cells(col, col + 2) == Some("●")
}
fn main_menu(rows: &[Row<'_>], cursor: (usize, usize)) -> Option<Context> {
    let root = compact(&rows[0].text).contains("主功能表")
        && rows[0].bar_text("主功能表")
        && rows[23].bar_text("主選單");
    let submenu = [
        ("私人信件區", "電子郵件"),
        ("休閒聊天區", "聊天說話"),
        ("個人設定區", "個人設定"),
        ("系統資訊區", "工具程式"),
        ("網路遊樂場", "休閒遊樂"),
        ("編特別名單", "名單編輯"),
        ("系統維護區", "系統維護"),
        ("金錢相關管理", "金錢管理"),
        ("使用者記錄管理", "記錄管理"),
        ("量販店", "量販商店"),
        ("棋院", "休閒棋院"),
    ]
    .iter()
    .any(|(title, status)| rows[0].bar_text(title) && rows[23].bar_text(status));
    if !(root || submenu) || !rows[23].text.contains("說明") {
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
            && item.get(3..).is_some_and(|tail| !tail.trim().is_empty())
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
        if items.len() >= (if root { 3 } else { 1 })
            && items.windows(2).all(|pair| pair[1] == pair[0] + 1)
            && items.contains(&cursor.1)
            && (20..=21).contains(&cursor.0)
            && pointer(&rows[cursor.1], 20)
        {
            if root {
                Context::MainMenu {
                    first_row,
                    last_row,
                    cursor_row: cursor.1,
                }
            } else {
                Context::SystemMenu {
                    first_row,
                    last_row,
                    cursor_row: cursor.1,
                }
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
            || (!(rows[23].bar_text("看板列表") || rows[23].bar_text("我的最愛"))
                && (!text.contains("人氣") || !text.contains("板主")))
            || !rows[header].bar_text("類別")
        {
            return Some(unknown(UnknownReason::BoardHeader));
        }
        header + 1
    };
    if !rows[0].mapped()
        || !rows[23].mapped()
        || (!class_root && !rows[first_row - 1].mapped())
        || (class_root && rows[first_row..].iter().any(|row| !row.mapped()))
    {
        return Some(unknown(UnknownReason::Unmapped));
    }
    if empty_list(rows, cursor, first_row, true) {
        return Some(Context::EmptyList);
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
        let structural = if class_root { 12..20 } else { 10..23 };
        if line.spans.iter().any(|span| {
            span.glyph.col < structural.end
                && span.glyph.col + span.glyph.width > structural.start
                && span.glyph.unmapped
        }) {
            return Some(unknown(UnknownReason::Unmapped));
        }
        let valid = if class_root {
            line.cells(12, 17)
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
        if items.contains(&cursor.1)
            && ((cursor.0 <= 2 && pointer(&rows[cursor.1], 0))
                || (class_root && (10..=11).contains(&cursor.0) && pointer(&rows[cursor.1], 10)))
        {
            Context::BoardList {
                first_row,
                last_row: *items.last()?,
                cursor_row: cursor.1,
            }
        } else {
            unknown(UnknownReason::InvalidCursor)
        },
    )
}

// Mail and archive browsers share row navigation, while retaining distinct
// title/header and number-field contracts. This says nothing about new mail.
fn document_list(rows: &[Row<'_>], cursor: (usize, usize)) -> Option<Context> {
    let (category, forward) = if rows[0].bar_text("郵件選單") {
        (DocumentKind::Mail, "讀信")
    } else if rows[0].bar_text("精華文章") {
        (DocumentKind::Archive, "讀取資料")
    } else {
        return None;
    };
    let header = (1..=2).find(|&r| {
        let text = compact(&rows[r].text);
        rows[r].bar_text("編號")
            && text.matches("編號").count() == 1
            && text.contains("日期")
            && text.contains("標題")
            && text.contains(if category == DocumentKind::Mail {
                "作者"
            } else {
                "編選"
            })
    });
    let Some(header) = header else {
        return Some(unknown(UnknownReason::UnsupportedLayout));
    };
    if !commands(rows, forward) {
        return Some(unknown(UnknownReason::Unrecognized));
    }
    if rows[0..=header].iter().any(|r| !r.mapped()) || !rows[23].mapped() {
        return Some(unknown(UnknownReason::Unmapped));
    }
    let first_row = header + 1;
    let mut last_row = None;
    let mut blank = false;
    for (row, line) in rows.iter().enumerate().take(23).skip(first_row) {
        if line.text.trim().is_empty() {
            blank = true;
            continue;
        }
        if blank {
            return Some(unknown(UnknownReason::Incomplete));
        }
        if !line.mapped() {
            return Some(unknown(UnknownReason::Unmapped));
        }
        let prefix = line.cells(2, 11).unwrap_or("").trim_start();
        let digits = prefix.bytes().take_while(u8::is_ascii_digit).count();
        let suffix = &prefix[digits..];
        let number_ok =
            (1..=6).contains(&digits) && number(&prefix[..digits]).is_some_and(|n| n > 0);
        let suffix_ok = match category {
            DocumentKind::Mail => {
                suffix.starts_with(' ') && suffix.bytes().all(|b| b" +mMrRD".contains(&b))
            }
            DocumentKind::Archive => {
                suffix.starts_with(['.', ')', 'X']) && suffix.bytes().all(|b| b" .)Xc".contains(&b))
            }
        };
        if !number_ok || !suffix_ok || line.cells(11, 80).is_none_or(|s| s.trim().is_empty()) {
            return Some(unknown(UnknownReason::Unrecognized));
        }
        last_row = Some(row);
    }
    let Some(last_row) = last_row else {
        return Some(unknown(UnknownReason::Unrecognized));
    };
    Some(
        if cursor.0 <= 2
            && (first_row..=last_row).contains(&cursor.1)
            && pointer(&rows[cursor.1], 0)
        {
            Context::DocumentList {
                category,
                first_row,
                last_row,
                cursor_row: cursor.1,
            }
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
    let date = line.cells(11, 17)?.trim();
    // Some legacy pinned announcements store this label in the date field.
    if !(prefix == "★" && date == "重要") {
        let (month, day) = date.split_once('/')?;
        if !number(month.trim()).is_some_and(|m| (1..=12).contains(&m))
            || !number(day.trim()).is_some_and(|d| (1..=31).contains(&d))
        {
            return None;
        }
    }
    let author = line.cells(17, 30)?.trim();
    if author.is_empty() {
        return None;
    }
    let title = line.cells(30, 80)?.trim();
    // A real article can have an empty title; its structural mark is still
    // required along with number/date/author and the surrounding list anchors.
    let mark = title.split_whitespace().next()?;
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
    Continue,
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
    RequiresNativeInput,
    Stale,
    AwaitingScreen,
    Disconnected,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&match self {
            Self::Unavailable => crate::msg!("navigation-unavailable"),
            Self::RequiresNativeInput => crate::msg!("navigation-help-native"),
            Self::Stale => crate::msg!("navigation-stale"),
            Self::AwaitingScreen => crate::msg!("navigation-awaiting"),
            Self::Disconnected => crate::msg!("navigation-disconnected"),
        })
    }
}
pub fn plan(context: &Context, intent: Intent) -> Result<Vec<u8>, Error> {
    if let (
        Context::MainMenu {
            first_row,
            last_row,
            cursor_row,
        }
        | Context::SystemMenu {
            first_row,
            last_row,
            cursor_row,
        }
        | Context::DocumentList {
            first_row,
            last_row,
            cursor_row,
            ..
        }
        | Context::BoardList {
            first_row,
            last_row,
            cursor_row,
        },
        Intent::Open { row },
    ) = (context, intent)
    {
        if *first_row > *last_row
            || *last_row >= 23
            || !(*first_row..=*last_row).contains(&row)
            || !(*first_row..=*last_row).contains(cursor_row)
        {
            return Err(Error::Unavailable);
        }
        return Ok(open_row(*cursor_row, row));
    }
    if let Intent::Lines { down, count } = intent {
        if count == 0 || count > crate::input::MAX_INPUT_BYTES / 3 {
            return Err(Error::Unavailable);
        }
        return match context {
            Context::MainMenu { .. }
            | Context::SystemMenu { .. }
            | Context::DocumentList { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. }
            | Context::CommandHelp { .. } => {
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
    if matches!(context, Context::EmptyList | Context::CommandHelp { .. }) {
        let bytes: &[u8] = match (context, intent) {
            (_, Intent::Back) => b"\x1b[D",
            (Context::CommandHelp { .. }, Intent::First) => b"\x1b[H",
            (Context::CommandHelp { .. }, Intent::Last) => b"\x1b[F",
            (Context::CommandHelp { .. }, Intent::PreviousPage) => b"\x1b[5~",
            (Context::CommandHelp { .. }, Intent::NextPage) => b"\x1b[6~",
            (Context::CommandHelp { .. }, Intent::PreviousItem) => b"\x1b[A",
            (Context::CommandHelp { .. }, Intent::NextItem) => b"\x1b[B",
            (Context::CommandHelp { .. }, Intent::Forward | Intent::Open { .. }) => {
                return Err(Error::RequiresNativeInput);
            }
            _ => return Err(Error::Unavailable),
        };
        return Ok(bytes.to_vec());
    }
    let bytes = match (context, intent) {
        (Context::ContinuePrompt, Intent::Continue) => b" ".as_slice(),
        (Context::ArticleReading { first_line: 1, .. }, Intent::First)
        | (Context::ArticleReading { percent: 100, .. }, Intent::Last)
        | (Context::ArticleReading { percent: 100, .. }, Intent::Forward) => b"".as_slice(),
        (
            Context::MainMenu { .. }
            | Context::SystemMenu { .. }
            | Context::DocumentList { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. }
            | Context::ArticleReading { .. },
            Intent::First,
        ) => b"\x1b[H".as_slice(),
        (
            Context::MainMenu { .. }
            | Context::SystemMenu { .. }
            | Context::DocumentList { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. }
            | Context::ArticleReading { .. },
            Intent::Last,
        ) => b"\x1b[F".as_slice(),
        (
            Context::MainMenu { .. }
            | Context::SystemMenu { .. }
            | Context::DocumentList { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. },
            Intent::Forward,
        ) => b"\r".as_slice(),
        (Context::ArticleReading { .. }, Intent::Forward) => b"\x1b[6~".as_slice(),
        (
            Context::MainMenu { .. }
            | Context::SystemMenu { .. }
            | Context::DocumentList { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. },
            Intent::Back,
        ) => b"\x1b[D".as_slice(),
        (
            Context::MainMenu { .. }
            | Context::SystemMenu { .. }
            | Context::DocumentList { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. },
            Intent::PreviousPage,
        ) => b"\x1b[5~".as_slice(),
        (
            Context::MainMenu { .. }
            | Context::SystemMenu { .. }
            | Context::DocumentList { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. },
            Intent::NextPage,
        ) => b"\x1b[6~".as_slice(),
        (
            Context::ArticleList {
                articles,
                cursor_row,
            },
            Intent::Open { row },
        ) if row < 23 && *cursor_row < 23 && articles.iter().any(|a| a.row == row) => {
            return Ok(open_row(*cursor_row, row));
        }
        (
            Context::MainMenu { .. }
            | Context::SystemMenu { .. }
            | Context::DocumentList { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. },
            Intent::PreviousItem,
        ) => b"\x1b[A".as_slice(),
        (
            Context::MainMenu { .. }
            | Context::SystemMenu { .. }
            | Context::DocumentList { .. }
            | Context::BoardList { .. }
            | Context::ArticleList { .. },
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

fn open_row(cursor: usize, row: usize) -> Vec<u8> {
    let mut bytes = if row < cursor {
        b"\x1b[A".repeat(cursor - row)
    } else {
        b"\x1b[B".repeat(row - cursor)
    };
    bytes.push(13);
    bytes
}
