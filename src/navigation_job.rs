//! Closed destination set sharing one Session-owned job, writer and cancellation.
use crate::{
    article_navigation,
    article_target::ArticleTarget,
    board_navigation::{self, BoardName},
    navigation::Stamp,
    screen::Frame,
};
use serde::Serialize;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Destination {
    Board {
        board: BoardName,
    },
    Article(ArticleTarget),
    Refresh {
        board: BoardName,
        #[serde(skip)]
        head: [String; 3],
    },
}
impl Destination {
    /// Local draft grammar; length is checked before trimming so a truncated
    /// over-limit paste cannot become a different valid destination.
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.len() > crate::article_target::MAX_INPUT || value.chars().any(char::is_control) {
            return Err(crate::msg!("destination-input-invalid"));
        }
        let value = value.trim();
        if value.starts_with('#') || value.contains('@') || value.contains("://") {
            ArticleTarget::parse(value)
                .map(Self::Article)
                .map_err(|error| error.to_string())
        } else {
            BoardName::parse(value)
                .map(|board| Self::Board { board })
                .map_err(|error| error.to_string())
        }
    }
    pub fn board(&self) -> &BoardName {
        match self {
            Self::Board { board } => board,
            Self::Article(target) => target.board(),
            Self::Refresh { board, .. } => board,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Progress {
    #[default]
    Idle,
    Active {
        serial: u64,
        target: Destination,
        operation: Stamp,
    },
    Reached {
        serial: u64,
        target: Destination,
    },
}
pub enum Flow {
    Board(board_navigation::Switch),
    Article(article_navigation::Open),
    Refresh(Box<crate::article_refresh::Refresh>),
}
pub enum Start {
    Reached(Destination),
    Run { flow: Flow, bytes: Vec<u8> },
}
pub enum Advance {
    Wait,
    Send(Vec<u8>),
    Reached(Destination),
}
impl Flow {
    pub fn begin(frame: &Frame, target: Destination, now: Instant) -> Result<Start, String> {
        match target {
            Destination::Board { board } => {
                match board_navigation::Switch::begin(frame, board, now)
                    .map_err(|e| e.to_string())?
                {
                    board_navigation::Start::Reached(board) => {
                        Ok(Start::Reached(Destination::Board { board }))
                    }
                    board_navigation::Start::Run { switch, bytes } => Ok(Start::Run {
                        flow: Self::Board(switch),
                        bytes,
                    }),
                }
            }
            Destination::Article(target) => {
                let (flow, bytes) = article_navigation::Open::begin(frame, target, now)
                    .map_err(|e| e.to_string())?;
                Ok(Start::Run {
                    flow: Self::Article(flow),
                    bytes,
                })
            }
            Destination::Refresh { board, head } => {
                let (flow, bytes) = crate::article_refresh::Refresh::begin(frame, board, head, now)
                    .map_err(|e| e.to_string())?;
                Ok(Start::Run {
                    flow: Self::Refresh(Box::new(flow)),
                    bytes,
                })
            }
        }
    }
    pub fn needs_frame(&self, revision: u64) -> bool {
        match self {
            Self::Board(flow) => flow.needs_frame(revision),
            Self::Article(flow) => flow.needs_frame(revision),
            Self::Refresh(flow) => flow.needs_frame(revision),
        }
    }
    pub fn expired(&self, now: Instant) -> bool {
        match self {
            Self::Board(flow) => flow.expired(now),
            Self::Article(flow) => flow.expired(now),
            Self::Refresh(flow) => flow.expired(now),
        }
    }
    pub fn timeout_message(&self) -> String {
        match self {
            Self::Board(_) => board_navigation::Error::Timeout.to_string(),
            Self::Article(_) => article_navigation::Error::Timeout.to_string(),
            Self::Refresh(_) => article_navigation::Error::Timeout.to_string(),
        }
    }
    pub fn observe(&mut self, frame: &Frame, now: Instant) -> Result<Advance, String> {
        match self {
            Self::Board(flow) => Ok(match flow.observe(frame, now).map_err(|e| e.to_string())? {
                board_navigation::Advance::Wait => Advance::Wait,
                board_navigation::Advance::Send(bytes) => Advance::Send(bytes),
                board_navigation::Advance::Reached(board) => {
                    Advance::Reached(Destination::Board { board })
                }
            }),
            Self::Article(flow) => {
                Ok(match flow.observe(frame, now).map_err(|e| e.to_string())? {
                    article_navigation::Advance::Wait => Advance::Wait,
                    article_navigation::Advance::Send(bytes) => Advance::Send(bytes),
                    article_navigation::Advance::Reached(target) => {
                        Advance::Reached(Destination::Article(target))
                    }
                })
            }
            Self::Refresh(flow) => {
                Ok(match flow.observe(frame, now).map_err(|e| e.to_string())? {
                    article_navigation::Advance::Wait => Advance::Wait,
                    article_navigation::Advance::Send(bytes) => Advance::Send(bytes),
                    article_navigation::Advance::Reached(target) => {
                        Advance::Reached(Destination::Article(target))
                    }
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_drafts_distinguish_boards_and_articles_without_losing_invalid_tails() {
        assert_eq!(
            Destination::parse(" Stella ").unwrap(),
            Destination::Board {
                board: BoardName::parse("Stella").unwrap()
            }
        );
        for draft in [
            "#00000101@Stella",
            "https://www.ptt.cc/bbs/Stella/M.1.A.001.html",
        ] {
            assert_eq!(
                Destination::parse(draft).unwrap(),
                Destination::Article(ArticleTarget::new("Stella", "00000101").unwrap())
            );
        }
        for draft in [
            "Stella_Test04",
            "#00000101",
            "Stella\r",
            "#00000101@Stella\n",
            "https://www.ptt.cc.evil.example/bbs/Stella/M.1.A.001.html",
        ] {
            assert!(Destination::parse(draft).is_err(), "{draft:?}");
        }
        let padded = format!("Stella{}x", " ".repeat(crate::article_target::MAX_INPUT));
        assert!(Destination::parse(&padded).is_err());
        // HTML caps the draft at 257 UTF-16 units, strictly above Rust's byte
        // limit. Even a cap before the invalid suffix cannot admit the prefix.
        assert!(Destination::parse(&padded[..257]).is_err());
        let maximum = format!("Stella{}", " ".repeat(crate::article_target::MAX_INPUT - 6));
        assert!(Destination::parse(&maximum).is_ok());
        assert!(Destination::parse(&(maximum + " ")).is_err());
    }
    #[test]
    fn typed_destination_progress_matches_independent_wire_examples() {
        let operation = Stamp {
            generation: 7,
            revision: 3,
            input_epoch: 2,
        };
        for (target, expected) in [
            (
                Destination::Board {
                    board: BoardName::parse("Stella").unwrap(),
                },
                serde_json::json!({"kind":"board","board":"Stella"}),
            ),
            (
                Destination::Article(ArticleTarget::new("Stella", "00000101").unwrap()),
                serde_json::json!({"kind":"article","board":"Stella","aid":"00000101"}),
            ),
        ] {
            assert_eq!(
                serde_json::to_value(Progress::Active {
                    serial: 4,
                    target: target.clone(),
                    operation
                })
                .unwrap(),
                serde_json::json!({"state":"active","serial":4,"target":expected,"operation":{"generation":7,"revision":3,"input_epoch":2}})
            );
            assert_eq!(
                serde_json::to_value(Progress::Reached { serial: 4, target }).unwrap(),
                serde_json::json!({"state":"reached","serial":4,"target":expected})
            );
        }
    }
}
