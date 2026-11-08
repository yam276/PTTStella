//! Native long-text viewport. Station loading and input use separate owners.
mod find_probe;
pub mod inline;
mod layout;
mod output_probe;
pub mod probe;
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
    article_cache::{ArticleCache, LoadState},
    article_flow::{Anchor, Flow},
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

pub struct ArticleView {
    ui: slint::Weak<TerminalWindow>,
    project: RowProjection,
    source: RefCell<Paint>,
    rows: Cell<usize>,
    scope: Cell<Option<(u64, u64, u64, usize)>>,
    renderer: RefCell<Renderer>,
    layout: Cell<Option<Layout>>,
    anchor: Cell<Anchor>,
    gaps: RefCell<Vec<(usize, u32)>>,
    flow: RefCell<Option<Rc<Flow>>>,
    x: Cell<f32>,
    painted: RefCell<Option<RasterKey>>,
    painted_anchor: Cell<Option<Anchor>>,
    footer_size: Cell<Option<Size>>,
    row_size: Cell<Option<Size>>,
    row_cache: RefCell<BTreeMap<usize, Image>>,
    row_raster_count: Cell<usize>,
    timer: Timer,
    closed: Cell<bool>,
    raster_count: Cell<usize>,
}
impl ArticleView {
    pub fn mount(
        ui: &TerminalWindow,
        cache: ArticleCache,
        source: Paint,
        renderer: Renderer,
    ) -> Result<Rc<Self>, String> {
        let rows = cache
            .window(&source, 0..1)
            .ok_or("article source has no confirmed cache")?
            .body_rows;
        Self::mount_projected(
            ui,
            source,
            rows,
            renderer,
            Box::new(move |source, rows| cache.window(source, rows)),
        )
    }
    pub fn mount_shared(
        ui: &TerminalWindow,
        core: Rc<RefCell<pttstella::surface_state::SurfaceState>>,
        source: Paint,
        rows: usize,
        renderer: Renderer,
    ) -> Result<Rc<Self>, String> {
        Self::mount_projected(
            ui,
            source,
            rows,
            renderer,
            Box::new(move |source, rows| {
                let core = core.borrow();
                let mut window = core.article_window(source, rows.clone())?;
                core.find_decorate(source, &mut window.paint.frame, rows.start);
                Some(window)
            }),
        )
    }
    fn mount_projected(
        ui: &TerminalWindow,
        source: Paint,
        rows: usize,
        renderer: Renderer,
        project: RowProjection,
    ) -> Result<Rc<Self>, String> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            renderer: RefCell::new(renderer),
            project,
            source: RefCell::new(source),
            rows: Cell::new(rows),
            scope: Cell::new(None),
            layout: Cell::new(None),
            anchor: Cell::new(Anchor::default()),
            gaps: RefCell::new(Vec::new()),
            flow: RefCell::new(None),
            x: Cell::new(0.),
            painted: RefCell::new(None),
            painted_anchor: Cell::new(None),
            footer_size: Cell::new(None),
            row_size: Cell::new(None),
            row_cache: RefCell::new(BTreeMap::new()),
            row_raster_count: Cell::new(0),
            timer: Timer::default(),
            closed: Cell::new(false),
            raster_count: Cell::new(0),
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
                } else if let (Some(layout), Some(flow)) = (owner.layout.get(), owner.flow()) {
                    owner.scroll_pixels(owner.position(&flow) - f64::from(delta * layout.dpi));
                }
            }
        });
        ui.on_article_navigate({
            let owner = owner.clone();
            move |code| {
                use pttstella::navigation::Intent;
                owner.navigate(match code {
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
    pub fn navigate(self: &Rc<Self>, intent: pttstella::navigation::Intent) {
        use pttstella::navigation::Intent;
        if !self.allowed() {
            return;
        }
        let Some(layout) = self.layout.get() else {
            return;
        };
        let Some(flow) = self.flow() else { return };
        let anchor = self.position(&flow);
        let line = f64::from(layout.row);
        let viewport = f64::from(layout.body_height * layout.dpi);
        let target = match intent {
            Intent::Lines { down, count } => {
                anchor
                    + if down {
                        count as f64 * line
                    } else {
                        -(count as f64) * line
                    }
            }
            Intent::PreviousPage => {
                (anchor / line).ceil() * line - (viewport / line).floor() * line + line
            }
            Intent::NextPage | Intent::Forward => {
                ((anchor + viewport) / line).floor() * line - line
            }
            Intent::First => 0.,
            Intent::Last => flow.maximum(f64::from(layout.body_height * layout.dpi)),
            _ => return,
        };
        let target = match intent {
            Intent::NextPage | Intent::Forward if target <= anchor => anchor + line,
            Intent::PreviousPage if target >= anchor => anchor - line,
            _ => target,
        };
        self.scroll_pixels(target);
    }
    fn flow(&self) -> Option<Rc<Flow>> {
        self.flow.borrow().clone()
    }
    fn position(&self, flow: &Flow) -> f64 {
        flow.position(self.anchor.get())
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
        self.scroll_pixels(self.position(&flow));
        Ok(())
    }
    pub fn geometry(&self) -> Option<crate::pointer::SurfaceGeometry> {
        let ui = self.ui.upgrade()?;
        if self.closed.get()
            || !ui.get_article_active()
            || self.painted_anchor.get() != Some(self.anchor.get())
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
                    ui.set_input_notice(error.into());
                    ui.set_article_tiles(ModelRc::default());
                    ui.set_article_footer(Image::default());
                    self.painted.take();
                    return;
                }
            };
            self.flow.replace(Some(Rc::new(flow)));
            self.anchor.set(flow.anchor(flow.clamp(
                self.position(&flow),
                f64::from(layout.body_height * layout.dpi),
            )));
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
            self.footer_size.set(None);
            self.row_cache.borrow_mut().clear();
            self.row_size.set(None);
        }
    }
    fn scroll_pixels(self: &Rc<Self>, position: f64) {
        let Some(layout) = self.layout.get() else {
            return;
        };
        let Some(flow) = self.flow() else { return };
        self.anchor
            .set(flow.anchor(flow.clamp(position, f64::from(layout.body_height * layout.dpi))));
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
                        ui.set_input_notice(error.into());
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
                for (row, glyphs) in rows.into_iter().enumerate() {
                    let mut frame = template.clone();
                    frame.glyphs = glyphs;
                    renderer.set_frame(frame);
                    cached.insert(
                        run.start + row,
                        Image::from_rgba8_premultiplied(renderer.paint(size, false)?),
                    );
                    self.row_raster_count.set(self.row_raster_count.get() + 1);
                }
            }
            self.painted.replace(Some(key.clone()));
            self.raster_count.set(self.raster_count.get() + 1);
        }
        let cached = self.row_cache.borrow();
        let tiles = key
            .rows
            .clone()
            .map(|row| {
                let image = cached
                    .get(&row)
                    .ok_or("confirmed article row raster is missing")?;
                Ok(ArticleTile {
                    image: image.clone(),
                    y: ((flow.top(row) as f64 - self.position(&flow)) / f64::from(layout.dpi))
                        as f32,
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
        if self.footer_size.get() != Some(size) {
            let mut footer = self.source.borrow().frame.clone();
            footer.glyphs.retain(|g| g.row + 1 == footer.rows);
            for g in &mut footer.glyphs {
                g.row = 0;
            }
            footer.rows = 1;
            footer.hyperlinks.clear();
            footer.cursor_visible = false;
            footer.cursor = (0, 0);
            let mut renderer = self.renderer.borrow_mut();
            renderer.set_frame(footer);
            ui.set_article_footer(Image::from_rgba8_premultiplied(
                renderer.paint(size, false)?,
            ));
            self.footer_size.set(Some(size));
        }
        self.painted_anchor.set(Some(self.anchor.get()));
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
    ) -> bool {
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
            return true;
        }
        if self.scope.replace(Some(scope)) != Some(scope) {
            self.anchor.set(Anchor::default());
            self.gaps.borrow_mut().clear();
            self.x.set(0.);
            self.row_cache.borrow_mut().clear();
            self.row_size.set(None);
        }
        self.source.replace(source);
        self.rows.set(rows);
        self.painted.take();
        self.footer_size.set(None);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_article_active(true);
        }
        self.resize();
        false
    }
    pub fn hide(&self) {
        self.retire_paint();
        self.row_cache.borrow_mut().clear();
        self.scope.set(None);
        self.flow.replace(None);
        self.gaps.borrow_mut().clear();
        self.anchor.set(Anchor::default());
        if let Some(ui) = self.ui.upgrade() {
            ui.set_article_active(false);
            ui.set_article_tiles(ModelRc::default());
            ui.set_article_footer(Image::default());
        }
    }
    /// Superseded source packets stop painting without removing the focused
    /// article component. Its next update installs the new source and geometry.
    pub fn copy_display(&self, source: &Renderer) {
        self.renderer.borrow_mut().copy_display(source);
        self.row_cache.borrow_mut().clear();
        self.row_size.set(None);
        self.footer_size.set(None);
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
        self.footer_size.set(None);
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
