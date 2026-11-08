//! One native article owner: shared reducer, one command lease, one loop deadline.
use super::ArticleView;
use crate::{
    TerminalWindow,
    renderer::{Renderer, Size},
    session::FrameId,
};
use pttstella::{
    connection::{Connection, NavigationCancellation, Phase, Reply, View},
    filters,
    settings::{AppSettings, DisplaySettings},
    surface_state::{Identity, SurfaceState},
};
use slint::{ComponentHandle, Timer};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    time::Duration,
};
use tokio::{sync::Notify, time::Instant};

type ReplyWait = Pin<Box<dyn Future<Output = Result<(), String>>>>;
#[path = "find.rs"]
mod find;
#[path = "input.rs"]
mod input;
#[path = "outputs.rs"]
mod outputs;

pub struct ArticleSession {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    pub(super) core: Rc<RefCell<SurfaceState>>,
    display: RefCell<DisplaySettings>,
    renderer: Rc<RefCell<Renderer>>,
    terminal_size: Rc<Cell<Option<Size>>>,
    terminal_presented: Rc<Cell<Option<FrameId>>>,
    terminal_pending: Cell<Option<Identity>>,
    terminal_shown: Cell<Option<Identity>>,
    pub(super) view: RefCell<Option<Rc<ArticleView>>>,
    permit: Cell<Option<(u64, u64, u64)>>,
    shown: Cell<Option<(u64, u64, u64)>>,
    notify: Rc<Notify>,
    job: RefCell<Option<slint::JoinHandle<()>>>,
    reply: RefCell<Option<Reply>>,
    cancellation: RefCell<Option<NavigationCancellation>>,
    busy: Cell<bool>,
    serial: Cell<u64>,
    next_paint: Cell<Instant>,
    closed: Cell<bool>,
    loop_done: Cell<bool>,
    stopped_notify: Notify,
    composition_handoff: Cell<Option<u64>>,
    vim_handoff: Cell<bool>,
    raw_generation: Cell<Option<u64>>,
    focus_after_paint: Cell<bool>,
    input_trace_left: Cell<u8>,
    timing: RefCell<pttstella::reading_timing::ReadingTiming>,
    find_timer: Timer,
    find_draft: RefCell<Option<String>>,
    find_serial: Cell<u64>,
    find_version: Cell<u64>,
    pub requests: Cell<usize>,
    close_media: RefCell<Option<std::rc::Weak<crate::media::Media>>>,
}
impl ArticleSession {
    pub fn preferences_changed(
        &self,
        settings: &pttstella::settings::AppSettings,
    ) -> Result<(), String> {
        self.display.replace(settings.display.resolve()?);
        if let Some(view) = self.view.borrow().as_ref() {
            view.copy_display(&self.renderer.borrow());
        }
        let mut core = self.core.borrow_mut();
        core.set_article_mode(settings.article_mode);
        core.set_article_boundary_navigation(settings.article_boundary_navigation);
        core.display_changed();
        drop(core);
        self.terminal_size.set(None);
        self.notify.notify_one();
        Ok(())
    }

    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        renderer: Rc<RefCell<Renderer>>,
        terminal_size: Rc<Cell<Option<Size>>>,
        terminal_presented: Rc<Cell<Option<FrameId>>>,
        settings: &AppSettings,
    ) -> Result<Rc<Self>, String> {
        let display = settings.display.resolve()?;
        let mut core = SurfaceState::new(filters::Rules::default());
        core.set_article_mode(settings.article_mode);
        core.set_article_boundary_navigation(settings.article_boundary_navigation);
        let owner = Rc::new(Self {
            close_media: RefCell::new(None),
            ui: ui.as_weak(),
            connection,
            core: Rc::new(RefCell::new(core)),
            display: RefCell::new(display),
            renderer,
            terminal_size,
            terminal_presented,
            terminal_pending: Cell::new(None),
            terminal_shown: Cell::new(None),
            view: RefCell::new(None),
            permit: Cell::new(None),
            shown: Cell::new(None),
            notify: Rc::new(Notify::new()),
            job: RefCell::new(None),
            reply: RefCell::new(None),
            cancellation: RefCell::new(None),
            busy: Cell::new(false),
            serial: Cell::new(0),
            next_paint: Cell::new(Instant::now()),
            closed: Cell::new(false),
            loop_done: Cell::new(false),
            stopped_notify: Notify::new(),
            composition_handoff: Cell::new(None),
            vim_handoff: Cell::new(false),
            raw_generation: Cell::new(None),
            focus_after_paint: Cell::new(false),
            input_trace_left: Cell::new(
                if std::env::var_os("PTTSTELLA_SLINT_INPUT_TRACE").is_some() {
                    128
                } else {
                    0
                },
            ),
            find_timer: Timer::default(),
            find_draft: RefCell::new(None),
            find_serial: Cell::new(0),
            find_version: Cell::new(0),
            timing: RefCell::new(pttstella::reading_timing::ReadingTiming::new()?),
            requests: Cell::new(0),
        });
        owner.install_find(ui);
        ui.on_article_rendered({
            let owner = owner.clone();
            move || owner.rendered()
        });
        ui.on_article_session_updated({
            let owner = owner.clone();
            move || owner.notify.notify_one()
        });
        ui.on_article_resume_reading({
            let owner = owner.clone();
            move || owner.resume_reading()
        });
        ui.on_article_keyboard_mode({
            let owner = owner.clone();
            move |raw| {
                if raw && !owner.vim_handoff.replace(true) {
                    owner.station_handoff();
                } else if !raw && owner.vim_handoff.replace(false) {
                    owner.resume_reading();
                }
            }
        });
        ui.on_article_load_action({
            let owner = owner.clone();
            move |resume| {
                if owner.closed.get() {
                    return;
                }
                let Some(scope) = owner.shown.get() else {
                    return;
                };
                let view = owner.connection.updates.borrow().clone();
                if owner.core.borrow().reading_scope() != Some(scope) {
                    return;
                }
                if owner
                    .core
                    .borrow_mut()
                    .full_load_action(scope.0, scope.1, resume, &view)
                {
                    if !resume {
                        owner.cancel_command();
                    }
                    owner.focus_after_paint.set(true);
                    owner.next_paint.set(Instant::now());
                    owner.notify.notify_one();
                }
            }
        });
        let weak = Rc::downgrade(&owner);
        let notify = owner.notify.clone();
        let mut updates = owner.connection.updates.clone();
        let job=slint::spawn_local(async move {
            let mut changed=true;
            // The reducer and queue must consume the same immutable snapshot.
            let mut observed=None;
            let mut pending:Option<(u64,ReplyWait)>=None;
            loop {
                let Some(owner)=weak.upgrade() else {break};if owner.closed.get(){break;}
                if pending.as_ref().is_some_and(|(serial,_)|*serial!=owner.serial.get()){pending=None;}
                if changed {let view=updates.borrow_and_update().clone();owner.observe(&view);observed=Some(view);}
                if pending.as_ref().is_some_and(|(serial,_)|*serial!=owner.serial.get()){pending=None;}
                if let Some(view)=observed.as_ref(){owner.step(view);}
                if let Some(reply)=owner.reply.take() {pending=Some((owner.serial.get(),Box::pin(reply.wait())));}
                let deadline=owner.deadline();drop(owner);
                tokio::select! {
                    result=updates.changed()=>{if result.is_err(){break;}changed=true;}
                    _=notify.notified()=>{changed=false;}
                    result=async {if let Some((serial,reply))=pending.as_mut(){(*serial,reply.await)}else{std::future::pending::<(u64,Result<(),String>)>().await}}=>{
                        pending=None;changed=false;
                        if let Some(owner)=weak.upgrade() && !owner.closed.get() && owner.serial.get()==result.0 {
                            owner.busy.set(false);
                            if let Some(cancel)=owner.cancellation.take() && result.1.is_err(){cancel.cancel();}
                            if let Err(error)=result.1 {owner.core.borrow_mut().stop_full_load();owner.notice(error);}
                        }
                        notify.notify_one();
                    }
                    _=async {if let Some(at)=deadline {tokio::time::sleep_until(at).await;}else{std::future::pending::<()>().await;}}=>{changed=false;}
                }
                // A constantly ready watch must still yield to native input.
                tokio::task::yield_now().await;
            }
            drop(pending);drop(updates);
            if let Some(owner)=weak.upgrade() {owner.loop_done.set(true);owner.stopped_notify.notify_waiters();}
        }).map_err(|e|e.to_string())?;
        owner.job.replace(Some(job));
        ui.window().on_close_requested({
            let owner = Rc::downgrade(&owner);
            move || {
                if let Some(owner) = owner.upgrade()
                    && !owner.closed.get()
                {
                    owner.finish();
                    let closer = owner.clone();
                    match slint::spawn_local(async move {
                        closer.shutdown().await;
                        let media = closer
                            .close_media
                            .borrow()
                            .as_ref()
                            .and_then(|m| m.upgrade());
                        if let Some(media) = media
                            && let Err(error) = media.shutdown().await
                        {
                            closer.notice(error);
                            return;
                        }
                        if let Some(ui) = closer.ui.upgrade() {
                            let _ = ui.hide();
                        }
                        if let Err(error) = slint::quit_event_loop() {
                            closer.notice(error.to_string());
                        }
                    }) {
                        Ok(_) => {}
                        Err(error) => {
                            owner.notice(error.to_string());
                        }
                    }
                }
                slint::CloseRequestResponse::KeepWindowShown
            }
        });
        Ok(owner)
    }
    pub fn attach_media(&self, media: &Rc<crate::media::Media>) {
        self.close_media.replace(Some(Rc::downgrade(media)));
    }
    fn allowed(&self, ui: &TerminalWindow) -> bool {
        ui.get_article_reading_ready()
            && self.permit.get().is_some()
            && self.permit.get() == self.core.borrow().reading_scope()
    }
    fn observe(&self, view: &View) {
        if self
            .composition_handoff
            .get()
            .is_some_and(|generation| generation != view.generation)
        {
            self.composition_handoff.set(None);
        }
        if self.raw_generation.get().is_some_and(|generation| {
            generation != view.generation
                || (view.phase == Phase::Connected
                    && !view.output_pending
                    && !view.input_pending
                    && view.frame.control_complete
                    && !matches!(
                        view.context(),
                        pttstella::navigation::Context::ArticleReading { .. }
                    ))
        }) && !self.vim_handoff.get()
            && self.composition_handoff.get().is_none()
        {
            self.raw_generation.set(None);
            self.core.borrow_mut().set_raw_reading(false);
            if let Some(ui) = self.ui.upgrade() {
                ui.set_article_station(false);
            }
        }
        self.core.borrow_mut().update(view);
        self.reading_timing("cached", view);
        self.retire_stale_projection(view);
        self.find_sync_ui();
        if let Some(ui) = self.ui.upgrade() {
            ui.invoke_inline_updated();
        }
    }
    fn retire_stale_projection(&self, view: &View) {
        let (no_article, scope_changed, stale_packet) = {
            let core = self.core.borrow();
            (
                core.reading_scope().is_none(),
                self.shown
                    .get()
                    .is_some_and(|scope| Some(scope) != core.reading_scope()),
                core.pending_identity().is_some_and(|p| {
                    p.generation != view.generation
                        || p.presentation != core.presentation()
                        || (!view.output_pending && p.revision != view.frame.revision)
                }),
            )
        };
        if no_article || scope_changed {
            self.permit.set(None);
            self.shown.set(None);
            self.cancel_command();
            if let Some(view) = self.view.borrow().as_ref() {
                view.hide();
            }
        }
        if stale_packet {
            if let Some(view) = self.view.borrow().as_ref() {
                view.retire_paint();
            }
            self.terminal_pending.set(None);
            self.terminal_shown.set(None);
            let pending = self.core.borrow().pending_identity();
            if let Some(p) = pending {
                // Drain an obsolete packet; current() still rejects its source.
                let _ = self.core.borrow_mut().acknowledge(p);
            }
        }
    }
    pub fn terminal_matches(&self, frame: FrameId) -> bool {
        self.terminal_pending
            .get()
            .or(self.terminal_shown.get())
            .is_some_and(|p| (p.generation, p.revision) == (frame.generation, frame.revision))
    }
    pub fn terminal_rendered(&self) {
        let Some(identity) = self.terminal_pending.get() else {
            return;
        };
        if self.closed.get()
            || self.terminal_presented.get()
                != Some(FrameId {
                    generation: identity.generation,
                    revision: identity.revision,
                })
        {
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        if ui.get_article_active() || self.terminal_size.get().is_none() {
            return;
        }
        if self.core.borrow().pending_identity() != Some(identity) {
            return;
        }
        if let Err(error) = self.core.borrow_mut().acknowledge(identity) {
            self.notice(error);
            return;
        }
        self.terminal_pending.set(None);
        self.terminal_shown.set(Some(identity));
        self.core
            .borrow_mut()
            .find_sync(&self.connection.updates.borrow());
        self.find_sync_ui();
        self.find_retry_draft();
        ui.invoke_output_updated();
        ui.invoke_paste_updated();
        ui.invoke_keep_alive_changed();
        self.notify.notify_one();
    }
    fn reading_timing(&self, event: &str, view: &View) {
        let result = if event == "painted" {
            self.timing
                .borrow_mut()
                .painted(self.core.borrow().last_displayed())
        } else {
            self.timing.borrow_mut().record(
                event,
                view.generation,
                view.frame.revision,
                self.core.borrow().reading_progress(),
            )
        };
        if let Err(error) = result {
            self.notice(error);
        }
    }
    fn rendered(&self) {
        if self.closed.get() {
            return;
        }
        let pending = self.core.borrow().pending_identity();
        if let Some(identity) = pending
            && let Err(error) = self.core.borrow_mut().acknowledge(identity)
        {
            self.notice(error);
            return;
        }
        // Native callbacks may read the watch again while the writer is queued.
        // Never retain a watch read guard across a reentrant UI callback.
        let view = self.connection.updates.borrow().clone();
        let generation = view.generation;
        self.reading_timing("painted", &view);
        self.core.borrow_mut().find_sync(&view);
        self.find_sync_ui();
        self.find_retry_draft();
        let scope = {
            let core = self.core.borrow();
            core.last_displayed().and_then(|p| {
                p.article.as_ref().and_then(|article| {
                    core.authorize_reading(
                        pttstella::navigation::Stamp {
                            generation: p.generation,
                            revision: p.frame.revision,
                            input_epoch: p.input_epoch,
                        },
                        article.id,
                    )
                })
            })
        };
        if let Some(ui) = self.ui.upgrade() {
            let previous = self.permit.replace(scope);
            if let Some(scope) = scope {
                if (previous != Some(scope) || self.focus_after_paint.get())
                    && ui.get_native_foreground()
                    && !ui.get_find_open()
                {
                    ui.invoke_focus_article();
                    self.focus_after_paint.set(!ui.get_article_focused());
                }
                if scope.0 != generation {
                    self.permit.set(None);
                }
            }
        }
        if let Some(ui) = self.ui.upgrade() {
            ui.invoke_output_updated();
            ui.invoke_paste_updated();
            ui.invoke_keep_alive_changed();
            ui.invoke_inline_updated();
        }
        self.notify.notify_one();
    }
    fn step(self: &Rc<Self>, view: &View) {
        if self.closed.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else { return };
        let now = Instant::now();
        self.core.borrow_mut().expire_full_load(now);
        self.retire_stale_projection(view);
        let progress = self.core.borrow().reading_progress();
        ui.set_article_load_available(progress.is_some());
        ui.set_article_load_active(self.core.borrow().full_load_active());
        let next_segment = progress.is_some_and(|(id, _, _, state)| {
            state == Some(pttstella::article_cache::LoadState::Limit)
                && self.core.borrow().last_displayed().is_some_and(|p| {
                    p.article
                        .as_ref()
                        .is_some_and(|a| a.id == id && a.next_segment)
                })
        });
        ui.set_article_load_next_segment(next_segment);
        ui.set_article_load_can_resume(
            next_segment
                || progress.is_some_and(|(_, _, _, state)| {
                    matches!(
                        state,
                        Some(
                            pttstella::article_cache::LoadState::Cancelled
                                | pttstella::article_cache::LoadState::Stopped
                        )
                    )
                }),
        );
        let label = progress.map_or_else(String::new, |(_, first, last, state)| {
            use pttstella::article_cache::LoadState;
            let lines = last.saturating_sub(first).saturating_add(1).to_string();
            match state {
                Some(LoadState::Loading) if !ui.get_native_foreground() => {
                    pttstella::msg!("article-load-paused-background", lines = lines)
                }
                Some(LoadState::Loading) if !self.allowed(&ui) => {
                    pttstella::msg!("article-load-paused-interaction", lines = lines)
                }
                Some(LoadState::Loading) => pttstella::msg!("article-load-loading", lines = lines),
                Some(LoadState::Complete) => {
                    pttstella::msg!("article-load-complete", lines = lines)
                }
                Some(LoadState::Cancelled) => {
                    pttstella::msg!("article-load-cancelled", lines = lines)
                }
                Some(LoadState::Stopped) => pttstella::msg!("article-load-stopped", lines = lines),
                Some(LoadState::Changed) => pttstella::msg!("article-load-changed"),
                Some(LoadState::Limit) => pttstella::msg!("article-load-limit", lines = lines),
                None => String::new(),
            }
        });
        ui.set_article_load_status(label.into());
        // Match the Dioxus surface: dispatch the next verified station request
        // before projecting/rasterizing the UI. Painting cannot pace the fetch.
        if !self.busy.get() && self.allowed(&ui) && view.phase == Phase::Connected {
            let intent = self.core.borrow_mut().next_full_page(view);
            if let Some(intent) = intent {
                self.reading_timing("request", view);
                match self.connection.request_cancellable_navigate(
                    view.stamp(),
                    intent,
                    pttstella::navigation::Source::Button,
                ) {
                    Ok((reply, cancellation)) => {
                        if let Ok(bytes) = pttstella::navigation::plan(&view.context(), intent) {
                            self.core.borrow_mut().article_input(view, &bytes);
                        }
                        self.busy.set(true);
                        self.cancellation.replace(Some(cancellation));
                        self.requests.set(self.requests.get() + 1);
                        self.serial.set(self.serial.get().wrapping_add(1));
                        self.reply.replace(Some(reply));
                    }
                    Err(error) => {
                        self.core.borrow_mut().stop_full_load();
                        self.notice(error);
                    }
                }
            }
        }
        if self.core.borrow().dirty()
            && self.composition_handoff.get().is_none()
            && !self.core.borrow().has_in_flight()
            && !view.input_pending
            && !view.output_pending
            && now >= self.next_paint.get()
        {
            let packet = {
                let mut core = self.core.borrow_mut();
                core.queue(view, false, &self.display.borrow()).map(|p| {
                    (
                        Identity::of(p),
                        p.article.clone(),
                        p.frame.rows.saturating_sub(1),
                        p.article.is_none().then(|| p.frame.clone()),
                    )
                })
            };
            if let Some((identity, article, rows, terminal)) = packet {
                self.reading_timing("paint_pending", view);
                if let Some(article) = article {
                    self.terminal_pending.set(None);
                    self.terminal_shown.set(None);
                    let source = self
                        .core
                        .borrow()
                        .article_source(view, &self.display.borrow());
                    if let Some(source) = source {
                        let scope = (
                            view.generation,
                            article.id,
                            self.core.borrow().presentation(),
                        );
                        self.shown.set(Some(scope));
                        let mut slot = self.view.borrow_mut();
                        let reused = if let Some(native) = slot.as_ref() {
                            native.update(
                                source,
                                rows,
                                (scope.0, scope.1, scope.2, article.first_line),
                            )
                        } else {
                            let renderer = self.renderer.borrow().fork(source.frame.clone());
                            match ArticleView::mount_shared(
                                &ui,
                                self.core.clone(),
                                source,
                                rows,
                                renderer,
                            ) {
                                Ok(native) => {
                                    native.scope.set(Some((
                                        scope.0,
                                        scope.1,
                                        scope.2,
                                        article.first_line,
                                    )));
                                    *slot = Some(native);
                                }
                                Err(error) => self.notice(error),
                            }
                            false
                        };
                        drop(slot);
                        if reused {
                            self.rendered();
                        }
                        self.next_paint.set(now + Duration::from_millis(250));
                    }
                } else {
                    let reused = self.terminal_shown.get() == Some(identity)
                        && self.terminal_size.get().is_some()
                        && !ui.get_frame_paint_pending()
                        && self.terminal_presented.get()
                            == Some(FrameId {
                                generation: identity.generation,
                                revision: identity.revision,
                            })
                        && terminal
                            .as_ref()
                            .is_some_and(|frame| self.renderer.borrow().frame() == frame);
                    self.terminal_pending.set(Some(identity));
                    self.terminal_shown.set(None);
                    if reused {
                        self.terminal_rendered();
                    } else if let Some(frame) = terminal {
                        self.terminal_presented.set(None);
                        self.renderer.borrow_mut().set_frame(frame);
                        self.terminal_size.set(None);
                        ui.invoke_geometry_changed();
                    }
                }
            }
        }
        if self.focus_after_paint.get()
            && ui.get_article_active()
            && ui.get_native_foreground()
            && !ui.get_article_blocked()
            && !self.core.borrow().has_in_flight()
            && !self.core.borrow().dirty()
            && self.input_context(view).is_some()
        {
            ui.invoke_focus_article();
            self.focus_after_paint.set(!ui.get_article_focused());
        }
        // Completion/cancel should not wait for the loading paint throttle.
        if !self.core.borrow().full_load_active()
            && self.core.borrow().dirty()
            && self.next_paint.get() > now
        {
            self.next_paint.set(now);
            self.notify.notify_one();
        }
    }
    fn deadline(&self) -> Option<Instant> {
        let core = self.core.borrow();
        let load = core.full_load_deadline();
        let paint = (core.dirty() && !core.has_in_flight())
            .then_some(self.next_paint.get())
            .filter(|at| *at > Instant::now());
        match (load, paint) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    fn cancel_command(&self) {
        if let Some(cancel) = self.cancellation.take() {
            cancel.cancel();
        }
        self.reply.take();
        self.serial.set(self.serial.get().wrapping_add(1));
        self.busy.set(false);
    }
    fn notice(&self, error: String) {
        if let Some(ui) = self.ui.upgrade() {
            ui.set_input_notice(error.into());
        }
    }
    pub fn finish(&self) {
        self.closed.set(true);
        self.find_timer.stop();
        self.find_draft.take();
        self.core.borrow_mut().find_finish();
        self.cancel_command();
        self.notify.notify_one();
        self.permit.set(None);
        self.core.borrow_mut().interrupt_full_load();
        if let Some(view) = self.view.borrow().as_ref() {
            view.finish();
        }
    }
    pub async fn shutdown(&self) {
        self.finish();
        let job = self.job.take();
        if let Some(job) = job {
            job.await;
        }
        while !self.loop_done.get() {
            self.stopped_notify.notified().await;
        }
    }
    pub fn stopped(&self) -> bool {
        self.closed.get()
            && self.loop_done.get()
            && self.job.borrow().is_none()
            && self.reply.borrow().is_none()
            && !self.busy.get()
    }
    pub fn raster_count(&self) -> usize {
        self.view
            .borrow()
            .as_ref()
            .map_or(0, |view| view.raster_count.get())
    }
    pub fn progress(
        &self,
    ) -> Option<(
        u64,
        usize,
        usize,
        Option<pttstella::article_cache::LoadState>,
    )> {
        self.core.borrow().reading_progress()
    }
    pub(crate) fn diagnostics(&self) -> serde_json::Value {
        let core = self.core.borrow();
        let ui = self.ui.upgrade();
        let view = self.connection.updates.borrow();
        let id = |p: Option<Identity>| p.map(|p| (p.generation, p.revision, p.presentation));
        serde_json::json!({
            "displayed":id(core.last_displayed().map(Identity::of)),"in_flight":id(core.pending_identity()),
            "terminal_pending":id(self.terminal_pending.get()),"terminal_shown":id(self.terminal_shown.get()),
            "terminal_presented":self.terminal_presented.get().map(|f|(f.generation,f.revision)),
            "size_installed":self.terminal_size.get().is_some(),"stamp":view.stamp(),"output_pending":view.output_pending,
            "input_pending":view.input_pending,"closed":self.closed.get(),
            "reading_scope": core.reading_scope(), "permit": self.permit.get(),
            "shown": self.shown.get(), "pending": format!("{:?}", core.pending_identity()),
            "article_boundary_navigation":core.last_displayed().map(|p|p.article_boundary_navigation),
            "dirty": core.dirty(), "progress": format!("{:?}", self.progress()),
            "busy": self.busy.get(), "requests": self.requests.get(),
            "raw_generation": self.raw_generation.get(), "vim_handoff": self.vim_handoff.get(),
            "ui":ui.map(|ui|serde_json::json!({"active":ui.get_article_active(),"focused":ui.get_article_focused(),
                "ready":ui.get_article_reading_ready(),"foreground":ui.get_native_foreground(),
                "blocked":ui.get_article_blocked(),"frame_pending":ui.get_frame_paint_pending(),
                "keyboard_pending":ui.get_keyboard_pending(),"selection":ui.get_selection_active()}))
        })
    }
}
