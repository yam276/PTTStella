//! Native preview adapter: only links from the acknowledged filtered projection.
#[path = "media_hover.rs"]
mod hover;
#[path = "media_settings.rs"]
mod preferences;
#[path = "media_settings_probe.rs"]
mod preferences_probe;
#[path = "media_probe.rs"]
pub mod probe;
#[cfg(feature = "native-video")]
#[path = "media_video.rs"]
pub(crate) mod video;
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
    inline: RefCell<Option<Rc<crate::article::inline::Inline>>>,
    state: RefCell<State>,
    feedback: RefCell<Option<String>>,
    closed: Cell<bool>,
    probe: bool,
    presentation: Cell<Presentation>,
    dwell: slint::Timer,
    candidate: RefCell<Option<Scope>>,
    pub(crate) preferences: Rc<crate::preferences::Preferences>,
    #[cfg(feature = "native-video")]
    video: RefCell<Option<Rc<video::Video>>>,
}
impl Media {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        output: Rc<Output>,
        probe: bool,
        preferences: pttstella::settings::preferences::Preferences,
    ) -> Rc<Self> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            connection,
            output: output.clone(),
            active: RefCell::new(None),
            sequence: Cell::new(0),
            worker: RefCell::new(None),
            task: RefCell::new(None),
            inline: RefCell::new(None),
            state: RefCell::new(State::default()),
            feedback: RefCell::new(None),
            closed: Cell::new(false),
            probe,
            presentation: Cell::new(Presentation::Hover),
            dwell: slint::Timer::default(),
            candidate: RefCell::new(None),
            preferences: crate::preferences::Preferences::mount(ui, output.clone(), preferences),
            #[cfg(feature = "native-video")]
            video: RefCell::new(None),
        });
        #[cfg(feature = "native-video")]
        match video::Video::mount(ui, Rc::downgrade(&owner)) {
            Ok(video) => {
                owner.video.replace(Some(video));
            }
            Err(error) => ui.set_input_notice(error.into()),
        }
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
        owner.mount_preferences(ui);
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
        worker.inline_automatic(self.preferences.app().auto_expand_images);
        let mut inline_updates = worker.inline_updates();
        let weak = Rc::downgrade(self);
        let task = slint::spawn_local(async move {
            let (mut preview_dirty, mut inline_dirty) = (true, true);
            loop {
                if let Some(owner) = weak.upgrade() {
                    if preview_dirty {
                        owner.install(updates.borrow_and_update().clone());
                    }
                    if inline_dirty {
                        let state = inline_updates.borrow_and_update().clone();
                        let inline = owner.inline.borrow().clone();
                        if let Some(inline) = inline {
                            inline.install(state);
                        }
                    }
                } else {
                    break;
                }
                let (preview, closed) = tokio::select! {r=updates.changed()=>(true,r.is_err()),r=inline_updates.changed()=>(false,r.is_err())};
                preview_dirty = preview;
                inline_dirty = !preview;
                if closed {
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
    pub fn mount_inline(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        article: &Rc<crate::article::session::ArticleSession>,
    ) {
        let inline = crate::article::inline::Inline::mount(
            ui,
            article,
            self,
            self.connection.clone(),
            self.preferences.app().auto_expand_images,
        );
        self.inline.replace(Some(inline));
        article.attach_presentation_media(self);
    }
    pub fn rebase_reply_focus(&self, old: (u64, u64, u64), new: (u64, u64, u64)) {
        if let Some(inline) = self.inline_owner() {
            inline.rebase_reply_focus(old, new);
        }
        self.close(false);
    }
    pub fn inline_owner(&self) -> Option<Rc<crate::article::inline::Inline>> {
        self.inline.borrow().clone()
    }
    pub fn inline_picture(&self, index: i32, tag: &str) -> Option<crate::context_menu::Picture> {
        let index = usize::try_from(index).ok()?;
        let inline = self.inline_owner()?;
        if !inline.picture_visible(index, tag) {
            return None;
        }
        let ui = self.ui.upgrade()?;
        let scope = self.output.media_lease(&ui, index, 0)?;
        let image = inline.original(&scope);
        Some(crate::context_menu::Picture {
            scope,
            image,
            probe: self.probe,
            tag: tag.to_owned(),
        })
    }
    pub fn inline_scope(&self, epoch: u64) {
        if let Some(worker) = self.worker.borrow().as_ref() {
            worker.inline_scope(epoch);
        }
    }
    pub fn inline_controls(&self, controls: crate::inline_engine::Controls) {
        if let Some(worker) = self.worker.borrow().as_ref() {
            worker.inline_controls(controls);
        }
    }
    pub fn inline_command(self: &Rc<Self>, epoch: u64, command: Command) -> Result<(), String> {
        self.ensure_worker()?;
        let worker = self.worker.borrow();
        let worker = worker.as_ref().ok_or("media worker missing")?;
        worker.inline_scope(epoch);
        worker.command(command)
    }
    pub fn open_inline(
        self: &Rc<Self>,
        index: usize,
        image: std::sync::Arc<pttstella::media::loader::Image>,
    ) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if let Some(scope) = self.output.media_lease(&ui, index, 0) {
            self.begin_original(&ui, scope, Presentation::Inspect, Some(image));
        }
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
                let original = self
                    .inline
                    .borrow()
                    .as_ref()
                    .and_then(|i| i.original(&scope));
                self.begin_original(&ui, scope, Presentation::Inspect, original)
            }
            #[cfg(feature = "native-video")]
            Ok(Some(target)) if target.kind == Kind::Video && self.video.borrow().is_some() => {
                self.begin_video(&ui, scope);
            }
            Ok(_) => ui.invoke_link_activated(scope.link.url.into()),
            Err(e) => ui.set_input_notice(e.into()),
        }
    }
    fn begin(self: &Rc<Self>, ui: &TerminalWindow, scope: Scope, presentation: Presentation) {
        self.begin_original(ui, scope, presentation, None)
    }
    fn begin_original(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        mut scope: Scope,
        presentation: Presentation,
        original: Option<std::sync::Arc<pttstella::media::loader::Image>>,
    ) {
        if !self.output.renew_media(ui, &mut scope) {
            return;
        }
        let target = match media::classify(&scope.link.url) {
            Ok(Some(target)) if target.kind == Kind::Image => target,
            _ => return,
        };
        #[cfg(feature = "native-video")]
        if let Some(video) = self.video.borrow().as_ref() {
            video.close();
        }
        ui.set_media_video(false);
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
        ui.invoke_inline_updated();
        let result = if let Some(worker) = self.worker.borrow().as_ref() {
            worker.scope(request, ui.get_media_play_allowed());
            worker.command(if let Some(image) = original {
                Command::OpenOriginal { request, image }
            } else if promote {
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
        #[cfg(feature = "native-video")]
        if ui.get_media_video() {
            match action {
                1 => self.begin_video(&ui, scope),
                2 => {
                    if let Some(video) = self.video.borrow().as_ref() {
                        video.toggle();
                    }
                }
                3 => ui.invoke_link_activated(scope.link.url.into()),
                6 => self.picture_output(&ui, &scope, 6),
                _ => {}
            }
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
        use crate::output::picture::PictureAction;
        let action = match action {
            4 => PictureAction::Copy,
            5 => PictureAction::Save,
            6 => PictureAction::CopyUrl,
            _ => return,
        };
        let image = self.state.borrow().original.clone();
        let request = scope.sequence;
        let weak = Rc::downgrade(self);
        self.output
            .picture_output(ui, scope, image, action, self.probe, move |ui, message| {
                if let Some(owner) = weak.upgrade() {
                    let scope = owner.active.borrow().clone();
                    if let Some(scope) = scope
                        && scope.sequence == request
                        && owner.valid(ui, &scope)
                    {
                        owner.feedback(ui, message);
                    }
                }
            });
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
        #[cfg(feature = "native-video")]
        if ui.get_media_video() {
            if let Some(video) = self.video.borrow().as_ref() {
                video.permit(ui.get_media_play_allowed());
            }
            return;
        }
        if let Some(worker) = self.worker.borrow().as_ref() {
            worker.scope(scope.sequence, ui.get_media_play_allowed());
        }
    }
    fn close(&self, focus: bool) {
        self.dwell.stop();
        self.candidate.borrow_mut().take();
        #[cfg(feature = "native-video")]
        if let Some(video) = self.video.borrow().as_ref() {
            video.close();
        }
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
            ui.set_media_video(false);
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
        if let Some(inline) = self.inline.borrow().as_ref() {
            inline.finish();
        }
        self.closed.set(true);
        self.close(false);
        #[cfg(feature = "native-video")]
        if let Some(video) = self.video.borrow().as_ref() {
            video.finish()?;
        }
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        if let Some(mut worker) = self.worker.borrow_mut().take() {
            worker.finish()?;
        }
        Ok(())
    }
    pub async fn shutdown(&self) -> Result<(), String> {
        self.closed.set(true);
        self.close(false);
        #[cfg(feature = "native-video")]
        {
            let video = self.video.borrow().clone();
            if let Some(video) = video {
                video.shutdown().await?;
            }
        }
        Ok(())
    }
    #[cfg(feature = "native-video")]
    fn video_valid(&self, sequence: u64) -> bool {
        let scope = self.active.borrow().clone();
        let Some(ui) = self.ui.upgrade() else {
            return false;
        };
        scope.is_some_and(|scope| scope.sequence == sequence && self.valid(&ui, &scope))
    }
    #[cfg(feature = "native-video")]
    fn begin_video(&self, ui: &TerminalWindow, mut scope: Scope) {
        if !self.output.renew_media(ui, &mut scope) {
            return;
        }
        let Some(sequence) = self.sequence.get().checked_add(1) else {
            ui.set_input_notice(msg!("media-sequence-exhausted").into());
            return;
        };
        self.close(false);
        self.sequence.set(sequence);
        scope.sequence = sequence;
        self.active.replace(Some(scope.clone()));
        self.presentation.set(Presentation::Inspect);
        self.output.revoke();
        self.output.prepare_context(ui);
        ui.invoke_reset_keyboard();
        ui.set_media_key(sequence.to_string().into());
        ui.set_media_url(scope.link.url.clone().into());
        ui.set_media_image(slint::Image::default());
        ui.set_media_animated(false);
        ui.set_media_playing(false);
        ui.set_media_ready(false);
        ui.set_media_video(true);
        ui.set_media_position(0.);
        ui.set_media_duration(0.);
        ui.set_media_motion_label(msg!("video-play").into());
        ui.set_media_status(msg!("video-ready").into());
        ui.set_media_open(true);
        ui.invoke_focus_media();
        ui.invoke_inline_updated();
        if let Some(video) = self.video.borrow().as_ref() {
            video.open(sequence, scope.link.url);
        }
    }
}
