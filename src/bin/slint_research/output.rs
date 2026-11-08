//! Native selection and explicit exports consume one acknowledged terminal frame.
#[path = "output_picture.rs"]
pub(crate) mod picture;
#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;
use crate::{
    SelectionRect, TerminalWindow,
    clipboard::Clipboard,
    pointer::{self, Geometry, Hit, SurfaceGeometry},
    renderer::{Renderer, Size},
    session::FrameId,
};
use pttstella::{
    connection::{Connection, NavigationCancellation, Phase, View},
    export::{self, Lease},
    links::{self, Link},
    msg,
    navigation::{Context, Intent, Source, Stamp},
    screen::Frame,
    selection::{self, Position, SelectedText, Selection},
    settings::DisplaySettings,
    surface_state::{CopyTarget, Identity, OutputLease},
};
use slint::{ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
    thread::JoinHandle,
};

#[path = "output_accounts.rs"]
mod accounts;
pub use accounts::AccountContext;

struct Gesture {
    stamp: Stamp,
    geometry: SurfaceGeometry,
    anchor: Option<Position>,
    focus: Option<Position>,
    shift: bool,
    origin: (f32, f32),
    moved: bool,
}

#[derive(Default)]
struct Selected {
    gesture: Option<Gesture>,
    text: Option<(Stamp, SelectedText)>,
}

struct Click {
    from: (f32, f32),
    to: (f32, f32),
}
enum SelectionUpdate {
    Keep,
    Clear,
    Set(Selection),
}
struct PointerResult {
    click: Option<Click>,
    selection: SelectionUpdate,
}
impl Selected {
    fn track(
        &mut self,
        position: Option<Position>,
        stamp: Stamp,
        geometry: SurfaceGeometry,
        kind: i32,
        point: (f32, f32),
        shift: bool,
    ) -> PointerResult {
        let mut result = PointerResult {
            click: None,
            selection: SelectionUpdate::Keep,
        };
        if kind == 0 {
            self.gesture = geometry.contains(point).then_some(Gesture {
                stamp,
                geometry,
                anchor: position,
                focus: position,
                shift,
                origin: point,
                moved: false,
            });
            result.selection = SelectionUpdate::Clear;
            return result;
        }
        let Some(g) = &mut self.gesture else {
            return result;
        };
        if g.stamp != stamp || g.geometry != geometry || !geometry.contains(point) || kind == 3 {
            self.gesture = None;
            result.selection = SelectionUpdate::Clear;
            return result;
        }
        g.shift |= shift;
        let moved = (point.0 - g.origin.0).hypot(point.1 - g.origin.1) >= 3. * geometry.dpi();
        if (moved && !g.moved) || g.moved && position != g.focus {
            g.moved = true;
            g.focus = position;
            result.selection = match (g.anchor, position) {
                (Some(anchor), Some(focus)) => {
                    SelectionUpdate::Set(Selection::Range { anchor, focus })
                }
                _ => SelectionUpdate::Clear,
            };
        }
        if kind == 2 {
            result.click = (!g.moved && !g.shift).then_some(Click {
                from: g.origin,
                to: point,
            });
            self.gesture = None;
        }
        result
    }
    #[cfg(test)]
    fn pointer(
        &mut self,
        frame: &Frame,
        stamp: Stamp,
        geometry: Geometry,
        kind: i32,
        point: (f32, f32),
        shift: bool,
    ) -> Result<Option<Click>, String> {
        let geometry = SurfaceGeometry::from(geometry);
        let result = self.track(
            geometry.cell(point, frame),
            stamp,
            geometry,
            kind,
            point,
            shift,
        );
        match result.selection {
            SelectionUpdate::Keep => {}
            SelectionUpdate::Clear => self.text = None,
            SelectionUpdate::Set(selection) => {
                self.text = Some((stamp, selection::extract(frame, &selection)?))
            }
        }
        Ok(result.click)
    }
}
#[derive(Clone)]
pub(super) enum WorkLease {
    Terminal(Lease),
    Article(OutputLease),
}
impl WorkLease {
    fn check(&self, view: &View) -> Result<(), String> {
        match self {
            Self::Terminal(lease) => lease.check(view),
            Self::Article(lease) => lease.check(view),
        }
    }
}
struct ActiveLease {
    lease: WorkLease,
    selected: bool,
}
struct Projection {
    stamp: Stamp,
    policy: u64,
    context: Context,
    links: Vec<Link>,
}
#[derive(Clone, PartialEq)]
struct Hover {
    stamp: Option<Stamp>,
    policy: u64,
    geometry: Option<SurfaceGeometry>,
    hit: Option<Hit>,
}

pub struct Output {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    renderer: Rc<RefCell<Renderer>>,
    size: Rc<Cell<Option<Size>>>,
    presented: Rc<Cell<Option<FrameId>>>,
    display: RefCell<DisplaySettings>,
    clipboard: Clipboard,
    selected: RefCell<Selected>,
    lease: RefCell<Option<ActiveLease>>,
    article: Option<Rc<crate::article::session::ArticleSession>>,
    registered: Cell<Option<(Identity, u64)>>,
    selection_sequence: Cell<u64>,
    last_geometry: RefCell<Option<SurfaceGeometry>>,
    worker: RefCell<Option<JoinHandle<()>>>,
    picker: Cell<bool>,
    closed: Cell<bool>,
    phase: Cell<Phase>,
    projection: RefCell<Option<Projection>>,
    hover: Cell<Option<(f32, f32)>>,
    hover_state: RefCell<Option<Hover>>,
    navigation: RefCell<Option<slint::JoinHandle<()>>>,
    navigation_cancel: RefCell<Option<NavigationCancellation>>,
    pending_navigation: Cell<Option<Stamp>>,
    click_acted: Cell<bool>,
}

impl Output {
    #[allow(clippy::too_many_arguments)]
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        renderer: Rc<RefCell<Renderer>>,
        size: Rc<Cell<Option<Size>>>,
        presented: Rc<Cell<Option<FrameId>>>,
        display: DisplaySettings,
        clipboard: Clipboard,
        article: Option<Rc<crate::article::session::ArticleSession>>,
    ) -> Rc<Self> {
        let phase = connection.updates.borrow().phase;
        let output = Rc::new(Self {
            ui: ui.as_weak(),
            phase: Cell::new(phase),
            connection,
            renderer,
            size,
            presented,
            display: RefCell::new(display),
            clipboard,
            selected: RefCell::new(Selected::default()),
            projection: RefCell::new(None),
            hover: Cell::new(None),
            hover_state: RefCell::new(None),
            navigation: RefCell::new(None),
            navigation_cancel: RefCell::new(None),
            pending_navigation: Cell::new(None),
            click_acted: Cell::new(false),
            lease: RefCell::new(None),
            article,
            registered: Cell::new(None),
            selection_sequence: Cell::new(0),
            last_geometry: RefCell::new(None),
            worker: RefCell::new(None),
            picker: Cell::new(false),
            closed: Cell::new(false),
        });
        ui.set_output_available(true);
        ui.on_output_geometry_changed({
            let output = output.clone();
            move || output.revoke()
        });
        ui.on_grid_pointer({
            let output = output.clone();
            move |kind, x, y, shift| output.pointer(kind, x, y, shift)
        });
        ui.on_grid_wheel({
            let output = output.clone();
            move |delta| {
                let Some(ui) = output.ui.upgrade() else {
                    return false;
                };
                if ui.get_terminal_pointer_blocked()
                    || ui.get_selection_active()
                    || ui.get_selection_dragging()
                {
                    return false;
                }
                let Some(stamp) = output.current(&ui) else {
                    return false;
                };
                if !output.can_navigate(&ui) {
                    return false;
                }
                if !output.project(&ui, stamp) {
                    return false;
                }
                let intent = output
                    .projection
                    .borrow()
                    .as_ref()
                    .and_then(|p| pointer::wheel(&p.context, delta));
                if let Some(intent) = intent {
                    output.navigate(&ui, stamp, intent);
                    return true;
                }
                false
            }
        });
        ui.on_clear_selection({
            let output = output.clone();
            move || {
                if let Some(ui) = output.ui.upgrade() {
                    output.clear(&ui);
                }
            }
        });
        ui.on_output_updated({
            let output = output.clone();
            move || {
                if let Some(ui) = output.ui.upgrade() {
                    let phase = output.connection.updates.borrow().phase;
                    if output.phase.replace(phase) != phase {
                        output.clear(&ui);
                    }
                    let geometry = output.geometry(&ui);
                    let previous = output.last_geometry.replace(geometry.clone());
                    if previous.is_some() && previous != geometry {
                        output.revoke();
                    }
                    let current = output.current(&ui);
                    if ui.get_terminal_pointer_blocked()
                        || output
                            .pending_navigation
                            .get()
                            .is_some_and(|stamp| Some(stamp) != current)
                    {
                        output.cancel_navigation(&ui);
                    }
                    let stale = {
                        let selected = output.selected.borrow();
                        selected
                            .text
                            .as_ref()
                            .is_some_and(|(stamp, _)| Some(*stamp) != current)
                            || output.registered.get().is_some_and(|(id, request)| {
                                output.article.as_ref().is_none_or(|a| {
                                    !a.copied_output(
                                        id,
                                        request,
                                        &output.connection.updates.borrow(),
                                    )
                                })
                            })
                            || selected.gesture.as_ref().is_some_and(|gesture| {
                                Some(gesture.stamp) != current
                                    || output.geometry(&ui) != Some(gesture.geometry.clone())
                            })
                    };
                    if stale
                        || ui.get_terminal_pointer_blocked()
                            && output.selected.borrow().gesture.is_some()
                    {
                        output.clear(&ui);
                    }
                    output.filter_status(&ui);
                    output.overlay(&ui);
                    output.hover_overlay(&ui);
                    ui.invoke_station_color_updated();
                    ui.invoke_media_updated();
                }
            }
        });
        ui.on_copy_selection({
            let output = output.clone();
            move |ansi| output.copy(ansi)
        });
        ui.on_output_action({
            let output = output.clone();
            move |action| {
                let Some(ui) = output.ui.upgrade() else {
                    return;
                };
                match action {
                    0 => output.copy(false),
                    1 => output.copy(true),
                    2..=5 => output.export(action == 3 || action == 5, action >= 4),
                    6 => {
                        if let Some(stamp) = output.current(&ui) {
                            output.select(&ui, stamp, &Selection::All);
                            output.overlay(&ui);
                        }
                    }
                    7 => output.clear(&ui),
                    _ => {}
                }
            }
        });
        ui.on_filter_status_refresh({
            let output = output.clone();
            move || {
                if let Some(ui) = output.ui.upgrade() {
                    output.filter_status(&ui);
                }
            }
        });
        ui.on_filter_reveal({
            let output = output.clone();
            move || {
                if let Some(article) = &output.article {
                    article.toggle_reveal();
                }
            }
        });
        output
    }

    pub fn current(&self, ui: &TerminalWindow) -> Option<Stamp> {
        let view = self.connection.updates.borrow();
        if self.closed.get()
            || ui.get_frame_paint_pending()
            || ui.get_switching_station()
            || ui.get_login_open()
            || ui.get_paste_open()
            || view.output_pending
            || view.phase == Phase::Connecting
            || view.phase != self.phase.get()
        {
            return None;
        }
        if self.article.is_some() {
            return self
                .article
                .as_ref()?
                .output_identity(&view)
                .map(|_| view.stamp());
        }
        (!ui.get_frame_paint_pending()
            && self.size.get().is_some()
            && self.presented.get()
                == Some(FrameId {
                    generation: view.generation,
                    revision: view.frame.revision,
                }))
        .then(|| view.stamp())
    }
    pub fn geometry(&self, ui: &TerminalWindow) -> Option<SurfaceGeometry> {
        if ui.get_article_active() {
            self.article.as_ref()?.output_geometry()
        } else {
            Geometry::for_ui(ui, self.size.get()).map(SurfaceGeometry::from)
        }
    }
    pub fn policy(&self, _ui: &TerminalWindow) -> u64 {
        if self.article.is_some() {
            self.article
                .as_ref()
                .and_then(|a| a.output_identity(&self.connection.updates.borrow()))
                .map_or(0, |i| i.presentation)
        } else {
            0
        }
    }
    pub fn with_frame<R>(&self, ui: &TerminalWindow, read: impl FnOnce(&Frame) -> R) -> Option<R> {
        self.current(ui)?;
        if self.article.is_some() {
            self.article
                .as_ref()?
                .with_output(&self.connection.updates.borrow(), |paint| {
                    read(&paint.frame)
                })
        } else {
            Some(read(self.renderer.borrow().frame()))
        }
    }
    fn clear_text(&self) {
        if let Some((id, request)) = self.registered.take()
            && let Some(article) = &self.article
        {
            article.clear_output_selection(id, request);
        }
        self.selected.borrow_mut().text = None;
        self.revoke_selection();
    }
    fn select(&self, ui: &TerminalWindow, stamp: Stamp, selection: &Selection) {
        self.clear_text();
        let request = self.selection_sequence.get().wrapping_add(1);
        self.selection_sequence.set(request);
        let text = if self.article.is_some() {
            self.article
                .as_ref()
                .and_then(|article| {
                    article.select_output(&self.connection.updates.borrow(), request, selection)
                })
                .map(|(id, text)| {
                    self.registered.set(Some((id, request)));
                    text
                })
        } else {
            self.with_frame(ui, |frame| selection::extract(frame, selection).ok())
                .flatten()
        };
        self.selected.borrow_mut().text = text.map(|text| (stamp, text));
    }
    fn can_navigate(&self, ui: &TerminalWindow) -> bool {
        let view = self.connection.updates.borrow();
        !ui.get_terminal_pointer_blocked()
            && view.phase == Phase::Connected
            && !view.input_pending
            && !view.login_pending
            && view.mouse.route == pttstella::mouse::input::Route::Local
            && self.pending_navigation.get() != Some(view.stamp())
    }
    fn project(&self, ui: &TerminalWindow, stamp: Stamp) -> bool {
        if self.current(ui) != Some(stamp) {
            return false;
        }
        let policy = self.policy(ui);
        if self
            .projection
            .borrow()
            .as_ref()
            .is_some_and(|p| p.stamp == stamp && p.policy == policy)
        {
            return true;
        }
        let links = if self.article.is_some() {
            self.article
                .as_ref()
                .and_then(|a| a.with_output(&self.connection.updates.borrow(), |p| p.links.clone()))
        } else {
            self.with_frame(ui, links::detect)
        };
        let Some(links) = links else { return false };
        self.projection.replace(Some(Projection {
            stamp,
            policy,
            context: self.connection.updates.borrow().context(),
            links,
        }));
        true
    }
    pub fn link_at(&self, ui: &TerminalWindow, point: (f32, f32)) -> Option<usize> {
        let stamp = self.current(ui)?;
        if !self.project(ui, stamp) {
            return None;
        }
        let geometry = self.geometry(ui)?;
        let pos = self
            .with_frame(ui, |frame| geometry.cell(point, frame))
            .flatten()?;
        self.projection
            .borrow()
            .as_ref()?
            .links
            .iter()
            .position(|link| link.row == pos.row && link.start <= pos.col && pos.col < link.end)
    }
    fn hit(
        &self,
        ui: &TerminalWindow,
        stamp: Stamp,
        geometry: &SurfaceGeometry,
        point: (f32, f32),
    ) -> Option<Hit> {
        if !self.project(ui, stamp) {
            return None;
        }
        let account = self
            .account_context(ui, point)
            .filter(|target| target.author && target.reply.is_some());
        let projection = self.projection.borrow();
        let projection = projection.as_ref()?;
        if self.can_navigate(ui)
            && let Some(target) = account
            && !target.cell.is_some_and(|(row, col)| {
                projection
                    .links
                    .iter()
                    .any(|l| l.row == row && l.start <= col && col < l.end)
            })
            && let Some(row) = target.reply
        {
            return Some(Hit::Reply { row });
        }
        self.with_frame(ui, |frame| match geometry {
            SurfaceGeometry::Terminal(g) => pointer::hit(
                &projection.context,
                &projection.links,
                frame,
                *g,
                point,
                self.can_navigate(ui),
            ),
            SurfaceGeometry::Article(g) => {
                if !geometry.contains(point)
                    || point.1 < g.body_height
                        && g.flow.hit(g.offset + f64::from(point.1)).is_none()
                {
                    // Images have their own controls; a gap must not fall
                    // through to text selection, Back or half-page navigation.
                    return None;
                }
                let cell = geometry.cell(point, frame);
                if let Some(cell) = cell {
                    if let Some(index) = projection
                        .links
                        .iter()
                        .position(|l| l.row == cell.row && l.start <= cell.col && cell.col < l.end)
                    {
                        return Some(Hit::Link(index));
                    }
                    if !self.can_navigate(ui) || cell.row >= g.body_rows {
                        return None;
                    }
                    if cell.col < 17 {
                        return Some(Hit::Navigate(Intent::Back));
                    }
                }
                if !self.can_navigate(ui) {
                    return None;
                }
                if point.0 < g.left.max(0.) {
                    Some(Hit::Navigate(Intent::Back))
                } else if point.1 < g.body_height {
                    Some(Hit::Navigate(if point.1 < g.body_height / 2. {
                        Intent::PreviousPage
                    } else {
                        Intent::NextPage
                    }))
                } else {
                    None
                }
            }
        })
        .flatten()
    }
    fn pointer(self: &Rc<Self>, kind: i32, x: f32, y: f32, shift: bool) {
        let Some(ui) = self.ui.upgrade() else { return };
        let dpi = ui.window().scale_factor();
        let point = (x * dpi, y * dpi);
        if kind == 4 {
            self.hover.set((!shift).then_some(point));
            self.hover_overlay(&ui);
            return;
        }
        if kind == 5 {
            self.hover.set(None);
            self.hover_overlay(&ui);
            return;
        }
        if ui.get_terminal_pointer_blocked() || ui.get_pan_mode() && !ui.get_article_active() {
            return;
        }
        let Some(stamp) = self.current(&ui) else {
            self.clear(&ui);
            return;
        };
        let Some(geometry) = self.geometry(&ui) else {
            return;
        };
        let local_selection = self
            .selected
            .borrow()
            .gesture
            .as_ref()
            .is_some_and(|gesture| gesture.shift);
        if !shift
            && !local_selection
            && self.connection.updates.borrow().mouse.route != pttstella::mouse::input::Route::Local
        {
            self.clear(&ui);
            return;
        }
        if kind == 6 {
            // Slint dispatches double-clicked before the second pointer-up.
            // An author click is a toggle: keep its pending release, while
            // Shift selection and other double-click actions retain their path.
            if self.click_acted.get()
                && !shift
                && !self
                    .selected
                    .borrow()
                    .gesture
                    .as_ref()
                    .is_some_and(|g| g.shift)
                && self
                    .account_context(&ui, point)
                    .is_some_and(|a| a.author && a.reply.is_some())
            {
                return;
            }
            self.selected.borrow_mut().gesture = None;
            if !self.click_acted.get()
                && let Some(position) = self
                    .with_frame(&ui, |frame| geometry.cell(point, frame))
                    .flatten()
            {
                self.select(&ui, stamp, &Selection::Line { row: position.row });
            }
            self.overlay(&ui);
            self.hover_overlay(&ui);
            return;
        }
        let position = self
            .with_frame(&ui, |frame| geometry.cell(point, frame))
            .flatten();
        let result =
            self.selected
                .borrow_mut()
                .track(position, stamp, geometry.clone(), kind, point, shift);
        match result.selection {
            SelectionUpdate::Keep => {}
            SelectionUpdate::Clear => self.clear_text(),
            SelectionUpdate::Set(selection) => self.select(&ui, stamp, &selection),
        }
        self.overlay(&ui);
        self.hover.set((kind != 3).then_some(point));
        if let Some(click) = result.click {
            self.click_acted.set(false);
            let from = self.hit(&ui, stamp, &geometry, click.from);
            let to = self.hit(&ui, stamp, &geometry, click.to);
            if from == to {
                match to {
                    Some(Hit::Reply { .. }) => {
                        if let Some(target) = self.account_context(&ui, click.to) {
                            self.click_acted.set(self.focus_account(&target, false));
                        }
                    }
                    Some(Hit::Navigate(intent)) => self.navigate(&ui, stamp, intent),
                    Some(Hit::Link(index)) => {
                        let url = self
                            .projection
                            .borrow()
                            .as_ref()
                            .and_then(|p| p.links.get(index))
                            .map(|link| link.url.clone());
                        if let Some(url) = url
                            && self.current(&ui) == Some(stamp)
                            && links::validate(&url).is_ok()
                        {
                            self.click_acted.set(true);
                            ui.invoke_media_request(index as i32);
                        }
                    }
                    None => {}
                }
            }
        }
        self.hover_overlay(&ui);
    }
    fn hover_overlay(&self, ui: &TerminalWindow) {
        ui.invoke_media_hover_changed();
        let (stamp, geometry) = (self.current(ui), self.geometry(ui));
        let hit = if !ui.get_terminal_pointer_blocked()
            && !ui.get_selection_active()
            && !ui.get_selection_dragging()
            && (!ui.get_pan_mode() || ui.get_article_active())
            && self.connection.updates.borrow().mouse.route == pttstella::mouse::input::Route::Local
        {
            stamp
                .zip(geometry.clone())
                .zip(self.hover.get())
                .and_then(|((stamp, geometry), point)| self.hit(ui, stamp, &geometry, point))
        } else {
            None
        };
        let state = Hover {
            stamp,
            policy: self.policy(ui),
            geometry,
            hit,
        };
        if self.hover_state.replace(Some(state.clone())) == Some(state) {
            return;
        }
        let mut rectangles = Vec::new();
        ui.set_pointer_cursor(match hit {
            Some(Hit::Link(_) | Hit::Reply { .. }) => 1,
            Some(Hit::Navigate(Intent::Back)) => 2,
            Some(Hit::Navigate(Intent::First | Intent::PreviousPage)) => 3,
            Some(Hit::Navigate(Intent::Last | Intent::NextPage)) => 4,
            Some(Hit::Navigate(_)) => 1,
            None => 0,
        });
        ui.set_pointer_url(match hit {
            Some(Hit::Link(index)) => self
                .projection
                .borrow()
                .as_ref()
                .and_then(|p| p.links.get(index))
                .map(|l| l.url.clone())
                .unwrap_or_default()
                .into(),
            _ => "".into(),
        });
        if let (Some(Hit::Navigate(Intent::Open { row })), Some(size)) = (hit, self.size.get()) {
            let frame = self.renderer.borrow();
            let dpi = ui.window().scale_factor();
            let rect = size.cell(0, row, frame.frame().cols, frame.frame());
            rectangles.push(SelectionRect {
                x: rect.left / dpi,
                y: rect.top / dpi,
                width: rect.width() / dpi,
                height: rect.height() / dpi,
            });
        }
        ui.set_navigation_rects(ModelRc::new(VecModel::from(rectangles)));
    }
    fn cancel_navigation(&self, ui: &TerminalWindow) {
        if let Some(cancel) = self.navigation_cancel.borrow_mut().take() {
            cancel.cancel();
        }
        if let Some(job) = self.navigation.borrow_mut().take() {
            job.abort();
        }
        self.pending_navigation.set(None);
        ui.set_navigation_busy(false);
    }
    fn navigate(self: &Rc<Self>, ui: &TerminalWindow, stamp: Stamp, intent: Intent) {
        if self.current(ui) != Some(stamp) || !self.can_navigate(ui) {
            return;
        }
        if ui.get_article_active()
            && let Some(article) = &self.article
        {
            self.click_acted.set(true);
            if article.local_intent(intent) {
                return;
            }
            article.station_handoff();
        }
        self.cancel_navigation(ui);
        let (reply, cancel) =
            match self
                .connection
                .request_cancellable_navigate(stamp, intent, Source::Pointer)
            {
                Ok(reply) => reply,
                Err(error) => {
                    ui.set_input_notice(error.into());
                    return;
                }
            };
        self.navigation_cancel.replace(Some(cancel));
        self.pending_navigation.set(Some(stamp));
        self.click_acted.set(true);
        ui.set_navigation_busy(true);
        ui.invoke_keep_alive_changed();
        let weak = Rc::downgrade(self);
        match slint::spawn_local(async move {
            let result = reply.wait().await;
            let Some(output) = weak.upgrade() else { return };
            if output.closed.get() || output.pending_navigation.get() != Some(stamp) {
                return;
            }
            output.pending_navigation.set(None);
            output.navigation_cancel.borrow_mut().take();
            if let Some(ui) = output.ui.upgrade() {
                ui.set_navigation_busy(false);
                if let Err(error) = result
                    && output.connection.updates.borrow().generation == stamp.generation
                {
                    ui.set_input_notice(error.into());
                }
                output.hover_overlay(&ui);
                ui.invoke_keep_alive_changed();
            }
        }) {
            Ok(job) => {
                self.navigation.replace(Some(job));
            }
            Err(error) => {
                self.cancel_navigation(ui);
                ui.set_input_notice(error.to_string().into());
            }
        }
    }
    pub(super) fn revoke(&self) {
        if let Some(active) = self.lease.borrow_mut().take() {
            match active.lease {
                WorkLease::Terminal(lease) => lease.revoke(),
                WorkLease::Article(_) => {
                    if let Some(article) = &self.article {
                        article.revoke_output();
                    }
                }
            }
        }
    }
    pub(super) fn clipboard(&self) -> Clipboard {
        self.clipboard.clone()
    }
    pub(super) fn busy(&self) -> bool {
        self.picker.get() || self.worker.borrow().is_some()
    }
    pub(super) fn media_output_lease(
        &self,
        ui: &TerminalWindow,
        scope: &pttstella::media::hover::Lease,
    ) -> Result<(WorkLease, Phase), String> {
        let mut scope = scope.clone();
        if !self.renew_media(ui, &mut scope) {
            return Err(msg!("export-stale"));
        }
        self.revoke();
        let view = self.connection.updates.borrow();
        let lease = if let Some(article) = &self.article {
            WorkLease::Article(
                article
                    .lease_media_output(&view)
                    .ok_or_else(|| msg!("export-stale"))?,
            )
        } else {
            WorkLease::Terminal(Lease::new(view.stamp(), true))
        };
        self.lease.replace(Some(ActiveLease {
            lease: lease.clone(),
            selected: false,
        }));
        Ok((lease, view.phase))
    }
    fn revoke_selection(&self) {
        if self
            .lease
            .borrow()
            .as_ref()
            .is_some_and(|active| active.selected)
        {
            self.revoke();
        }
    }
    fn clear(&self, ui: &TerminalWindow) {
        self.clear_text();
        self.selected.borrow_mut().gesture = None;
        self.hover.set(None);
        self.cancel_navigation(ui);
        self.overlay(ui);
        self.hover_overlay(ui);
    }
    fn overlay(&self, ui: &TerminalWindow) {
        let selected = self.selected.borrow();
        let mut rectangles = Vec::new();
        if let (Some(geometry), Some((_, text))) = (self.geometry(ui), selected.text.as_ref()) {
            self.with_frame(ui, |frame| {
                for span in &text.spans {
                    if let Some(rect) = geometry.span(span, frame) {
                        let dpi = geometry.dpi();
                        rectangles.push(SelectionRect {
                            x: rect.left / dpi,
                            y: rect.top / dpi,
                            width: rect.width() / dpi,
                            height: rect.height() / dpi,
                        });
                    }
                }
            });
        }
        ui.set_selection_rects(ModelRc::new(VecModel::from(rectangles)));
        ui.set_selection_active(selected.text.is_some());
        ui.set_selection_rows_ready(
            selected
                .text
                .as_ref()
                .is_some_and(|(_, text)| text.rows.len() <= export::ROWS),
        );
        ui.set_selection_dragging(selected.gesture.is_some());
        drop(selected);
        ui.invoke_context_updated();
        ui.invoke_station_color_updated();
        ui.invoke_keep_alive_changed();
    }
    pub fn selection_key(&self) -> Option<(Range<usize>, Vec<selection::Span>)> {
        self.selected
            .borrow()
            .text
            .as_ref()
            .map(|(_, text)| (text.rows.clone(), text.spans.clone()))
    }
    pub fn prepare_context(&self, ui: &TerminalWindow) {
        self.selected.borrow_mut().gesture = None;
        self.cancel_navigation(ui);
        self.hover.set(None);
        self.overlay(ui);
        self.hover_overlay(ui);
    }
    pub fn media_url(&self, ui: &TerminalWindow, index: usize) -> Option<String> {
        self.media_lease(ui, index, 0).map(|lease| lease.link.url)
    }
    pub fn forget_hover(&self) {
        self.hover.set(None);
    }
    /// Hover may survive a stationary press to promote the same download.
    /// Selection/drag/Shift still cancel before a new image can be admitted.
    pub fn media_hover_source(
        &self,
        ui: &TerminalWindow,
    ) -> Option<(pttstella::media::hover::Lease, (f32, f32))> {
        if ui.get_media_hover_blocked()
            || self
                .selected
                .borrow()
                .gesture
                .as_ref()
                .is_some_and(|g| g.moved || g.shift)
            || self.connection.updates.borrow().mouse.route != pttstella::mouse::input::Route::Local
        {
            return None;
        }
        let stamp = self.current(ui)?;
        let geometry = self.geometry(ui)?;
        let point = self.hover.get()?;
        let Hit::Link(index) = self.hit(ui, stamp, &geometry, point)? else {
            return None;
        };
        let scope = self.media_lease(ui, index, 0)?;
        Some((scope, (point.0 / geometry.dpi(), point.1 / geometry.dpi())))
    }
    pub fn media_lease(
        &self,
        ui: &TerminalWindow,
        index: usize,
        sequence: u64,
    ) -> Option<pttstella::media::hover::Lease> {
        let stamp = self.current(ui)?;
        if !self.project(ui, stamp) {
            return None;
        }
        let link = self.projection.borrow().as_ref()?.links.get(index)?.clone();
        Some(pttstella::media::hover::Lease {
            sequence,
            stamp,
            index,
            link,
        })
    }
    pub fn renew_media(
        &self,
        ui: &TerminalWindow,
        lease: &mut pttstella::media::hover::Lease,
    ) -> bool {
        let view = self.connection.updates.borrow();
        if let Some(article) = &self.article {
            return article.renew_media(&view, lease);
        }
        let complete = view.phase == Phase::Connected
            && !view.output_pending
            && !view.input_pending
            && view.frame.control_complete;
        let _ = ui;
        lease.renew(view.stamp(), complete, &links::detect(&view.frame))
    }
    pub fn context_action(
        self: &Rc<Self>,
        action: pttstella::context_menu::Action,
        link: Option<usize>,
    ) {
        use pttstella::context_menu::Action as A;
        let Some(ui) = self.ui.upgrade() else { return };
        match action {
            A::Settings => ui.invoke_settings_show(),
            A::EditorColor => ui.global::<crate::StationColorState>().invoke_action(0),
            A::Copy => self.copy(false),
            A::CopyAnsi => self.copy(true),
            A::ExportRows => self.export(true, false),
            A::ExportPage => self.export(false, false),
            A::CopyImageRows => self.export(true, true),
            A::CopyImagePage => self.export(false, true),
            A::SelectAll => ui.invoke_output_action(6),
            A::SelectLink => {
                if let (Some(stamp), Some(index)) = (self.current(&ui), link) {
                    self.select(&ui, stamp, &Selection::Link { index });
                    self.overlay(&ui);
                }
            }
            A::CopyLink | A::OpenLink => {
                if let (Some(stamp), Some(index)) = (self.current(&ui), link) {
                    if !self.project(&ui, stamp) {
                        return;
                    }
                    let url = self
                        .projection
                        .borrow()
                        .as_ref()
                        .and_then(|p| p.links.get(index))
                        .map(|l| l.url.clone());
                    if let Some(url) = url {
                        if action == A::OpenLink
                            && self.connection.updates.borrow().mouse.route
                                == pttstella::mouse::input::Route::Local
                        {
                            ui.invoke_link_activated(url.into());
                        } else if action == A::CopyLink && !self.busy() {
                            let phase = self.phase.get();
                            let Some(lease) = self.new_lease(&ui, stamp, false) else {
                                return;
                            };
                            let source = self.connection.updates.clone();
                            let clipboard = self.clipboard.clone();
                            self.work(
                                &ui,
                                move || {
                                    check_output(&lease, &source.borrow(), phase)?;
                                    clipboard.write_text(url, || {
                                        check_output(&lease, &source.borrow(), phase)
                                    })
                                },
                                |_, ui, result| {
                                    ui.set_input_notice(
                                        match result {
                                            Ok(()) => msg!("context-copied"),
                                            Err(e) => e,
                                        }
                                        .into(),
                                    );
                                },
                            );
                        }
                    }
                }
            }
            A::Back | A::Previous | A::Next => {
                if let Some(stamp) = self.current(&ui) {
                    self.navigate(
                        &ui,
                        stamp,
                        match action {
                            A::Back => Intent::Back,
                            A::Previous => Intent::PreviousPage,
                            _ => Intent::NextPage,
                        },
                    );
                }
            }
            A::Paste => ui.invoke_paste_request(),
            _ => {}
        }
    }
    fn new_lease(&self, _ui: &TerminalWindow, stamp: Stamp, selected: bool) -> Option<WorkLease> {
        self.revoke();
        let lease = if self.article.is_some() {
            let request = if selected {
                Some(self.registered.get()?.1)
            } else {
                None
            };
            WorkLease::Article(
                self.article
                    .as_ref()?
                    .lease_output(&self.connection.updates.borrow(), request)?,
            )
        } else {
            WorkLease::Terminal(Lease::new(stamp, true))
        };
        self.lease.replace(Some(ActiveLease {
            lease: lease.clone(),
            selected,
        }));
        Some(lease)
    }
    pub(super) fn prepare_copy(
        &self,
        ui: &TerminalWindow,
        ansi: bool,
    ) -> Result<(String, WorkLease, Phase), String> {
        let stamp = self.current(ui).ok_or_else(|| msg!("copy-not-ready"))?;
        let phase = self.phase.get();
        let text = if self.article.is_some() {
            let (_, request) = self
                .registered
                .get()
                .ok_or_else(|| msg!("copy-not-ready"))?;
            self.article.as_ref().and_then(|a| {
                a.copy_output(
                    &self.connection.updates.borrow(),
                    &if ansi {
                        CopyTarget::SelectionAnsi { request }
                    } else {
                        CopyTarget::Selection { request }
                    },
                )
            })
        } else {
            let selected = self.selected.borrow();
            selected
                .text
                .as_ref()
                .filter(|(source, _)| *source == stamp)
                .and_then(|(_, text)| {
                    self.with_frame(ui, |frame| {
                        if ansi {
                            selection::ansi(frame, &text.spans, text.rows.clone())
                        } else {
                            text.text.clone()
                        }
                    })
                })
        };
        let text = text.ok_or_else(|| msg!("copy-not-ready"))?;
        let lease = self
            .new_lease(ui, stamp, true)
            .ok_or_else(|| msg!("copy-not-ready"))?;
        Ok((text, lease, phase))
    }
    fn copy(self: &Rc<Self>, ansi: bool) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.busy() {
            return;
        }
        let (text, lease, phase) = match self.prepare_copy(&ui, ansi) {
            Ok(prepared) => prepared,
            Err(error) => {
                ui.set_input_notice(error.into());
                return;
            }
        };
        let source = self.connection.updates.clone();
        let clipboard = self.clipboard.clone();
        self.work(
            &ui,
            move || {
                check_output(&lease, &source.borrow(), phase)?;
                clipboard.write_text(text, || check_output(&lease, &source.borrow(), phase))
            },
            |_, ui, result| {
                if let Err(error) = result {
                    ui.set_input_notice(error.into());
                }
            },
        );
    }
    pub(super) fn prepare_export(
        &self,
        ui: &TerminalWindow,
        selected: bool,
    ) -> Result<(Frame, Range<usize>, WorkLease, Phase), String> {
        if selected
            && self
                .selected
                .borrow()
                .text
                .as_ref()
                .is_some_and(|(_, text)| text.rows.len() > export::ROWS)
        {
            return Err(msg!("export-cache-selection-limit"));
        }
        let stamp = self
            .current(ui)
            .ok_or_else(|| msg!("export-screen-changed"))?;
        let phase = self.phase.get();
        let snapshot = if self.article.is_some() {
            let request = if selected {
                self.registered.get().map(|(_, request)| request)
            } else {
                None
            };
            if selected && request.is_none() {
                Err(msg!("export-selection-changed"))
            } else {
                self.article
                    .as_ref()
                    .ok_or_else(|| msg!("export-screen-changed"))
                    .and_then(|a| a.export_output(&self.connection.updates.borrow(), request))
            }
        } else {
            let rows = if selected {
                self.selected
                    .borrow()
                    .text
                    .as_ref()
                    .filter(|(source, _)| *source == stamp)
                    .map(|(_, text)| text.rows.clone())
            } else {
                Some(0..export::ROWS)
            };
            rows.ok_or_else(|| msg!("export-selection-changed"))
                .map(|rows| (self.renderer.borrow().frame().clone(), rows))
        };
        let (frame, rows) = snapshot?;
        let lease = self
            .new_lease(ui, stamp, selected)
            .ok_or_else(|| msg!("export-screen-changed"))?;
        Ok((frame, rows, lease, phase))
    }
    fn export(self: &Rc<Self>, selected: bool, clipboard: bool) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.busy() {
            return;
        }
        let (frame, rows, lease, phase) = match self.prepare_export(&ui, selected) {
            Ok(prepared) => prepared,
            Err(error) => {
                ui.set_input_notice(error.into());
                return;
            }
        };
        let display = self.display.borrow().clone();
        let source = self.connection.updates.clone();
        let render_lease = lease.clone();
        let render_source = source.clone();
        self.work(
            &ui,
            move || {
                check_output(&render_lease, &render_source.borrow(), phase)?;
                render_png(frame, display, rows)
            },
            move |output, ui, result| {
                let image = match result {
                    Ok(image) => image,
                    Err(error) => {
                        ui.set_input_notice(error.into());
                        return;
                    }
                };
                if let Err(error) = check_output(&lease, &source.borrow(), phase) {
                    ui.set_input_notice(error.into());
                    return;
                }
                if clipboard {
                    let clipboard = output.clipboard.clone();
                    output.work(
                        ui,
                        move || {
                            check_output(&lease, &source.borrow(), phase)?;
                            clipboard.write_image(image.image, || {
                                check_output(&lease, &source.borrow(), phase)
                            })
                        },
                        |_, ui, result| {
                            ui.set_input_notice(
                                match result {
                                    Ok(()) => msg!("export-copied"),
                                    Err(error) => error,
                                }
                                .into(),
                            )
                        },
                    );
                } else {
                    // Keep the native picker parented, matching the existing desktop export owner.
                    let path = ui
                        .window()
                        .with_winit_window(|window| {
                            rfd::FileDialog::new()
                                .set_parent(window)
                                .set_title(msg!("export-title"))
                                .add_filter("PNG", &["png"])
                                .set_file_name("PTTStella.png")
                                .save_file()
                        })
                        .flatten();
                    let Some(path) = path else {
                        ui.set_input_notice(msg!("export-cancelled").into());
                        return;
                    };
                    output.work(
                        ui,
                        move || {
                            check_output(&lease, &source.borrow(), phase)?;
                            if !path
                                .extension()
                                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
                            {
                                return Err(msg!("export-extension"));
                            }
                            pttstella::storage::replace(&path, &image.png)
                        },
                        |_, ui, result| {
                            ui.set_input_notice(
                                match result {
                                    Ok(()) => msg!("export-saved"),
                                    Err(error) => error,
                                }
                                .into(),
                            )
                        },
                    );
                }
            },
        );
    }
    pub(super) fn begin_picker(&self, ui: &TerminalWindow) -> bool {
        if self.closed.get() || self.busy() {
            return false;
        }
        self.picker.set(true);
        ui.set_output_busy(true);
        ui.invoke_keep_alive_changed();
        true
    }
    pub(super) fn end_picker(&self, ui: &TerminalWindow) {
        self.picker.set(false);
        if self.worker.borrow().is_none() {
            ui.set_output_busy(false);
            ui.invoke_output_idle();
        }
        ui.invoke_keyboard_updated();
        ui.invoke_keep_alive_changed();
    }

    pub(super) fn work<T: Send + 'static>(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        work: impl FnOnce() -> Result<T, String> + Send + 'static,
        complete: impl FnOnce(&Rc<Self>, &TerminalWindow, Result<T, String>) + 'static,
    ) {
        if self.busy() {
            ui.set_input_notice(msg!("file-busy").into());
            return;
        }
        ui.set_output_busy(true);
        ui.invoke_keep_alive_changed();
        let (send, receive) = tokio::sync::oneshot::channel();
        match std::thread::Builder::new()
            .name("pttstella-slint-output".into())
            .spawn(move || {
                let _ = send.send(work());
            }) {
            Ok(worker) => self.worker.replace(Some(worker)),
            Err(error) => {
                ui.set_output_busy(false);
                ui.invoke_keep_alive_changed();
                ui.set_input_notice(error.to_string().into());
                return;
            }
        };
        let output = self.clone();
        if let Err(error) = slint::spawn_local(async move {
            let result = receive
                .await
                .unwrap_or_else(|_| Err(msg!("export-prepare-failed")));
            let joined = output
                .worker
                .borrow_mut()
                .take()
                .map(|worker| worker.join());
            if output.closed.get() {
                return;
            }
            if let Some(ui) = output.ui.upgrade() {
                let result = if joined.is_some_and(|result| result.is_err()) {
                    Err(msg!("export-prepare-failed"))
                } else {
                    result
                };
                complete(&output, &ui, result);
                if output.worker.borrow().is_none() {
                    ui.set_output_busy(false);
                }
                // Completion is outside the native keyboard borrow. Retire
                // the cached disabled actions after this worker has joined.
                ui.invoke_keyboard_updated();
                ui.invoke_keep_alive_changed();
                if !output.busy() {
                    ui.invoke_output_idle();
                }
            }
        }) {
            self.revoke();
            self.worker.borrow_mut().take().map(|worker| worker.join());
            ui.set_output_busy(false);
            ui.set_input_notice(error.to_string().into());
        }
    }
    pub fn set_display(&self, display: DisplaySettings) {
        self.revoke();
        self.display.replace(display);
        self.size.set(None);
        if let Some(ui) = self.ui.upgrade() {
            ui.invoke_geometry_changed();
        }
    }

    pub fn finish(&self) -> Result<(), String> {
        self.closed.set(true);
        if let Some(ui) = self.ui.upgrade() {
            self.cancel_navigation(&ui);
        }
        self.revoke();
        if let Some(worker) = self.worker.borrow_mut().take() {
            worker.join().map_err(|_| msg!("export-prepare-failed"))?;
        }
        Ok(())
    }
}

pub(super) fn check_output(lease: &WorkLease, view: &View, phase: Phase) -> Result<(), String> {
    lease.check(view)?;
    if view.phase != phase {
        return Err(msg!("export-stale"));
    }
    Ok(())
}

/// A history export copies at most 24 projected rows, never a whole document
/// or an allocation proportional to its height. Input glyph ordering is the
/// same Screen/presentation contract used by selection::extract.
pub(super) fn crop_rows(frame: &Frame, rows: Range<usize>) -> Result<Frame, String> {
    if frame.cols != 80 || rows.is_empty() || rows.len() > export::ROWS || rows.end > frame.rows {
        return Err(msg!("export-rows-invalid"));
    }
    let start = frame.glyphs.partition_point(|g| g.row < rows.start);
    let end = frame.glyphs.partition_point(|g| g.row < rows.end);
    Ok(Frame {
        cols: 80,
        rows: 24,
        cursor: (0, 0),
        cursor_visible: false,
        revision: frame.revision,
        control_complete: frame.control_complete,
        unsupported: frame.unsupported,
        glyphs: frame.glyphs[start..end]
            .iter()
            .map(|g| {
                let mut g = g.clone();
                g.row -= rows.start;
                g
            })
            .collect(),
        hyperlinks: frame
            .hyperlinks
            .iter()
            .filter_map(|link| {
                let spans = link
                    .spans
                    .iter()
                    .filter(|span| rows.contains(&span.row))
                    .map(|span| {
                        let mut span = span.clone();
                        span.row -= rows.start;
                        span
                    })
                    .collect::<Vec<_>>();
                (!spans.is_empty()).then(|| {
                    let mut link = link.clone();
                    link.spans = spans;
                    link
                })
            })
            .collect(),
    })
}

pub(super) struct Png {
    pub(super) png: Vec<u8>,
    pub(super) image: arboard::ImageData<'static>,
}
pub(super) fn render_png(
    frame: Frame,
    display: DisplaySettings,
    rows: Range<usize>,
) -> Result<Png, String> {
    if (frame.cols, frame.rows) != (80, export::ROWS)
        || rows.start >= rows.end
        || rows.end > export::ROWS
    {
        return Err(msg!("export-rows-invalid"));
    }
    let renderer = Renderer::new(frame, &display).map_err(str::to_string)?;
    let pixels = renderer
        .paint(
            Size {
                width: export::WIDTH,
                height: export::HEIGHT,
            },
            false,
        )
        .map_err(str::to_string)?;
    let stride = export::WIDTH as usize * 4;
    let height = rows.len() * export::ROW_HEIGHT as usize;
    let begin = rows.start * export::ROW_HEIGHT as usize * stride;
    let rgba = pixels.as_bytes()[begin..begin + height * stride].to_vec();
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, export::WIDTH, height as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&rgba))
            .map_err(|_| msg!("export-encode-failed"))?;
    }
    export::check_export_header(&png, &rows)?;
    Ok(Png {
        png,
        image: arboard::ImageData {
            width: export::WIDTH as usize,
            height,
            bytes: Cow::Owned(rgba),
        },
    })
}
