//! Logical window coordinates. DPI is applied once by Slint, never here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
}
pub struct Layout {
    pub scale: f32,
    pub ball: Rect,
    pub petals: [Rect; 5],
    pub panel: Rect,
    pub compact: bool,
}
fn clamp(value: f32, low: f32, high: f32) -> f32 {
    value.clamp(low, high.max(low))
}
impl Viewport {
    pub fn new(width: f32, height: f32) -> Option<Self> {
        (width.is_finite() && height.is_finite() && width >= 320. && height >= 240.)
            .then_some(Self { width, height })
    }
    pub fn scale(self) -> f32 {
        (self.width / 1280.).min(self.height / 800.).clamp(1., 1.75)
    }
    pub fn initial(self) -> Point {
        self.clamp(Point {
            x: self.width - 80. * self.scale(),
            y: self.height * 0.65,
        })
    }
    pub fn clamp(self, point: Point) -> Point {
        let s = self.scale();
        Point {
            x: clamp(point.x, 16. * s, self.width - 72. * s),
            y: clamp(
                point.y,
                (56. * s).min((self.height - 72. * s).max(0.)),
                self.height - 72. * s,
            ),
        }
    }
    pub fn left(self, point: Point) -> bool {
        point.x + 28. * self.scale() < self.width / 2.
    }
    pub fn dock(self, point: Point, left: bool) -> Point {
        self.clamp(Point {
            x: if left {
                16. * self.scale()
            } else {
                self.width - 72. * self.scale()
            },
            y: point.y,
        })
    }
    pub fn layout(self, point: Point) -> Layout {
        let s = self.scale();
        let p = self.clamp(point);
        let direction = if self.left(p) { 1. } else { -1. };
        let fan_y = clamp(p.y, 196. * s, self.height - 226. * s);
        let mut petals = std::array::from_fn(|index| {
            let angle = (-90. + index as f32 * 45.).to_radians();
            Rect {
                x: 144. * s * angle.cos(),
                y: fan_y + 148. * s * angle.sin(),
                width: 62. * s,
                height: 62. * s,
            }
        });
        let inset = petals
            .iter()
            .filter(|r| r.y < p.y + 72. * s && r.y + 78. * s > p.y)
            .map(|r| (if direction > 0. { 72. } else { 78. }) * s - r.x)
            .fold(8. * s, f32::max);
        for r in &mut petals {
            r.x = p.x + direction * (inset + r.x);
        }
        let ball = Rect {
            x: p.x,
            y: p.y,
            width: 56. * s,
            height: 56. * s,
        };
        let fan = self.width >= 500. * s
            && self.height >= 420. * s
            && petals.iter().all(|r| {
                r.x >= 16. * s
                    && r.x + r.width <= self.width - 16. * s
                    && r.y >= 48. * s
                    && r.y + r.height <= self.height - 16. * s
            });
        let areas = [
            Rect {
                x: 16. * s,
                y: 48. * s,
                width: p.x - 28. * s,
                height: self.height - 64. * s,
            },
            Rect {
                x: p.x + 68. * s,
                y: 48. * s,
                width: self.width - p.x - 84. * s,
                height: self.height - 64. * s,
            },
            Rect {
                x: 16. * s,
                y: 48. * s,
                width: self.width - 32. * s,
                height: p.y - 60. * s,
            },
            Rect {
                x: 16. * s,
                y: p.y + 68. * s,
                width: self.width - 32. * s,
                height: self.height - p.y - 84. * s,
            },
        ];
        let side = areas[if direction > 0. { 1 } else { 0 }];
        let area = if side.width >= 240. * s && side.height >= 160. * s {
            side
        } else {
            areas
                .into_iter()
                .filter(|r| r.width > 0. && r.height > 0.)
                .max_by(|a, b| {
                    (a.width.min(330. * s) * a.height.min(260. * s))
                        .total_cmp(&(b.width.min(330. * s) * b.height.min(260. * s)))
                })
                .unwrap_or(Rect {
                    x: 16.,
                    y: 48.,
                    width: self.width - 32.,
                    height: self.height - 64.,
                })
        };
        let width = area.width.min(330. * s);
        let height = area.height.min(260. * s);
        let panel = Rect {
            x: clamp(p.x, area.x, area.x + area.width - width),
            y: clamp(p.y - height / 2., area.y, area.y + area.height - height),
            width,
            height,
        };
        if !fan {
            let columns = ((width - 16. * s) / (72. * s)).floor().max(1.) as usize;
            let cell = (width - 16. * s) / columns as f32;
            for (index, r) in petals.iter_mut().enumerate() {
                r.x = panel.x + 8. * s + (index % columns) as f32 * cell;
                r.y = panel.y + 8. * s + (index / columns) as f32 * 72. * s;
                r.width = cell - 10. * s;
            }
        }
        Layout {
            scale: s,
            ball,
            petals,
            panel,
            compact: !fan,
        }
    }
}
