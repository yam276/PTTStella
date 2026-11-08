//! Retained local editor owner: one disk task and one latest coalesced edit.
use crate::drafts::{
    Entry, Scope,
    store::{Saved, Store},
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub token: u64,
    pub sequence: u64,
    pub title: String,
    pub body: String,
}
#[derive(Serialize)]
pub struct Summary {
    pub id: String,
    pub title: String,
    pub saved_at: u64,
    pub conflict: bool,
}
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Reply {
    List {
        request: u64,
        scope: Scope,
        entries: Vec<Summary>,
        can_create: bool,
    },
    Selected {
        token: u64,
        entry: Entry,
    },
    Saved {
        token: u64,
        sequence: u64,
        conflicted: bool,
    },
    Error {
        message: String,
    },
    Transferred {
        message: String,
    },
    Colored {
        token: u64,
        revision: u64,
        changes: Vec<crate::composer::Change>,
    },
    Projected {
        token: u64,
        revision: u64,
        decorations: Vec<crate::composer::Decoration>,
    },
    DecorationError {
        token: u64,
        revision: u64,
        coloring: bool,
        message: String,
    },
    Deleted {
        id: String,
    },
}
enum Job {
    Delete {
        id: String,
        task: JoinHandle<Result<(), String>>,
    },
    Read {
        request: u64,
        scope: Scope,
        task: JoinHandle<Result<crate::drafts::Drafts, String>>,
    },
    Save {
        token: u64,
        sequence: u64,
        task: JoinHandle<Result<Saved, String>>,
    },
}
pub struct State {
    store: Result<Store, String>,
    scope: Option<Scope>,
    entries: Vec<Entry>,
    active: Option<Entry>,
    base: Option<Entry>,
    token: u64,
    sequence: u64,
    job: Option<Job>,
    queued: Option<Edit>,
    last_error: Option<String>,
    latest: Option<Edit>,
    pub template: Option<crate::editor_source::Snapshot>,
}
impl Default for State {
    fn default() -> Self {
        Self::new(None)
    }
}
impl State {
    pub fn new(probe: Option<&std::path::Path>) -> Self {
        Self::with_store(probe.map_or_else(Store::system, |p| {
            Ok(Store::at(p.join("probe-drafts.json")))
        }))
    }
    /// Native candidates supply an explicit isolated profile store. The disk
    /// owner, baseline checks and coalesced save queue remain identical.
    pub fn with_store(store: Result<Store, String>) -> Self {
        Self {
            store,
            scope: None,
            entries: Vec::new(),
            active: None,
            base: None,
            token: 0,
            sequence: 0,
            job: None,
            queued: None,
            last_error: None,
            latest: None,
            template: None,
        }
    }
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.queued.is_some()
    }
    pub fn fixture_matches(&self) -> bool {
        self.base.as_ref().is_some_and(|e| {
            e.body == "第一行中文\n\x1b[1;31m亮紅中文\x1b[m\n第三行" && e.title == "本機草稿測試"
        })
    }
    pub fn scope(&self) -> Option<&Scope> {
        self.scope.as_ref()
    }
    pub fn owns(&self, token: u64) -> Result<(), String> {
        if self.active.is_some() && self.token == token {
            Ok(())
        } else {
            Err(crate::msg!("draft-invalid"))
        }
    }
    pub fn active(&self, token: u64) -> Result<&Entry, String> {
        self.active
            .as_ref()
            .filter(|_| {
                self.token == token && !self.busy() && self.base.as_ref() == self.active.as_ref()
            })
            .ok_or(crate::msg!("draft-save-first"))
    }
    pub fn open(&mut self, request: u64, scope: Scope) -> Result<(), String> {
        if self.busy() {
            return Err(crate::msg!("draft-save-first"));
        }
        if !scope.valid() {
            return Err(crate::msg!("draft-unavailable"));
        }
        let store = self.store.clone()?;
        self.job = Some(Job::Read {
            request,
            scope,
            task: tokio::task::spawn_blocking(move || store.load()),
        });
        Ok(())
    }
    pub fn delete(&mut self, id: String) -> Result<(), String> {
        if self.busy() {
            return Err(crate::msg!("draft-save-first"));
        }
        let entry = self
            .entries
            .iter()
            .find(|e| e.id == id && self.scope.as_ref().is_some_and(|scope| e.scope.same(scope)))
            .cloned()
            .ok_or(crate::msg!("draft-invalid"))?;
        let store = self.store.clone()?;
        self.job = Some(Job::Delete {
            id,
            task: tokio::task::spawn_blocking(move || store.remove(entry)),
        });
        Ok(())
    }
    pub fn choose(&mut self, id: Option<String>) -> Result<Reply, String> {
        if self.busy() {
            return Err(crate::msg!("draft-save-first"));
        }
        let scope = self.scope.clone().ok_or(crate::msg!("draft-unavailable"))?;
        if id.is_none() && !self.template.as_ref().is_some_and(|t| t.scope.same(&scope)) {
            return Err(crate::msg!("draft-unavailable"));
        }
        let token = self
            .token
            .checked_add(1)
            .ok_or_else(|| crate::msg!("draft-limit"))?;
        self.base = if let Some(id) = id {
            Some(
                self.entries
                    .iter()
                    .find(|e| e.id == id && e.scope.same(&scope))
                    .cloned()
                    .ok_or(crate::msg!("draft-invalid"))?,
            )
        } else {
            None
        };
        let entry = self.base.clone().unwrap_or_else(|| Entry {
            id: self.id(),
            scope: scope.clone(),
            revision: 0,
            title: self
                .template
                .as_ref()
                .filter(|t| t.scope.same(&scope))
                .map(|t| t.title.clone())
                .unwrap_or_default(),
            body: self
                .template
                .as_ref()
                .filter(|t| t.scope.same(&scope))
                .map(|t| t.body.clone())
                .unwrap_or_default(),
            saved_at: now(),
            conflict: false,
        });
        self.active = Some(entry.clone());
        self.token = token;
        self.sequence = 0;
        self.last_error = None;
        self.latest = None;
        Ok(Reply::Selected {
            token: self.token,
            entry,
        })
    }
    fn id(&self) -> String {
        format!(
            "{:x}-{:x}-{:x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            self.token
        )
    }
    pub fn edit(&mut self, edit: Edit) -> Result<(), String> {
        if edit.token != self.token || edit.sequence <= self.sequence {
            return Err(crate::msg!("draft-invalid"));
        }
        let mut entry = self
            .active
            .clone()
            .ok_or(crate::msg!("draft-unavailable"))?;
        entry.title = edit.title.clone();
        entry.body = edit.body.clone();
        entry.validate()?;
        self.sequence = edit.sequence;
        self.latest = Some(edit.clone());
        self.queued = Some(edit);
        self.last_error = None;
        self.start();
        Ok(())
    }
    fn start(&mut self) {
        if self.job.is_some() {
            return;
        }
        let Some(edit) = self.queued.take() else {
            return;
        };
        let Some(mut entry) = self.active.clone() else {
            return;
        };
        entry.title = edit.title;
        entry.body = edit.body;
        entry.saved_at = now();
        let store = self.store.clone();
        let base = self.base.clone();
        let conflict_id = self.id();
        self.job = Some(Job::Save {
            token: edit.token,
            sequence: edit.sequence,
            task: tokio::task::spawn_blocking(move || store?.save(base, entry, conflict_id)),
        });
    }
    pub async fn next(&mut self) -> Reply {
        let Some(job) = self.job.as_mut() else {
            return std::future::pending().await;
        };
        let reply = match job {
            Job::Delete { id, task } => {
                let id = id.clone();
                match task.await.map_err(|e| e.to_string()).and_then(|r| r) {
                    Ok(()) => {
                        self.entries.retain(|e| e.id != id);
                        Reply::Deleted { id }
                    }
                    Err(message) => Reply::Error { message },
                }
            }
            Job::Read {
                request,
                scope,
                task,
            } => {
                let request = *request;
                let scope = scope.clone();
                let result = task.await.map_err(|e| e.to_string()).and_then(|r| r);
                match result {
                    Ok(data) => {
                        self.entries = data
                            .entries
                            .into_iter()
                            .filter(|e| e.scope.same(&scope))
                            .collect();
                        self.scope = Some(scope.clone());
                        self.active = None;
                        self.base = None;
                        Reply::List {
                            request,
                            can_create: self
                                .template
                                .as_ref()
                                .is_some_and(|t| t.scope.same(&scope)),
                            scope,
                            entries: self
                                .entries
                                .iter()
                                .map(|e| Summary {
                                    id: e.id.clone(),
                                    title: e.title.clone(),
                                    saved_at: e.saved_at,
                                    conflict: e.conflict,
                                })
                                .collect(),
                        }
                    }
                    Err(message) => Reply::Error { message },
                }
            }
            Job::Save {
                token,
                sequence,
                task,
            } => {
                let token = *token;
                let sequence = *sequence;
                let result = task.await.map_err(|e| e.to_string()).and_then(|r| r);
                match result {
                    Ok(saved) => {
                        self.base = Some(saved.entry.clone());
                        self.active = Some(saved.entry);
                        Reply::Saved {
                            token,
                            sequence,
                            conflicted: saved.conflicted,
                        }
                    }
                    Err(message) => {
                        self.last_error = Some(message.clone());
                        Reply::Error { message }
                    }
                }
            }
        };
        self.job = None;
        // Never silently retry a failed write. A new edit or explicit save is
        // needed; a newer queued snapshot is retained for shutdown recovery.
        if !matches!(reply, Reply::Error { .. }) {
            self.start();
        }
        reply
    }
    pub async fn finish(&mut self, final_edit: Option<Edit>) -> Result<(), String> {
        while self.job.is_some() {
            self.next().await;
        }
        if let Some(edit) =
            final_edit.filter(|e| e.token == self.token && e.sequence > self.sequence)
        {
            self.edit(edit)?;
        }
        if self.last_error.is_some() && self.queued.is_none() {
            self.queued = self.latest.clone();
            self.last_error = None;
        }
        self.start();
        while self.job.is_some() {
            self.next().await;
        }
        self.last_error.take().map_or(Ok(()), Err)
    }
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope() -> Scope {
        Scope {
            site: Default::default(),
            account: "StellaTest".into(),
            board: crate::board_navigation::BoardName::parse("Stella").unwrap(),
        }
    }
    async fn create(state: &mut State) {
        state.open(1, scope()).unwrap();
        assert!(matches!(state.next().await, Reply::List { .. }));
        state.template = Some(crate::editor_source::Snapshot {
            scope: scope(),
            title: String::new(),
            body: String::new(),
        });
        state.choose(None).unwrap();
    }
    fn edit(state: &State, sequence: u64, body: &str) -> Edit {
        Edit {
            token: state.token,
            sequence,
            title: "中文草稿".into(),
            body: body.into(),
        }
    }
    #[tokio::test]
    async fn explicit_native_store_and_dioxus_wire_share_the_same_retained_owner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("isolated-native-drafts.json");
        let mut state = State::with_store(Ok(Store::at(path.clone())));
        create(&mut state).await;
        let edit:Edit=serde_json::from_value(serde_json::json!({"token":state.token,"sequence":1,"title":"共同標題","body":"中文\n\x1b[31;66;34m雙色\n"})).unwrap();
        state.edit(edit).unwrap();
        let reply = state.next().await;
        assert_eq!(
            serde_json::to_value(reply).unwrap(),
            serde_json::json!({"state":"saved","token":1,"sequence":1,"conflicted":false})
        );
        state.finish(None).await.unwrap();
        let saved = Store::at(path).load().unwrap();
        assert_eq!(saved.entries[0].body, "中文\n\x1b[31;66;34m雙色\n");
        assert!(
            serde_json::from_value::<Edit>(
                serde_json::json!({"token":1,"sequence":2,"title":"","body":"","unknown":true})
            )
            .is_err()
        );
        state.token = u64::MAX;
        let before = state.active.clone();
        assert!(state.choose(None).is_err());
        assert_eq!(state.active, before);
    }
    #[tokio::test]
    async fn new_document_requires_imported_same_board_template_and_seeds_title_and_offscreen_source()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::new(Some(dir.path()));
        state.open(1, scope()).unwrap();
        state.next().await;
        assert!(state.choose(None).is_err());
        let body = (0..32)
            .map(|n| format!("範本第{n}行"))
            .collect::<Vec<_>>()
            .join("\n");
        state.template = Some(crate::editor_source::Snapshot {
            scope: scope(),
            title: "[分類] 範本".into(),
            body: body.clone(),
        });
        let Reply::Selected { entry, .. } = state.choose(None).unwrap() else {
            panic!()
        };
        assert_eq!(entry.body, body);
        assert_eq!(entry.title, "[分類] 範本");
        state.template.as_mut().unwrap().scope.account = "Other".into();
        assert!(state.choose(None).is_err());
    }
    #[tokio::test]
    async fn shutdown_joins_earlier_saves_and_flushes_final_debounced_ansi_source() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::new(Some(dir.path()));
        create(&mut state).await;
        state.edit(edit(&state, 1, "第一版")).unwrap();
        state.edit(edit(&state, 2, "過渡版")).unwrap();
        let final_edit = edit(&state, 3, "關閉時中文\n\x1b[1;31m紅\x1b[m");
        state.finish(Some(final_edit)).await.unwrap();
        assert!(!state.busy());
        let saved = Store::at(dir.path().join("probe-drafts.json"))
            .load()
            .unwrap();
        assert_eq!(saved.entries.len(), 1);
        assert_eq!(saved.entries[0].body, "關閉時中文\n\x1b[1;31m紅\x1b[m");
        let mut restarted = State::new(Some(dir.path()));
        restarted.open(1, scope()).unwrap();
        let Reply::List { entries, .. } = restarted.next().await else {
            panic!()
        };
        assert_eq!(entries.len(), 1);
        restarted.choose(Some(entries[0].id.clone())).unwrap();
        assert_eq!(
            restarted.active(restarted.token).unwrap().body,
            saved.entries[0].body
        );
        let mut other = scope();
        other.account = "Another".into();
        restarted.open(2, other).unwrap();
        let Reply::List { entries, .. } = restarted.next().await else {
            panic!()
        };
        assert!(entries.is_empty());
    }
    #[tokio::test]
    async fn final_snapshot_cannot_switch_owners_and_repeated_local_saves_do_not_create_false_conflicts()
     {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::new(Some(dir.path()));
        create(&mut state).await;
        state.edit(edit(&state, 1, "原稿")).unwrap();
        state.next().await;
        let mut foreign = edit(&state, 2, "外來");
        foreign.token += 1;
        state.finish(Some(foreign)).await.unwrap();
        state.edit(edit(&state, 2, "中文第二版")).unwrap();
        state.edit(edit(&state, 3, "中文第三版")).unwrap();
        state.finish(None).await.unwrap();
        let entries = state.store.clone().unwrap().load().unwrap().entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].body, "中文第三版");
        assert!(!entries[0].conflict);
    }
    #[tokio::test]
    async fn failed_save_does_not_discard_final_source_or_claim_shutdown_success() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::new(Some(dir.path()));
        create(&mut state).await;
        std::fs::write(dir.path().join("probe-drafts.json"), b"corrupt").unwrap();
        state.edit(edit(&state, 1, "不可遺失中文")).unwrap();
        assert!(matches!(state.next().await, Reply::Error { .. }));
        assert!(state.finish(None).await.is_err());
        assert_eq!(
            std::fs::read(dir.path().join("probe-drafts.json")).unwrap(),
            b"corrupt"
        );
        assert_eq!(state.latest.unwrap().body, "不可遺失中文");
    }
}
