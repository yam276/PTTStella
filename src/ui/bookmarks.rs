//! Local bookmark state commits only completed Files results, never screen guesses.
use super::outbound::{Feedback, SaveStatus};
use pttstella::{
    article_target::ArticleTarget,
    bookmarks::{Bookmarks, Entry, store::Store},
};
use serde::Serialize;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize)]
pub(super) struct Snapshot {
    pub entries: Vec<Entry>,
    pub writable: bool,
    pub warning: Option<String>,
}
pub(super) struct Write {
    pub store: Store,
    pub draft: Bookmarks,
}
pub(super) struct State {
    store: Result<Store, String>,
    bookmarks: Bookmarks,
    warning: Option<String>,
}
impl State {
    /// Called by the startup blocking worker, never the UI event loop.
    pub(super) fn load(probe: Option<&Path>) -> Self {
        let store = probe.map_or_else(Store::system, |dir| {
            Ok(Store::at(dir.join("probe-bookmarks.json")))
        });
        let (bookmarks, warning) = match store.as_ref().map_err(Clone::clone).and_then(Store::load)
        {
            Ok(bookmarks) => (bookmarks, None),
            Err(error) => (Bookmarks::default(), Some(error)),
        };
        Self {
            store,
            bookmarks,
            warning,
        }
    }
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            entries: self.bookmarks.entries().to_vec(),
            writable: self.warning.is_none(),
            warning: self.warning.clone(),
        }
    }
    pub(super) fn edit(&self, value: &str, label: &str, remove: bool) -> Result<Write, String> {
        if let Some(warning) = &self.warning {
            return Err(warning.clone());
        }
        let target = ArticleTarget::parse(value).map_err(|e| e.to_string())?;
        let draft = if remove {
            self.bookmarks.remove(&target)?
        } else {
            let created_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| pttstella::msg!("bookmarks-time-invalid"))?
                .as_secs();
            self.bookmarks.upsert(target, label, created_at)?
        };
        Ok(Write {
            store: self.store.clone()?,
            draft,
        })
    }
    pub(super) fn complete(&mut self, result: Result<Bookmarks, String>) -> Feedback<SaveStatus> {
        match result {
            Ok(saved) => {
                self.bookmarks = saved;
                Feedback::new(SaveStatus::Saved, pttstella::msg!("bookmarks-saved"))
            }
            Err(error) => Feedback::new(SaveStatus::Rejected, error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_are_explicit_and_do_not_commit_until_the_file_result_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::load(Some(dir.path()));
        assert!(state.bookmarks.entries().is_empty());
        let write = state.edit("#00000101@Stella", "自製", false).unwrap();
        assert!(state.bookmarks.entries().is_empty());
        assert!(!dir.path().join("probe-bookmarks.json").exists());
        let saved = write.store.save(write.draft).unwrap();
        state.complete(Ok(saved));
        let original = state.bookmarks.clone();
        let write = state.edit("#00000101@stella", "changed", false).unwrap();
        std::fs::create_dir(dir.path().join("probe-bookmarks.json.bak")).unwrap();
        state.complete(write.store.save(write.draft));
        assert_eq!(state.bookmarks, original);
        assert_eq!(State::load(Some(dir.path())).bookmarks, original);
        assert!(state.edit("Stella", "", false).is_err());
        assert!(state.edit("#00000102@Stella", "", true).is_err());
        let deletion = state.edit("#00000101@STELLA", "", true).unwrap();
        assert!(deletion.draft.entries().is_empty());
        assert_eq!(state.bookmarks, original);
    }
    #[test]
    fn a_corrupt_startup_snapshot_is_read_only_and_does_not_overwrite_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("probe-bookmarks.json");
        std::fs::write(&path, b"broken").unwrap();
        let state = State::load(Some(dir.path()));
        let packet = serde_json::to_value(state.snapshot()).unwrap();
        assert_eq!(packet["entries"], serde_json::json!([]));
        assert_eq!(packet["writable"], false);
        assert!(packet["warning"].is_string());
        assert!(state.edit("#00000101@Stella", "", false).is_err());
        assert!(state.edit("#00000101@Stella", "", true).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"broken");
    }
}
