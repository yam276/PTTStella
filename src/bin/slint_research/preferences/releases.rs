//! Native presentation of the shared stable-release Checker; no URL from UI.
use super::*;
use pttstella::updates::{Checker, Status, Worker};

impl Preferences {
    pub(super) fn start_releases(
        self: &Rc<Self>,
        checker: Checker,
        automatic: bool,
    ) -> Result<(), String> {
        if self.stopped.get() || self.release_worker.borrow().is_some() {
            return Err("release checker already owned or stopped".into());
        }
        let worker = Worker::start(checker, automatic)?;
        let mut updates = worker.updates();
        self.release_status
            .replace(updates.borrow_and_update().clone());
        self.release_worker.replace(Some(worker));
        let weak = Rc::downgrade(self);
        match slint::spawn_local(async move {
            while updates.changed().await.is_ok() {
                let Some(owner) = weak.upgrade() else { break };
                if owner.stopped.get() {
                    break;
                }
                owner
                    .release_status
                    .replace(updates.borrow_and_update().clone());
                if let Some(window) = owner.window.borrow().as_ref() {
                    owner.refresh_releases(window);
                }
            }
        }) {
            Ok(task) => {
                self.release_listener_joined.set(false);
                self.release_task.replace(Some(task));
            }
            Err(error) => {
                let stopped = self.finish_releases();
                return Err(match stopped {
                    Ok(()) => error.to_string(),
                    Err(cleanup) => format!("{error}; release checker: {cleanup}"),
                });
            }
        }
        if let Some(window) = self.window.borrow().as_ref() {
            self.refresh_releases(window);
        }
        Ok(())
    }

    pub(super) fn refresh_releases(&self, window: &crate::TerminalWindow) {
        let status = self.release_status.borrow();
        window
            .global::<SettingsState>()
            .set_release_enabled(!self.stopped.get() && self.release_worker.borrow().is_some());
        window
            .global::<SettingsState>()
            .set_release_checking(status.checking);
        window
            .global::<SettingsState>()
            .set_release_available(status.available.is_some());
        window.global::<SettingsState>().set_release_status(
            match &*status {
                Status {
                    available: Some(release),
                    ..
                } => msg!("update-available", version = release.version.clone()),
                Status {
                    up_to_date: true, ..
                } => msg!("update-current"),
                _ => String::new(),
            }
            .into(),
        );
        window
            .global::<SettingsState>()
            .set_release_labels(slint::ModelRc::new(slint::VecModel::from(vec![
                if status.checking {
                    msg!("update-checking")
                } else {
                    msg!("update-check")
                }
                .into(),
                msg!("update-open").into(),
                msg!("update-help").into(),
                env!("CARGO_PKG_REPOSITORY").into(),
            ])));
    }

    fn release_action_allowed(&self) -> bool {
        !self.stopped.get()
            && !self.closing.get()
            && !self.composing.get()
            && self
                .ui
                .upgrade()
                .is_some_and(|ui| !ui.get_app_closing() && ui.get_settings_open())
            && self.window.borrow().as_ref().is_some_and(|w| {
                w.global::<SettingsState>().get_page() == 7
                    && w.global::<SettingsState>().get_query().is_empty()
            })
    }

    pub(super) fn check_release(&self) {
        if self.release_action_allowed()
            && self
                .release_worker
                .borrow()
                .as_ref()
                .is_some_and(Worker::request)
        {
            let mut status = self.release_status.borrow_mut();
            status.checking = true;
            status.up_to_date = false;
            drop(status);
            if let Some(window) = self.window.borrow().as_ref() {
                self.refresh_releases(window);
            }
        }
    }

    pub(super) fn open_release(&self, repository: bool) {
        if !self.release_action_allowed() {
            return;
        }
        let url = if repository {
            Some(env!("CARGO_PKG_REPOSITORY").to_owned())
        } else {
            self.release_status
                .borrow()
                .available
                .as_ref()
                .map(|r| r.url.clone())
        };
        if let (Some(ui), Some(url)) = (self.ui.upgrade(), url) {
            // The existing application adapter opens validated links only in
            // Live. Synthetic probes record the target without opening a browser.
            ui.invoke_link_activated(url.into());
        }
    }

    pub(crate) async fn shutdown_releases(&self) -> Result<(), String> {
        let listener = self.release_task.borrow_mut().take();
        let worker = self.release_worker.borrow_mut().take();
        let result = if let Some(mut worker) = worker {
            worker.shutdown().await
        } else {
            Ok(())
        };
        if let Some(listener) = listener {
            // Channel closure wakes the Slint listener. Await its real exit
            // while the explicit UI loop is alive; abort alone is not a join.
            listener.await;
            self.release_listener_joined.set(true);
        }
        result
    }

    pub(super) fn finish_releases(&self) -> Result<(), String> {
        let listener = self.release_task.borrow_mut().take();
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let mut unjoined = false;
        if let Some(listener) = listener {
            if listener.is_finished() {
                runtime.block_on(listener);
                self.release_listener_joined.set(true);
            } else {
                listener.abort();
                unjoined = true;
            }
        }
        if let Some(mut worker) = self.release_worker.borrow_mut().take() {
            // Exceptional backend exits reach this after the Slint loop, on
            // its UI thread, while the existing multi-thread runtime is alive.
            runtime.block_on(worker.shutdown())?;
        }
        if unjoined || !self.release_listener_joined.get() {
            return Err(
                "release status listener was cancelled after backend exit; join unproven".into(),
            );
        }
        Ok(())
    }
}
