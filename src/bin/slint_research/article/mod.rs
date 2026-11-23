//! Native long-text viewport. Station loading and input use separate owners.
mod find_probe;
mod footer;
pub mod inline;
mod layout;
pub(crate) mod output_probe;
pub mod probe;
pub mod replies_probe;
pub mod session;
pub mod session_probe;
#[cfg(test)]
mod tests;

use crate::{
    ArticleTile, TerminalWindow,
    renderer::{Renderer, Size},
};
use layout::Layout;
use pttstella::{
    article_cache::{ArticleCache, LoadState, Reading},
    article_flow::{Anchor, Flow},
    navigation::Intent,
    presentation::{self, Paint},
    screen::Screen,
    settings::DisplaySettings,
};
use slint::{ComponentHandle, Image, ModelRc, Timer, VecModel};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ops::Range,
    rc::Rc,
    time::Duration,
};

#[derive(Clone, Debug, PartialEq)]
struct RasterKey {
    rows: Range<usize>,
    width: u32,
    row: u32,
    dpi: f32,
    flow: Rc<Flow>,
}

type RowProjection = Box<dyn Fn(&Paint, Range<usize>) -> Option<pttstella::article_cache::Window>>;
type ReplyIndex = Option<(
    pttstella::surface_state::Identity,
    Rc<pttstella::reply_navigation::Index>,
)>;

#[derive(Clone, Copy)]
enum ReadingMove {
    Key(Intent),
    Scroll(f64),
}

impl ReadingMove {
    fn station_intent(self) -> Intent {
        match self {
            Self::Key(intent @ (Intent::First | Intent::Last)) => intent,
            Self::Key(Intent::PreviousPage | Intent::Lines { down: false, .. }) => {
                Intent::PreviousPage
            }
            Self::Scroll(delta) if delta < 0. => Intent::PreviousPage,
            _ => Intent::NextPage,
        }
    }
}

pub struct ArticleView {
    ui: slint::Weak<TerminalWindow>,
    project: RowProjection,
    session: std::rc::Weak<session::ArticleSession>,
    motion_revision: Cell<u64>,
    source: RefCell<Paint>,
    rows: Cell<usize>,
    scope: Cell<Option<(u64, u64, u64, usize)>>,
    renderer: RefCell<Renderer>,
    layout: Cell<Option<Layout>>,
    anchor: Cell<Anchor>,
    free_scroll: Cell<bool>,
    gaps: RefCell<Vec<(usize, u32)>>,
    flow: RefCell<Option<Rc<Flow>>>,
    x: Cell<f32>,
    painted: RefCell<Option<RasterKey>>,
    painted_anchor: Cell<Option<(Anchor, bool)>>,
    reading: RefCell<Reading>,
    footer_key: Cell<Option<(Size, footer::Display)>>,
    row_size: Cell<Option<Size>>,
    row_cache: RefCell<BTreeMap<usize, Image>>,
    row_raster_count: Cell<usize>,
    timer: Timer,
    closed: Cell<bool>,
    raster_count: Cell<usize>,
    reply_index: RefCell<ReplyIndex>,
}
impl ArticleView {
    pub fn mount(
        ui: &TerminalWindow,
        cache: ArticleCache,
        source: Paint,
        renderer: Renderer,
    ) -> Result<Rc<Self>, String> {
        let window = cache
            .window(&source, 0..1)
            .ok_or("article source has no confirmed cache")?;
        Self::mount_projected(
            ui,
            source,
            window.body_rows,
            window
                .paint
                .article
                .ok_or("article reading status unavailable")?,
            renderer,
            Box::new(move |source, rows| cache.window(source, rows)),
            std::rc::Weak::new(),
        )
    }
    pub fn mount_shared(
        ui: &TerminalWindow,
        core: Rc<RefCell<pttstella::surface_state::SurfaceState>>,
        source: Paint,
        rows: usize,
        reading: Reading,
        renderer: Renderer,
        session: std::rc::Weak<session::ArticleSession>,
    ) -> Result<Rc<Self>, String> {
        Self::mount_projected(
            ui,
            source,
            rows,
            reading,
            renderer,
            Box::new(move |source, rows| {
                let core = core.borrow();
                let mut window = core.article_window(source, rows.clone())?;
                core.find_decorate(source, &mut window.paint.frame, rows.start);
                Some(window)
            }),
            session,
        )
    }
    fn mount_projected(
        ui: &TerminalWindow,
        source: Paint,
        rows: usize,
        reading: Reading,
        renderer: Renderer,
        project: RowProjection,
        session: std::rc::Weak<session::ArticleSession>,
    ) -> Result<Rc<Self>, String> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            renderer: RefCell::new(renderer),
            project,
            session,
            motion_revision: Cell::new(0),
            source: RefCell::new(source),
            rows: Cell::new(rows),
            scope: Cell::new(None),
            layout: Cell::new(None),
            anchor: Cell::new(Anchor::default()),
            free_scroll: Cell::new(false),
            gaps: RefCell::new(Vec::new()),
            flow: RefCell::new(None),
            x: Cell::new(0.),
            painted: RefCell::new(None),
            painted_anchor: Cell::new(None),
            reading: RefCell::new(reading),
            footer_key: Cell::new(None),
            row_size: Cell::new(None),
            row_cache: RefCell::new(BTreeMap::new()),
            row_raster_count: Cell::new(0),
            timer: Timer::default(),
            closed: Cell::new(false),
            raster_count: Cell::new(0),
            reply_index: RefCell::new(None),
        });
        ui.on_article_layout_changed({
            let owner = owner.clone();
            move || owner.resize()
        });
        ui.on_article_scroll({
            let owner = owner.clone();
            move |delta, horizontal| {
                if !owner.allowed() || !delta.is_finite() {
                    return;
                }
                if horizontal {
                    if let Some(layout) = owner.layout.get() {
                        owner.x.set(
                            (owner.x.get() - delta / (layout.line_height() / 2.))
                                .clamp(0., layout.maximum_x() / (layout.line_height() / 2.)),
                        );
                        owner.publish();
                    }
                } else if delta != 0. {
                    owner.move_from_input(ReadingMove::Scroll(-f64::from(delta)));
                }
            }
        });
        ui.on_article_navigate({
            let owner = owner.clone();
            move |code| {
                owner.navigate_from_input(match code {
                    0 => Intent::Lines {
                        down: false,
                        count: 1,
                    },
                    1 => Intent::Lines {
                        down: true,
                        count: 1,
                    },
                    2 => Intent::PreviousPage,
                    3 => Intent::NextPage,
                    4 => Intent::First,
                    5 => Intent::Last,
                    _ => return,
                });
            }
        });
        ui.on_article_scroll_to({
            let owner = owner.clone();
            move |vertical, fraction| {
                if !owner.allowed() || !fraction.is_finite() {
                    return;
                }
                let Some(layout) = owner.layout.get() else {
                    return;
                };
                let fraction = fraction.clamp(0., 1.);
                if vertical {
                    if let Some(flow) = owner.flow() {
                        owner.scroll_pixels(
                            flow.maximum(f64::from(layout.body_height * layout.dpi))
                                * f64::from(fraction),
                        );
                    }
                } else {
                    owner
                        .x
                        .set(layout.maximum_x() / (layout.line_height() / 2.) * fraction);
                    owner.publish();
                }
            }
        });
        ui.on_article_zoom_request({
            let weak = ui.as_weak();
            move |delta| {
                if let Some(ui) = weak.upgrade()
                    && ui.get_article_active()
                    && !ui.get_article_blocked()
                    && delta.is_finite()
                    && delta != 0.
                {
                    ui.set_zoom(
                        (ui.get_zoom() + if delta > 0. { 0.25 } else { -0.25 }).clamp(0.25, 4.),
                    );
                }
            }
        });
        ui.set_article_active(true);
        owner.resize();
        Ok(owner)
    }
    fn allowed(&self) -> bool {
        !self.closed.get()
            && self
                .ui
                .upgrade()
                .is_some_and(|ui| ui.get_article_active() && !ui.get_article_blocked())
    }
    /// Both toolkit and native keyboard routes use the same row geometry.
    fn navigate_from_input(self: &Rc<Self>, intent: Intent) {
        self.move_from_input(ReadingMove::Key(intent));
    }

    fn move_from_input(self: &Rc<Self>, movement: ReadingMove) {
        if self.move_reading(movement)
            && let Some(session) = self.session.upgrade()
        {
            session.at_reading_edge(movement, self.motion_revision.get());
        }
    }

    pub fn navigate(self: &Rc<Self>, intent: Intent) {
        self.move_reading(ReadingMove::Key(intent));
    }

    /// Reports a move that was already at the requested edge.
    fn move_reading(self: &Rc<Self>, movement: ReadingMove) -> bool {
        if !self.allowed() {
            return false;
        }
        let (Some(layout), Some(flow)) = (self.layout.get(), self.flow()) else {
            return false;
        };
        let anchor = self.position(&flow);
        let viewport = f64::from(layout.body_height * layout.dpi);
        let maximum = flow.maximum(viewport);
        let target = match movement {
            ReadingMove::Scroll(delta) => anchor + delta * f64::from(layout.dpi),
            ReadingMove::Key(intent) => match intent {
                Intent::Lines { down, count } => flow.move_lines(anchor, viewport, down, count),
                Intent::PreviousPage => flow.page(anchor, viewport, false),
                Intent::NextPage | Intent::Forward => flow.page(anchor, viewport, true),
                Intent::First => 0.,
                Intent::Last => maximum,
                _ => return false,
            },
        };
        self.move_viewport(target, matches!(movement, ReadingMove::Scroll(_)));
        match movement.station_intent() {
            Intent::First | Intent::PreviousPage => anchor <= 0.01,
            _ => maximum - anchor <= 0.01,
        }
    }
    fn flow(&self) -> Option<Rc<Flow>> {
        self.flow.borrow().clone()
    }
    fn position(&self, flow: &Flow) -> f64 {
        let position = flow.position(self.anchor.get());
        self.layout.get().map_or(position, |layout| {
            flow.clamp(position, f64::from(layout.body_height * layout.dpi))
        })
    }
    fn whole_rows(&self, flow: &Flow, layout: Layout) -> Option<Range<usize>> {
        let viewport = f64::from(layout.body_height * layout.dpi);
        (!self.free_scroll.get() && viewport >= f64::from(layout.row))
            .then(|| flow.whole_rows(self.position(flow), viewport))
    }
    /// Numeric text anchor for the original uniform-row conformance workload.
    fn row_anchor(&self) -> f64 {
        let a = self.anchor.get();
        a.row as f64 + a.within
    }
    pub(super) fn set_gaps(self: &Rc<Self>, gaps: Vec<(usize, u32)>) -> Result<(), String> {
        let Some(layout) = self.layout.get() else {
            return Err("article layout missing".into());
        };
        let flow =
            Flow::new(self.rows.get(), layout.row, gaps.iter().copied()).map_err(str::to_owned)?;
        if *self.gaps.borrow() == gaps {
            return Ok(());
        }
        self.gaps.replace(gaps);
        self.flow.replace(Some(Rc::new(flow)));
        self.painted.take();
        if let Some(ui) = self.ui.upgrade() {
            ui.invoke_output_geometry_changed();
        }
        self.publish();
        self.schedule();
        Ok(())
    }
    pub fn geometry(&self) -> Option<crate::pointer::SurfaceGeometry> {
        let ui = self.ui.upgrade()?;
        if self.closed.get()
            || !ui.get_article_active()
            || self.painted_anchor.get() != Some((self.anchor.get(), self.free_scroll.get()))
        {
            return None;
        }
        let l = self.layout.get()?;
        let flow = self.flow()?;
        if Some(l)
            != Layout::fit(
                ui.get_view_width(),
                ui.get_view_height(),
                ui.window().scale_factor(),
                ui.get_zoom(),
                self.rows.get(),
            )
        {
            return None;
        }
        let key = RasterKey {
            rows: flow.visible(self.position(&flow), f64::from(l.body_height * l.dpi), 2),
            width: l.width,
            row: l.row,
            dpi: l.dpi,
            flow: flow.clone(),
        };
        if self.painted.borrow().as_ref() != Some(&key) {
            return None;
        }
        Some(crate::pointer::SurfaceGeometry::Article(
            crate::pointer::ArticleGeometry {
                left: ((l.view_width - l.logical_width()).max(0.) / 2. - ui.get_article_offset_x())
                    * l.dpi,
                width: l.view_width * l.dpi,
                body_height: l.body_height * l.dpi,
                offset: self.position(&flow),
                whole_rows: self.whole_rows(&flow, l),
                flow,
                line_width: l.width,
                row: l.row,
                body_rows: self.rows.get(),
                dpi: l.dpi,
            },
        ))
    }
    pub(super) fn find_changed(self: &Rc<Self>) {
        if self.closed.get() || self.ui.upgrade().is_none_or(|ui| !ui.get_article_active()) {
            return;
        }
        self.row_cache.borrow_mut().clear();
        self.painted.take();
        self.schedule();
    }
    pub(super) fn find_reveal(self: &Rc<Self>, hit: pttstella::article_find::Hit) {
        let Some(layout) = self.layout.get() else {
            return;
        };
        let Some(flow) = self.flow() else { return };
        let visible = layout.body_height / layout.line_height();
        self.x.set(
            (hit.start as f32 - (layout.view_width / layout.line_height() * 2.) / 2.)
                .clamp(0., layout.maximum_x() / (layout.line_height() / 2.)),
        );
        self.scroll_pixels(flow.top(hit.row) as f64 - f64::from(visible * layout.row as f32) / 2.);
    }
    fn resize(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.closed.get() || !ui.get_article_active() {
            return;
        }
        let layout = Layout::fit(
            ui.get_view_width(),
            ui.get_view_height(),
            ui.window().scale_factor(),
            ui.get_zoom(),
            self.rows.get(),
        );
        if layout != self.layout.get() {
            ui.invoke_output_geometry_changed();
        }
        self.layout.set(layout);
        self.flow.replace(None);
        if let Some(layout) = layout {
            let flow = match Flow::new(
                self.rows.get(),
                layout.row,
                self.gaps.borrow().iter().copied(),
            ) {
                Ok(flow) => flow,
                Err(error) => {
                    crate::notice::error(&ui, error);
                    ui.set_article_tiles(ModelRc::default());
                    ui.set_article_footer(Image::default());
                    self.painted.take();
                    return;
                }
            };
            self.flow.replace(Some(Rc::new(flow)));
            // Layout may temporarily expose more rows while controls reflow.
            // Clamp the effective viewport, not the requested text/image anchor;
            // a later layout or append must not inherit that transient clamp.
            // Explicit scrolling owns changes to the requested anchor.
            self.x.set(
                self.x
                    .get()
                    .clamp(0., layout.maximum_x() / (layout.line_height() / 2.)),
            );
            self.publish();
            self.schedule();
        } else {
            ui.set_article_tiles(ModelRc::default());
            ui.set_article_footer(Image::default());
            self.painted.take();
            self.footer_key.set(None);
            self.row_cache.borrow_mut().clear();
            self.row_size.set(None);
        }
    }
    fn scroll_pixels(self: &Rc<Self>, position: f64) {
        self.move_viewport(position, true);
    }
    fn move_viewport(self: &Rc<Self>, position: f64, free_scroll: bool) {
        let Some(layout) = self.layout.get() else {
            return;
        };
        let Some(flow) = self.flow() else { return };
        self.free_scroll.set(free_scroll);
        let anchor = flow.anchor(flow.clamp(position, f64::from(layout.body_height * layout.dpi)));
        if self.anchor.replace(anchor) != anchor {
            self.motion_revision
                .set(self.motion_revision.get().wrapping_add(1));
        }
        self.publish();
        self.schedule();
    }
    fn publish(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let Some(layout) = self.layout.get() else {
            return;
        };
        let Some(flow) = self.flow() else { return };
        ui.set_article_line_height(layout.line_height());
        ui.set_article_line_width(layout.logical_width());
        ui.set_article_body_height(layout.body_height);
        ui.set_article_document_height(flow.height() as f32 / layout.dpi);
        ui.set_article_offset_y((self.position(&flow) / f64::from(layout.dpi)) as f32);
        ui.set_article_offset_x(self.x.get() * layout.line_height() / 2.);
        self.reply_marks();
        ui.invoke_article_updates_scrolled();
    }
    fn schedule(self: &Rc<Self>) {
        if self.timer.running() {
            return;
        }
        let owner = Rc::downgrade(self);
        self.timer
            .start(slint::TimerMode::SingleShot, Duration::ZERO, move || {
                if let Some(owner) = owner.upgrade() {
                    if owner.closed.get() {
                        return;
                    }
                    if let Err(error) = owner.paint()
                        && let Some(ui) = owner.ui.upgrade()
                    {
                        crate::notice::error(&ui, error);
                    }
                }
            });
    }
    fn paint(&self) -> Result<(), String> {
        let Some(ui) = self.ui.upgrade() else {
            return Ok(());
        };
        if !ui.get_article_active() {
            return Ok(());
        }
        let Some(layout) = self.layout.get() else {
            return Ok(());
        };
        let flow = self.flow().ok_or("article row geometry unavailable")?;
        let key = RasterKey {
            rows: flow.visible(
                self.position(&flow),
                f64::from(layout.body_height * layout.dpi),
                2,
            ),
            width: layout.width,
            row: layout.row,
            dpi: layout.dpi,
            flow: flow.clone(),
        };
        if self.painted.borrow().as_ref() != Some(&key) {
            let size = Size {
                width: layout.width,
                height: layout.row,
            };
            let mut cached = self.row_cache.borrow_mut();
            if self.row_size.replace(Some(size)) != Some(size) {
                cached.clear();
            }
            cached.retain(|row, _| key.rows.contains(row));
            let mut runs: Vec<Range<usize>> = Vec::new();
            for row in key.rows.clone().filter(|row| !cached.contains_key(row)) {
                if let Some(run) = runs.last_mut()
                    && run.end == row
                    && run.len() < layout.tile_rows()
                {
                    run.end += 1;
                } else {
                    runs.push(row..row + 1);
                }
            }
            let mut renderer = self.renderer.borrow_mut();
            for run in runs {
                let window = (self.project)(&self.source.borrow(), run.clone())
                    .ok_or("article row source expired")?;
                let mut template = window.paint.frame;
                let mut rows = (0..run.len()).map(|_| Vec::new()).collect::<Vec<_>>();
                let mut links = vec![Vec::new(); run.len()];
                for link in window.paint.links {
                    if let Some(columns) = links.get_mut(link.row) {
                        columns.push(link.start..link.end);
                    }
                }
                for mut glyph in std::mem::take(&mut template.glyphs) {
                    if glyph.row < rows.len() {
                        let row = glyph.row;
                        glyph.row = 0;
                        rows[row].push(glyph);
                    }
                }
                template.rows = 1;
                template.hyperlinks.clear();
                template.cursor = (0, 0);
                template.cursor_visible = false;
                for (row, (glyphs, columns)) in rows.into_iter().zip(links).enumerate() {
                    let mut frame = template.clone();
                    frame.glyphs = glyphs;
                    renderer.set_frame(frame);
                    cached.insert(
                        run.start + row,
                        Image::from_rgba8_premultiplied(
                            renderer.paint_article_row(size, &columns)?,
                        ),
                    );
                    self.row_raster_count.set(self.row_raster_count.get() + 1);
                }
            }
            self.painted.replace(Some(key.clone()));
            self.raster_count.set(self.raster_count.get() + 1);
        }
        let cached = self.row_cache.borrow();
        let top = self.position(&flow);
        let whole_rows = self.whole_rows(&flow, layout);
        let tiles = key
            .rows
            .clone()
            .filter(|row| whole_rows.as_ref().is_none_or(|rows| rows.contains(row)))
            .map(|row| {
                let image = cached
                    .get(&row)
                    .ok_or("confirmed article row raster is missing")?;
                Ok(ArticleTile {
                    image: image.clone(),
                    y: ((flow.top(row) as f64 - top) / f64::from(layout.dpi)) as f32,
                    height: layout.line_height(),
                })
            })
            .collect::<Result<Vec<_>, &str>>()?;
        ui.set_article_tiles(ModelRc::new(VecModel::from(tiles)));
        drop(cached);
        let size = Size {
            width: layout.width,
            height: layout.row,
        };
        let reading = self.reading.borrow();
        let display = footer::display(
            &reading,
            &flow,
            layout.row,
            top,
            top + f64::from(layout.body_height * layout.dpi),
            whole_rows,
        );
        let footer_key = (size, display);
        if self.footer_key.get() != Some(footer_key) {
            let source = self.source.borrow();
            let footer = footer::frame(&source.frame, &reading, display);
            let columns = source
                .links
                .iter()
                .filter(|link| link.row + 1 == source.frame.rows)
                .map(|link| link.start..link.end)
                .collect::<Vec<_>>();
            drop(source);
            let mut renderer = self.renderer.borrow_mut();
            renderer.set_frame(footer);
            ui.set_article_footer(Image::from_rgba8_premultiplied(
                renderer.paint_article_row(size, &columns)?,
            ));
            self.footer_key.set(Some(footer_key));
        }
        drop(reading);
        self.painted_anchor
            .set(Some((self.anchor.get(), self.free_scroll.get())));
        self.reply_marks();
        ui.invoke_article_rendered();
        Ok(())
    }
    /// Reuse installed pixels for a status-only packet. The caller acknowledges
    /// after releasing its view slot, so UI callbacks cannot reborrow that slot.
    pub fn update(
        self: &Rc<Self>,
        source: Paint,
        rows: usize,
        scope: (u64, u64, u64, usize),
        reading: Reading,
    ) -> bool {
        let footer_changed = footer::changed(&self.reading.borrow(), &reading);
        self.reading.replace(reading);
        let unchanged = self.scope.get() == Some(scope)
            && self.rows.get() == rows
            && self.geometry().is_some()
            && {
                let old = self.source.borrow();
                old.generation == source.generation
                    && old.presentation == source.presentation
                    && old.frame == source.frame
                    && old.settings == source.settings
            };
        if unchanged {
            self.source.replace(source);
            if footer_changed {
                self.footer_key.set(None);
                self.schedule();
            }
            return !footer_changed;
        }
        let previous_scope = self.scope.replace(Some(scope));
        if previous_scope != Some(scope) {
            self.anchor.set(
                if let Some(previous) = previous_scope
                    && (previous.0, previous.1, previous.2) == (scope.0, scope.1, scope.2)
                    && scope.3 < previous.3
                {
                    // Keep the same source row when a verified page is prepended.
                    let anchor = self.anchor.get();
                    Anchor {
                        row: anchor.row + previous.3 - scope.3,
                        within: anchor.within,
                    }
                } else {
                    Anchor::default()
                },
            );
            self.free_scroll.set(false);
            self.gaps.borrow_mut().clear();
            self.x.set(0.);
            self.row_cache.borrow_mut().clear();
            self.row_size.set(None);
        }
        self.source.replace(source);
        self.rows.set(rows);
        self.painted.take();
        self.footer_key.set(None);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_article_active(true);
        }
        self.resize();
        false
    }
    pub(super) fn rebase_reply_focus(&self, old: (u64, u64, u64), new: (u64, u64, u64)) {
        if let Some(scope) = self.scope.get()
            && (scope.0, scope.1, scope.2) == old
        {
            self.timer.stop();
            self.scope.set(Some((new.0, new.1, new.2, scope.3)));
            self.row_cache.borrow_mut().clear();
            self.row_size.set(None);
            self.footer_key.set(None);
            self.painted.take();
        }
    }
    pub fn hide(&self) {
        self.reply_index.borrow_mut().take();
        self.retire_paint();
        self.footer_key.set(None);
        self.row_cache.borrow_mut().clear();
        self.scope.set(None);
        self.flow.replace(None);
        self.gaps.borrow_mut().clear();
        self.anchor.set(Anchor::default());
        self.free_scroll.set(false);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_article_active(false);
            ui.set_article_tiles(ModelRc::default());
            ui.set_article_footer(Image::default());
            ui.set_reply_side_marks(ModelRc::default());
        }
    }
    /// Superseded source packets stop painting without removing the focused
    /// article component. Its next update installs the new source and geometry.
    pub fn copy_display(&self, source: &Renderer) {
        self.renderer.borrow_mut().copy_display(source);
        self.row_cache.borrow_mut().clear();
        self.row_size.set(None);
        self.footer_key.set(None);
        self.painted.take();
    }

    pub fn retire_paint(&self) {
        self.timer.stop();
        self.painted.take();
    }
    pub fn finish(&self) {
        self.closed.set(true);
        self.flow.replace(None);
        self.gaps.borrow_mut().clear();
        self.timer.stop();
        self.row_cache.borrow_mut().clear();
        self.row_size.set(None);
        self.footer_key.set(None);
        self.painted.take();
        if let Some(ui) = self.ui.upgrade() {
            ui.set_article_active(false);
            ui.set_article_tiles(ModelRc::default());
            ui.set_article_footer(Image::default());
        }
    }
}

/// The existing 38-page workload, with no HTTP, station or credential access.
pub fn fixture(display: &DisplaySettings) -> Result<(ArticleCache, Paint), String> {
    let mut cache = ArticleCache::default();
    let mut screen = Screen::new(80, 24).map_err(|e| e.to_string())?;
    let mut source = None;
    for page in 1..=38 {
        screen.feed(&pttstella::replay::long_load_page(
            page,
            Some("http://127.0.0.1:9"),
        )?);
        let mut packet = presentation::sample_packet(&screen.frame(), display);
        packet.connected = true;
        packet.ptt_features = true;
        packet.generation = 7;
        cache.capture_source(&packet, None);
        source = Some(packet);
    }
    if cache.load_state() != Some(LoadState::Complete) {
        return Err("reading fixture did not reach confirmed end".into());
    }
    Ok((cache, source.ok_or("reading fixture is empty")?))
}
