//! Native preview adapter: only links from the acknowledged filtered projection.
#[path = "media_hover.rs"]
mod hover;
#[path = "media_probe.rs"]
pub mod probe;
use crate::{
    TerminalWindow,
    media_worker::{Command, State, Worker},
    output::Output,
};
use pttstella::{
    connection::Connection,
    media::hover::Lease as Scope,
    media::preview::Presentation,
    media::{self, Kind},
    msg,
};
use slint::ComponentHandle;
use slint::winit_030::WinitWindowAccessor;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub struct Media {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    output: Rc<Output>,
    active: RefCell<Option<Scope>>,
    sequence: Cell<u64>,
    worker: RefCell<Option<Worker>>,
    task: RefCell<Option<slint::JoinHandle<()>>>,
    state: RefCell<State>,
    feedback: RefCell<Option<String>>,
    closed: Cell<bool>,
    probe: bool,
    presentation: Cell<Presentation>,
    dwell: slint::Timer,
    candidate: RefCell<Option<Scope>>,
    settings: RefCell<pttstella::settings::AppSettings>,
    store: pttstella::settings::store::Store,
}
impl Media {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        output: Rc<Output>,
        probe: bool,
        settings: pttstella::settings::AppSettings,
        store: pttstella::settings::store::Store,
    ) -> Rc<Self> {
        ui.set_media_close_label(msg!("close").into());
        ui.set_media_retry_label(msg!("retry").into());
        ui.set_media_external_label(msg!("media-external").into());
        ui.set_media_fit_label(msg!("media-fit").into());
        ui.set_media_original_label(msg!("media-original").into());
        ui.set_media_copy_label(msg!("context-copy-picture").into());
        ui.set_media_save_label(msg!("context-save-picture").into());
        ui.set_media_copy_url_label(msg!("context-copy-link").into());
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            connection,
            output,
            active: RefCell::new(None),
            sequence: Cell::new(0),
            worker: RefCell::new(None),
            task: RefCell::new(None),
            state: RefCell::new(State::default()),
            feedback: RefCell::new(None),
            closed: Cell::new(false),
            probe,
            presentation: Cell::new(Presentation::Hover),
            dwell: slint::Timer::default(),
            candidate: RefCell::new(None),
            settings: RefCell::new(settings),
            store,
        });
        ui.on_media_request({
            let owner = owner.clone();
            move |index| owner.open(index)
        });
        ui.on_media_action({
            let owner = owner.clone();
            move |action| owner.action(action)
        });
        ui.on_media_updated({
            let owner = owner.clone();
            move || owner.update()
        });
        owner.mount_hover(ui);
        owner
    }
    fn valid(&self, ui: &TerminalWindow, scope: &Scope) -> bool {
        if self.closed.get()
            || self.connection.updates.borrow().generation != scope.stamp.generation
        {
            return false;
        }
        let mut renewed = scope.clone();
        if !self.output.renew_media(ui, &mut renewed) {
            return false;
        }
        if self
            .active
            .borrow()
            .as_ref()
            .is_some_and(|active| active.sequence == scope.sequence)
        {
            self.active.replace(Some(renewed));
        }
        true
    }
    fn ensure_worker(self: &Rc<Self>) -> Result<(), String> {
        if self.worker.borrow().is_some() {
            return Ok(());
        }
        let (mut worker, mut updates) = Worker::start()?;
        let weak = Rc::downgrade(self);
        let task = slint::spawn_local(async move {
            loop {
                let state = updates.borrow_and_update().clone();
                if let Some(owner) = weak.upgrade() {
                    owner.install(state);
                } else {
                    break;
                }
                if updates.changed().await.is_err() {
                    if let Some(owner) = weak.upgrade()
                        && !owner.closed.get()
                        && let Some(ui) = owner.ui.upgrade()
                        && owner.active.borrow().is_some()
                    {
                        owner.state.replace(State::default());
                        ui.set_media_ready(false);
                        ui.set_media_playing(false);
                        ui.set_media_animated(false);
                        ui.set_media_image(slint::Image::default());
                        ui.set_media_status(msg!("image-prepare-failed").into());
                    }
                    break;
                }
            }
        });
        let task = match task {
            Ok(task) => task,
            Err(error) => {
                return Err(match worker.finish() {
                    Ok(()) => error.to_string(),
                    Err(cleanup) => format!("{error}; {cleanup}"),
                });
            }
        };
        self.worker.replace(Some(worker));
        self.task.replace(Some(task));
        Ok(())
    }
    fn open(self: &Rc<Self>, index: i32) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.closed.get() || ui.get_context_source_blocked() {
            return;
        }
        let Some(scope) = usize::try_from(index)
            .ok()
            .and_then(|i| self.output.media_lease(&ui, i, 0))
        else {
            return;
        };
        match media::classify(&scope.link.url) {
            Ok(Some(target)) if target.kind == Kind::Image => {
                self.begin(&ui, scope, Presentation::Inspect)
            }
            Ok(_) => ui.invoke_link_activated(scope.link.url.into()),
            Err(e) => ui.set_input_notice(e.into()),
        }
    }
    fn begin(self: &Rc<Self>, ui: &TerminalWindow, mut scope: Scope, presentation: Presentation) {
        if !self.output.renew_media(ui, &mut scope) {
            return;
        }
        let target = match media::classify(&scope.link.url) {
            Ok(Some(target)) if target.kind == Kind::Image => target,
            _ => return,
        };
        let previous = self.active.borrow().clone();
        let promote = presentation == Presentation::Inspect
            && self.presentation.get() == Presentation::Hover
            && previous.as_ref().is_some_and(|old| old.same_source(&scope));
        let Some(request) = (if promote {
            previous.as_ref().map(|old| old.sequence)
        } else {
            self.sequence.get().checked_add(1)
        }) else {
            ui.set_input_notice(msg!("media-sequence-exhausted").into());
            return;
        };
        if let Err(e) = self.ensure_worker() {
            ui.set_input_notice(e.into());
            return;
        }
        self.dwell.stop();
        self.candidate.borrow_mut().take();
        self.sequence.set(request);
        scope.sequence = request;
        let generation = scope.stamp.generation;
        self.presentation.set(presentation);
        self.active.replace(Some(scope));
        if presentation == Presentation::Inspect {
            self.output.revoke();
            self.output.prepare_context(ui);
            ui.invoke_reset_keyboard();
        }
        ui.set_media_key(request.to_string().into());
        self.feedback.borrow_mut().take();
        ui.set_media_url(target.url.clone().into());
        if !promote {
            self.state.replace(State::default());
            ui.set_media_ready(false);
            ui.set_media_animated(false);
            ui.set_media_playing(false);
            ui.set_media_image(slint::Image::default());
            ui.set_media_status(msg!("image-loading").into());
        }
        ui.set_media_hover_open(presentation == Presentation::Hover);
        ui.set_media_open(presentation == Presentation::Inspect);
        if presentation == Presentation::Inspect {
            ui.invoke_focus_media();
        }
        let result = if let Some(worker) = self.worker.borrow().as_ref() {
            worker.scope(request, ui.get_media_play_allowed());
            worker.command(if promote {
                Command::Promote(request)
            } else {
                Command::Open {
                    request,
                    generation,
                    target,
                    presentation,
                }
            })
        } else {
            Err(msg!("image-prepare-failed"))
        };
        if let Err(error) = result {
            self.close(presentation == Presentation::Inspect);
            if presentation == Presentation::Inspect {
                ui.set_input_notice(error.into());
            }
        }
    }
    fn install(&self, state: State) {
        let Some(ui) = self.ui.upgrade() else { return };
        let Some(scope) = self.active.borrow().clone() else {
            return;
        };
        if !self.valid(&ui, &scope) {
            self.close(false);
            return;
        }
        if scope.sequence != state.request {
            return;
        }
        ui.set_media_ready(state.pixels.is_some() && state.error.is_none());
        ui.set_media_animated(state.original.as_ref().is_some_and(|i| i.animated_format));
        ui.set_media_playing(state.playing);
        ui.set_media_motion_label(
            if state.playing {
                msg!("media-stop")
            } else {
                msg!("media-play")
            }
            .into(),
        );
        let previous = self.state.borrow();
        let changed = (previous.request, previous.frame, previous.pixels.is_some())
            != (state.request, state.frame, state.pixels.is_some());
        drop(previous);
        if changed {
            ui.set_media_image(
                state
                    .pixels
                    .clone()
                    .map(slint::Image::from_rgba8)
                    .unwrap_or_default(),
            );
        }
        ui.set_media_status(
            state
                .error
                .clone()
                .or_else(|| self.feedback.borrow().clone())
                .unwrap_or_else(|| {
                    if state.loading {
                        msg!("image-loading")
                    } else if state.cached {
                        msg!("image-ready-cached")
                    } else {
                        msg!("image-ready")
                    }
                })
                .into(),
        );
        self.state.replace(state);
    }
    fn action(self: &Rc<Self>, action: i32) {
        if action == 0 {
            self.close(true);
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        if !ui.get_native_foreground() || self.output.busy() {
            return;
        }
        let Some(scope) = self.active.borrow().clone() else {
            return;
        };
        if !self.valid(&ui, &scope) {
            self.close(false);
            return;
        }
        let command = match action {
            1 => {
                match media::classify(&scope.link.url) {
                    Ok(Some(target)) if target.kind == Kind::Image => {
                        self.begin(&ui, scope.clone(), Presentation::Inspect)
                    }
                    _ => self.close(false),
                }
                None
            }
            2 => Some(Command::Motion(
                scope.sequence,
                !self.state.borrow().playing,
            )),
            3 => {
                ui.invoke_link_activated(scope.link.url.into());
                None
            }
            4..=6 => {
                self.picture_output(&ui, &scope, action);
                None
            }
            _ => None,
        };
        if let Some(command) = command
            && let Some(worker) = self.worker.borrow().as_ref()
            && let Err(e) = worker.command(command)
        {
            ui.set_media_status(e.into());
        }
    }
    fn picture_output(self: &Rc<Self>, ui: &TerminalWindow, scope: &Scope, action: i32) {
        let image = self.state.borrow().original.clone();
        if action != 6 && image.is_none() {
            self.feedback(ui, msg!("copy-not-ready"));
            return;
        }
        let (lease, phase) = match self.output.media_output_lease(ui, scope) {
            Ok(lease) => lease,
            Err(error) => {
                self.feedback(ui, error);
                return;
            }
        };
        let path = if action == 5 {
            let extension = match image.as_ref().map(|image| media::picture::extension(image)) {
                Some(Ok(extension)) => extension,
                _ => {
                    self.feedback(ui, msg!("image-invalid"));
                    return;
                }
            };
            let path = ui
                .window()
                .with_winit_window(|window| {
                    rfd::FileDialog::new()
                        .set_parent(window)
                        .set_title(msg!("picture-save-title"))
                        .add_filter("Image", &[extension])
                        .set_file_name(format!("PTTStella-image.{extension}"))
                        .save_file()
                })
                .flatten();
            let Some(path) = path else {
                self.feedback(ui, msg!("export-cancelled"));
                return;
            };
            Some(path)
        } else {
            None
        };
        let clipboard = self.output.clipboard();
        let source = self.connection.updates.clone();
        let url = scope.link.url.clone();
        let request = scope.sequence;
        let probe = self.probe;
        let weak = Rc::downgrade(self);
        self.output.work(
            ui,
            move || {
                let verify = || crate::output::check_output(&lease, &source.borrow(), phase);
                verify()?;
                if action == 6 {
                    clipboard.write_text(url.clone(), verify)?;
                    if probe {
                        crate::media_fixture::verify_url(&clipboard, &url)?;
                    }
                    return Ok(());
                }
                let image = image.ok_or_else(|| msg!("copy-not-ready"))?;
                if let Some(path) = path {
                    media::picture::save(&path, &image, verify)
                } else {
                    let mime = image.mime;
                    let pixels = media::picture::pixels(image)?;
                    clipboard.write_image(pixels, verify)?;
                    if probe {
                        crate::media_fixture::verify_picture(&clipboard, mime)?;
                    }
                    Ok(())
                }
            },
            move |_, ui, result| {
                if let Some(owner) = weak.upgrade() {
                    let scope = owner.active.borrow().clone();
                    if let Some(scope) = scope
                        && scope.sequence == request
                        && owner.valid(ui, &scope)
                    {
                        owner.feedback(
                            ui,
                            match result {
                                Ok(()) if action == 5 => msg!("picture-saved"),
                                Ok(()) if action == 6 => msg!("context-copied"),
                                Ok(()) => msg!("export-copied"),
                                Err(error) => error,
                            },
                        );
                    }
                }
            },
        );
    }
    fn feedback(&self, ui: &TerminalWindow, message: String) {
        self.feedback.replace(Some(message.clone()));
        ui.set_media_status(message.into());
    }
    fn update(self: &Rc<Self>) {
        if self.closed.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        if self.presentation.get() == Presentation::Hover {
            self.hover_changed();
            return;
        }
        let Some(scope) = self.active.borrow().clone() else {
            return;
        };
        if !self.valid(&ui, &scope) {
            self.close(false);
            return;
        }
        if let Some(worker) = self.worker.borrow().as_ref() {
            worker.scope(scope.sequence, ui.get_media_play_allowed());
        }
    }
    fn close(&self, focus: bool) {
        self.dwell.stop();
        self.candidate.borrow_mut().take();
        let active = self.active.borrow_mut().take();
        if active.is_none() {
            return;
        }
        if self.presentation.replace(Presentation::Hover) == Presentation::Inspect {
            self.output.revoke();
        }
        self.state.replace(State::default());
        self.feedback.borrow_mut().take();
        if let Some(worker) = self.worker.borrow().as_ref() {
            worker.scope(0, false);
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.set_media_hover_open(false);
            ui.set_media_open(false);
            ui.set_media_ready(false);
            ui.set_media_image(slint::Image::default());
            ui.set_media_animated(false);
            ui.set_media_playing(false);
            if focus {
                ui.invoke_focus_terminal();
            }
        }
    }
    pub fn finish(&self) -> Result<(), String> {
        self.closed.set(true);
        self.close(false);
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        if let Some(mut worker) = self.worker.borrow_mut().take() {
            worker.finish()?;
        }
        Ok(())
    }
}
