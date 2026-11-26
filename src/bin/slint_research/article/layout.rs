//! Physical-pixel row geometry. Anchors are article row plus fractional row.
use pttstella::article_cache::MAX_WINDOW_ROWS;
#[cfg(test)]
use std::ops::Range;

/// Width of the horizontal scrollbar strip, reserved only while zoom
/// overflows the view horizontally.
pub const SCROLLBAR: f32 = 12.;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub width: u32,
    pub row: u32,
    pub dpi: f32,
    pub view_width: f32,
    pub view_height: f32,
    /// Logical inset above the BBS box; matches the terminal raster's centring.
    pub top: f32,
    pub body_height: f32,
    pub rows: usize,
}
impl Layout {
    /// Web reading occupies the terminal raster's own box: the same
    /// whole-pixel row and the same centring, so entering or leaving an
    /// article neither rescales nor moves the BBS cells. The pinned footer is
    /// that box's last row; the body is the rows above it. Only zoom past
    /// the view lets the box fill a dimension, and only horizontal overflow
    /// reserves the bottom scrollbar strip.
    pub fn fit(width: f32, height: f32, dpi: f32, zoom: f32, rows: usize) -> Option<Self> {
        if [width, height, dpi, zoom]
            .iter()
            .any(|x| !x.is_finite() || *x <= 0.)
            || rows == 0
        {
            return None;
        }
        let row = crate::renderer::Size::row(width, height, dpi, zoom)?;
        let line = row as f32 / dpi;
        let overflow = row as f32 * 40. > width * dpi + 0.5;
        let bar = if overflow { SCROLLBAR } else { 0. };
        let boxed = (line * 24. + bar).min(height);
        let layout = Self {
            width: row * 40,
            row,
            dpi,
            rows,
            view_width: width,
            view_height: height,
            top: (height - boxed) / 2.,
            body_height: (boxed - line - bar).max(1.),
        };
        // Includes the two buffered rows on both sides. Whole document length
        // never controls a raster allocation.
        if [
            line,
            layout.logical_width(),
            layout.body_height,
            layout.top,
            rows as f32 * line,
        ]
        .iter()
        .any(|x| !x.is_finite())
        {
            return None;
        }
        let buffered = ((layout.body_height / line).ceil().min(rows as f32) as usize)
            .saturating_add(5)
            .min(rows);
        (u128::from(layout.width) * u128::from(row) * buffered as u128 <= 16 * 1024 * 1024)
            .then_some(layout)
    }
    pub fn line_height(self) -> f32 {
        self.row as f32 / self.dpi
    }
    pub fn logical_width(self) -> f32 {
        self.width as f32 / self.dpi
    }
    pub fn maximum(self) -> f64 {
        (self.rows as f64 - (f64::from(self.body_height) / f64::from(self.line_height()))).max(0.)
    }
    #[cfg(test)]
    pub fn clamp(self, anchor: f64) -> f64 {
        if anchor.is_finite() {
            anchor.clamp(0., self.maximum())
        } else {
            0.
        }
    }
    /// Horizontal scroll range; zero unless zoom overflows by over half a pixel.
    pub fn maximum_x(self) -> f32 {
        let overflow = self.logical_width() - self.view_width;
        if overflow * self.dpi > 0.5 {
            overflow
        } else {
            0.
        }
    }
    #[cfg(test)]
    pub fn visible(self, anchor: f64) -> Range<usize> {
        let anchor = self.clamp(anchor);
        let first = (anchor.floor() as usize).saturating_sub(2);
        let end = ((anchor + f64::from(self.body_height) / f64::from(self.line_height()))
            .ceil()
            .min(self.rows as f64) as usize)
            .saturating_add(2)
            .min(self.rows);
        first..end
    }
    pub fn tile_rows(self) -> usize {
        MAX_WINDOW_ROWS.min(8192 / self.row as usize).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_geometry_buffers_only_visible_rows_and_reaches_exact_last_row() {
        let l = Layout::fit(980., 600., 1., 1., 837).unwrap();
        assert_eq!((l.width, l.row, l.top, l.body_height), (960, 24, 12., 552.));
        assert_eq!(l.visible(0.), 0..25);
        assert_eq!(l.visible(250.25), 248..276);
        assert_eq!(l.visible(l.maximum()).end, 837);
        assert!((l.maximum() - 814.).abs() < 1e-6);
        assert_eq!(l.clamp(f64::NAN), 0.);
        assert!(l.visible(400.).len() < 30);
    }
    #[test]
    fn dpi_zoom_and_resize_keep_the_row_anchor_and_bound_every_tile() {
        let anchor = 250.25;
        for (w, h, dpi, z) in [
            (980., 600., 1., 1.5),
            (980., 600., 2., 1.25),
            (3840., 2160., 1., 1.),
            (580., 4096., 1., 1.),
        ] {
            let l = Layout::fit(w, h, dpi, z, 4096).unwrap();
            assert_eq!(l.clamp(anchor), anchor);
            assert!(l.tile_rows() <= 256);
            assert!(l.tile_rows() * l.row as usize <= 8192);
            assert_eq!(l.logical_width() / l.line_height(), 40.);
            let r = l.visible(anchor);
            assert!(r.start <= 250 && r.end > 250);
        }
        assert!(Layout::fit(f32::INFINITY, 600., 1., 1., 837).is_none());
        assert!(Layout::fit(980., 600., 0., 1., 837).is_none());
        assert!(Layout::fit(980., 600., 1., 1., 0).is_none());
        assert!(Layout::fit(8192., 8192., 2., 1., 4096).is_none());
        assert!(Layout::fit(980., 600., f32::MIN_POSITIVE, 1., 837).is_none());
        if let Some(extreme) = Layout::fit(f32::MAX, f32::MAX, f32::MAX, 1., 837) {
            assert_eq!(extreme.visible(0.), 0..837);
            assert_eq!(extreme.maximum(), 0.);
        }
        let l = Layout::fit(980., 600., 1., 1., 23).unwrap();
        assert_eq!(l.maximum(), 0.);
    }
    #[test]
    fn reading_box_is_the_terminal_raster_box() {
        for (w, h, dpi) in [
            (1920., 1080., 1.),
            (1470., 956., 2.),
            (1001., 599., 1.25),
            (980., 600., 1.),
        ] {
            let l = Layout::fit(w, h, dpi, 1., 837).unwrap();
            let size = crate::renderer::Size::fit(w, h, dpi, 1.).unwrap();
            assert_eq!((l.width, l.row * 24), (size.width, size.height));
            // Centred like the Flickable raster; no strip is reserved at fit.
            assert!((l.top - (h - size.height as f32 / dpi) / 2.).abs() < 1e-3);
            assert!((l.body_height - l.line_height() * 23.).abs() < 1e-3);
            assert_eq!(l.maximum_x(), 0.);
        }
        let zoomed = Layout::fit(980., 600., 1., 1.5, 837).unwrap();
        assert_eq!((zoomed.row, zoomed.top), (36, 0.));
        assert!(zoomed.maximum_x() > 0.);
        assert_eq!(zoomed.body_height, 600. - 36. - SCROLLBAR);
    }
}
