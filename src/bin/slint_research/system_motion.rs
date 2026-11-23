//! Bind one OS preference subscription to the root, retaining unknown status.
use crate::TerminalWindow;
use pttstella::motion_preferences::{Audit, Preference, Subscription};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

#[derive(Clone, serde::Serialize)]
pub struct Closed {
    source: Option<Audit>,
    ui_listener_joined: Option<bool>,
}
pub struct Owner {
    ui: slint::Weak<TerminalWindow>,
    source: RefCell<Option<Subscription>>,
    task: RefCell<Option<slint::JoinHandle<()>>>,
    latest: RefCell<Preference>,
    deliveries: Cell<u64>,
    closing: Cell<bool>,
    outcome: RefCell<Option<Result<Closed, String>>>,
}
impl Owner {
    pub fn mount(ui: &TerminalWindow, system: bool) -> Result<Rc<Self>, String> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            source: RefCell::new(None),
            task: RefCell::new(None),
            latest: RefCell::new(Preference::Pending),
            deliveries: Cell::new(0),
            closing: Cell::new(false),
            outcome: RefCell::new(None),
        });
        if !system {
            owner.apply(Preference::Known {
                reduced: true,
                source: "controlled_synthetic_probe",
                raw_value: None,
            });
            return Ok(owner);
        }
        let (subscription, mut values) = match Subscription::start() {
            Ok(pair) => pair,
            Err(error) => {
                owner.apply(Preference::failed("subscribe", error));
                return Ok(owner);
            }
        };
        owner.apply(values.borrow_and_update().clone());
        owner.source.replace(Some(subscription));
        let weak = Rc::downgrade(&owner);
        match slint::spawn_local(async move {
            while values.changed().await.is_ok() {
                let Some(owner) = weak.upgrade() else {
                    break;
                };
                if owner.closing.get() {
                    break;
                }
                owner.apply(values.borrow_and_update().clone());
            }
        }) {
            Ok(task) => {
                owner.task.replace(Some(task));
            }
            Err(error) => {
                if let Some(mut source) = owner.source.borrow_mut().take() {
                    let _ = source.cancel();
                }
                return Err(error.to_string());
            }
        }
        Ok(owner)
    }
    fn apply(&self, preference: Preference) {
        if self.closing.get() {
            return;
        }
        if let Preference::Unavailable { stage, detail } = &preference {
            eprintln!("system motion {stage}: {detail}");
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_tools_reduced_motion(preference.reduced());
        }
        self.latest.replace(preference);
        self.deliveries.set(self.deliveries.get().saturating_add(1));
    }
    pub fn preference(&self) -> Preference {
        self.latest.borrow().clone()
    }
    pub fn deliveries(&self) -> u64 {
        self.deliveries.get()
    }
    pub fn prepare_close(&self) -> Result<(), String> {
        self.closing.set(true);
        if let Some(source) = self.source.borrow_mut().as_mut() {
            source.cancel()?;
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    pub fn verify_notification(&self) -> Result<(), String> {
        self.source
            .borrow()
            .as_ref()
            .ok_or("system motion source missing")?
            .verify_notification()
    }
    pub async fn shutdown(&self) -> Result<Closed, String> {
        if let Some(result) = self.outcome.borrow().clone() {
            return result;
        }
        self.closing.set(true);
        let mut source = self.source.borrow_mut().take();
        let mut listener = self.task.borrow_mut().take();
        let operation = async {
            let source_audit = if let Some(source) = source.as_mut() {
                Some(source.shutdown().await?)
            } else {
                None
            };
            // Release any remaining provider-held sender before awaiting the
            // actual UI listener. In-flight native blocks retain their own
            // sendable state and cannot publish into this closing owner.
            source.take();
            let joined = if let Some(task) = listener.as_mut() {
                task.await;
                listener.take();
                Some(true)
            } else {
                None
            };
            Ok(Closed {
                source: source_audit,
                ui_listener_joined: joined,
            })
        };
        let result = bounded(operation).await;
        // A deadline/error retains unfinished handles for explicit fallback
        // cancellation; dropping the waiter must not silently detach a task.
        self.source.replace(source);
        self.task.replace(listener);
        self.outcome.replace(Some(result.clone()));
        result
    }
    pub fn finish(&self) -> Result<Closed, String> {
        if let Some(Ok(closed)) = self.outcome.borrow().clone() {
            return Ok(closed);
        }
        self.closing.set(true);
        let cancel = self
            .source
            .borrow_mut()
            .as_mut()
            .map(|source| source.cancel())
            .transpose();
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        let mut error = self
            .outcome
            .borrow()
            .as_ref()
            .and_then(|r| r.as_ref().err())
            .cloned()
            .unwrap_or_else(|| {
                "system motion stopped after backend exit; asynchronous join unproven".into()
            });
        if let Err(cancel) = cancel {
            error.push_str(&format!("; motion cancel: {cancel}"));
        }
        self.outcome.replace(Some(Err(error.clone())));
        Err(error)
    }
    pub fn audit_closed(&self, dir: &std::path::Path) -> Result<(), String> {
        let closed = self.finish()?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join("motion-closed.json"))
            .map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(
            file,
            &serde_json::json!({"status":"passed","ownership":closed}),
        )
        .map_err(|e| e.to_string())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.closing.set(true);
        if let Some(source) = self.source.get_mut().as_mut() {
            let _ = source.cancel();
        }
        if let Some(task) = self.task.get_mut().take() {
            task.abort();
        }
    }
}

async fn bounded<F: std::future::Future<Output = Result<Closed, String>>>(
    operation: F,
) -> Result<Closed, String> {
    use futures_util::future::{Either, select};
    let timer = Timer::default();
    let (send, receive) = tokio::sync::oneshot::channel();
    let mut send = Some(send);
    timer.start(TimerMode::SingleShot, Duration::from_secs(5), move || {
        if let Some(send) = send.take() {
            let _ = send.send(());
        }
    });
    let operation = std::pin::pin!(operation);
    let result = match select(operation, receive).await {
        Either::Left((result, _)) => result,
        Either::Right(_) => Err("system motion shutdown deadline expired; join unproven".into()),
    };
    timer.stop();
    result
}
