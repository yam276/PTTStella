//! Explicit refresh of the current article, never an unchecked left/right macro.
use crate::{
    article_navigation::{self, Advance, Error, Open},
    article_target::ArticleTarget,
    board_navigation::{self, BoardName},
    navigation::{self, Context},
    screen::Frame,
};
use std::time::{Duration, Instant};
#[cfg(test)]
#[path = "article_refresh_tests.rs"]
mod tests;

enum Stage {
    Information,
    Return(ArticleTarget),
    Open(Open),
    Tail(ArticleTarget),
    Finished,
}
pub struct Refresh {
    board: Option<BoardName>,
    head: [String; 3],
    line: usize,
    revision: u64,
    deadline: Instant,
    step_deadline: Instant,
    stage: Stage,
}
impl Refresh {
    pub fn begin(
        frame: &Frame,
        board: Option<BoardName>,
        head: [String; 3],
        now: Instant,
    ) -> Result<(Self, Vec<u8>), Error> {
        let Context::ArticleReading {
            first_line,
            percent: 100,
            ..
        } = navigation::analyze(frame)
        else {
            return Err(Error::Manual);
        };
        Ok((
            Self {
                board,
                head,
                line: first_line,
                revision: frame.revision,
                deadline: now + Duration::from_secs(30),
                step_deadline: now + board_navigation::STEP_LIMIT,
                stage: Stage::Information,
            },
            b"Q".to_vec(),
        ))
    }
    pub fn needs_frame(&self, revision: u64) -> bool {
        !matches!(self.stage, Stage::Finished) && revision > self.revision
    }
    pub fn expired(&self, now: Instant) -> bool {
        now >= self.deadline || now >= self.step_deadline
    }
    pub fn observe(&mut self, frame: &Frame, now: Instant) -> Result<Advance, Error> {
        let result = self.advance(frame, now);
        if result.is_err() || matches!(result, Ok(Advance::Reached(_))) {
            self.stage = Stage::Finished;
        }
        result
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
        let next = match &mut self.stage {
            Stage::Information => {
                let Some(target) = article_navigation::reading_information(frame) else {
                    return Ok(Advance::Wait);
                };
                if self
                    .board
                    .as_ref()
                    .is_some_and(|board| !target.board().same(board))
                {
                    return Err(Error::Identity);
                }
                // Headerless articles obtain their board only from this owned
                // Q reply. The list and second Q must independently confirm it.
                self.board = Some(target.board().clone());
                self.stage = Stage::Return(target);
                Advance::Send(b" ".to_vec())
            }
            Stage::Return(target) => {
                if !matches!(navigation::analyze(frame), Context::ArticleList { .. }) {
                    return Ok(Advance::Wait);
                }
                if !board_navigation::current_board(frame)
                    .is_some_and(|board| board.same(target.board()))
                {
                    return Err(Error::Identity);
                }
                let (flow, bytes) =
                    Open::begin_reentry(frame, target.clone(), self.head.clone(), now)?;
                self.stage = Stage::Open(flow);
                Advance::Send(bytes)
            }
            Stage::Open(flow) => match flow.observe(frame, now)? {
                Advance::Reached(target) => {
                    if self.line == 1 {
                        return Ok(Advance::Reached(target));
                    }
                    self.stage = Stage::Tail(target);
                    Advance::Send(format!(":{}\r", self.line).into_bytes())
                }
                next => next,
            },
            Stage::Tail(target) => {
                if matches!(navigation::analyze(frame),Context::ArticleReading{first_line,last_line,..} if first_line<=self.line && last_line>=self.line)
                {
                    return Ok(Advance::Reached(target.clone()));
                }
                Advance::Wait
            }
            Stage::Finished => return Err(Error::Manual),
        };
        if matches!(next, Advance::Send(_)) {
            self.step_deadline = now + board_navigation::STEP_LIMIT;
        }
        Ok(next)
    }
}
