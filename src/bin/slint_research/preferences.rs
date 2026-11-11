//! Native preference owner. The shared Store still owns merge/backup/replace;
//! Output owns the one file worker. UI drafts never become effective on edit.
mod form;
mod probe;
mod releases;
mod releases_probe;
mod search_probe;
mod window;

use crate::SettingsState;
use crate::{TerminalWindow, output::Output};
use pttstella::{
    msg,
    settings::{AppSettings, preferences::Preferences as Model},
};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
enum Handoff {
    Vim,
    Mouse,
    KeepAlive,
    Pan,
    VimOnce,
}
type Apply = Box<dyn Fn(&AppSettings) -> Result<(), String>>;
type Saved = crate::file_job::ResultSlot<AppSettings>;
struct InFlight {
    draft: AppSettings,
    result: Saved,
}
pub struct Preferences {
    pub(super) model: RefCell<Model>,
    ui: slint::Weak<TerminalWindow>,
    output: Rc<Output>,
    window: RefCell<Option<crate::TerminalWindow>>,
    preview_renderer: RefCell<Option<crate::renderer::Renderer>>,
    pending: RefCell<Option<AppSettings>>,
    due: Cell<Option<Instant>>,
    valid: Cell<bool>,
    inflight: RefCell<Option<InFlight>>,
    effects: RefCell<Vec<Apply>>,
    timer: Timer,
    composing: Cell<bool>,
    search_index: RefCell<Option<pttstella::settings::catalog::Index>>,
    search_focus: Timer,
    closing: Cell<bool>,
    handoff: Cell<Option<Handoff>>,
    export_pending: Cell<bool>,
    failed: Cell<bool>,
    stopped: Cell<bool>,
    release_worker: RefCell<Option<pttstella::updates::Worker>>,
    release_task: RefCell<Option<slint::JoinHandle<()>>>,
    release_status: RefCell<pttstella::updates::Status>,
    release_listener_joined: Cell<bool>,
    release_probe: RefCell<Option<releases_probe::Fixture>>,
}
impl Preferences {
    pub fn mount(
        ui: &TerminalWindow,
        output: Rc<Output>,
        model: Model,
        check_releases: bool,
    ) -> Rc<Self> {
        let owner = Rc::new(Self {
            model: RefCell::new(model),
            ui: ui.as_weak(),
            output,
            window: RefCell::new(None),
            preview_renderer: RefCell::new(None),
            pending: RefCell::new(None),
            due: Cell::new(None),
            valid: Cell::new(true),
            inflight: RefCell::new(None),
            effects: RefCell::new(Vec::new()),
            timer: Timer::default(),
            composing: Cell::new(false),
            search_index: RefCell::new(None),
            search_focus: Timer::default(),
            closing: Cell::new(false),
            handoff: Cell::new(None),
            export_pending: Cell::new(false),
            failed: Cell::new(false),
            stopped: Cell::new(false),
            release_worker: RefCell::new(None),
            release_task: RefCell::new(None),
            release_status: RefCell::new(pttstella::updates::Status::default()),
            release_listener_joined: Cell::new(true),
            release_probe: RefCell::new(None),
        });
        ui.set_settings_available(true);
        ui.on_settings_show({
            let weak = Rc::downgrade(&owner);
            move || {
                if let Some(owner) = weak.upgrade() {
                    owner.open();
                }
            }
        });
        ui.on_settings_composition({
            let weak = Rc::downgrade(&owner);
            move |active, committed| {
                if let Some(owner) = weak.upgrade() {
                    owner.composing.set(active);
                    if active {
                        owner.timer.stop();
                    } else if committed {
                        let weak = Rc::downgrade(&owner);
                        Timer::single_shot(Duration::ZERO, move || {
                            if let Some(owner) = weak.upgrade() {
                                owner.edited();
                            }
                        });
                    }
                }
            }
        });
        ui.on_settings_runtime_updated({
            let owner = Rc::downgrade(&owner);
            move || {
                if let Some(owner) = owner.upgrade()
                    && !owner.stopped.get()
                    && let Some(window) = owner.window.borrow().as_ref()
                {
                    owner.refresh_runtime(window);
                }
            }
        });
        ui.on_output_idle({
            let weak = Rc::downgrade(&owner);
            move || {
                if let Some(owner) = weak.upgrade()
                    && !owner.stopped.get()
                {
                    if !owner.failed.get()
                        && (owner.pending.borrow().is_some()
                            || owner.closing.get()
                            || owner.export_pending.get())
                    {
                        owner.arm(Duration::ZERO);
                    } else {
                        owner.load_font_catalog_if_needed();
                    }
                }
            }
        });
        if check_releases
            && let Err(error) = owner.start_releases(pttstella::updates::Checker::default(), true)
        {
            owner.notice(error);
        }
        owner
    }
    pub fn prepare_app_close(&self) {
        self.handoff.set(None);
        if let Some(worker) = self.release_worker.borrow_mut().as_mut() {
            worker.cancel();
        }
        self.hide();
    }
    pub fn app(&self) -> AppSettings {
        self.model.borrow().app().clone()
    }
    pub fn subscribe(&self, effect: impl Fn(&AppSettings) -> Result<(), String> + 'static) {
        self.effects.borrow_mut().push(Box::new(effect));
    }
    fn notice(&self, text: String) {
        self.inline_notice(text.clone());
        if let Some(ui) = self.ui.upgrade() {
            crate::notice::error(&ui, text);
        }
    }
    fn inline_notice(&self, text: String) {
        if let Some(window) = self.window.borrow().as_ref() {
            window.global::<SettingsState>().set_notice(text.into());
        }
    }
    fn queue(self: &Rc<Self>, draft: AppSettings, delay: Duration) {
        if self.stopped.get() {
            return;
        }
        self.pending.replace(Some(draft));
        self.due.set(Some(Instant::now() + delay));
        self.valid.set(true);
        self.failed.set(false);
        self.arm(delay);
    }
    fn arm(self: &Rc<Self>, delay: Duration) {
        let weak = Rc::downgrade(self);
        self.timer.start(TimerMode::SingleShot, delay, move || {
            if let Some(owner) = weak.upgrade() {
                owner.flush();
            }
        });
    }
    pub fn change(self: &Rc<Self>, change: impl FnOnce(&mut AppSettings)) {
        // A quick media control changes the same latest draft as the window.
        let mut draft = self.pending.borrow().clone().unwrap_or_else(|| self.app());
        change(&mut draft);
        self.queue(draft, Duration::ZERO);
        self.flush();
    }
    fn flush(self: &Rc<Self>) {
        if self.stopped.get()
            || self.composing.get()
            || self.failed.get()
            || !self.valid.get() && !self.closing.get()
            || self.inflight.borrow().is_some()
        {
            return;
        }
        if let Some(due) = self.due.get()
            && due > Instant::now()
        {
            self.arm(due.saturating_duration_since(Instant::now()));
            return;
        }
        // Output completion wakes this queue; no polling while a picker,
        // font catalog, export or another owned file job is busy.
        if self.output.busy() {
            return;
        }
        let Some(draft) = self.pending.borrow_mut().take() else {
            if self.closing.get() {
                self.hide();
            } else if self.export_pending.replace(false) {
                self.profile(true);
            }
            return;
        };
        self.due.set(None);
        let baseline = self.model.borrow().edit_base().clone();
        if draft == baseline {
            // Restore effective UI choices even after a previous failed edit.
            if let Err(error) = self.publish_effects() {
                self.notice(error);
            }
            if self.closing.get() {
                self.hide();
            } else if self.export_pending.replace(false) {
                self.profile(true);
            }
            return;
        }
        let store = match self.model.borrow().store() {
            Ok(store) => store,
            Err(error) => {
                self.pending.replace(Some(draft));
                self.fail(error);
                return;
            }
        };
        let Some(ui) = self.ui.upgrade() else {
            self.pending.replace(Some(draft));
            return;
        };
        let result = Saved::new();
        self.inflight.replace(Some(InFlight {
            draft: draft.clone(),
            result: result.clone(),
        }));
        self.inline_notice(msg!("settings-saving"));
        let weak = Rc::downgrade(self);
        self.output.work(
            &ui,
            move || result.publish(store.save_from(&baseline, draft)),
            move |_, _, _| {
                if let Some(owner) = weak.upgrade() {
                    if let Err(error) = owner.collect(true) {
                        owner.fail(error);
                    }
                    if !owner.failed.get() {
                        owner.arm(Duration::ZERO);
                    }
                }
            },
        );
        // A thread/spawn failure is reported by Output without a completion.
        if !ui.get_output_busy() && self.inflight.borrow().is_some() {
            if let Some(job) = self.inflight.borrow_mut().take() {
                self.pending.replace(Some(job.draft));
            }
            self.fail(ui.get_input_notice().to_string());
        }
    }
    fn collect(&self, effects: bool) -> Result<(), String> {
        let Some(job) = self.inflight.borrow_mut().take() else {
            return Ok(());
        };
        let result = job.result.take();
        match result {
            Ok(saved) => {
                let mut model = self.model.borrow_mut();
                model.commit(saved)?;
                model.submitted(job.draft);
                drop(model);
                let applied = if effects {
                    self.publish_effects()
                } else {
                    Ok(())
                };
                if let Some(window) = self.window.borrow().as_ref() {
                    window.global::<SettingsState>().set_failed(false);
                }
                match applied {
                    Ok(()) => self.inline_notice(msg!("settings-saved")),
                    Err(error) => self.notice(format!("{} {error}", msg!("settings-saved"))),
                }
                Ok(())
            }
            Err(error) => {
                // Keep the newest edit; retry still uses the last successful
                // submitted baseline, including after an external writer.
                if self.pending.borrow().is_none() {
                    self.pending.replace(Some(job.draft));
                }
                Err(error)
            }
        }
    }
    fn publish_effects(&self) -> Result<(), String> {
        let app = self.app();
        let mut errors = Vec::new();
        for apply in self.effects.borrow().iter() {
            if let Err(error) = apply(&app) {
                errors.push(error);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
    fn fail(&self, error: String) {
        self.failed.set(true);
        self.notice(format!("{} {error}", msg!("settings-save-failed")));
        if let Some(window) = self.window.borrow().as_ref() {
            window.global::<SettingsState>().set_failed(true);
        }
    }
    /// App shutdown runs after its event loop. Join the exact Output worker,
    /// collect its saved result without a UI callback, then drain one latest
    /// authorized valid draft using the same Store/baseline. No detached tasks.
    pub fn finish(&self) -> Result<(), String> {
        self.stopped.set(true);
        let releases = self.finish_releases();
        self.timer.stop();
        self.search_focus.stop();
        self.search_index.borrow_mut().take();
        let joined = self.output.finish();
        let collected = self.collect(false);
        let pending = self.pending.borrow_mut().take();
        let saved = if joined.is_ok() && collected.is_ok() {
            pending
                .map(|draft| {
                    let result = (|| {
                        let model = self.model.borrow();
                        let saved = model.store()?.save_from(model.edit_base(), draft.clone())?;
                        drop(model);
                        let mut model = self.model.borrow_mut();
                        model.commit(saved)?;
                        model.submitted(draft.clone());
                        Ok::<_, String>(())
                    })();
                    if result.is_err() {
                        self.pending.replace(Some(draft));
                    }
                    result
                })
                .transpose()
        } else {
            self.pending.replace(pending);
            Ok(None)
        };
        self.hide();
        self.window.borrow_mut().take();
        joined?;
        collected?;
        saved?;
        releases?;
        Ok(())
    }
}
