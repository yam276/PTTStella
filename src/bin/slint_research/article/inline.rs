//! Scoped native inline controls and geometry; HTTP/decoding stays on media worker.
#[path = "inline_geometry.rs"]
mod geometry;
#[path = "inline_key.rs"]
mod key;
#[path = "inline_menu_probe.rs"]
mod menu_probe;
#[path = "inline_probe.rs"]
pub mod probe;
#[path = "updates_probe.rs"]
pub mod updates_probe;
use super::session::ArticleSession;
use crate::{
    InlineArrow, InlinePicture, TerminalWindow,
    inline_engine::{self, Controls},
    media::Media,
    media_worker::Command,
};
use pttstella::{
    connection::Connection,
    media::{
        self,
        inline::{Projection, Target},
    },
    surface_state::Identity,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeSet,
    rc::{Rc, Weak},
};

struct Updating<'a>(&'a Cell<bool>);
impl Drop for Updating<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}
pub struct Inline {
    ui: slint::Weak<TerminalWindow>,
    article: Weak<ArticleSession>,
    media: Weak<Media>,
    connection: Rc<Connection>,
    epoch: Cell<u64>,
    scope: Cell<Option<(u64, u64, u64, usize)>>,
    projection: RefCell<Option<Projection<'static>>>,
    candidates: RefCell<Vec<(usize, usize, usize, usize)>>,
    state: RefCell<inline_engine::State>,
    pictures: Rc<VecModel<InlinePicture>>,
    last_geometry: RefCell<Option<crate::pointer::SurfaceGeometry>>,
    pixel_updates: Cell<usize>,
    layout_updates: Cell<usize>,
    sent: Cell<Option<Identity>>,
    active: Cell<bool>,
    tag: RefCell<String>,
    press: RefCell<Option<(i32, String, crate::pointer::SurfaceGeometry)>>,
    wide: RefCell<BTreeSet<usize>>,
    automatic: Cell<bool>,
    closed: Cell<bool>,
    updating: Cell<bool>,
    trace_left: Cell<u8>,
    resource_trace_left: Cell<u8>,
    update_timer: slint::Timer,
    enter_timer: slint::Timer,
    enter_held: Cell<bool>,
    enter_press: RefCell<Option<key::Press>>,
}
impl Inline {
    pub fn mount(
        ui: &TerminalWindow,
        article: &Rc<ArticleSession>,
        media: &Rc<Media>,
        connection: Rc<Connection>,
        automatic: bool,
    ) -> Rc<Self> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            article: Rc::downgrade(article),
            media: Rc::downgrade(media),
            connection,
            epoch: Cell::new(0),
            scope: Cell::new(None),
            projection: RefCell::new(None),
            candidates: RefCell::new(Vec::new()),
            state: RefCell::new(Default::default()),
            pictures: Rc::new(VecModel::default()),
            last_geometry: RefCell::new(None),
            pixel_updates: Cell::new(0),
            layout_updates: Cell::new(0),
            sent: Cell::new(None),
            active: Cell::new(false),
            tag: RefCell::new(String::new()),
            press: RefCell::new(None),
            wide: RefCell::new(BTreeSet::new()),
            automatic: Cell::new(automatic),
            closed: Cell::new(false),
            updating: Cell::new(false),
            update_timer: slint::Timer::default(),
            enter_timer: slint::Timer::default(),
            enter_held: Cell::new(false),
            enter_press: RefCell::new(None),
            resource_trace_left: Cell::new(
                if std::env::var_os("PTTSTELLA_INLINE_TRACE").is_some() {
                    8
                } else {
                    0
                },
            ),
            trace_left: Cell::new(if std::env::var_os("PTTSTELLA_INLINE_TRACE").is_some() {
                16
            } else {
                0
            }),
        });
        ui.set_inline_pictures(ModelRc::from(owner.pictures.clone()));
        ui.on_inline_updated({
            let owner = owner.clone();
            move || {
                owner.update();
                owner.schedule();
            }
        });
        ui.on_inline_enter({
            let owner = owner.clone();
            move |down, repeat, eligible| owner.enter_key(down, repeat, eligible)
        });
        ui.on_inline_pointer({
            let owner = owner.clone();
            move |kind, index, tag| owner.pointer(kind, index, tag.as_str())
        });
        owner
    }
    fn schedule(self: &Rc<Self>) {
        if self.closed.get() || self.update_timer.running() {
            return;
        }
        let weak = Rc::downgrade(self);
        self.update_timer.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::ZERO,
            move || {
                if let Some(owner) = weak.upgrade() {
                    owner.update();
                }
            },
        );
    }
    pub fn install(self: &Rc<Self>, state: inline_engine::State) {
        if state.epoch != self.epoch.get()
            || self.closed.get()
            || state.automatic && !self.automatic.get()
        {
            return;
        }
        let stable = {
            let old = self.state.borrow();
            old.epoch == state.epoch
                && old.identity == state.identity
                && old.limited == state.limited
                && old.error == state.error
                && old.automatic == state.automatic
                && old.entries.len() == state.entries.len()
                && old
                    .entries
                    .iter()
                    .zip(&state.entries)
                    .all(|(a, b)| (a.index, a.row, a.key) == (b.index, b.row, b.key))
                && old.resources.len() == state.resources.len()
                && old.resources.iter().all(|(key, a)| {
                    state.resources.get(key).is_some_and(|b| {
                        a.error == b.error
                            && match (&a.original, &b.original) {
                                (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
                                (None, None) => true,
                                _ => false,
                            }
                    })
                })
        };
        if stable
            && let (Some(ui), Some(article)) = (self.ui.upgrade(), self.article.upgrade())
            && ui.get_inline_enabled()
            && article.output_geometry() == *self.last_geometry.borrow()
            && article.output_identity(&self.connection.updates.borrow()) == state.identity
        {
            for row in 0..self.pictures.row_count() {
                if let Some(mut picture) = self.pictures.row_data(row)
                    && let Ok(key) = usize::try_from(picture.key)
                    && let Some(resource) = state.resources.get(&key)
                    && let Some(pixels) = resource.pixels.clone()
                {
                    if self
                        .state
                        .borrow()
                        .resources
                        .get(&key)
                        .is_some_and(|r| r.frame == resource.frame)
                    {
                        continue;
                    }
                    picture.image = slint::Image::from_rgba8(pixels);
                    self.pictures.set_row_data(row, picture);
                    self.pixel_updates.set(self.pixel_updates.get() + 1);
                }
            }
            self.state.replace(state);
            return;
        }
        self.state.replace(state);
        self.update();
        if let Some(ui) = self.ui.upgrade() {
            ui.invoke_context_updated();
        }
        if let Some(left) = self.resource_trace_left.get().checked_sub(1) {
            self.resource_trace_left.set(left);
            for row in 0..self.pictures.row_count().min(3) {
                if let Some(p) = self.pictures.row_data(row) {
                    let pixels = p.image.to_rgba8();
                    eprintln!(
                        "inline publish key={} box={:?} clip={:?} pixels={:?} first={:?}",
                        p.key,
                        (p.x, p.y, p.width, p.height, p.image_y, p.image_height),
                        (p.source_y, p.source_width, p.source_height),
                        pixels.as_ref().map(|p| (p.width(), p.height())),
                        pixels.as_ref().and_then(|p| p.as_bytes().get(..4))
                    );
                }
            }
        }
    }
    pub fn set_automatic(self: &Rc<Self>, enabled: bool) {
        if self.automatic.replace(enabled) == enabled || self.closed.get() {
            return;
        }
        self.sent.set(None);
        if !enabled && self.state.borrow().automatic {
            let identity = self.state.borrow().identity;
            self.state.replace(inline_engine::State {
                epoch: self.epoch.get(),
                identity,
                ..Default::default()
            });
            self.wide.borrow_mut().clear();
        }
        self.update();
        self.schedule();
    }
    pub fn rebase_reply_focus(&self, old: (u64, u64, u64), new: (u64, u64, u64)) {
        if let Some(scope) = self.scope.get()
            && (scope.0, scope.1, scope.2) == old
        {
            self.cancel_enter();
            self.press.take();
            self.tag.borrow_mut().clear();
            self.scope.set(Some((new.0, new.1, new.2, scope.3)));
        }
    }
    fn reset(&self, ui: &TerminalWindow) {
        self.cancel_enter();
        if self.active.replace(false)
            && let Some(media) = self.media.upgrade()
        {
            media.inline_scope(0);
        }
        self.sent.set(None);
        self.projection.take();
        self.candidates.borrow_mut().clear();
        self.press.take();
        self.wide.borrow_mut().clear();
        self.state.replace(Default::default());
        self.tag.borrow_mut().clear();
        ui.set_inline_arrows(ModelRc::default());
        while self.pictures.row_count() > 0 {
            self.pictures.remove(self.pictures.row_count() - 1);
        }
        self.last_geometry.take();
        ui.set_inline_enabled(false);
    }
    pub fn finish(&self) {
        self.enter_held.set(false);
        self.closed.set(true);
        self.update_timer.stop();
        if let Some(ui) = self.ui.upgrade() {
            self.reset(&ui);
        }
    }
    pub fn update(self: &Rc<Self>) {
        if self.closed.get() || self.updating.replace(true) {
            return;
        }
        let _updating = Updating(&self.updating);
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        // OS focus loss may omit the physical release edge entirely.
        if !ui.get_native_foreground() {
            self.cancel_enter();
            self.enter_held.set(false);
        }
        let Some(article) = self.article.upgrade() else {
            return;
        };
        let view = self.connection.updates.borrow().clone();
        let native = article.view.borrow().clone();
        let scope = native.as_ref().and_then(|v| v.scope.get()).filter(|s| {
            article.core.borrow().reading_scope() == Some((s.0, s.1, s.2))
                && ui.get_article_active()
                && view.phase == pttstella::connection::Phase::Connected
                && view.site.ptt_features()
                && view.mouse.route == pttstella::mouse::input::Route::Local
        });
        if scope != self.scope.get() {
            self.reset(&ui);
            self.scope.set(scope);
            let Some(next) = self.epoch.get().checked_add(1) else {
                crate::notice::error(&ui, "inline epoch exhausted");
                self.closed.set(true);
                return;
            };
            self.epoch.set(next);
        }
        let (Some(scope), Some(native)) = (scope, native) else {
            return;
        };
        let current = article.output_identity(&view).and_then(|id| {
            let core = article.core.borrow();
            let paint = core.inline_page(id, view.stamp(), &view)?;
            Some((
                id,
                (self
                    .projection
                    .borrow()
                    .as_ref()
                    .is_none_or(|p| p.identity() != id))
                .then(|| Projection::of(paint).into_owned()),
            ))
        });
        let ready = current.is_some() && !ui.get_inline_blocked();
        ui.set_inline_enabled(ready);
        if let Some((id, projection)) = current {
            let tag = format!(
                "{}:{}:{}:{}:{}",
                self.epoch.get(),
                id.generation,
                id.presentation,
                id.revision,
                view.input_epoch
            );
            if *self.tag.borrow() != tag {
                self.press.take();
                self.tag.replace(tag);
            }
            if let Some(projection) = projection {
                let candidates = projection
                    .images()
                    .into_iter()
                    .filter_map(|(index, row, url)| {
                        let link = projection
                            .links()
                            .iter()
                            .filter(|l| {
                                l.row == row
                                    && media::classify(&l.url)
                                        .ok()
                                        .flatten()
                                        .is_some_and(|t| t.url == url)
                            })
                            .max_by_key(|l| l.end)?;
                        Some((index, row, link.start, link.end))
                    })
                    .collect();
                self.candidates.replace(candidates);
                self.projection.replace(Some(projection));
            }
            if (self.active.get() || self.automatic.get())
                && self.sent.get() != Some(id)
                && let Err(e) = self.request(None)
            {
                crate::notice::error(&ui, e);
            }
        }
        let Some(layout) = native.layout.get() else {
            return;
        };
        let state = self.state.borrow().clone();
        let mut gaps = Vec::new();
        let mut boxes = Vec::new();
        let mut offsets = std::collections::BTreeMap::<usize, u32>::new();
        if state.epoch == self.epoch.get() {
            for entry in &state.entries {
                if entry.row >= native.rows.get() {
                    continue;
                }
                let r = state.resources.get(&entry.key);
                let dimensions = r
                    .and_then(|r| r.first.as_ref())
                    .map(|p| (p.width(), p.height()));
                let (width, height) = if let Some((width, height)) = dimensions {
                    let scale = if self.wide.borrow().contains(&entry.key) {
                        f64::from(layout.logical_width()) / f64::from(width)
                    } else {
                        (f64::from(layout.logical_width()) / f64::from(width))
                            .min(560. / f64::from(height))
                            .min(1.)
                    };
                    (f64::from(width) * scale, f64::from(height) * scale)
                } else {
                    (f64::from(layout.logical_width()).min(440.), 36.)
                };
                let physical = ((height + 8.) * f64::from(layout.dpi)).ceil();
                if !physical.is_finite() || physical > f64::from(u32::MAX) {
                    crate::notice::error(&ui, "inline image geometry exceeds limit");
                    continue;
                }
                gaps.push((entry.row, physical as u32));
                let prior = offsets.entry(entry.row).or_default();
                boxes.push((entry.clone(), width, height, *prior));
                let Some(next) = prior.checked_add(physical as u32) else {
                    crate::notice::error(&ui, "inline row gap exceeds limit");
                    return;
                };
                *prior = next;
            }
        }
        // Preserve the row/image anchor; the viewport applies and ACKs this map.
        if let Err(e) = native.set_gaps(gaps) {
            crate::notice::error(&ui, e);
            return;
        }
        let ready = ready && native.geometry().is_some();
        ui.set_inline_enabled(ready);
        let Some(flow) = native.flow() else {
            return;
        };
        let offset = native.position(&flow);
        let left =
            (layout.view_width - layout.logical_width()).max(0.) / 2. - ui.get_article_offset_x();
        let mut pictures = Vec::new();
        let mut motion = Vec::new();
        for (entry, width, height, prior) in boxes {
            let top = (flow.top(entry.row) as f64 + f64::from(layout.row) + f64::from(prior)
                - offset)
                / f64::from(layout.dpi)
                + 4.;
            if top + height < 0. || top > f64::from(layout.body_height) {
                continue;
            }
            let r = state.resources.get(&entry.key);
            let image = r
                .and_then(|r| r.pixels.clone())
                .map(slint::Image::from_rgba8)
                .unwrap_or_default();
            let status = match r {
                Some(r) => r.error.clone().unwrap_or_default(),
                None => pttstella::msg!("image-loading"),
            };
            let (source_width, source_height) = dimensions_for(&state, entry.key).unwrap_or((1, 1));
            let Some(crop) =
                geometry::crop(top, height, f64::from(layout.body_height), source_height)
            else {
                continue;
            };
            pictures.push(InlinePicture {
                index: entry.index as i32,
                key: entry.key as i32,
                stamp: self.tag.borrow().as_str().into(),
                x: left + ((f64::from(layout.logical_width()) - width).max(0.) / 2.) as f32,
                y: crop.top,
                width: width as f32,
                height: crop.height,
                image_y: crop.image_y,
                image_height: crop.image_height,
                source_y: crop.source_y,
                source_height: crop.source_height,
                source_width: source_width as i32,
                image,
                status: status.into(),
            });
            if r.filter(|r| r.error.is_none())
                .and_then(|r| r.original.as_ref())
                .is_some_and(|i| i.mime == "image/gif")
            {
                motion.push((
                    ((top + height / 2.) - f64::from(layout.body_height) / 2.).abs(),
                    entry.key,
                ));
            }
        }
        motion.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut selected = [None; 2];
        for (out, (_, key)) in selected.iter_mut().zip(motion) {
            *out = Some(key);
        }
        if let Some(media) = self.media.upgrade() {
            media.inline_controls(Controls {
                epoch: self.epoch.get(),
                permit: ready && ui.get_native_foreground() && !ui.get_native_composing(),
                visible: selected,
            });
        }
        // Keep the visible rows attached and notify only changed entries.
        // Resource metadata updates may move rows; animation updates above
        // remain pixel-only and do not reset this model.
        for (index, picture) in pictures.iter().enumerate() {
            if index >= self.pictures.row_count() {
                self.pictures.push(picture.clone());
            } else if self.pictures.row_data(index).as_ref() != Some(picture) {
                self.pictures.set_row_data(index, picture.clone());
            }
        }
        while self.pictures.row_count() > pictures.len() {
            self.pictures.remove(self.pictures.row_count() - 1);
        }
        self.last_geometry.replace(native.geometry());
        self.layout_updates.set(self.layout_updates.get() + 1);
        let mut arrows = Vec::new();
        for &(index, row, start, end) in self.candidates.borrow().iter() {
            let y = (flow.top(row) as f64 - offset) / f64::from(layout.dpi);
            if y + f64::from(layout.line_height()) < 0. || y >= f64::from(layout.body_height) {
                continue;
            }
            let cell = layout.logical_width() / 80.;
            let size = (layout.line_height() * 0.8).min((end - start) as f32 * cell);
            let opened = state.entries.iter().any(|e| e.index == index);
            arrows.push(InlineArrow {
                index: index as i32,
                stamp: self.tag.borrow().as_str().into(),
                x: left + end as f32 * cell - size,
                y: y as f32 + (layout.line_height() - size) / 2.,
                size,
                opened,
            });
        }
        ui.set_inline_arrows(ModelRc::new(VecModel::from(arrows)));
        self.validate_enter();
        let _ = scope;
    }
    pub fn original(
        &self,
        scope: &pttstella::media::hover::Lease,
    ) -> Option<std::sync::Arc<pttstella::media::loader::Image>> {
        if self.closed.get() {
            return None;
        }
        let article = self.article.upgrade()?;
        let view = self.connection.updates.borrow().clone();
        if scope.stamp != view.stamp() {
            return None;
        }
        let identity = article.output_identity(&view)?;
        let state = self.state.borrow();
        if state.epoch != self.epoch.get() || state.identity != Some(identity) {
            return None;
        }
        let core = article.core.borrow();
        let paint = core.inline_page(identity, view.stamp(), &view)?;
        if paint.links.get(scope.index) != Some(&scope.link) {
            return None;
        }
        state
            .entries
            .iter()
            .find(|e| e.index == scope.index)
            .and_then(|e| state.resources.get(&e.key))
            .filter(|r| r.error.is_none())
            .and_then(|r| r.original.clone())
    }
    pub fn picture_visible(&self, index: usize, tag: &str) -> bool {
        !self.closed.get()
            && tag == self.tag.borrow().as_str()
            && (0..self.pictures.row_count())
                .filter_map(|n| self.pictures.row_data(n))
                .any(|p| p.index == index as i32)
            && self.article.upgrade().is_some_and(|a| {
                a.output_identity(&self.connection.updates.borrow())
                    == self.projection.borrow().as_ref().map(Projection::identity)
            })
    }
    pub fn menu_ready(&self) -> bool {
        self.ui.upgrade().is_some_and(|ui| {
            !self.closed.get()
                && ui.get_article_active()
                && !ui.get_context_source_blocked()
                && !ui.get_keyboard_pending()
                && (!ui.get_vim_enabled() || ui.get_keyboard_read_only())
                && !self.candidates.borrow().is_empty()
                && self.article.upgrade().is_some_and(|a| {
                    a.output_identity(&self.connection.updates.borrow())
                        .is_some_and(|id| {
                            self.projection
                                .borrow()
                                .as_ref()
                                .is_some_and(|p| p.identity() == id)
                        })
                })
        })
    }
    pub fn has_images(&self) -> bool {
        !self.candidates.borrow().is_empty()
    }
    pub fn expand_all(self: &Rc<Self>) {
        if self.menu_ready()
            && let Err(error) = self.request(Some(Target::All))
            && let Some(ui) = self.ui.upgrade()
        {
            crate::notice::error(&ui, error);
        }
    }
    fn request(self: &Rc<Self>, target: Option<Target>) -> Result<(), String> {
        let media = self.media.upgrade().ok_or("media owner closed")?;
        let projection = self
            .projection
            .borrow()
            .clone()
            .ok_or("inline projection not ready")?;
        let identity = projection.identity();
        media.inline_command(
            self.epoch.get(),
            Command::Inline {
                epoch: self.epoch.get(),
                projection,
                target,
            },
        )?;
        self.sent.set(Some(identity));
        self.active.set(true);
        Ok(())
    }
    fn pointer(self: &Rc<Self>, kind: i32, index: i32, tag: &str) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if let Some(left) = self.trace_left.get().checked_sub(1) {
            self.trace_left.set(left);
            eprintln!(
                "inline pointer kind={kind} index={index} enabled={} tag_matches={} press={}",
                ui.get_inline_enabled(),
                tag == self.tag.borrow().as_str(),
                self.press.borrow().is_some()
            );
        }
        if !ui.get_inline_enabled() || tag != self.tag.borrow().as_str() {
            self.press.take();
            return;
        }
        if kind == 0 {
            if self.trace_left.get() > 0
                && let Some(article) = self.article.upgrade()
            {
                let native = article.view.borrow().clone();
                eprintln!(
                    "inline press geometry={} view={} timer={:?} layout={:?} painted={:?}",
                    article.output_geometry().is_some(),
                    native.is_some(),
                    native.as_ref().map(|n| n.timer.running()),
                    native.as_ref().and_then(|n| n.layout.get()),
                    native
                        .as_ref()
                        .map(|n| n.painted.borrow().as_ref().map(|p| (
                            p.rows.clone(),
                            p.width,
                            p.row
                        )))
                );
            }
            if let Some(g) = self.article.upgrade().and_then(|a| a.output_geometry()) {
                self.press.replace(Some((index, tag.to_owned(), g)));
            }
            return;
        }
        let press = self.press.take();
        if kind != 1 && kind != 3 || press.as_ref().is_none_or(|p| p.0 != index || p.1 != tag) {
            return;
        }
        let Some(article) = self.article.upgrade() else {
            return;
        };
        let view = self.connection.updates.borrow().clone();
        if press
            .as_ref()
            .is_none_or(|p| Some(p.2.clone()) != article.output_geometry())
        {
            return;
        }
        if article.output_identity(&view)
            != self.projection.borrow().as_ref().map(Projection::identity)
        {
            return;
        }
        let Ok(index) = usize::try_from(index) else {
            return;
        };
        if kind == 1 {
            if let Err(e) = self.request(Some(Target::Link { index })) {
                crate::notice::error(&ui, e);
            }
        } else {
            let state = self.state.borrow();
            if state.identity != self.projection.borrow().as_ref().map(Projection::identity) {
                return;
            }
            if let Some(image) = state
                .entries
                .iter()
                .find(|e| e.index == index)
                .and_then(|e| state.resources.get(&e.key))
                .and_then(|r| r.original.clone())
                && let Some(media) = self.media.upgrade()
            {
                media.open_inline(index, image);
            }
        }
    }
}

fn dimensions_for(state: &inline_engine::State, key: usize) -> Option<(u32, u32)> {
    let p = state.resources.get(&key)?.first.as_ref()?;
    Some((p.width(), p.height()))
}
