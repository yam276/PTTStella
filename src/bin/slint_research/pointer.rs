//! Geometry and hit priority for the confirmed native terminal viewport.
use crate::renderer::Size;
use pttstella::{
    links::Link,
    navigation::{Context, Intent},
    screen::Frame,
    selection::Position,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub size: Size,
    pub left: f32,
    pub top: f32,
    pub width: f32,
    pub height: f32,
    pub dpi: f32,
}
/// The article has uniform physical rows and a separately pinned source footer.
/// This is geometry only: document height never allocates a raster.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArticleGeometry {
    pub left: f32,
    pub width: f32,
    pub body_height: f32,
    pub offset: f32,
    pub line_width: u32,
    pub row: u32,
    pub body_rows: usize,
    pub dpi: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfaceGeometry {
    Terminal(Geometry),
    Article(ArticleGeometry),
}
impl From<Geometry> for SurfaceGeometry {
    fn from(value: Geometry) -> Self {
        Self::Terminal(value)
    }
}
impl SurfaceGeometry {
    pub fn terminal(self) -> Option<Geometry> {
        if let Self::Terminal(g) = self {
            Some(g)
        } else {
            None
        }
    }
    pub fn dpi(self) -> f32 {
        match self {
            Self::Terminal(g) => g.dpi,
            Self::Article(g) => g.dpi,
        }
    }
    pub fn contains(self, point: (f32, f32)) -> bool {
        match self {
            Self::Terminal(g) => g.contains(point),
            Self::Article(g) => {
                point.0.is_finite()
                    && point.1.is_finite()
                    && point.0 >= 0.
                    && point.0 < g.width
                    && point.1 >= 0.
                    && point.1 < g.body_height + g.row as f32
            }
        }
    }
    pub fn cell(self, point: (f32, f32), frame: &Frame) -> Option<Position> {
        match self {
            Self::Terminal(g) => g.cell(point, frame),
            Self::Article(g) => {
                if !self.contains(point) || frame.rows != g.body_rows + 1 {
                    return None;
                }
                let y = if point.1 < g.body_height {
                    point.1 + g.offset
                } else {
                    g.body_rows as f32 * g.row as f32 + point.1 - g.body_height
                };
                let pos = Size {
                    width: g.line_width,
                    height: g.row.checked_mul(u32::try_from(frame.rows).ok()?)?,
                }
                .hit(point.0 - g.left, y, frame)?;
                // No hit can fall through the pinned footer onto hidden body text.
                (point.1 >= g.body_height || pos.row < g.body_rows).then_some(pos)
            }
        }
    }
    /// Article rectangles are clipped in viewport space. Terminal rectangles
    /// retain the raster-local coordinates consumed by its Flickable.
    pub fn span(self, span: &pttstella::selection::Span, frame: &Frame) -> Option<skia_safe::Rect> {
        match self {
            Self::Terminal(g) => {
                Some(
                    g.size
                        .cell(span.start, span.row, span.end - span.start, frame),
                )
            }
            Self::Article(g) => {
                if frame.rows != g.body_rows + 1
                    || span.row > g.body_rows
                    || span.start >= span.end
                    || span.end > frame.cols
                {
                    return None;
                }
                let size = Size {
                    width: g.line_width,
                    height: g.row.checked_mul(u32::try_from(frame.rows).ok()?)?,
                };
                let rect = size.cell(span.start, span.row, span.end - span.start, frame);
                let (top, bottom, clip_top, clip_bottom) = if span.row == g.body_rows {
                    (
                        g.body_height,
                        g.body_height + g.row as f32,
                        g.body_height,
                        g.body_height + g.row as f32,
                    )
                } else {
                    (
                        rect.top - g.offset,
                        rect.bottom - g.offset,
                        0.,
                        g.body_height,
                    )
                };
                let r = skia_safe::Rect::new(
                    (rect.left + g.left).max(0.),
                    top.max(clip_top),
                    (rect.right + g.left).min(g.width),
                    bottom.min(clip_bottom),
                );
                (r.width() > 0. && r.height() > 0.).then_some(r)
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Navigate(Intent),
    Link(usize),
}
impl Geometry {
    pub fn for_ui(ui: &crate::TerminalWindow, size: Option<Size>) -> Option<Self> {
        use slint::ComponentHandle;
        let dpi = ui.window().scale_factor();
        Some(Self {
            size: size?,
            left: ui.get_raster_left() * dpi,
            top: ui.get_raster_top() * dpi,
            width: ui.get_view_width() * dpi,
            height: ui.get_view_height() * dpi,
            dpi,
        })
    }
    pub fn contains(self, (x, y): (f32, f32)) -> bool {
        x.is_finite() && y.is_finite() && x >= 0. && y >= 0. && x < self.width && y < self.height
    }
    pub fn cell(self, point: (f32, f32), frame: &Frame) -> Option<Position> {
        self.contains(point)
            .then(|| {
                self.size
                    .hit(point.0 - self.left, point.1 - self.top, frame)
            })
            .flatten()
    }
    fn page(self, y: f32, top: f32, bottom: f32, halves: bool) -> Option<Intent> {
        let (top, bottom) = (top.max(0.), bottom.min(self.height));
        if y < top || y >= bottom {
            return None;
        }
        let edge = ((self.size.height as f32 / 24.).clamp(16. * self.dpi, 32. * self.dpi))
            .min((bottom - top) / 4.);
        if y < top + edge {
            Some(Intent::First)
        } else if y >= bottom - edge {
            Some(Intent::Last)
        } else if halves {
            Some(if y < (top + bottom) / 2. {
                Intent::PreviousPage
            } else {
                Intent::NextPage
            })
        } else {
            None
        }
    }
}

fn back(context: &Context) -> Option<(usize, usize)> {
    let (first, end) = match context {
        Context::Unknown { .. } | Context::MainMenu { .. } | Context::ContinuePrompt => {
            return None;
        }
        Context::ArticleReading { .. } => (1, 17),
        Context::ArticleList { articles, .. } => (articles.first().map_or(3, |a| a.row), 17),
        Context::BoardList { first_row, .. } => (*first_row, 10),
        Context::SystemMenu { first_row, .. }
        | Context::DocumentList { first_row, .. }
        | Context::CommandHelp { first_row, .. } => (*first_row, 20),
        Context::EmptyList => (1, 20),
    };
    // The original reader admits body rows 0..23; other profiles start below their headers.
    Some((
        if matches!(context, Context::ArticleReading { .. }) {
            0
        } else {
            first
        },
        end,
    ))
}
fn edges(context: &Context) -> bool {
    !matches!(
        context,
        Context::Unknown { .. } | Context::EmptyList | Context::ContinuePrompt
    )
}

pub fn hit(
    context: &Context,
    links: &[Link],
    frame: &Frame,
    geometry: Geometry,
    point: (f32, f32),
    can_navigate: bool,
) -> Option<Hit> {
    if !geometry.contains(point) {
        return None;
    }
    let cell = geometry.cell(point, frame);
    if matches!(context, Context::ContinuePrompt) {
        return (can_navigate && cell.is_some()).then_some(Hit::Navigate(Intent::Continue));
    }
    if let Some(cell) = cell {
        if let Some(index) = links
            .iter()
            .position(|link| link.row == cell.row && link.start <= cell.col && cell.col < link.end)
        {
            return Some(Hit::Link(index));
        }
        if !can_navigate {
            return None;
        }
        if back(context)
            .is_some_and(|(first, end)| cell.row >= first && cell.row < 23 && cell.col < end)
        {
            return Some(Hit::Navigate(Intent::Back));
        }
        if matches!(context, Context::ArticleReading { .. }) {
            if cell.row >= 23 {
                return None;
            }
            return geometry
                .page(
                    point.1,
                    geometry.top,
                    geometry.top + geometry.size.height as f32 * 23. / 24.,
                    true,
                )
                .map(Hit::Navigate);
        }
        if edges(context) {
            if cell.row == frame.rows - 1 {
                return Some(Hit::Navigate(Intent::Last));
            }
            if let Some(intent) = geometry
                .page(point.1, 0., geometry.height, false)
                .or_else(|| {
                    geometry.page(
                        point.1,
                        geometry.top,
                        geometry.top + geometry.size.height as f32 * 23. / 24.,
                        false,
                    )
                })
            {
                return Some(Hit::Navigate(intent));
            }
        }
        let open = match context {
            Context::MainMenu {
                first_row,
                last_row,
                ..
            }
            | Context::SystemMenu {
                first_row,
                last_row,
                ..
            } => cell.col >= 20 && (*first_row..=*last_row).contains(&cell.row),
            Context::BoardList {
                first_row,
                last_row,
                ..
            }
            | Context::DocumentList {
                first_row,
                last_row,
                ..
            } => cell.col >= 1 && (*first_row..=*last_row).contains(&cell.row),
            Context::ArticleList { articles, .. } => articles.iter().any(|a| a.row == cell.row),
            _ => false,
        };
        return open.then_some(Hit::Navigate(Intent::Open { row: cell.row }));
    }
    if !can_navigate || !edges(context) {
        return None;
    }
    if let Some(intent) = geometry.page(point.1, 0., geometry.height, false) {
        return Some(Hit::Navigate(intent));
    }
    let (x, y) = point;
    let top = geometry.top.max(0.);
    let bottom = (geometry.top + geometry.size.height as f32).min(geometry.height);
    if y < top {
        return Some(Hit::Navigate(Intent::PreviousPage));
    }
    if y >= bottom {
        return Some(Hit::Navigate(Intent::NextPage));
    }
    if x < geometry.left.max(0.) {
        return back(context).map(|_| Hit::Navigate(Intent::Back));
    }
    if x >= (geometry.left + geometry.size.width as f32).min(geometry.width)
        && !matches!(context, Context::CommandHelp { .. })
    {
        return Some(Hit::Navigate(Intent::Forward));
    }
    None
}

pub fn wheel(context: &Context, delta: f32) -> Option<Intent> {
    (delta.is_finite() && delta != 0. && edges(context)).then_some(if delta > 0. {
        Intent::PreviousPage
    } else {
        Intent::NextPage
    })
}

#[cfg(test)]
#[path = "pointer_tests.rs"]
mod tests;
