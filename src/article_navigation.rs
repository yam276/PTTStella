//! A bounded article workflow, composed with the same board switch reducer.
use crate::{
    article_target::ArticleTarget,
    board_navigation::{self as board, Switch},
    navigation::{self, Context},
    screen::Frame,
    text,
};
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum Advance {
    Wait,
    Send(Vec<u8>),
    Reached(ArticleTarget),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Board(board::Error),
    Manual,
    Identity,
    Missing,
    Timeout,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Board(error) => error.fmt(f),
            Self::Manual => f.write_str(&crate::msg!("article-open-manual")),
            Self::Identity => f.write_str(&crate::msg!("article-open-identity")),
            Self::Missing => f.write_str(&crate::msg!("article-open-missing")),
            Self::Timeout => f.write_str(&crate::msg!("article-open-timeout")),
        }
    }
}

#[derive(Clone)]
struct Selected {
    row: usize,
    text: String,
}
impl Selected {
    fn from(frame: &Frame) -> Option<Self> {
        let Context::ArticleList { articles, .. } = navigation::analyze(frame) else {
            return None;
        };
        let row = frame.cursor.1;
        if !articles.iter().any(|article| article.row == row) {
            return None;
        }
        let rows = text::rows(frame)?;
        rows[row].mapped().then(|| Self {
            row,
            text: rows[row].text.clone(),
        })
    }
    fn preserved(&self, frame: &Frame) -> bool {
        text::rows(frame).is_some_and(|rows| {
            rows.get(self.row)
                .is_some_and(|row| row.mapped() && row.text == self.text)
        })
    }
    fn restored(&self, frame: &Frame) -> bool {
        frame.cursor.1 == self.row && Self::from(frame).is_some() && self.preserved(frame)
    }
}
enum Stage {
    Board(Switch),
    Prompt,
    Selected,
    Information(Selected),
    Return(Selected),
    Reading,
    Finished,
}
pub struct Open {
    target: ArticleTarget,
    stage: Stage,
    revision: u64,
    deadline: Instant,
    step_deadline: Instant,
}
impl Open {
    pub fn begin(
        frame: &Frame,
        mut target: ArticleTarget,
        now: Instant,
    ) -> Result<(Self, Vec<u8>), Error> {
        let (stage, bytes) =
            match Switch::begin(frame, target.board().clone(), now).map_err(Error::Board)? {
                board::Start::Reached(board) => {
                    target.confirm_board(board);
                    (Stage::Prompt, b"#".to_vec())
                }
                board::Start::Run { switch, bytes } => (Stage::Board(switch), bytes),
            };
        Ok((
            Self {
                target,
                stage,
                revision: frame.revision,
                deadline: now + board::TOTAL_LIMIT,
                step_deadline: now + board::STEP_LIMIT,
            },
            bytes,
        ))
    }
    pub fn needs_frame(&self, revision: u64) -> bool {
        !matches!(self.stage, Stage::Finished) && revision > self.revision
    }
    pub fn expired(&self, now: Instant) -> bool {
        now >= self.deadline
            || match &self.stage {
                Stage::Board(flow) => flow.expired(now),
                _ => now >= self.step_deadline,
            }
    }
    pub fn observe(&mut self, frame: &Frame, now: Instant) -> Result<Advance, Error> {
        let result = self.advance(frame, now);
        if result.is_err() || matches!(result, Ok(Advance::Reached(_))) {
            self.stage = Stage::Finished;
        }
        result
    }
    fn send(&mut self, stage: Stage, bytes: Vec<u8>, now: Instant) -> Advance {
        self.stage = stage;
        self.step_deadline = now + board::STEP_LIMIT;
        Advance::Send(bytes)
    }
    fn advance(&mut self, frame: &Frame, now: Instant) -> Result<Advance, Error> {
        if matches!(self.stage, Stage::Finished) {
            return Err(Error::Manual);
        }
        if self.expired(now) {
            return Err(Error::Timeout);
        }
        if !frame.control_complete || !self.needs_frame(frame.revision) {
            return Ok(Advance::Wait);
        }
        self.revision = frame.revision;
        if let Stage::Board(flow) = &mut self.stage {
            return match flow.observe(frame, now).map_err(Error::Board)? {
                board::Advance::Wait => Ok(Advance::Wait),
                board::Advance::Send(bytes) => Ok(Advance::Send(bytes)),
                board::Advance::Reached(board) => {
                    self.target.confirm_board(board);
                    Ok(self.send(Stage::Prompt, b"#".to_vec(), now))
                }
            };
        }
        if missing(frame) {
            return Err(Error::Missing);
        }
        // This exact, owned query is the only pause the article job may dismiss.
        if let Stage::Information(selected) = &self.stage
            && let Some(identity) = information(frame, selected)
        {
            if !identity.board().same(self.target.board()) || identity.aid() != self.target.aid() {
                return Err(Error::Identity);
            }
            let selected = selected.clone();
            return Ok(self.send(Stage::Return(selected), b" ".to_vec(), now));
        }
        if matches!(
            navigation::analyze(frame),
            Context::ContinuePrompt | Context::CommandHelp { .. }
        ) || crate::login::prompt(frame).is_some()
            || crate::login::verification(frame).is_some()
        {
            return Err(Error::Manual);
        }
        let current = board::current_board(frame);
        if current
            .as_ref()
            .is_some_and(|board| !board.same(self.target.board()))
        {
            return Err(Error::Identity);
        }
        match &self.stage {
            Stage::Prompt if search_prompt(frame) => {
                let mut bytes = self.target.aid().as_str().as_bytes().to_vec();
                bytes.push(b'\r');
                Ok(self.send(Stage::Selected, bytes, now))
            }
            Stage::Selected
                if current
                    .as_ref()
                    .is_some_and(|board| board.same(self.target.board())) =>
            {
                let Some(selected) = Selected::from(frame) else {
                    return Err(Error::Missing);
                };
                Ok(self.send(Stage::Information(selected), b"Q".to_vec(), now))
            }
            Stage::Return(selected)
                if current
                    .as_ref()
                    .is_some_and(|board| board.same(self.target.board())) =>
            {
                if !selected.restored(frame) {
                    return Err(Error::Identity);
                }
                Ok(self.send(Stage::Reading, b"\x1b[C".to_vec(), now))
            }
            Stage::Reading
                if matches!(
                    navigation::analyze(frame),
                    Context::ArticleReading { first_line: 1, .. }
                ) =>
            {
                let rows = text::rows(frame).ok_or(Error::Identity)?;
                let header = rows[0].text.trim_end();
                let board = header
                    .starts_with("作者")
                    .then(|| header.rsplit_once("看板").map(|(_, name)| name.trim()))
                    .flatten()
                    .and_then(|name| board::BoardName::parse(name).ok())
                    .ok_or(Error::Identity)?;
                if !board.same(self.target.board()) {
                    return Err(Error::Identity);
                }
                self.target.confirm_board(board);
                Ok(Advance::Reached(self.target.clone()))
            }
            _ => Ok(Advance::Wait),
        }
    }
}

pub fn search_prompt(frame: &Frame) -> bool {
    if (frame.cols, frame.rows) != (80, 24) || !frame.control_complete || frame.cursor.1 != 23 {
        return false;
    }
    let Some(rows) = text::rows(frame) else {
        return false;
    };
    let col = frame.cursor.0;
    col > 0
        && col < 80
        && rows[23].mapped()
        && rows[23]
            .cells(0, col)
            .is_some_and(|text| text == "搜尋文章代碼(AID): #")
        && rows[23]
            .cells(col, 80)
            .is_some_and(|text| text.trim().is_empty())
}
fn missing(frame: &Frame) -> bool {
    text::rows(frame).is_some_and(|rows| {
        rows[22].mapped()
            && [
                "不合法的文章代碼(AID)，請確定輸入是正確的",
                "找不到這個文章代碼(AID)，可能是文章已消失，或是你找錯看板了",
                "此狀態下無法使用搜尋文章代碼(AID)功能",
                "此文章無內容",
            ]
            .contains(&rows[22].text.trim())
    })
}
fn information(frame: &Frame, selected: &Selected) -> Option<ArticleTarget> {
    if !matches!(navigation::analyze(frame), Context::ContinuePrompt) || !selected.preserved(frame)
    {
        return None;
    }
    let rows = text::rows(frame)?;
    let top = format!("┌{}┐", "─".repeat(37));
    let bottom = "└──────────────────────── [再按 Q 切換價格列表]  ─┘";
    for height in [4, 5] {
        let mut first = selected.row + 1;
        if first + height > 23 || selected.row >= 15 {
            first = selected.row.checked_sub(height)?;
        }
        let last = first + height - 1;
        if !rows[first].mapped() || rows[first].text.trim_end() != top {
            continue;
        }
        if !rows[last].mapped() || rows[last].text.trim_end() != bottom {
            continue;
        }
        let line = &rows[first + 1];
        if !line.mapped() {
            continue;
        }
        let rest = line.text.strip_prefix("│ 文章代碼(AID): ")?;
        let mut parts = rest.split_whitespace();
        let aid = parts.next()?;
        let board = parts.next()?.strip_prefix('(')?.strip_suffix(')')?;
        if parts.next() != Some("[ptt.cc]") || !line.text.trim_end().ends_with('│') {
            return None;
        }
        let start = line.text.find(aid)?;
        if !line
            .spans
            .iter()
            .filter(|span| span.bytes.start < start + aid.len() && span.bytes.end > start)
            .all(|span| span.glyph.left.bright && span.glyph.right.bright)
        {
            return None;
        }
        return ArticleTarget::new(board, aid).ok();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encoding, screen::Screen};
    use std::time::Duration;
    fn scene(name: &str) -> Vec<u8> {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/article-open.json")).unwrap();
        encoding::encode(data["screens"][name].as_str().unwrap()).unwrap()
    }
    fn frame(name: &str) -> Frame {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&scene(name));
        screen.frame()
    }
    fn begin() -> (Screen, Open, Instant) {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&scene("articles"));
        let now = Instant::now();
        let (open, bytes) = Open::begin(
            &screen.frame(),
            ArticleTarget::new("stella", "00000101").unwrap(),
            now,
        )
        .unwrap();
        assert_eq!(bytes, b"#");
        (screen, open, now)
    }
    fn ready_information() -> (Screen, Open, Instant) {
        let (mut screen, mut open, now) = begin();
        screen.feed(&scene("aid_prompt"));
        assert_eq!(
            open.observe(&screen.frame(), now).unwrap(),
            Advance::Send(b"00000101\r".to_vec())
        );
        screen.feed(&scene("articles"));
        assert_eq!(
            open.observe(&screen.frame(), now).unwrap(),
            Advance::Send(b"Q".to_vec())
        );
        (screen, open, now)
    }
    #[test]
    fn a_search_selects_then_verifies_then_restores_and_only_then_opens() {
        let (mut screen, mut open, now) = ready_information();
        screen.feed(&scene("information"));
        assert_eq!(
            open.observe(&screen.frame(), now).unwrap(),
            Advance::Send(b" ".to_vec())
        );
        assert_eq!(open.observe(&screen.frame(), now).unwrap(), Advance::Wait);
        screen.feed(&scene("articles"));
        assert_eq!(
            open.observe(&screen.frame(), now).unwrap(),
            Advance::Send(b"\x1b[C".to_vec())
        );
        screen.feed(&scene("reading"));
        assert_eq!(
            open.observe(&screen.frame(), now).unwrap(),
            Advance::Reached(ArticleTarget::new("Stella", "00000101").unwrap())
        );
        assert!(!open.needs_frame(screen.revision() + 1));
        assert!(open.observe(&screen.frame(), now).is_err());
    }
    #[test]
    fn composing_with_board_switch_preserves_each_confirmed_step_and_the_total_deadline() {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&scene("main"));
        let now = Instant::now();
        let (mut open, bytes) = Open::begin(
            &screen.frame(),
            ArticleTarget::new("Stella", "00000101").unwrap(),
            now,
        )
        .unwrap();
        assert_eq!(bytes, b"\r");
        for (step, (name, bytes)) in [
            ("boards", b"s".as_slice()),
            ("board_prompt", b"Stella\r"),
            ("selected", b"\x1b[C"),
            ("articles", b"#"),
            ("aid_prompt", b"00000101\r"),
            ("articles", b"Q"),
            ("information", b" "),
        ]
        .into_iter()
        .enumerate()
        {
            screen.feed(&scene(name));
            assert_eq!(
                open.observe(
                    &screen.frame(),
                    now + Duration::from_secs((step as u64 + 1) * 4)
                )
                .unwrap(),
                Advance::Send(bytes.to_vec())
            );
        }
        // Last send was at 28s, so its 5s step allowance is still valid.
        // Only the 30s overall deadline may refuse this otherwise-correct row.
        screen.feed(&scene("articles"));
        assert!(matches!(
            open.observe(&screen.frame(), now + Duration::from_secs(31)),
            Err(Error::Timeout)
        ));
        let (mut screen, mut open, now) = begin();
        screen.feed(&scene("aid_prompt"));
        assert!(matches!(
            open.observe(&screen.frame(), now + board::STEP_LIMIT),
            Err(Error::Timeout)
        ));
    }
    #[test]
    fn prompt_requires_exact_empty_input_and_every_packet_boundary_converges() {
        let bytes = scene("aid_prompt");
        assert!(search_prompt(&frame("aid_prompt")));
        for edit in [
            b"x".as_slice(),
            b"\x1b[D",
            b"\x1b[23;1H",
            b"\x1b[",
            b"\x1b[?2026h",
        ] {
            let mut screen = Screen::new(80, 24).unwrap();
            screen.feed(&bytes);
            screen.feed(edit);
            assert!(!search_prompt(&screen.frame()), "{edit:?}");
        }
        for split in 0..=bytes.len() {
            let (mut screen, mut open, now) = begin();
            screen.feed(&bytes[..split]);
            screen.feed(&bytes[split..]);
            assert_eq!(
                open.observe(&screen.frame(), now).unwrap(),
                Advance::Send(b"00000101\r".to_vec()),
                "split {split}"
            );
            assert_eq!(open.observe(&screen.frame(), now).unwrap(), Advance::Wait);
        }
    }
    #[test]
    fn only_owned_complete_information_can_dismiss_a_pause_and_mismatches_are_terminal() {
        for (name, error) in [
            ("wrong_aid", Error::Identity),
            ("wrong_board", Error::Identity),
            ("other_articles", Error::Identity),
            ("entry", Error::Manual),
            ("help", Error::Manual),
            ("missing", Error::Missing),
            ("deleted", Error::Missing),
            ("permission", Error::Manual),
        ] {
            let (mut screen, mut open, now) = ready_information();
            screen.feed(&scene(name));
            assert_eq!(open.observe(&screen.frame(), now), Err(error), "{name}");
            screen.feed(&scene("information"));
            assert!(open.observe(&screen.frame(), now).is_err());
        }
        for edit in [
            "\x1b[7;1Hnot an info row\x1b[24;80H",
            "\x1b[10;1Hnot a box\x1b[24;80H",
            "\x1b[5;1Hwrong selected row\x1b[24;80H",
        ] {
            let (mut screen, mut open, now) = ready_information();
            screen.feed(&scene("information"));
            screen.feed(&encoding::encode(edit).unwrap());
            assert_eq!(
                open.observe(&screen.frame(), now),
                Err(Error::Manual),
                "{edit:?}"
            );
        }
    }
    #[test]
    fn digest_identity_uses_the_same_verified_workflow_on_a_recognized_list() {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&scene("articles"));
        let now = Instant::now();
        let target = ArticleTarget::new("Stella", "40000101").unwrap();
        let (mut flow, bytes) = Open::begin(&screen.frame(), target.clone(), now).unwrap();
        assert_eq!(bytes, b"#");
        screen.feed(&scene("aid_prompt"));
        assert_eq!(
            flow.observe(&screen.frame(), now).unwrap(),
            Advance::Send(b"40000101\r".to_vec())
        );
        screen.feed(&scene("articles"));
        assert_eq!(
            flow.observe(&screen.frame(), now).unwrap(),
            Advance::Send(b"Q".to_vec())
        );
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/article-open.json")).unwrap();
        let information = fixture["screens"]["information"]
            .as_str()
            .unwrap()
            .replace("#00000101", "#40000101")
            .replace("/M.1.A.001.html", "/G.1.A.001.html");
        screen.feed(&encoding::encode(&information).unwrap());
        assert_eq!(
            flow.observe(&screen.frame(), now).unwrap(),
            Advance::Send(b" ".to_vec())
        );
        screen.feed(&scene("articles"));
        assert_eq!(
            flow.observe(&screen.frame(), now).unwrap(),
            Advance::Send(b"\x1b[C".to_vec())
        );
        screen.feed(&scene("reading"));
        assert_eq!(
            flow.observe(&screen.frame(), now).unwrap(),
            Advance::Reached(target)
        );
    }
    #[test]
    fn synchronization_and_changed_selection_cannot_open_an_unverified_article() {
        let (mut screen, mut open, now) = ready_information();
        screen.feed(b"\x1b[?2026h");
        screen.feed(&scene("information"));
        assert_eq!(open.observe(&screen.frame(), now).unwrap(), Advance::Wait);
        screen.feed(b"\x1b[?2026l");
        assert_eq!(
            open.observe(&screen.frame(), now).unwrap(),
            Advance::Send(b" ".to_vec())
        );
        screen.feed(&scene("articles"));
        screen.feed(b"\x1b[5;1H \x1b[6;1H>\x1b[6;1H");
        assert_eq!(open.observe(&screen.frame(), now), Err(Error::Identity));
        let (mut screen, mut open, now) = ready_information();
        screen.feed(&scene("information"));
        open.observe(&screen.frame(), now).unwrap();
        screen.feed(&scene("articles"));
        open.observe(&screen.frame(), now).unwrap();
        screen.feed(&scene("wrong_reading"));
        assert!(open.observe(&screen.frame(), now).is_err());
    }
}
