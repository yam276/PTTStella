//! One writing snapshot and one coalesced latest snapshot, both owned by Files.
use pttstella::recent_boards::{RecentBoards, store::Store};
use tokio::task::JoinHandle;

pub(super) struct Write {
    pub revision: u64,
    pub store: Store,
    pub history: RecentBoards,
    pub clear: bool,
}
pub(super) struct Completed {
    pub revision: u64,
    pub clear: bool,
    pub result: Result<(), String>,
}
struct Running {
    revision: u64,
    clear_revision: Option<u64>,
    task: JoinHandle<Result<(), String>>,
}
#[derive(Default)]
pub(super) struct Writer {
    running: Option<Running>,
    queued: Option<Write>,
    clear_revision: Option<u64>,
}
impl Writer {
    pub(super) fn busy(&self) -> bool {
        self.running.is_some()
    }
    pub(super) fn enqueue(&mut self, write: Write) {
        if write.clear {
            self.clear_revision = Some(write.revision);
        }
        self.queued = Some(write);
        self.start();
    }
    fn start(&mut self) {
        if self.running.is_some() {
            return;
        }
        let Some(write) = self.queued.take() else {
            return;
        };
        let clear_revision = self.clear_revision;
        self.running = Some(Running {
            revision: write.revision,
            clear_revision,
            task: tokio::task::spawn_blocking(move || {
                write.store.save(&write.history, clear_revision.is_some())
            }),
        });
    }
    pub(super) async fn next(&mut self) -> Completed {
        let Some(running) = self.running.as_mut() else {
            return std::future::pending().await;
        };
        let revision = running.revision;
        let clear_revision = running.clear_revision;
        let result = (&mut running.task)
            .await
            .map_err(|e| pttstella::msg!("file-worker-failed", e = e.to_string()))
            .and_then(|result| result);
        self.running = None;
        if result.is_ok() && self.clear_revision == clear_revision {
            self.clear_revision = None;
        }
        let complete = Completed {
            revision,
            clear: clear_revision.is_some(),
            result,
        };
        self.start();
        complete
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn write(store: &Store, revision: u64, name: &str, clear: bool) -> Write {
        let mut history = RecentBoards::default();
        history.visit(pttstella::board_navigation::BoardName::parse(name).unwrap());
        Write {
            store: store.clone(),
            revision,
            history,
            clear,
        }
    }
    #[tokio::test]
    async fn a_slow_write_coalesces_to_the_latest_without_losing_an_explicit_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recent.json");
        let store = Store::at(path.clone());
        store
            .save(&write(&store, 0, "Old", false).history, false)
            .unwrap();
        let (started, start) = tokio::sync::oneshot::channel();
        let (release, gate) = std::sync::mpsc::channel();
        let mut writer = Writer {
            running: Some(Running {
                revision: 1,
                clear_revision: None,
                task: tokio::task::spawn_blocking(move || {
                    started.send(()).unwrap();
                    gate.recv_timeout(Duration::from_secs(3)).unwrap();
                    Ok(())
                }),
            }),
            ..Writer::default()
        };
        start.await.unwrap();
        writer.enqueue(write(&store, 2, "Discarded", false));
        writer.enqueue(Write {
            history: RecentBoards::default(),
            ..write(&store, 3, "Unused", true)
        });
        writer.enqueue(write(&store, 4, "Latest", false));
        tokio::select! { _=writer.next()=>panic!("lost slow worker"), _=tokio::time::sleep(Duration::from_millis(5))=>{} }
        release.send(()).unwrap();
        assert_eq!(writer.next().await.revision, 1);
        assert!(writer.busy());
        let done = writer.next().await;
        assert_eq!(done.revision, 4);
        assert!(done.clear);
        done.result.unwrap();
        assert!(!writer.busy());
        assert!(writer.clear_revision.is_none());
        assert_eq!(store.load().unwrap().boards()[0].as_str(), "Latest");
        assert!(
            Store::at(path.with_extension("json.bak"))
                .load()
                .unwrap()
                .boards()
                .is_empty()
        );
    }
    #[tokio::test]
    async fn failed_clear_is_retained_for_the_next_snapshot_without_automatic_retry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recent.json");
        let store = Store::at(path.clone());
        std::fs::create_dir(path.with_extension("json.bak")).unwrap();
        let mut writer = Writer::default();
        writer.enqueue(write(&store, 1, "First", true));
        assert!(writer.next().await.result.is_err());
        assert!(!writer.busy());
        std::fs::remove_dir(path.with_extension("json.bak")).unwrap();
        writer.enqueue(write(&store, 2, "Next", false));
        let done = writer.next().await;
        assert!(done.clear);
        done.result.unwrap();
        assert!(
            Store::at(path.with_extension("json.bak"))
                .load()
                .unwrap()
                .boards()
                .is_empty()
        );
        assert_eq!(store.load().unwrap().boards()[0].as_str(), "Next");
    }
}
