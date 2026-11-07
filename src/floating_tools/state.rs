//! Only an active drag/coast owns scheduling. No input bytes or display frames.
use super::geometry::{Point, Viewport};
use std::{
    collections::VecDeque,
    time::{Duration, Instant, SystemTime},
};
struct Sample {
    point: Point,
    at: Instant,
}
struct Drag {
    pointer: Point,
    origin: Point,
    moved: bool,
    history: VecDeque<Sample>,
    direction: Option<Point>,
}
struct Coast {
    wall: SystemTime,
    velocity: Point,
    last: Instant,
    until: Instant,
}
pub struct State {
    pub viewport: Viewport,
    pub point: Point,
    pub open: bool,
    pub category: Option<usize>,
    pub selected: usize,
    parked: Option<Point>,
    drag: Option<Drag>,
    coast: Option<Coast>,
}
impl State {
    pub fn new(viewport: Viewport) -> Self {
        Self {
            point: viewport.initial(),
            viewport,
            open: false,
            category: None,
            selected: 0,
            parked: None,
            drag: None,
            coast: None,
        }
    }
    pub fn busy(&self) -> bool {
        self.open || self.drag.is_some()
    }
    pub fn animating(&self) -> bool {
        self.coast.is_some()
    }
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }
    pub fn close(&mut self) {
        self.open = false;
        self.category = None;
        self.selected = 0;
        if let Some(parked) = self.parked.take() {
            self.point = self.viewport.clamp(parked);
        }
    }
    pub fn stop_motion(&mut self) {
        self.drag = None;
        self.coast = None;
    }
    pub fn cancel(&mut self) {
        self.stop_motion();
        self.close();
    }
    pub fn toggle(&mut self) {
        self.coast = None;
        if let Some(category) = self.category.take() {
            self.selected = category;
        } else if self.open {
            self.close();
        } else {
            self.open = true;
            self.selected = 0;
        }
    }
    pub fn show_at(&mut self, point: Point) {
        self.cancel();
        self.parked = Some(self.point);
        self.point = self.viewport.clamp(Point {
            x: point.x - 28. * self.viewport.scale(),
            y: point.y - 28. * self.viewport.scale(),
        });
        self.open = true;
    }
    pub fn resize(&mut self, viewport: Viewport) {
        let left = self.viewport.left(self.point);
        self.drag = None;
        self.coast = None;
        self.viewport = viewport;
        self.point = if self.open {
            viewport.clamp(self.point)
        } else {
            viewport.dock(self.point, left)
        };
    }
    pub fn down(&mut self, pointer: Point, now: Instant) {
        self.coast = None;
        self.drag = Some(Drag {
            pointer,
            origin: self.point,
            moved: false,
            history: VecDeque::from([Sample {
                point: self.point,
                at: now,
            }]),
            direction: None,
        });
    }
    fn track(&mut self, pointer: Point, now: Instant) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let dx = pointer.x - drag.pointer.x;
        let dy = pointer.y - drag.pointer.y;
        if !drag.moved && dx.hypot(dy) > 6. {
            drag.moved = true;
        }
        if !drag.moved {
            return;
        }
        let next = self.viewport.clamp(Point {
            x: drag.origin.x + dx,
            y: drag.origin.y + dy,
        });
        let dx = next.x - self.point.x;
        let dy = next.y - self.point.y;
        let distance = dx.hypot(dy);
        if distance > 0. {
            drag.direction = Some(Point {
                x: dx / distance,
                y: dy / distance,
            });
        }
        if drag.history.back().is_some_and(|s| s.at == now) {
            drag.history.pop_back();
        }
        drag.history.push_back(Sample {
            point: next,
            at: now,
        });
        while drag.history.len() > 16
            || drag
                .history
                .front()
                .is_some_and(|s| now.saturating_duration_since(s.at) > Duration::from_millis(120))
        {
            drag.history.pop_front();
        }
        self.point = next;
        self.open = false;
        self.category = None;
        self.parked = None;
    }
    pub fn motion(&mut self, pointer: Point, now: Instant, left_down: bool) {
        if !left_down {
            self.cancel();
        } else {
            self.track(pointer, now);
        }
    }
    pub fn up(&mut self, pointer: Point, now: Instant, reduced: bool) -> bool {
        self.track(pointer, now);
        let Some(drag) = self.drag.take() else {
            return false;
        };
        if !drag.moved {
            self.toggle();
            return true;
        }
        let mut velocity = Point::default();
        if let Some(first) = drag.history.front() {
            let n = drag.history.len() as f32;
            let mean = drag.history.iter().fold((0., 0., 0.), |(t, x, y), s| {
                (
                    t + s.at.saturating_duration_since(first.at).as_secs_f32() * 1000.,
                    x + s.point.x,
                    y + s.point.y,
                )
            });
            let (mt, mx, my) = (mean.0 / n, mean.1 / n, mean.2 / n);
            let (variance, x, y) = drag.history.iter().fold((0., 0., 0.), |(v, x, y), s| {
                let dt = s.at.saturating_duration_since(first.at).as_secs_f32() * 1000. - mt;
                (
                    v + dt * dt,
                    x + dt * (s.point.x - mx),
                    y + dt * (s.point.y - my),
                )
            });
            if variance > 0. {
                velocity = Point {
                    x: (x / variance).clamp(-2., 2.),
                    y: (y / variance).clamp(-2., 2.),
                };
            }
        }
        if velocity.x.hypot(velocity.y) < 0.1
            && let Some(direction) = drag.direction
        {
            velocity = Point {
                x: direction.x * 0.1,
                y: direction.y * 0.1,
            };
        }
        if reduced {
            self.point = self
                .viewport
                .dock(self.point, self.viewport.left(self.point));
        } else {
            self.coast = Some(Coast {
                wall: SystemTime::now(),
                velocity,
                last: now,
                until: now + Duration::from_millis(900),
            });
        }
        false
    }
    pub fn tick(&mut self, now: Instant) {
        self.tick_with_clock(now, SystemTime::now());
    }
    pub(super) fn tick_with_clock(&mut self, now: Instant, wall: SystemTime) {
        let Some(coast) = self.coast.as_mut() else {
            return;
        };
        let elapsed = now.saturating_duration_since(coast.last);
        if elapsed > Duration::from_millis(250)
            || wall
                .duration_since(coast.wall)
                .ok()
                .is_none_or(|elapsed| elapsed > Duration::from_millis(250))
        {
            self.point = self
                .viewport
                .dock(self.point, self.viewport.left(self.point));
            self.coast = None;
            return;
        }
        let dt = elapsed.as_secs_f32().min(0.032) * 1000.;
        coast.last = now;
        coast.wall = wall;
        if now < coast.until && coast.velocity.x.hypot(coast.velocity.y) > 0.025 {
            let next = Point {
                x: self.point.x + coast.velocity.x * dt,
                y: self.point.y + coast.velocity.y * dt,
            };
            self.point = self.viewport.clamp(next);
            if self.point.x != next.x {
                coast.velocity.x = 0.;
            }
            if self.point.y != next.y {
                coast.velocity.y = 0.;
            }
            coast.velocity.x *= (-dt / 220.).exp();
            coast.velocity.y *= (-dt / 220.).exp();
        } else {
            let goal = self
                .viewport
                .dock(self.point, self.viewport.left(self.point));
            self.point.x += (goal.x - self.point.x) * (1. - (-dt / 110.).exp());
            if (goal.x - self.point.x).abs() < 0.5 {
                self.point = goal;
                self.coast = None;
            }
        }
    }
}
