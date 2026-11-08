//! One bounded native pointer transaction and one latest continuous sample.
use crate::pointer::Geometry;
use pttstella::{
    connection::{Phase, View},
    mouse::{
        self, Action, Button, Modifiers, Tracking,
        input::{Event, Lease, Route, Sample, SampleReport},
    },
};
use std::time::{Duration, Instant, SystemTime};

pub const CADENCE: Duration = Duration::from_millis(34);
const SAMPLE_MAX_AGE: Duration = Duration::from_millis(250);
pub struct Context<'a> {
    pub view: &'a View,
    pub geometry: Option<Geometry>,
    pub presented: bool,
    pub blocked: bool,
    pub selected: bool,
    pub buttons: u8,
    pub now: Instant,
    pub wall: SystemTime,
}
pub enum Effect {
    Prepare,
    Edge(Event),
    Sample(Sample),
    Cancel(Lease),
    Revoke(u64),
}
struct Held {
    lease: Lease,
    button: Button,
    geometry: Geometry,
    observed: bool,
    deadline: Instant,
}
struct Pending {
    lease: Lease,
    geometry: Geometry,
    report: SampleReport,
    at: Instant,
    wall: SystemTime,
}
#[derive(Default)]
pub struct State {
    held: Option<Held>,
    motion: Option<Pending>,
    wheel: Option<Pending>,
    generation: Option<u64>,
    transaction: u64,
    sequence: u64,
    last_flush: Option<Instant>,
    remainder: f32,
    wheel_source: Option<(pttstella::navigation::Stamp, u64, u64)>,
    local_button: Option<Button>,
    gate: Option<(u64, bool)>,
}
fn mask(button: Button) -> u8 {
    match button {
        Button::Left => 1,
        Button::Middle => 4,
        Button::Right => 2,
    }
}
impl State {
    pub fn active(&self) -> bool {
        self.held.is_some()
    }
    fn clear_samples(&mut self) {
        self.motion = None;
        self.wheel = None;
        self.remainder = 0.;
        self.wheel_source = None;
    }
    fn cancel_held(&mut self, effects: &mut Vec<Effect>) {
        if let Some(held) = self.held.take() {
            effects.push(Effect::Cancel(held.lease));
        }
        self.clear_samples();
    }
    pub fn revoke(&mut self, generation: u64) -> Vec<Effect> {
        self.held = None;
        self.clear_samples();
        self.local_button = None;
        vec![Effect::Revoke(generation)]
    }
    pub fn observe(&mut self, c: &Context<'_>) -> Vec<Effect> {
        let mut effects = Vec::new();
        if self.generation != Some(c.view.generation) {
            self.cancel_held(&mut effects);
            self.local_button = None;
            self.transaction = 0;
            self.sequence = 0;
            self.last_flush = None;
            self.generation = Some(c.view.generation);
        }
        if c.blocked {
            if self.gate != Some((c.view.generation, true)) && c.view.mouse.route != Route::Local {
                effects.push(Effect::Revoke(c.view.generation));
            }
            self.held = None;
            self.clear_samples();
            self.local_button = None;
        } else if c.view.phase != Phase::Connected || c.view.mouse.route != Route::Station {
            self.cancel_held(&mut effects);
        } else if let Some(held) = &mut self.held {
            let lease = held.lease;
            if c.geometry != Some(held.geometry)
                || lease.mode_epoch != c.view.mouse.modes.epoch()
                || lease.policy_epoch != c.view.mouse.policy_epoch
                || c.now >= held.deadline
                || c.view.mouse.active != Some(lease)
                    && (held.observed || c.view.input_epoch > lease.stamp.input_epoch)
            {
                self.cancel_held(&mut effects);
            } else if c.view.mouse.active == Some(lease) {
                held.observed = true;
            }
        }
        self.gate = Some((c.view.generation, c.blocked));
        if self.motion.as_ref().is_some_and(|p| !self.valid(c, p)) {
            self.motion = None;
        }
        if self.wheel.as_ref().is_some_and(|p| !self.valid(c, p)) {
            self.wheel = None;
            self.remainder = 0.;
            self.wheel_source = None;
        }
        effects
    }
    fn valid(&self, c: &Context<'_>, p: &Pending) -> bool {
        !c.blocked
            && c.now.saturating_duration_since(p.at) <= SAMPLE_MAX_AGE
            && c.wall
                .duration_since(p.wall)
                .is_ok_and(|age| age <= SAMPLE_MAX_AGE)
            && c.view.phase == Phase::Connected
            && c.view.mouse.route == Route::Station
            && c.geometry == Some(p.geometry)
            && p.lease.stamp.generation == c.view.generation
            && p.lease.mode_epoch == c.view.mouse.modes.epoch()
            && p.lease.policy_epoch == c.view.mouse.policy_epoch
            && (self.held.as_ref().is_some_and(|held| held.lease == p.lease)
                || c.presented && p.lease.stamp == c.view.stamp())
    }
    fn lease(&mut self, c: &Context<'_>) -> Option<Lease> {
        if let Some(held) = &self.held {
            return Some(held.lease);
        }
        self.transaction = self.transaction.checked_add(1)?;
        Some(Lease {
            stamp: c.view.stamp(),
            mode_epoch: c.view.mouse.modes.epoch(),
            policy_epoch: c.view.mouse.policy_epoch,
            transaction: self.transaction,
        })
    }
    pub fn pointer(
        &mut self,
        c: &Context<'_>,
        kind: i32,
        button: Option<Button>,
        point: (f32, f32),
        modifiers: Modifiers,
        meta: bool,
    ) -> (bool, Vec<Effect>) {
        let mut effects = self.observe(c);
        if kind == 3 {
            effects.extend(self.revoke(c.view.generation));
            return (false, effects);
        }
        if self.local_button.is_some() && (kind == 1 || kind == 2 && button == self.local_button) {
            if kind == 2 {
                self.local_button = None;
            }
            return (false, effects);
        }
        if kind == 0 && self.held.is_some() {
            return (true, effects);
        }
        if c.view.mouse.route == Route::Local {
            return (false, effects);
        }
        if kind == 0 && modifiers.shift && !c.blocked {
            self.local_button = button;
            self.clear_samples();
            effects.push(Effect::Revoke(c.view.generation));
            return (false, effects);
        }
        if kind == 1 {
            if self
                .held
                .as_ref()
                .is_some_and(|held| c.buttons & mask(held.button) == 0)
            {
                self.cancel_held(&mut effects);
                return (true, effects);
            }
            if self.held.is_none() && c.buttons != 0 {
                self.clear_samples();
                return (true, effects);
            }
        }
        if meta && kind == 2 {
            self.cancel_held(&mut effects);
        }
        if c.blocked || meta || c.view.mouse.route != Route::Station {
            return (true, effects);
        }
        let Some(geometry) = c.geometry else {
            return (true, effects);
        };
        let Some(cell) = geometry.cell(point, &c.view.frame) else {
            if kind == 2 {
                self.cancel_held(&mut effects);
            } else {
                self.clear_samples();
            }
            return (true, effects);
        };
        match kind {
            0 if c.presented && c.view.mouse.active.is_none() => {
                let Some(button) = button else {
                    return (true, effects);
                };
                if c.buttons & mask(button) == 0 {
                    return (true, effects);
                }
                self.clear_samples();
                if let Some(lease) = self.lease(c) {
                    self.held = Some(Held {
                        lease,
                        button,
                        geometry,
                        observed: false,
                        deadline: c.now + mouse::input::HOLD_LIMIT,
                    });
                    effects.push(Effect::Prepare);
                    effects.push(Effect::Edge(Event {
                        lease,
                        report: mouse::Report {
                            col: cell.col,
                            row: cell.row,
                            button,
                            action: Action::Press,
                            modifiers,
                        },
                    }));
                }
            }
            2 => {
                if let Some(held) = &self.held
                    && button == Some(held.button)
                {
                    effects.extend(self.flush(c, true));
                    if let Some(held) = self.held.take() {
                        effects.push(Effect::Edge(Event {
                            lease: held.lease,
                            report: mouse::Report {
                                col: cell.col,
                                row: cell.row,
                                button: held.button,
                                action: Action::Release,
                                modifiers,
                            },
                        }));
                    }
                    self.clear_samples();
                }
            }
            1 if self.held.is_some() || c.presented => {
                if self.held.is_none() && (modifiers.shift || c.selected) {
                    return (true, effects);
                }
                if c.view.mouse.modes.tracking() == Tracking::Motion
                    || c.view.mouse.modes.tracking() == Tracking::Drag && self.held.is_some()
                {
                    let button = self.held.as_ref().map(|held| held.button);
                    if let Some(lease) = self.lease(c) {
                        self.motion = Some(Pending {
                            lease,
                            geometry,
                            at: c.now,
                            wall: c.wall,
                            report: SampleReport::Motion(mouse::MotionReport {
                                col: cell.col,
                                row: cell.row,
                                button,
                                modifiers,
                            }),
                        });
                    }
                }
            }
            _ => {}
        }
        (true, effects)
    }
    pub fn wheel(
        &mut self,
        c: &Context<'_>,
        point: (f32, f32),
        delta: f32,
        modifiers: Modifiers,
        meta: bool,
    ) -> (bool, Vec<Effect>) {
        let effects = self.observe(c);
        if c.view.mouse.route == Route::Local {
            return (false, effects);
        }
        if c.blocked
            || c.selected
            || self.local_button.is_some()
            || modifiers.ctrl
            || modifiers.shift
            || meta
            || c.view.mouse.route != Route::Station
            || self.held.is_none() && !c.presented
            || !delta.is_finite()
            || delta == 0.
        {
            return (true, effects);
        }
        let Some(geometry) = c.geometry else {
            return (true, effects);
        };
        let Some(cell) = geometry.cell(point, &c.view.frame) else {
            self.clear_samples();
            return (true, effects);
        };
        let source = (
            c.view.stamp(),
            c.view.mouse.modes.epoch(),
            c.view.mouse.policy_epoch,
        );
        if self.wheel_source != Some(source) {
            self.remainder = 0.;
            self.wheel_source = Some(source);
        }
        self.remainder = (self.remainder + delta).clamp(-160., 160.);
        if self.remainder.abs() >= 40. {
            let direction = if self.remainder > 0. {
                mouse::WheelDirection::Up
            } else {
                mouse::WheelDirection::Down
            };
            self.remainder %= 40.;
            if let Some(lease) = self.lease(c) {
                self.wheel = Some(Pending {
                    lease,
                    geometry,
                    at: c.now,
                    wall: c.wall,
                    report: SampleReport::Wheel(mouse::WheelReport {
                        col: cell.col,
                        row: cell.row,
                        direction,
                        modifiers,
                    }),
                });
            }
        }
        (true, effects)
    }
    pub fn flush(&mut self, c: &Context<'_>, force: bool) -> Vec<Effect> {
        let mut effects = self.observe(c);
        if !force
            && self
                .last_flush
                .is_some_and(|time| c.now.saturating_duration_since(time) < CADENCE)
        {
            return effects;
        }
        let pending = self.wheel.take().or_else(|| self.motion.take());
        self.motion = None;
        if let Some(pending) = pending
            && self.valid(c, &pending)
            && let Some(sequence) = self.sequence.checked_add(1)
        {
            self.sequence = sequence;
            self.last_flush = Some(c.now);
            effects.push(Effect::Sample(Sample {
                lease: pending.lease,
                sequence,
                report: pending.report,
            }));
        }
        effects
    }
    pub fn wake_after(&self, now: Instant) -> Option<Duration> {
        let sample = (self.motion.is_some() || self.wheel.is_some()).then(|| {
            self.last_flush.map_or(Duration::ZERO, |last| {
                CADENCE.saturating_sub(now.saturating_duration_since(last))
            })
        });
        let hold = self
            .held
            .as_ref()
            .map(|held| held.deadline.saturating_duration_since(now));
        match (sample, hold) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}
