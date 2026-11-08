//! The common draft owner runs on the existing Tokio runtime, independently of
//! the UI loop. There is one latest offered snapshot and one common disk job.
use pttstella::{
    drafts::{
        Scope,
        session::{Edit, Reply, State},
        store::Store,
    },
    editor_source::Snapshot,
};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};

pub enum Command {
    Open(Scope, Option<Snapshot>),
    Choose(Option<String>),
    Delete(String),
    Replacement(
        u64,
        oneshot::Sender<Result<pttstella::editor_source::Replacement, String>>,
    ),
    Close(Option<Edit>, oneshot::Sender<Result<(), String>>),
}
pub struct Storage {
    pub commands: mpsc::Sender<Command>,
    pub edits: watch::Sender<Option<Edit>>,
    pub replies: watch::Receiver<Option<Arc<Reply>>>,
    task: Option<tokio::task::JoinHandle<Result<(), String>>>,
}
impl Storage {
    pub fn stopped(&self) -> bool {
        self.task.is_none()
    }
    pub fn start(store: Result<Store, String>) -> Self {
        let (commands, mut receive) = mpsc::channel(8);
        let (edits, mut latest) = watch::channel::<Option<Edit>>(None);
        let (notify, replies) = watch::channel(None);
        let task = tokio::spawn(async move {
            let mut state = State::with_store(store);
            loop {
                let result = tokio::select! {
                    command=receive.recv()=>match command {
                        Some(Command::Close(final_edit,done))=> {
                            let result=state.finish(final_edit).await;
                            let _=done.send(result.clone());
                            return result;
                        }
                        Some(Command::Open(scope,template))=> {
                            state.template=template;
                            state.open(1,scope).map(|()| None)
                        }
                        Some(Command::Choose(id))=>state.choose(id).map(Some),
                        Some(Command::Delete(id))=>state.delete(id).map(|()|None),
                        Some(Command::Replacement(token,answer))=> {
                            let result=state.active(token).and_then(|entry| {
                                let baseline=state.template.clone().filter(|t|t.scope.same(&entry.scope)).ok_or_else(||pttstella::msg!("draft-transfer-stopped"))?;
                                Ok(pttstella::editor_source::Replacement { baseline,title:entry.title.clone(),body:entry.body.clone() })
                            });
                            let _=answer.send(result);
                            Ok(None)
                        }
                        None=> {let edit=latest.borrow().clone();return state.finish(edit).await;},
                    },
                    changed=latest.changed()=> {
                        if changed.is_err() { return state.finish(None).await; }
                        let edit=latest.borrow_and_update().clone();
                        edit.map_or(Ok(()),|edit|state.edit(edit)).map(|()|None)
                    }
                    reply=state.next()=>Ok(Some(reply)),
                };
                if let Some(reply) = match result {
                    Ok(reply) => reply,
                    Err(message) => Some(Reply::Error { message }),
                } {
                    notify.send_replace(Some(Arc::new(reply)));
                }
            }
        });
        Self {
            commands,
            edits,
            replies,
            task: Some(task),
        }
    }
    pub fn finish(&mut self, final_edit: Option<Edit>) -> Result<(), String> {
        let Some(task) = self.task.take() else {
            return Ok(());
        };
        // Main's event loop runs under runtime.enter(), not inside block_on.
        // Draining here also covers native window-manager close and UI failure.
        tokio::runtime::Handle::current().block_on(async {
            let (done, wait) = oneshot::channel();
            let saved = match self.commands.send(Command::Close(final_edit, done)).await {
                Ok(()) => wait
                    .await
                    .map_err(|_| pttstella::msg!("slint-draft-worker-unconfirmed"))
                    .and_then(|r| r),
                Err(_) => Err(pttstella::msg!("slint-draft-worker-stopped")),
            };
            let joined = task.await.map_err(|e| e.to_string()).and_then(|r| r);
            saved.and(joined)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap()
    }
    async fn selected(storage: &mut Storage) -> u64 {
        let scope = Scope {
            site: Default::default(),
            account: "StellaTest".into(),
            board: pttstella::board_navigation::BoardName::parse("Stella").unwrap(),
        };
        storage
            .commands
            .send(Command::Open(
                scope.clone(),
                Some(Snapshot {
                    scope,
                    title: "範本".into(),
                    body: "原文\n".into(),
                }),
            ))
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                storage.replies.changed().await.unwrap();
                if matches!(
                    storage.replies.borrow_and_update().as_deref(),
                    Some(Reply::List { .. })
                ) {
                    break;
                }
            }
        })
        .await
        .unwrap();
        storage.commands.send(Command::Choose(None)).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                storage.replies.changed().await.unwrap();
                if let Some(Reply::Selected { token, .. }) =
                    storage.replies.borrow_and_update().as_deref()
                {
                    break *token;
                }
            }
        })
        .await
        .unwrap()
    }
    #[test]
    fn close_drains_latest_snapshot_without_a_ui_reply_consumer_and_joins_actor() {
        let runtime = runtime();
        let _context = runtime.enter();
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path().join("drafts.json"));
        let mut storage = Storage::start(Ok(store.clone()));
        let token = runtime.block_on(selected(&mut storage));
        for sequence in 1..=50 {
            storage.edits.send_replace(Some(Edit {
                token,
                sequence,
                title: "合併保存".into(),
                body: format!("第{sequence}次\n\x1b[31m中文\x1b[m\n"),
            }));
        }
        storage
            .finish(Some(Edit {
                token,
                sequence: 51,
                title: "最後保存".into(),
                body: "最終正文\n".into(),
            }))
            .unwrap();
        assert!(storage.task.is_none());
        let data = store.load().unwrap();
        assert_eq!(data.entries.len(), 1);
        assert_eq!(data.entries[0].body, "最終正文\n");
        assert_eq!(data.entries[0].title, "最後保存");
        storage.finish(None).unwrap();
    }
    #[test]
    fn failed_final_write_is_reported_and_the_actor_still_joins() {
        let runtime = runtime();
        let _context = runtime.enter();
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("store");
        std::fs::create_dir(&parent).unwrap();
        let mut storage = Storage::start(Ok(Store::at(parent.join("drafts.json"))));
        let token = runtime.block_on(selected(&mut storage));
        std::fs::remove_dir(&parent).unwrap();
        std::fs::write(&parent, b"not a directory").unwrap();
        assert!(
            storage
                .finish(Some(Edit {
                    token,
                    sequence: 1,
                    title: "待保存".into(),
                    body: "保留原文".into()
                }))
                .is_err()
        );
        assert!(storage.task.is_none());
        assert_eq!(std::fs::read(&parent).unwrap(), b"not a directory");
    }
}
