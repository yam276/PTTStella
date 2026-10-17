//! Local list state; Files owns every load/write and Connection owns navigation.
use super::{files::Files, files_recent, outbound::RecentBoardsPacket};
use pttstella::{
    connection::View,
    recent_boards::{RecentBoards, store::Store},
};
use std::path::Path;

pub(super) struct State {
    store: Result<Store, String>,
    history: RecentBoards,
    observed: Option<(u64, u64)>,
    writable: bool,
    revision: u64,
    notice: Option<String>,
}
impl State {
    // Called only from the initial blocking file task.
    pub(super) fn load(probe: Option<&Path>) -> Self {
        let store = probe.map_or_else(Store::system, |p| {
            Ok(Store::at(p.join("probe-recent-boards.json")))
        });
        let (history, writable, notice) =
            match store.as_ref().map_err(Clone::clone).and_then(Store::load) {
                Ok(history) => (history, true, None),
                Err(error) => (
                    RecentBoards::default(),
                    false,
                    Some(pttstella::msg!("recent-load-failed", error = error)),
                ),
            };
        Self {
            store,
            history,
            writable,
            notice,
            observed: None,
            revision: 0,
        }
    }
    pub(super) fn packet(&self) -> RecentBoardsPacket {
        RecentBoardsPacket {
            boards: self.history.boards().to_vec(),
            notice: self.notice.clone(),
        }
    }
    fn save(&mut self, files: &mut Files, clear: bool) {
        self.revision = self.revision.saturating_add(1);
        let result = self.store.clone().and_then(|store| {
            files.save_recent(files_recent::Write {
                store,
                revision: self.revision,
                history: self.history.clone(),
                clear,
            })
        });
        self.notice = Some(match result {
            Ok(()) => pttstella::msg!("recent-saving"),
            Err(error) => pttstella::msg!("recent-failed", error = error),
        });
    }
    pub(super) fn observe(&mut self, view: &View, files: &mut Files) -> bool {
        let Some(visit) = &view.board_visit else {
            return false;
        };
        let identity = (visit.generation, visit.serial);
        if self
            .observed
            .is_some_and(|seen| seen.0 > identity.0 || seen.0 == identity.0 && seen.1 >= identity.1)
        {
            return false;
        }
        self.observed = Some(identity);
        if !self.history.visit(visit.board.clone()) {
            return false;
        }
        if self.writable {
            self.save(files, false);
        }
        true
    }
    pub(super) fn clear(&mut self, files: &mut Files) {
        self.history = RecentBoards::default();
        self.writable = self.store.is_ok();
        self.save(files, true);
    }
    pub(super) fn complete(&mut self, done: files_recent::Completed) {
        if done.revision != self.revision {
            return;
        }
        self.notice = Some(match done.result {
            Ok(()) if done.clear && self.history.boards().is_empty() => {
                pttstella::msg!("recent-cleared")
            }
            Ok(()) => pttstella::msg!("recent-saved"),
            Err(error) if done.clear => pttstella::msg!("recent-clear-failed", error = error),
            Err(error) => pttstella::msg!("recent-failed", error = error),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::connection::Phase;
    fn view(scene: &str) -> View {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/board-switch.json")).unwrap();
        let mut screen = pttstella::screen::Screen::new(80, 24).unwrap();
        screen.feed(
            &pttstella::encoding::encode(fixture["screens"][scene].as_str().unwrap()).unwrap(),
        );
        View {
            mouse: pttstella::mouse::input::Snapshot::default(),
            frame: screen.frame(),
            output_pending: false,
            generation: 7,
            input_epoch: 0,
            input_pending: false,
            login_pending: false,
            login_guidance: pttstella::login::Guidance::Idle,
            destination: pttstella::navigation_job::Progress::Idle,
            board_visit: match scene {
                "articles" => Some(pttstella::recent_boards::BoardVisit {
                    generation: 7,
                    serial: 1,
                    board: pttstella::board_navigation::BoardName::parse("Stella").unwrap(),
                }),
                "other_articles" => Some(pttstella::recent_boards::BoardVisit {
                    generation: 7,
                    serial: 2,
                    board: pttstella::board_navigation::BoardName::parse("Other").unwrap(),
                }),
                _ => None,
            },
            phase: Phase::Connected,
            message: String::new(),
            received: 1,
            keep_alive: false,
            keep_alive_ready: false,
            keep_alive_sent: 0,
            keep_alive_failure: None,
        }
    }
    async fn drain(state: &mut State, files: &mut Files) {
        while files.busy() {
            match files.next().await {
                super::super::files::Completion::Recent(done) => state.complete(done),
                _ => panic!("unexpected work"),
            }
        }
    }
    #[tokio::test]
    async fn confirmed_facts_persist_and_clear_does_not_readd_an_unchanged_board() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::load(Some(dir.path()));
        let mut files = Files::default();
        for scene in ["main", "boards", "prompt", "selected", "entry"] {
            assert!(!state.observe(&view(scene), &mut files));
        }
        let mut held = view("entry");
        held.output_pending = true;
        held.input_pending = true;
        assert!(!state.observe(&held, &mut files));
        // A previously confirmed arrival survives later pending input/old pixels.
        held.board_visit = view("articles").board_visit;
        assert!(state.observe(&held, &mut files));
        assert!(!state.observe(&view("articles"), &mut files));
        assert!(state.observe(&view("other_articles"), &mut files));
        drain(&mut state, &mut files).await;
        let reloaded = State::load(Some(dir.path()));
        assert_eq!(
            reloaded
                .history
                .boards()
                .iter()
                .map(|b| b.as_str())
                .collect::<Vec<_>>(),
            ["Other", "Stella"]
        );
        assert!(!state.observe(&view("other_articles"), &mut files));
        state.clear(&mut files);
        assert!(!state.observe(&view("other_articles"), &mut files));
        drain(&mut state, &mut files).await;
        assert!(State::load(Some(dir.path())).history.boards().is_empty());
        assert_eq!(
            state.notice.as_deref(),
            Some(pttstella::msg!("recent-cleared").as_str())
        );
        files.finish().await.unwrap();
    }
    #[tokio::test]
    async fn corrupt_load_keeps_visits_in_memory_until_explicit_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("probe-recent-boards.json");
        std::fs::write(&path, b"broken").unwrap();
        let mut state = State::load(Some(dir.path()));
        let mut files = Files::default();
        assert!(state.notice.is_some());
        assert!(!state.writable);
        assert!(state.observe(&view("articles"), &mut files));
        assert!(!files.busy());
        assert_eq!(state.history.boards()[0].as_str(), "Stella");
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        state.clear(&mut files);
        drain(&mut state, &mut files).await;
        assert!(state.writable);
        assert!(!state.observe(&view("articles"), &mut files));
        assert!(state.observe(&view("other_articles"), &mut files));
        drain(&mut state, &mut files).await;
        assert_eq!(
            Store::at(path).load().unwrap().boards()[0].as_str(),
            "Other"
        );
        files.finish().await.unwrap();
    }
}
