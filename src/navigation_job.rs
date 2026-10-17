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
    Board { board: BoardName },
    Article(ArticleTarget),
}
impl Destination {
    pub fn board(&self) -> &BoardName {
        match self {
            Self::Board { board } => board,
            Self::Article(target) => target.board(),
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
        }
    }
    pub fn needs_frame(&self, revision: u64) -> bool {
        match self {
            Self::Board(flow) => flow.needs_frame(revision),
            Self::Article(flow) => flow.needs_frame(revision),
        }
    }
    pub fn expired(&self, now: Instant) -> bool {
        match self {
            Self::Board(flow) => flow.expired(now),
            Self::Article(flow) => flow.expired(now),
        }
    }
    pub fn timeout_message(&self) -> String {
        match self {
            Self::Board(_) => board_navigation::Error::Timeout.to_string(),
            Self::Article(_) => article_navigation::Error::Timeout.to_string(),
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
