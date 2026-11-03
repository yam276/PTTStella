//! Retained local editor owner: one disk task and one latest coalesced edit.
use pttstella::drafts::{
    Entry, Scope,
    store::{Saved, Store},
};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Edit {
    pub token: u64,
    pub sequence: u64,
    pub title: String,
    pub body: String,
}
#[derive(Serialize)]
pub(super) struct Summary {
    id: String,
    title: String,
    saved_at: u64,
    conflict: bool,
}
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub(super) enum Reply {
    List {
        request: u64,
        scope: Scope,
        entries: Vec<Summary>,
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
        body: String,
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
        task: JoinHandle<Result<pttstella::drafts::Drafts, String>>,
    },
    Save {
        token: u64,
        sequence: u64,
        task: JoinHandle<Result<Saved, String>>,
    },
}
pub(super) struct State {
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
}
impl Default for State {
    fn default() -> Self {
        Self::new(None)
    }
}
impl State {
    pub(super) fn new(probe: Option<&std::path::Path>) -> Self {
        Self {
            store: probe.map_or_else(Store::system, |p| {
                Ok(Store::at(p.join("probe-drafts.json")))
            }),
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
        }
    }
    pub(super) fn busy(&self) -> bool {
        self.job.is_some() || self.queued.is_some()
    }
    pub(super) fn fixture_matches(&self) -> bool {
        self.base.as_ref().is_some_and(|e| {
            e.body == "第一行中文\n\x1b[1;31m亮紅中文\x1b[m\n第三行" && e.title == "本機草稿測試"
        })
    }
    pub(super) fn scope(&self) -> Option<&Scope> {
        self.scope.as_ref()
    }
    pub(super) fn active(&self, token: u64) -> Result<&Entry, String> {
        self.active
            .as_ref()
            .filter(|_| {
                self.token == token && !self.busy() && self.base.as_ref() == self.active.as_ref()
            })
            .ok_or(pttstella::msg!("draft-save-first"))
    }
    pub(super) fn open(&mut self, request: u64, scope: Scope) -> Result<(), String> {
        if self.busy() {
            return Err(pttstella::msg!("draft-save-first"));
        }
        if !scope.valid() {
            return Err(pttstella::msg!("draft-unavailable"));
        }
        let store = self.store.clone()?;
        self.job = Some(Job::Read {
            request,
            scope,
            task: tokio::task::spawn_blocking(move || store.load()),
        });
        Ok(())
    }
    pub(super) fn delete(&mut self, id: String) -> Result<(), String> {
        if self.busy() {
            return Err(pttstella::msg!("draft-save-first"));
        }
        let entry = self
            .entries
            .iter()
            .find(|e| e.id == id && self.scope.as_ref().is_some_and(|scope| e.scope.same(scope)))
            .cloned()
            .ok_or(pttstella::msg!("draft-invalid"))?;
        let store = self.store.clone()?;
        self.job = Some(Job::Delete {
            id,
            task: tokio::task::spawn_blocking(move || store.remove(entry)),
        });
        Ok(())
    }
    pub(super) fn choose(&mut self, id: Option<String>) -> Result<Reply, String> {
        if self.busy() {
            return Err(pttstella::msg!("draft-save-first"));
        }
        let scope = self
            .scope
            .clone()
            .ok_or(pttstella::msg!("draft-unavailable"))?;
        self.base = if let Some(id) = id {
            Some(
                self.entries
                    .iter()
                    .find(|e| e.id == id && e.scope.same(&scope))
                    .cloned()
                    .ok_or(pttstella::msg!("draft-invalid"))?,
            )
        } else {
            None
        };
        let entry = self.base.clone().unwrap_or_else(|| Entry {
            id: self.id(),
            scope,
            revision: 0,
            title: String::new(),
            body: String::new(),
            saved_at: now(),
            conflict: false,
        });
        self.active = Some(entry.clone());
        self.token = self.token.saturating_add(1);
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
    pub(super) fn edit(&mut self, edit: Edit) -> Result<(), String> {
        if edit.token != self.token || edit.sequence <= self.sequence {
            return Err(pttstella::msg!("draft-invalid"));
        }
        let mut entry = self
            .active
            .clone()
            .ok_or(pttstella::msg!("draft-unavailable"))?;
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
    pub(super) async fn next(&mut self) -> Reply {
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
    pub(super) async fn finish(&mut self, final_edit: Option<Edit>) -> Result<(), String> {
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

pub(super) fn scope(view: &pttstella::connection::View) -> Option<Scope> {
    if view.phase != pttstella::connection::Phase::Connected
        || view.input_pending
        || view.output_pending
        || !view.site.ptt_features()
    {
        return None;
    }
    Some(Scope {
        site: view.site.clone(),
        account: view.account.clone()?,
        board: pttstella::board_navigation::current_board(&view.frame)?,
    })
}
pub(super) fn preview(body: &str, first: usize) -> Result<pttstella::screen::Frame, String> {
    if body.len() > pttstella::drafts::MAX_BODY || first > 4096 {
        return Err(pttstella::msg!("draft-limit"));
    }
    // Validate using the same safe SGR grammar before feeding the production
    // terminal core. No renderer-side ANSI parser or HTML source interpolation.
    pttstella::drafts::transfer(body, pttstella::encoding::TextEncoding::Uao)
        .map_err(|e| e.to_string())?;
    let lines: Vec<_> = body.split('\n').collect();
    if first >= lines.len() {
        return Err(pttstella::msg!("draft-invalid"));
    }
    let end = (first + 23).min(lines.len());
    let prefix = lines[..end].join("\r\n");
    let mut screen = pttstella::screen::Screen::new(80, 24).map_err(str::to_owned)?;
    screen.feed(&pttstella::encoding::encode(&prefix).map_err(|e| e.to_string())?);
    // Pad the last page too: its first requested source row must be at the
    // top, not mixed with earlier rows left in the terminal ring.
    screen.feed(&b"\r\n".repeat(first + 24 - end));
    Ok(screen.frame())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope() -> Scope {
        Scope {
            site: Default::default(),
            account: "StellaTest".into(),
            board: pttstella::board_navigation::BoardName::parse("Stella").unwrap(),
        }
    }
    async fn create(state: &mut State) {
        state.open(1, scope()).unwrap();
        assert!(matches!(state.next().await, Reply::List { .. }));
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
    #[test]
    fn preview_last_page_keeps_requested_top_and_carries_prior_ansi() {
        let body = (0..25)
            .map(|n| format!("{}LINE{n:02}", if n == 0 { "\x1b[31m" } else { "" }))
            .collect::<Vec<_>>()
            .join("\n");
        let frame = preview(&body, 23).unwrap();
        let rows = pttstella::text::rows(&frame).unwrap();
        assert!(rows[0].text.starts_with("LINE23"));
        assert!(rows[1].text.starts_with("LINE24"));
        assert!(rows[2].text.trim().is_empty());
        assert_eq!(rows[0].spans[0].glyph.left.fg, 1);
        let short = preview("第一行\n\x1b[32m中文", 0).unwrap();
        let rows = pttstella::text::rows(&short).unwrap();
        assert!(rows[0].text.starts_with("第一行"));
        assert!(rows[1].text.starts_with("中文"));
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
