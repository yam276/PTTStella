//! Local bookmark state commits only completed Files results, never screen guesses.
use crate::{
    article_target::ArticleTarget,
    bookmarks::{Bookmarks, archive::Archive, store::Store},
};
use serde::Serialize;
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Serialize)]
pub struct Snapshot {
    pub entries: Vec<Summary>,
    pub writable: bool,
    pub warning: Option<String>,
}
#[derive(Serialize)]
pub struct Summary {
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
pub struct LocalArticle {
    pub target: ArticleTarget,
    pub saved_at: u64,
    #[serde(serialize_with = "crate::render_wire::serialize_frame")]
    pub frame: crate::screen::Frame,
    pub settings: crate::settings::DisplaySettings,
}
pub struct Read {
    target: ArticleTarget,
    archive: std::sync::Arc<Archive>,
    rules: crate::filters::Rules,
    settings: crate::settings::DisplaySettings,
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
pub fn now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|_| crate::msg!("bookmarks-time-invalid"))
}
pub struct Acquisition {
    pub request: u64,
    stamp: crate::navigation::Stamp,
    frame: crate::article_cache::ArchiveSnapshot,
    expected: Option<ArticleTarget>,
}
impl Acquisition {
    pub fn new(
        request: u64,
        stamp: crate::navigation::Stamp,
        frame: crate::article_cache::ArchiveSnapshot,
    ) -> Self {
        Self {
            request,
            stamp,
            frame,
            expected: None,
        }
    }
    pub fn expect(mut self, target: ArticleTarget) -> Self {
        self.expected = Some(target);
        self
    }
    pub fn resolve(
        &self,
        view: &crate::connection::View,
        failed: bool,
    ) -> Option<Result<ArticleTarget, String>> {
        use crate::navigation_job::{Destination, Progress};
        if view.generation != self.stamp.generation
            || view.phase != crate::connection::Phase::Connected
            || failed
        {
            return Some(Err(crate::msg!("bookmarks-capture-failed")));
        }
        match &view.destination {
            Progress::Reached {
                operation,
                target: Destination::Article(target),
                ..
            } if *operation == self.stamp => Some(
                if self.expected.as_ref().is_none_or(|expected| {
                    expected.board().same(target.board()) && expected.aid() == target.aid()
                }) {
                    Ok(target.clone())
                } else {
                    Err(crate::msg!("bookmarks-capture-failed"))
                },
            ),
            Progress::Active { operation, .. } if *operation != self.stamp => {
                Some(Err(crate::msg!("bookmarks-capture-failed")))
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

/// An explicitly selected list row opens through the Session, then the normal
/// full-load owner gathers it. No background reader, second writer or queue.
pub struct FromList {
    pub request: u64,
    pub stamp: crate::navigation::Stamp,
    target: Option<ArticleTarget>,
    scope: Option<(u64, u64, u64)>,
    gate_seen: bool,
    failed: bool,
    active_seen: bool,
    deadline: tokio::time::Instant,
}
impl FromList {
    pub fn new(request: u64, stamp: crate::navigation::Stamp) -> Self {
        Self {
            request,
            stamp,
            target: None,
            scope: None,
            gate_seen: false,
            failed: false,
            active_seen: false,
            deadline: tokio::time::Instant::now() + tokio::time::Duration::from_secs(35),
        }
    }
    pub fn deadline(&self) -> Option<tokio::time::Instant> {
        self.target.is_none().then_some(self.deadline)
    }
    pub fn fail(&mut self) {
        self.failed = true;
    }
    pub fn observe(
        &mut self,
        view: &crate::connection::View,
        scope: Option<(u64, u64, u64)>,
        progress: Option<(u64, usize, usize, Option<crate::article_cache::LoadState>)>,
        gate: Option<(u64, u64, u64)>,
    ) -> Option<Result<(u64, ArticleTarget), String>> {
        use crate::article_cache::LoadState;
        use crate::navigation_job::{Destination, Progress};
        let bad = || Some(Err(crate::msg!("bookmarks-capture-failed")));
        if self.target.is_none() && tokio::time::Instant::now() >= self.deadline {
            self.failed = true;
        }
        if matches!(&view.destination,Progress::Active{operation,..} if *operation==self.stamp) {
            self.active_seen = true;
        }
        if self.active_seen && matches!(view.destination, Progress::Idle) {
            self.failed = true;
        }
        if self.failed
            || view.generation != self.stamp.generation
            || view.phase != crate::connection::Phase::Connected
        {
            return bad();
        }
        if self.target.is_some()
            && !matches!(&view.destination,
            Progress::Reached{operation,target:Destination::Article(target),..}
                if *operation==self.stamp&&self.target.as_ref().is_some_and(|expected|expected.board().same(target.board())&&expected.aid()==target.aid()))
        {
            return bad();
        }
        if self.target.is_none() {
            match &view.destination {
                Progress::Reached {
                    operation,
                    target: Destination::Article(target),
                    ..
                } if *operation == self.stamp => self.target = Some(target.clone()),
                Progress::Active { operation, .. } if *operation != self.stamp => return bad(),
                _ => return None,
            }
        }
        let Some(scope) = scope else {
            return if self.scope.is_some() { bad() } else { None };
        };
        if let Some(previous) = self.scope {
            if previous != scope {
                return bad();
            }
        } else {
            self.scope = Some(scope);
        }
        if gate == Some(scope) {
            self.gate_seen = true;
        } else if self.gate_seen {
            return bad();
        }
        let (id, first, _, load) = progress?;
        if id != scope.1 {
            return bad();
        }
        if matches!(
            load,
            Some(LoadState::Cancelled | LoadState::Stopped | LoadState::Changed | LoadState::Limit)
        ) {
            return bad();
        }
        if first == 1 && load == Some(LoadState::Complete) && gate == Some(scope) {
            return Some(Ok((scope.1, self.target.clone()?)));
        }
        None
    }
}
pub struct ArchiveWrite {
    base: Bookmarks,
    store: Store,
    target: ArticleTarget,
    label: String,
    snapshot: crate::article_cache::ArchiveSnapshot,
}
impl ArchiveWrite {
    pub fn run(self) -> Result<Bookmarks, String> {
        let time = now()?;
        let archive = Archive::new(&self.snapshot.frame(), time)?;
        let draft = self.base.archive(self.target, &self.label, time, archive)?;
        self.store.save_from(&self.base, draft)
    }
}

pub struct Write {
    pub base: Bookmarks,
    pub store: Store,
    pub draft: Bookmarks,
}
pub struct State {
    store: Result<Store, String>,
    bookmarks: Bookmarks,
    warning: Option<String>,
}
impl State {
    /// Called by the startup blocking worker, never the UI event loop.
    pub fn load(probe: Option<&Path>) -> Self {
        Self::with_store(probe.map_or_else(Store::system, |dir| {
            Ok(Store::at(dir.join("probe-bookmarks.json")))
        }))
    }
    /// The caller chooses the product or isolated candidate namespace.
    pub fn with_store(store: Result<Store, String>) -> Self {
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
    pub fn check_write(&self) -> Result<(), String> {
        if let Some(error) = &self.warning {
            return Err(error.clone());
        }
        self.store.as_ref().map(|_| ()).map_err(Clone::clone)
    }
    pub fn snapshot(&self) -> Snapshot {
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
    pub fn edit(&self, value: &str, label: &str, remove: bool) -> Result<Write, String> {
        if let Some(warning) = &self.warning {
            return Err(warning.clone());
        }
        let target = ArticleTarget::parse(value).map_err(|e| e.to_string())?;
        let draft = if remove {
            self.bookmarks.remove(&target)?
        } else {
            let created_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| crate::msg!("bookmarks-time-invalid"))?
                .as_secs();
            self.bookmarks.upsert(target, label, created_at)?
        };
        Ok(Write {
            base: self.bookmarks.clone(),
            store: self.store.clone()?,
            draft,
        })
    }
    pub fn read(
        &self,
        value: &str,
        rules: crate::filters::Rules,
        settings: crate::settings::DisplaySettings,
    ) -> Result<Read, String> {
        let target = ArticleTarget::parse(value).map_err(|e| e.to_string())?;
        let entry = self
            .bookmarks
            .entries()
            .iter()
            .find(|e| e.target().board().same(target.board()) && e.target().aid() == target.aid())
            .ok_or(crate::msg!("bookmarks-not-found"))?;
        let archive = entry
            .archive_handle()
            .ok_or(crate::msg!("bookmarks-no-archive"))?;
        Ok(Read {
            target: entry.target().clone(),
            archive,
            rules,
            settings,
        })
    }
    pub fn complete(&mut self, result: Result<Bookmarks, String>) -> Result<(), String> {
        self.bookmarks = result?;
        Ok(())
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
        state.complete(Ok(saved)).unwrap();
        let original = state.bookmarks.clone();
        let write = state.edit("#00000101@stella", "changed", false).unwrap();
        std::fs::create_dir(dir.path().join("probe-bookmarks.json.bak")).unwrap();
        assert!(
            state
                .complete(write.store.save_from(&write.base, write.draft))
                .is_err()
        );
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
