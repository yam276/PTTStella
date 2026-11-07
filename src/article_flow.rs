//! Article row/spacing map. Image gaps never acquire terminal cell coordinates.
//! Fixed-size metadata keeps a long document independent of bitmap allocation.
use std::ops::Range;

pub const MAX_GAPS: usize = 32;
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize)]
pub struct Anchor {
    pub row: usize,
    /// 0..1 is the text row; 1..2 is its following image gap.
    pub within: f64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Gap {
    after: usize,
    height: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flow {
    rows: usize,
    line: u32,
    gaps: [Gap; MAX_GAPS],
    count: usize,
}
impl Flow {
    pub fn new(
        rows: usize,
        line: u32,
        gaps: impl IntoIterator<Item = (usize, u32)>,
    ) -> Result<Self, &'static str> {
        if rows == 0 || rows > crate::article_cache::MAX_FULL_LINES || line == 0 {
            return Err("invalid article flow dimensions");
        }
        let mut flow = Self {
            rows,
            line,
            gaps: [Gap::default(); MAX_GAPS],
            count: 0,
        };
        for (n, (after, height)) in gaps.into_iter().enumerate() {
            if n >= MAX_GAPS || after >= rows {
                return Err("article flow gap budget exceeded");
            }
            if height == 0 {
                continue;
            }
            if let Some(gap) = flow.gaps[..flow.count]
                .iter_mut()
                .find(|g| g.after == after)
            {
                gap.height = gap
                    .height
                    .checked_add(height)
                    .ok_or("article flow gap overflow")?;
            } else {
                flow.gaps[flow.count] = Gap { after, height };
                flow.count += 1;
            }
        }
        flow.gaps[..flow.count].sort_by_key(|g| g.after);
        Ok(flow)
    }
    pub fn gap(&self, row: usize) -> u32 {
        self.gaps[..self.count]
            .iter()
            .find(|g| g.after == row)
            .map_or(0, |g| g.height)
    }
    pub fn top(&self, row: usize) -> u64 {
        // At most 4096 text rows and 32 u32 gaps; all sums fit u64 exactly.
        let row = row.min(self.rows);
        row as u64 * u64::from(self.line)
            + self.gaps[..self.count]
                .iter()
                .filter(|g| g.after < row)
                .map(|g| u64::from(g.height))
                .sum::<u64>()
    }
    pub fn height(&self) -> u64 {
        self.top(self.rows)
    }
    pub fn position(&self, anchor: Anchor) -> f64 {
        if anchor.row >= self.rows {
            return self.height() as f64;
        }
        let within = if anchor.within.is_finite() {
            anchor.within.clamp(0., 2.)
        } else {
            0.
        };
        self.top(anchor.row) as f64
            + if within < 1. {
                within * f64::from(self.line)
            } else {
                f64::from(self.line) + (within - 1.) * f64::from(self.gap(anchor.row))
            }
    }
    pub fn anchor(&self, y: f64) -> Anchor {
        let y = if y.is_finite() {
            y.clamp(0., self.height() as f64)
        } else {
            0.
        };
        if y >= self.height() as f64 {
            return Anchor {
                row: self.rows,
                within: 0.,
            };
        }
        let (mut low, mut high) = (0, self.rows);
        while low + 1 < high {
            let mid = low + (high - low) / 2;
            if self.top(mid) as f64 <= y {
                low = mid;
            } else {
                high = mid;
            }
        }
        let local = y - self.top(low) as f64;
        Anchor {
            row: low,
            within: if local < f64::from(self.line) {
                local / f64::from(self.line)
            } else {
                let gap = self.gap(low);
                if gap == 0 {
                    1.
                } else {
                    1. + (local - f64::from(self.line)) / f64::from(gap)
                }
            },
        }
    }
    pub fn hit(&self, y: f64) -> Option<usize> {
        if !y.is_finite() || y < 0. || y >= self.height() as f64 {
            return None;
        }
        let anchor = self.anchor(y);
        (anchor.row < self.rows && anchor.within < 1.).then_some(anchor.row)
    }
    pub fn maximum(&self, viewport: f64) -> f64 {
        if viewport.is_finite() && viewport > 0. {
            (self.height() as f64 - viewport).max(0.)
        } else {
            0.
        }
    }
    pub fn clamp(&self, y: f64, viewport: f64) -> f64 {
        if y.is_finite() {
            y.clamp(0., self.maximum(viewport))
        } else {
            0.
        }
    }
    pub fn visible(&self, y: f64, viewport: f64, buffer: usize) -> Range<usize> {
        if !viewport.is_finite() || viewport <= 0. {
            return 0..0;
        }
        let y = self.clamp(y, viewport);
        let start = self.anchor(y).row.min(self.rows);
        let last = self.anchor((y + viewport).min(self.height() as f64));
        let end = (last.row + usize::from(last.within > 0.)).min(self.rows);
        start.saturating_sub(buffer)..end.saturating_add(buffer).min(self.rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gaps_do_not_become_cells_and_rows_keep_independent_expected_positions() {
        let f = Flow::new(4, 20, [(1, 100)]).unwrap();
        assert_eq!(
            (0..=4).map(|r| f.top(r)).collect::<Vec<_>>(),
            [0, 20, 140, 160, 180]
        );
        for (y, expected) in [
            (0., Some(0)),
            (19.9, Some(0)),
            (20., Some(1)),
            (39.9, Some(1)),
            (40., None),
            (90., None),
            (139.9, None),
            (140., Some(2)),
            (179., Some(3)),
            (180., None),
        ] {
            assert_eq!(f.hit(y), expected, "{y}");
        }
        assert_eq!(f.visible(60., 40., 2), 0..4);
    }
    #[test]
    fn image_arrival_and_resizing_preserve_text_or_image_anchors() {
        let before = Flow::new(4, 20, [(1, 100)]).unwrap();
        let after = Flow::new(4, 20, [(1, 300)]).unwrap();
        assert_eq!(
            before.position(Flow::new(4, 20, []).unwrap().anchor(45.)),
            145.
        );
        assert_eq!(after.position(before.anchor(30.)), 30.);
        assert_eq!(after.position(before.anchor(90.)), 190.);
        assert_eq!(after.position(before.anchor(145.)), 345.);
        for y in [0., 19.5, 20., 30., 40., 90., 139., 140., 145., 179., 180.] {
            assert!((before.position(before.anchor(y)) - y).abs() < 1e-8);
        }
    }
    #[test]
    fn very_tall_gap_has_no_large_allocation_or_lost_terminal_row() {
        let f = Flow::new(4096, 24, [(0, u32::MAX)]).unwrap();
        assert_eq!(f.top(1), u64::from(u32::MAX) + 24);
        assert_eq!(f.hit(f.top(1) as f64 + 6.), Some(1));
        assert_eq!(
            f.anchor(f.top(1) as f64 + 6.),
            Anchor {
                row: 1,
                within: 0.25
            }
        );
        assert_eq!(f.visible(1_000_000., 576., 2), 0..3);
        assert!(std::mem::size_of::<Flow>() < 1024);
    }
    #[test]
    fn admission_and_duplicates_are_bounded_and_extremes_do_not_invent_hits() {
        assert!(Flow::new(0, 24, []).is_err());
        assert!(Flow::new(4097, 24, []).is_err());
        assert!(Flow::new(4, 0, []).is_err());
        assert!(Flow::new(4, 24, [(4, 1)]).is_err());
        assert!(Flow::new(40, 24, (0..33).map(|r| (r, 1))).is_err());
        assert!(Flow::new(4, 24, [(1, u32::MAX), (1, 1)]).is_err());
        let f = Flow::new(4, 24, [(1, 10), (1, 20)]).unwrap();
        assert_eq!(f.height(), 126);
        assert!(f.hit(f64::NAN).is_none());
        assert!(f.hit(f64::INFINITY).is_none());
        assert_eq!(f.anchor(f64::NAN), Anchor::default());
        assert_eq!(f.clamp(-10., 40.), 0.);
        assert_eq!(f.visible(0., f64::NAN, 2), 0..0);
        assert_eq!(f.visible(0., 0., 2), 0..0);
    }
}
