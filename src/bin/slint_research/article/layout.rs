//! Physical-pixel row geometry. Anchors are article row plus fractional row.
use pttstella::article_cache::MAX_WINDOW_ROWS;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub width: u32,
    pub row: u32,
    pub dpi: f32,
    pub view_width: f32,
    pub body_height: f32,
    pub rows: usize,
}
impl Layout {
    pub fn fit(width: f32, height: f32, dpi: f32, zoom: f32, rows: usize) -> Option<Self> {
        if [width, height, dpi, zoom]
            .iter()
            .any(|x| !x.is_finite() || *x <= 0.)
            || width <= 12.
            || height <= 13.
            || rows == 0
        {
            return None;
        }
        let row = (((width - 12.) / 960.).min((height - 12.) / 576.) * dpi * zoom * 24.)
            .floor()
            .clamp(1., 204.) as u32;
        let layout = Self {
            width: row * 40,
            row,
            dpi,
            rows,
            view_width: width - 12.,
            body_height: (height - 12. - row as f32 / dpi).max(1.),
        };
        // Includes the two buffered rows on both sides. Whole document length
        // never controls a raster allocation.
        let line = layout.line_height();
        if [
            line,
            layout.logical_width(),
            layout.body_height,
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
    pub fn clamp(self, anchor: f64) -> f64 {
        if anchor.is_finite() {
            anchor.clamp(0., self.maximum())
        } else {
            0.
        }
    }
    pub fn maximum_x(self) -> f32 {
        (self.logical_width() - self.view_width).max(0.)
    }
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
    pub fn page(self, anchor: f64, down: bool) -> f64 {
        let anchor = self.clamp(anchor);
        let height = f64::from(self.body_height) / f64::from(self.line_height());
        let next = if down {
            (anchor + height).floor() - 1.
        } else {
            anchor.ceil() - height.floor() + 1.
        };
        let next = if down && next <= anchor || !down && next >= anchor {
            anchor + if down { 1. } else { -1. }
        } else {
            next
        };
        self.clamp(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_geometry_buffers_only_visible_rows_and_reaches_exact_last_row() {
        let l = Layout::fit(980., 600., 1., 1., 837).unwrap();
        assert_eq!((l.width, l.row, l.body_height), (960, 24, 564.));
        assert_eq!(l.visible(0.), 0..26);
        assert_eq!(l.visible(250.25), 248..276);
        assert_eq!(l.visible(l.maximum()).end, 837);
        assert!((l.maximum() - 813.5).abs() < 1e-6);
        assert_eq!(l.page(0., true), 22.);
        assert_eq!(l.page(22., false), 0.);
        assert_eq!(l.page(0.25, true), 22.);
        assert_eq!(l.page(22.25, false), 1.);
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
        let small = Layout::fit(20., 14., 1., 4., 837).unwrap();
        assert!(small.page(2., true) > 2.);
        assert!(small.page(2., false) < 2.);
        let l = Layout::fit(980., 600., 1., 1., 23).unwrap();
        assert_eq!(l.maximum(), 0.);
    }
}
