//! One native credential operation and one explicit Connection login attempt.
use crate::{TerminalWindow, session::FrameId};
use pttstella::{
    connection::{Connection, Phase, View},
    login::{
        self, Prompt,
        vault::{Action, Job, Outcome, Scope, Vault},
    },
    msg,
    navigation::Stamp,
};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use zeroize::Zeroizing;

pub struct Login {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    presented: Rc<Cell<Option<FrameId>>>,
    probe: bool,
    job: RefCell<Option<Job>>,
    timer: Timer,
    generation: Cell<Option<u64>>,
    submitting: Cell<Option<Stamp>>,
}

impl Login {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        presented: Rc<Cell<Option<FrameId>>>,
        probe: bool,
    ) -> Rc<Self> {
        let login = Rc::new(Self {
            ui: ui.as_weak(),
            connection,
            presented,
            probe,
            job: RefCell::new(None),
            timer: Timer::default(),
            generation: Cell::new(None),
            submitting: Cell::new(None),
        });
        ui.on_login_limit_text(|text, maximum| {
            text.chars()
                .take(maximum.clamp(0, 72) as usize)
                .collect::<String>()
                .into()
        });
        ui.on_login_show({
            let login = login.clone();
            move || {
                let Some(ui) = login.ui.upgrade() else { return };
                if ui.get_app_closing()
                    || ui.get_manager_open()
                    || ui.get_settings_open()
                    || login.job.borrow().is_some()
                    || ui.get_paste_open()
                    || ui.get_switching_station()
                {
                    return;
                }
                let generation = {
                    let view = login.connection.updates.borrow();
                    if !view.site.ptt_features() {
                        return;
                    }
                    view.generation
                };
                login.generation.set(Some(generation));
                login.submitting.set(None);
                ui.set_login_account("".into());
                ui.set_login_saved(false);
                ui.set_login_message(msg!("vault-reading").into());
                ui.set_login_open(true);
                ui.invoke_show_login_popup();
                ui.invoke_keep_alive_changed();
                login.begin(&ui, Action::Open);
            }
        });
        ui.on_login_submit({
            let login = login.clone();
            move |account, password, remember| {
                let password = Zeroizing::new(password.as_bytes().to_vec());
                let Some(ui) = login.ui.upgrade() else { return };
                ui.invoke_clear_login_password();
                if !ui.get_login_open() || login.job.borrow().is_some() {
                    return;
                }
                let stamp = {
                    let view = login.connection.updates.borrow();
                    login_stamp(&view, login.presented.get(), ui.get_switching_station())
                };
                let Some(stamp) = stamp else {
                    ui.set_login_message(msg!("login-not-account-prompt").into());
                    return;
                };
                if login.generation.get() != Some(stamp.generation) {
                    ui.set_login_message(msg!("login-stale").into());
                    return;
                }
                login.submitting.set(Some(stamp));
                ui.set_login_message(msg!("login-preparing").into());
                login.begin(
                    &ui,
                    Action::Submit {
                        account: account.to_string(),
                        password,
                        remember,
                    },
                );
            }
        });
        ui.on_login_forget({
            let login = login.clone();
            move || {
                if let Some(ui) = login.ui.upgrade()
                    && ui.get_login_open()
                    && login.job.borrow().is_none()
                {
                    login.submitting.set(None);
                    ui.invoke_clear_login_password();
                    login.begin(&ui, Action::Forget);
                }
            }
        });
        ui.on_login_cancel({
            let login = login.clone();
            move || login.cancel(true)
        });
        ui.on_login_updated({
            let login = login.clone();
            move || {
                let Some(ui) = login.ui.upgrade() else { return };
                let (available, pending, generation) = {
                    let view = login.connection.updates.borrow();
                    (
                        view.site.ptt_features() && !ui.get_switching_station(),
                        view.login_pending,
                        view.generation,
                    )
                };
                ui.set_login_available(available);
                ui.set_login_pending(pending);
                if ui.get_login_open() && (!available || login.generation.get() != Some(generation))
                {
                    login.cancel(false);
                }
            }
        });
        login
    }

    fn begin(self: &Rc<Self>, ui: &TerminalWindow, action: Action) {
        let scope = if self.probe {
            Scope::SlintSyntheticProbe
        } else {
            Scope::SlintResearch
        };
        match Job::start(Vault::for_scope(scope), action) {
            Ok(job) => self.job.replace(Some(job)),
            Err(error) => {
                ui.set_login_message(error.into());
                return;
            }
        };
        ui.set_login_busy(true);
        let weak = Rc::downgrade(self);
        self.timer
            .start(TimerMode::Repeated, Duration::from_millis(50), move || {
                let Some(login) = weak.upgrade() else { return };
                let result = {
                    let mut job = login.job.borrow_mut();
                    let Some(job) = job.as_mut() else { return };
                    match job.try_wait() {
                        Ok(None) => return,
                        Ok(Some(outcome)) => Ok(outcome),
                        Err(error) => Err(error),
                    }
                };
                login.timer.stop();
                // A ready reply means the native operation finished. Join its owned thread.
                login.job.borrow_mut().take();
                if let Some(ui) = login.ui.upgrade() {
                    ui.set_login_busy(false);
                    if ui.get_login_open() {
                        login.complete(&ui, result);
                    }
                }
            });
    }

    fn complete(&self, ui: &TerminalWindow, result: Result<Outcome, String>) {
        match result {
            Ok(Outcome::Opened { account, saved }) => {
                ui.set_login_account(account.into());
                ui.set_login_saved(saved);
                ui.set_login_message(
                    if saved {
                        msg!("login-saved-help")
                    } else {
                        msg!("login-unsaved-help")
                    }
                    .into(),
                );
            }
            Ok(Outcome::Forgotten) => {
                ui.set_login_account("".into());
                ui.set_login_saved(false);
                ui.set_login_message(msg!("login-forgotten").into());
            }
            Ok(Outcome::Ready(credentials)) => {
                let Some(stamp) = self.submitting.take() else {
                    return;
                };
                let current = {
                    let view = self.connection.updates.borrow();
                    login_stamp(&view, self.presented.get(), ui.get_switching_station())
                };
                if current != Some(stamp) {
                    ui.set_login_message(msg!("login-stale").into());
                    return;
                }
                match self.connection.request_login(stamp, credentials) {
                    Ok(reply) => {
                        ui.set_login_open(false);
                        ui.invoke_hide_login_popup(true);
                        ui.invoke_keep_alive_changed();
                        let (weak, connection) = (self.ui.clone(), self.connection.clone());
                        if let Err(error) = slint::spawn_local(async move {
                            if let Err(error) = reply.wait().await
                                && connection.updates.borrow().generation == stamp.generation
                                && let Some(ui) = weak.upgrade()
                            {
                                crate::notice::error(&ui, error);
                            }
                        }) {
                            crate::notice::error(ui, error.to_string());
                        }
                    }
                    Err(error) => ui.set_login_message(error.into()),
                }
            }
            Err(error) => {
                self.submitting.set(None);
                ui.set_login_message(error.into());
            }
        }
    }

    fn cancel(&self, restore_focus: bool) {
        self.generation.set(None);
        self.submitting.set(None);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_login_open(false);
            ui.invoke_hide_login_popup(restore_focus);
            ui.invoke_keep_alive_changed();
            if let Err(error) = self.connection.cancel_login() {
                crate::notice::error(&ui, error);
            }
        }
    }

    pub fn finish(&self) {
        self.cancel(false);
        self.timer.stop();
        // System authorization dialogs must finish or be dismissed by their user.
        // Retain and join the native job through shutdown.
        self.job.borrow_mut().take();
    }
}

fn login_stamp(view: &View, presented: Option<FrameId>, switching: bool) -> Option<Stamp> {
    let stamp = view.stamp();
    (!switching
        && view.phase == Phase::Connected
        && view.site.ptt_features()
        && !view.output_pending
        && !view.input_pending
        && !view.login_pending
        && presented
            == Some(FrameId {
                generation: stamp.generation,
                revision: stamp.revision,
            })
        && login::prompt(&view.frame) == Some(Prompt::Account))
    .then_some(stamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::{
        connection::CloseOutcome,
        login::{Credentials, Guidance},
    };

    async fn wait(connection: &Connection, predicate: impl Fn(&View) -> bool) -> View {
        let mut updates = connection.updates.clone();
        tokio::time::timeout(Duration::from_secs(5), updates.wait_for(predicate))
            .await
            .unwrap()
            .unwrap()
            .clone()
    }

    #[tokio::test]
    async fn explicit_unsaved_login_and_manual_verification_use_the_strict_peer() {
        let connection = Connection::start_verification_probe().unwrap();
        connection.connect().unwrap();
        let view = wait(&connection, |view| {
            login::prompt(&view.frame) == Some(Prompt::Account)
        })
        .await;
        let id = FrameId {
            generation: view.generation,
            revision: view.frame.revision,
        };
        let stamp = login_stamp(&view, Some(id), false).unwrap();
        assert!(login_stamp(&view, None, false).is_none());
        assert!(login_stamp(&view, Some(id), true).is_none());
        let mut other_site = view.clone();
        other_site.site.encoding = pttstella::encoding::TextEncoding::Big5;
        assert!(login_stamp(&other_site, Some(id), false).is_none());

        // With a supplied password and no consent to save, the shared reducer never accesses a native store.
        let mut job = Job::start(
            Vault::for_scope(Scope::SlintSyntheticProbe),
            Action::Submit {
                account: "StellaTest".into(),
                password: Zeroizing::new(b"synthetic+42".to_vec()),
                remember: false,
            },
        )
        .unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if let Some(outcome) = job.try_wait().unwrap() {
                    break outcome;
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        drop(job);
        let Outcome::Ready(credentials) = outcome else {
            panic!("expected validated credentials")
        };
        connection
            .request_login(stamp, credentials)
            .unwrap()
            .wait()
            .await
            .unwrap();
        let challenge = wait(&connection, |view| {
            view.login_guidance == Guidance::CodeRequired
        })
        .await;
        assert!(login_stamp(&challenge, Some(id), false).is_none());
        let repeated = Credentials::new(
            "StellaTest".into(),
            Zeroizing::new(b"synthetic+42".to_vec()),
        )
        .unwrap();
        assert!(
            connection
                .request_login(stamp, repeated)
                .unwrap()
                .wait()
                .await
                .is_err()
        );
        connection
            .input(challenge.generation, b"123456\r".to_vec())
            .unwrap();
        let rejected = wait(&connection, |view| {
            view.login_guidance == Guidance::CodeRejected
        })
        .await;
        connection
            .input(rejected.generation, b"ABCDEFGH\r".to_vec())
            .unwrap();
        wait(&connection, |view| {
            view.login_guidance == Guidance::Browsing
        })
        .await;
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
    }
}
