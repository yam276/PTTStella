//! Confirmed board visits only; no account, article text or session recording.
use crate::{board_navigation::BoardName, navigation, screen::Frame};
use serde::{Deserialize, Serialize};

#[cfg(feature = "desktop")]
pub mod store;
pub const LIMIT: usize = 20;
pub const MAX_BYTES: usize = 4096;

/// Persistent confirmation from the Session, consumed once despite watch coalescing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardVisit {
    pub generation: u64,
    pub serial: u64,
    pub board: BoardName,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RecentBoards {
    boards: Vec<BoardName>,
}
impl RecentBoards {
    pub fn boards(&self) -> &[BoardName] {
        &self.boards
    }
    pub fn visit(&mut self, board: BoardName) -> bool {
        if self.boards.first() == Some(&board) {
            return false;
        }
        self.boards.retain(|old| !old.same(&board));
        self.boards.insert(0, board);
        self.boards.truncate(LIMIT);
        true
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec_pretty(&Document {
            version: 1,
            boards: self.boards.iter().map(|b| b.as_str().to_owned()).collect(),
        })
        .map_err(|e| e.to_string())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err(crate::msg!("recent-invalid"));
        }
        let document: Document =
            serde_json::from_slice(bytes).map_err(|_| crate::msg!("recent-invalid"))?;
        if document.version != 1 || document.boards.len() > LIMIT {
            return Err(crate::msg!("recent-invalid"));
        }
        let mut boards = Vec::with_capacity(document.boards.len());
        for name in document.boards {
            let board = BoardName::parse(&name).map_err(|_| crate::msg!("recent-invalid"))?;
            if board.as_str() != name || boards.iter().any(|old: &BoardName| old.same(&board)) {
                return Err(crate::msg!("recent-invalid"));
            }
            boards.push(board);
        }
        Ok(Self { boards })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    boards: Vec<String>,
}

/// Retains the visible board across reading/partial output. Clearing history
/// does not reset this observer and therefore does not immediately re-add it.
#[derive(Default)]
pub struct Visits {
    generation: u64,
    last: Option<BoardName>,
}
impl Visits {
    pub fn observe(&mut self, generation: u64, frame: &Frame) -> Option<BoardName> {
        if self.generation != generation {
            self.generation = generation;
            self.last = None;
        }
        if let Some(board) = crate::board_navigation::current_board(frame) {
            if self.last.as_ref() == Some(&board) {
                return None;
            }
            self.last = Some(board.clone());
            return Some(board);
        }
        if matches!(
            navigation::analyze(frame),
            navigation::Context::MainMenu { .. }
        ) || crate::board_navigation::browser(frame)
        {
            self.last = None;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmed_names_are_recent_first_case_insensitive_and_bounded() {
        let mut recent = RecentBoards::default();
        for n in 0..21 {
            assert!(recent.visit(BoardName::parse(&format!("Board{n}")).unwrap()));
        }
        assert_eq!(recent.boards().len(), 20);
        assert_eq!(recent.boards()[0].as_str(), "Board20");
        assert_eq!(recent.boards()[19].as_str(), "Board1");
        assert!(recent.visit(BoardName::parse("board10").unwrap()));
        assert_eq!(recent.boards()[0].as_str(), "board10");
        assert_eq!(recent.boards().len(), 20);
        assert!(!recent.visit(BoardName::parse("board10").unwrap()));
        assert_eq!(
            RecentBoards::decode(&recent.encode().unwrap()).unwrap(),
            recent
        );
    }
    #[test]
    fn malformed_versions_names_duplicates_and_capacity_are_rejected() {
        let valid = br#"{"version":1,"boards":["Stella","C_Chat"]}"#;
        let recent = RecentBoards::decode(valid).unwrap();
        assert_eq!(recent.boards()[1].as_str(), "C_Chat");
        for invalid in [
            br#"{"version":2,"boards":[]}"#.as_slice(),
            br#"{"version":1,"boards":["Stella","stella"]}"#,
            br#"{"version":1,"boards":[" Stella "]}"#,
            br#"{"version":1,"boards":["bad\rname"]}"#,
            br#"{"version":1,"boards":[],"account":"unused"}"#,
            b"broken",
        ] {
            assert!(RecentBoards::decode(invalid).is_err());
        }
        assert!(RecentBoards::decode(&vec![b' '; MAX_BYTES + 1]).is_err());
        let names: Vec<_> = (0..21).map(|n| format!("Board{n}")).collect();
        assert!(
            RecentBoards::decode(
                &serde_json::to_vec(&serde_json::json!({"version":1,"boards":names})).unwrap()
            )
            .is_err()
        );
    }
    #[test]
    fn only_known_arrivals_count_and_clear_does_not_readd_the_same_screen() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/board-switch.json")).unwrap();
        let frame = |name: &str| {
            let bytes = fixture["screens"][name].as_str().unwrap();
            let mut screen = crate::screen::Screen::new(80, 24).unwrap();
            screen.feed(&crate::encoding::encode(bytes).unwrap());
            screen.frame()
        };
        let mut visits = Visits::default();
        for scene in ["main", "boards", "prompt", "selected", "entry", "help"] {
            assert!(visits.observe(7, &frame(scene)).is_none(), "{scene}");
        }
        assert_eq!(
            visits.observe(7, &frame("articles")).unwrap().as_str(),
            "Stella"
        );
        assert!(visits.observe(7, &frame("reading")).is_none());
        assert!(visits.observe(7, &frame("articles")).is_none());
        // History can be cleared without changing the observation identity.
        let mut recent = RecentBoards::default();
        assert!(visits.observe(7, &frame("articles")).is_none());
        assert!(recent.boards().is_empty());
        assert!(visits.observe(7, &frame("boards")).is_none());
        recent.visit(visits.observe(7, &frame("empty_articles")).unwrap());
        assert_eq!(recent.boards()[0].as_str(), "Stella");
        assert!(visits.observe(8, &frame("articles")).is_some());
    }
}
