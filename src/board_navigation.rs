//! One bounded, screen-confirmed switch. The Connection owns all resulting writes.
use crate::{
    navigation::{self, Context},
    screen::Frame,
    text,
};
use serde::Serialize;
use std::{
    fmt,
    time::{Duration, Instant},
};

pub const STEP_LIMIT: Duration = Duration::from_secs(5);
pub const TOTAL_LIMIT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct BoardName(String);
impl BoardName {
    pub fn parse(value: &str) -> Result<Self, Error> {
        let value = value.trim();
        if !(2..=12).contains(&value.len())
            || !value.as_bytes()[0].is_ascii_alphabetic()
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
        {
            return Err(Error::InvalidName);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidName,
    Unsupported,
    Manual,
    Timeout,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&match self {
            Self::InvalidName => crate::msg!("board-name-invalid"),
            Self::Unsupported => crate::msg!("board-switch-unsupported"),
            Self::Manual => crate::msg!("board-switch-manual"),
            Self::Timeout => crate::msg!("board-switch-timeout"),
        })
    }
}

pub fn browser(frame: &Frame) -> bool {
    match navigation::analyze(frame) {
        Context::BoardList { .. } => true,
        Context::EmptyList => text::rows(frame).is_some_and(|rows| {
            rows[2].bar_text("類別")
                && rows[2]
                    .text
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>()
                    .contains("看板")
        }),
        _ => false,
    }
}

pub fn current_board(frame: &Frame) -> Option<BoardName> {
    match navigation::analyze(frame) {
        Context::ArticleList { .. } => {}
        Context::EmptyList if !browser(frame) => {}
        _ => return None,
    }
    let rows = text::rows(frame)?;
    let header = &rows[0];
    if !header.mapped() || !header.bar_text("看板") {
        return None;
    }
    let line = header.text.trim_end();
    if let Some(line) = line.strip_suffix('》') {
        return BoardName::parse(line.rsplit_once("看板《")?.1).ok();
    }
    let tail = line
        .rsplit_once("看板:")
        .or_else(|| line.rsplit_once("看板："))?
        .1;
    let mut parts = tail.split_whitespace();
    let board = BoardName::parse(parts.next()?).ok()?;
    let population = parts.next()?.strip_prefix("人氣:")?;
    if population.is_empty()
        || !population.bytes().all(|b| b.is_ascii_digit())
        || parts.next().is_some()
    {
        return None;
    }
    Some(board)
}

pub fn selected_board(frame: &Frame) -> Option<BoardName> {
    if !matches!(navigation::analyze(frame), Context::BoardList { .. }) {
        return None;
    }
    let rows = text::rows(frame)?;
    if rows[0].bar_text("分類看板") {
        return None;
    }
    let row = &rows[frame.cursor.1];
    let category: String = row
        .spans
        .iter()
        .filter(|span| span.glyph.col >= 23 && span.glyph.col < 30)
        .map(|span| span.glyph.text.as_str())
        .collect();
    if category.contains("目錄") {
        return None;
    }
    BoardName::parse(row.cells(10, 23)?).ok()
}

pub fn search_prompt(frame: &Frame) -> bool {
    if (frame.cols, frame.rows) != (80, 24) || !frame.control_complete || frame.cursor.1 != 1 {
        return false;
    }
    let Some(rows) = text::rows(frame) else {
        return false;
    };
    let col = frame.cursor.0;
    rows[0].mapped()
        && rows[1].mapped()
        && rows[0].bar_text("【 搜尋全站看板 】")
        && col > 0
        && col < 80
        && rows[1]
            .cells(0, col)
            .is_some_and(|s| s.trim() == "請輸入看板名稱(按空白鍵自動搜尋):")
        && rows[1].cells(col, 80).is_some_and(|s| s.trim().is_empty())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    Articles,
    Boards,
    Prompt,
    Target,
    Entered,
}

pub struct Switch {
    target: BoardName,
    stage: Stage,
    revision: u64,
    deadline: Instant,
    step_deadline: Instant,
}
pub enum Start {
    Reached(BoardName),
    Run { switch: Switch, bytes: Vec<u8> },
}
#[derive(Debug, PartialEq, Eq)]
pub enum Advance {
    Wait,
    Send(Vec<u8>),
    Reached(BoardName),
}
impl Switch {
    pub fn needs_frame(&self, revision: u64) -> bool {
        revision > self.revision
    }
    pub fn begin(frame: &Frame, target: BoardName, now: Instant) -> Result<Start, Error> {
        if let Some(board) = current_board(frame).filter(|board| board.same(&target)) {
            return Ok(Start::Reached(board));
        }
        let (stage, bytes) = if browser(frame) {
            (Stage::Prompt, b"s".to_vec())
        } else {
            match navigation::analyze(frame) {
                Context::MainMenu { .. }
                    if text::rows(frame).is_some_and(|rows| {
                        rows.iter().any(|row| {
                            row.cells(22, 80).is_some_and(|item| {
                                item.starts_with("(F)avorite") && item.contains("最愛")
                            })
                        })
                    }) =>
                {
                    (Stage::Boards, b"f".to_vec())
                }
                Context::ArticleReading { .. } => (Stage::Articles, b"\x1b[D".to_vec()),
                Context::ArticleList { .. } | Context::EmptyList => {
                    (Stage::Boards, b"\x1b[D".to_vec())
                }
                _ => return Err(Error::Unsupported),
            }
        };
        Ok(Start::Run {
            switch: Self {
                target,
                stage,
                revision: frame.revision,
                deadline: now + TOTAL_LIMIT,
                step_deadline: now + STEP_LIMIT,
            },
            bytes,
        })
    }
    pub fn expired(&self, now: Instant) -> bool {
        now >= self.deadline || now >= self.step_deadline
    }
    pub fn observe(&mut self, frame: &Frame, now: Instant) -> Result<Advance, Error> {
        if self.expired(now) {
            return Err(Error::Timeout);
        }
        if !frame.control_complete || frame.revision <= self.revision {
            return Ok(Advance::Wait);
        }
        self.revision = frame.revision;
        if let Some(board) = current_board(frame).filter(|board| board.same(&self.target)) {
            return Ok(Advance::Reached(board));
        }
        let context = navigation::analyze(frame);
        if matches!(
            context,
            Context::ContinuePrompt | Context::CommandHelp { .. }
        ) || crate::login::prompt(frame).is_some()
            || crate::login::verification(frame).is_some()
        {
            return Err(Error::Manual);
        }
        let (next, bytes) = match self.stage {
            Stage::Articles
                if matches!(context, Context::ArticleList { .. })
                    || matches!(context, Context::EmptyList) && !browser(frame) =>
            {
                (Stage::Boards, b"\x1b[D".to_vec())
            }
            Stage::Boards if browser(frame) => (Stage::Prompt, b"s".to_vec()),
            Stage::Prompt if search_prompt(frame) => {
                let mut bytes = self.target.as_str().as_bytes().to_vec();
                bytes.push(b'\r');
                (Stage::Target, bytes)
            }
            Stage::Target
                if selected_board(frame).is_some_and(|board| board.same(&self.target)) =>
            {
                (Stage::Entered, b"\x1b[C".to_vec())
            }
            _ => return Ok(Advance::Wait),
        };
        self.stage = next;
        self.step_deadline = now + STEP_LIMIT;
        Ok(Advance::Send(bytes))
    }
}
