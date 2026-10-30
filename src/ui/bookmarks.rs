//! Local bookmark state commits only completed Files results, never screen guesses.
use super::outbound::{Feedback, SaveStatus};
use pttstella::{
    article_target::ArticleTarget,
    bookmarks::{Bookmarks, archive::Archive, store::Store},
};
use serde::Serialize;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize)]
pub(super) struct Snapshot {
    pub entries: Vec<Summary>,
    pub writable: bool,
    pub warning: Option<String>,
}
#[derive(Serialize)]
pub(super) struct Summary {
    #[serde(flatten)]
    target: ArticleTarget,
    label: Option<String>,
    created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    cached_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<usize>,
}
impl Summary {
    pub fn target(&self) -> &ArticleTarget {
        &self.target
    }
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
}
#[derive(Serialize)]
pub(super) struct LocalArticle {
    pub target: ArticleTarget,
    pub saved_at: u64,
    #[serde(serialize_with = "pttstella::render_wire::serialize_frame")]
    pub frame: pttstella::screen::Frame,
    pub settings: pttstella::settings::DisplaySettings,
}
pub(super) struct Read {
    target: ArticleTarget,
    archive: std::sync::Arc<Archive>,
    rules: pttstella::filters::Rules,
    settings: pttstella::settings::DisplaySettings,
}
impl Read {
    pub fn run(self) -> LocalArticle {
        LocalArticle {
            target: self.target,
            saved_at: self.archive.saved_at(),
            frame: self.archive.filtered(&self.rules),
            settings: self.settings,
        }
    }
}
pub(super) fn now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| pttstella::msg!("bookmarks-time-invalid"))
}
pub(super) struct Acquisition {
    pub request: u64,
    stamp: pttstella::navigation::Stamp,
    frame: super::article_cache::ArchiveSnapshot,
}
impl Acquisition {
    pub fn new(
        request: u64,
        stamp: pttstella::navigation::Stamp,
        frame: super::article_cache::ArchiveSnapshot,
    ) -> Self {
        Self {
            request,
            stamp,
            frame,
        }
    }
    pub fn resolve(
        &self,
        view: &pttstella::connection::View,
        failed: bool,
    ) -> Option<Result<ArticleTarget, String>> {
        use pttstella::navigation_job::{Destination, Progress};
        if view.generation != self.stamp.generation
            || view.phase != pttstella::connection::Phase::Connected
            || failed
        {
            return Some(Err(pttstella::msg!("bookmarks-capture-failed")));
        }
        match &view.destination {
            Progress::Reached {
                operation,
                target: Destination::Article(target),
                ..
            } if *operation == self.stamp => Some(Ok(target.clone())),
            Progress::Active { operation, .. } if *operation != self.stamp => {
                Some(Err(pttstella::msg!("bookmarks-capture-failed")))
            }
            _ => None,
        }
    }
    pub fn into_write(self, state: &State, target: ArticleTarget) -> Result<ArchiveWrite, String> {
        if let Some(warning) = &state.warning {
            return Err(warning.clone());
        }
        Ok(ArchiveWrite {
            base: state.bookmarks.clone(),
            store: state.store.clone()?,
            target,
            label: self.frame.title(),
            snapshot: self.frame,
        })
    }
}
pub(super) struct ArchiveWrite {
    base: Bookmarks,
    store: Store,
    target: ArticleTarget,
    label: String,
    snapshot: super::article_cache::ArchiveSnapshot,
}
impl ArchiveWrite {
    pub fn run(self) -> Result<Bookmarks, String> {
        let time = now()?;
        let archive = Archive::new(&self.snapshot.frame(), time)?;
        let draft = self.base.archive(self.target, &self.label, time, archive)?;
        self.store.save_from(&self.base, draft)
    }
}

pub(super) struct Write {
    pub base: Bookmarks,
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
    pub(super) fn check_write(&self) -> Result<(), String> {
        if let Some(error) = &self.warning {
            return Err(error.clone());
        }
        self.store.as_ref().map(|_| ()).map_err(Clone::clone)
    }
    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot {
            entries: self
                .bookmarks
                .entries()
                .iter()
                .map(|e| Summary {
                    target: e.target().clone(),
                    label: e.label().map(str::to_owned),
                    created_at: e.created_at(),
                    cached_at: e.archive().map(Archive::saved_at),
                    lines: e.archive().map(Archive::row_count),
                })
                .collect(),
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
            base: self.bookmarks.clone(),
            store: self.store.clone()?,
            draft,
        })
    }
    pub(super) fn read(
        &self,
        value: &str,
        rules: pttstella::filters::Rules,
        settings: pttstella::settings::DisplaySettings,
    ) -> Result<Read, String> {
        let target = ArticleTarget::parse(value).map_err(|e| e.to_string())?;
        let entry = self
            .bookmarks
            .entries()
            .iter()
            .find(|e| e.target().board().same(target.board()) && e.target().aid() == target.aid())
            .ok_or(pttstella::msg!("bookmarks-not-found"))?;
        let archive = entry
            .archive_handle()
            .ok_or(pttstella::msg!("bookmarks-no-archive"))?;
        Ok(Read {
            target: entry.target().clone(),
            archive,
            rules,
            settings,
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
        let saved = write.store.save_from(&write.base, write.draft).unwrap();
        state.complete(Ok(saved));
        let original = state.bookmarks.clone();
        let write = state.edit("#00000101@stella", "changed", false).unwrap();
        std::fs::create_dir(dir.path().join("probe-bookmarks.json.bak")).unwrap();
        state.complete(write.store.save_from(&write.base, write.draft));
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
