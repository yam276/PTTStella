//! Local list state; Files owns every load/write and Connection owns navigation.
use crate::{
    connection::View,
    recent_boards::{
        RecentBoards,
        store::Store,
        writer::{Completed, Write},
    },
};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct Snapshot {
    pub boards: Vec<crate::board_navigation::BoardName>,
    pub notice: Option<String>,
}

pub struct State {
    store: Result<Store, String>,
    history: RecentBoards,
    observed: Option<(u64, u64)>,
    writable: bool,
    revision: u64,
    notice: Option<String>,
}
impl State {
    // Called only from the initial blocking file task.
    pub fn load(probe: Option<&Path>) -> Self {
        let store = probe.map_or_else(Store::system, |p| {
            Ok(Store::at(p.join("probe-recent-boards.json")))
        });
        Self::with_store(store)
    }
    /// Initial load belongs to a blocking file worker or pre-event-loop worker.
    pub fn with_store(store: Result<Store, String>) -> Self {
        let (history, writable, notice) =
            match store.as_ref().map_err(Clone::clone).and_then(Store::load) {
                Ok(history) => (history, true, None),
                Err(error) => (
                    RecentBoards::default(),
                    false,
                    Some(crate::msg!("recent-load-failed", error = error)),
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
    pub fn packet(&self) -> Snapshot {
        Snapshot {
            boards: self.history.boards().to_vec(),
            notice: self.notice.clone(),
        }
    }
    fn save(
        &mut self,
        enqueue: impl FnOnce(Write) -> Result<(), String>,
        clear: bool,
        visits: RecentBoards,
    ) {
        self.revision = self.revision.saturating_add(1);
        let result = self.store.clone().and_then(|store| {
            enqueue(Write {
                store,
                revision: self.revision,
                history: visits,
                clear,
            })
        });
        self.notice = Some(match result {
            Ok(()) => crate::msg!("recent-saving"),
            Err(error) => crate::msg!("recent-failed", error = error),
        });
    }
    pub fn observe(
        &mut self,
        view: &View,
        enqueue: impl FnOnce(Write) -> Result<(), String>,
    ) -> bool {
        self.observe_visit(view.board_visit.as_ref(), enqueue)
    }
    /// A native adapter can copy this small confirmed fact without cloning a
    /// terminal Frame or holding a watch borrow across UI callbacks.
    pub fn observe_visit(
        &mut self,
        visit: Option<&super::BoardVisit>,
        enqueue: impl FnOnce(Write) -> Result<(), String>,
    ) -> bool {
        let Some(visit) = visit else {
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
            let mut visits = RecentBoards::default();
            visits.visit(visit.board.clone());
            self.save(enqueue, false, visits);
        }
        true
    }
    pub fn clear(&mut self, enqueue: impl FnOnce(Write) -> Result<(), String>) {
        self.history = RecentBoards::default();
        self.writable = self.store.is_ok();
        self.save(enqueue, true, RecentBoards::default());
    }
    pub fn complete(&mut self, done: Completed) {
        if done.revision != self.revision {
            return;
        }
        self.notice = Some(match done.result {
            Ok(history) => {
                self.history = history;
                if done.clear && self.history.boards().is_empty() {
                    crate::msg!("recent-cleared")
                } else {
                    crate::msg!("recent-saved")
                }
            }
            Err(error) if done.clear => crate::msg!("recent-clear-failed", error = error),
            Err(error) => crate::msg!("recent-failed", error = error),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::Phase;
    fn view(scene: &str) -> View {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/board-switch.json")).unwrap();
        let mut screen = crate::screen::Screen::new(80, 24).unwrap();
        screen.feed(&crate::encoding::encode(fixture["screens"][scene].as_str().unwrap()).unwrap());
        View {
            site: crate::sites::Target::default(),
            mouse: crate::mouse::input::Snapshot::default(),
            frame: screen.frame(),
            output_pending: false,
            generation: 7,
            input_epoch: 0,
            input_pending: false,
            login_pending: false,
            login_guidance: crate::login::Guidance::Idle,
            account: None,
            draft_post_scope: None,
            draft_source: None,
            destination: crate::navigation_job::Progress::Idle,
            board_visit: match scene {
                "articles" => Some(crate::recent_boards::BoardVisit {
                    generation: 7,
                    serial: 1,
                    board: crate::board_navigation::BoardName::parse("Stella").unwrap(),
                }),
                "other_articles" => Some(crate::recent_boards::BoardVisit {
                    generation: 7,
                    serial: 2,
                    board: crate::board_navigation::BoardName::parse("Other").unwrap(),
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
    async fn drain(state: &mut State, files: &mut crate::recent_boards::writer::Writer) {
        while files.busy() {
            state.complete(files.next().await);
        }
    }
    #[tokio::test]
    async fn confirmed_facts_persist_and_clear_does_not_readd_an_unchanged_board() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::load(Some(dir.path()));
        let mut files = crate::recent_boards::writer::Writer::default();
        for scene in ["main", "boards", "prompt", "selected", "entry"] {
            assert!(!state.observe(&view(scene), |write| {
                files.enqueue(write);
                Ok(())
            }));
        }
        let mut held = view("entry");
        held.output_pending = true;
        held.input_pending = true;
        assert!(!state.observe(&held, |write| {
            files.enqueue(write);
            Ok(())
        }));
        // A previously confirmed arrival survives later pending input/old pixels.
        held.board_visit = view("articles").board_visit;
        assert!(state.observe(&held, |write| {
            files.enqueue(write);
            Ok(())
        }));
        assert!(!state.observe(&view("articles"), |write| {
            files.enqueue(write);
            Ok(())
        }));
        assert!(state.observe(&view("other_articles"), |write| {
            files.enqueue(write);
            Ok(())
        }));
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
        assert!(!state.observe(&view("other_articles"), |write| {
            files.enqueue(write);
            Ok(())
        }));
        state.clear(|write| {
            files.enqueue(write);
            Ok(())
        });
        assert!(!state.observe(&view("other_articles"), |write| {
            files.enqueue(write);
            Ok(())
        }));
        drain(&mut state, &mut files).await;
        assert!(State::load(Some(dir.path())).history.boards().is_empty());
        assert_eq!(
            state.notice.as_deref(),
            Some(crate::msg!("recent-cleared").as_str())
        );
    }
    #[tokio::test]
    async fn corrupt_load_keeps_visits_in_memory_until_explicit_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("probe-recent-boards.json");
        std::fs::write(&path, b"broken").unwrap();
        let mut state = State::load(Some(dir.path()));
        let mut files = crate::recent_boards::writer::Writer::default();
        assert!(state.notice.is_some());
        assert!(!state.writable);
        assert!(state.observe(&view("articles"), |write| {
            files.enqueue(write);
            Ok(())
        }));
        assert!(!files.busy());
        assert_eq!(state.history.boards()[0].as_str(), "Stella");
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        state.clear(|write| {
            files.enqueue(write);
            Ok(())
        });
        drain(&mut state, &mut files).await;
        assert!(state.writable);
        assert!(!state.observe(&view("articles"), |write| {
            files.enqueue(write);
            Ok(())
        }));
        assert!(state.observe(&view("other_articles"), |write| {
            files.enqueue(write);
            Ok(())
        }));
        drain(&mut state, &mut files).await;
        assert_eq!(
            Store::at(path).load().unwrap().boards()[0].as_str(),
            "Other"
        );
    }
}
