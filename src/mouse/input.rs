//! One bounded button transaction. No I/O, retries, timers or display ownership.
#[cfg(any(feature = "desktop", test))]
use super::{Action, Button, encode};
use super::{Error as FormatError, Modes, Report};
use crate::navigation::Stamp;
use serde::{Deserialize, Serialize};
use std::time::Duration;
#[cfg(any(feature = "desktop", test))]
use std::time::Instant;

pub const HOLD_LIMIT: Duration = Duration::from_secs(30);
pub const PRESSES_PER_SECOND: u8 = 20;
pub const SAMPLES_PER_SECOND: u8 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lease {
    pub stamp: Stamp,
    pub mode_epoch: u64,
    pub policy_epoch: u64,
    pub transaction: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub lease: Lease,
    pub report: Report,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SampleReport {
    Motion(super::MotionReport),
    Wheel(super::WheelReport),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    pub lease: Lease,
    pub sequence: u64,
    pub report: SampleReport,
}
#[cfg(any(feature = "desktop", test))]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SampleOutcome {
    Bytes(Vec<u8>),
    SameCell,
    RateLimited,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub enabled: bool,
    pub policy_epoch: u64,
    pub modes: Modes,
    pub ready: bool,
    pub route: Route,
    pub active: Option<Lease>,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            enabled: true,
            policy_epoch: 0,
            modes: Modes::default(),
            ready: false,
            route: Route::Local,
            active: None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    Local,
    Station,
    Blocked,
}
#[derive(Clone, Copy)]
#[cfg(any(feature = "desktop", test))]
pub(crate) struct Context {
    pub stamp: Stamp,
    pub modes: Modes,
    pub ready: bool,
    pub cols: usize,
    pub rows: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Disabled,
    Stale,
    Busy,
    RateLimited,
    Format(FormatError),
}
impl Error {
    pub fn message(self) -> String {
        match self {
            Self::Disabled => crate::msg!("station-mouse-disabled"),
            Self::Stale => crate::msg!("station-mouse-stale"),
            Self::Busy => crate::msg!("station-mouse-busy"),
            Self::RateLimited => crate::msg!("station-mouse-rate"),
            Self::Format(_) => crate::msg!("station-mouse-format"),
        }
    }
}
#[cfg(any(feature = "desktop", test))]
struct Held {
    lease: Lease,
    button: Button,
    deadline: Instant,
}
#[cfg(any(feature = "desktop", test))]
pub(crate) struct State {
    enabled: bool,
    policy_epoch: u64,
    control_serial: u64,
    generation: u64,
    mode_epoch: u64,
    last_transaction: u64,
    cancelled_through: u64,
    held: Option<Held>,
    window: Option<Instant>,
    presses: u8,
    last_sample: u64,
    last_motion: Option<(usize, usize)>,
    sample_window: Option<Instant>,
    samples: u8,
}
#[cfg(any(feature = "desktop", test))]
impl Default for State {
    fn default() -> Self {
        Self {
            enabled: true,
            policy_epoch: 0,
            control_serial: 0,
            generation: 0,
            mode_epoch: 0,
            last_transaction: 0,
            cancelled_through: 0,
            held: None,
            window: None,
            presses: 0,
            last_sample: 0,
            last_motion: None,
            sample_window: None,
            samples: 0,
        }
    }
}
#[cfg(any(feature = "desktop", test))]
impl State {
    pub(crate) fn observe(&mut self, context: Context, now: Instant) {
        if self.generation != context.stamp.generation {
            self.generation = context.stamp.generation;
            self.last_transaction = 0;
            self.cancelled_through = 0;
            self.window = None;
            self.presses = 0;
            self.held = None;
            self.last_sample = 0;
            self.last_motion = None;
            self.sample_window = None;
            self.samples = 0;
        }
        if self.mode_epoch != context.modes.epoch() {
            self.mode_epoch = context.modes.epoch();
            self.held = None;
            self.last_motion = None;
        }
        self.expire(now);
    }
    pub(crate) fn expire(&mut self, now: Instant) -> bool {
        if self.held.as_ref().is_some_and(|held| now >= held.deadline) {
            self.held = None;
            self.last_motion = None;
            true
        } else {
            false
        }
    }
    pub(crate) fn enable(&mut self, serial: u64, enabled: bool) {
        if serial > self.control_serial {
            self.enabled = enabled;
            self.control_serial = serial;
            self.policy_epoch = self.policy_epoch.saturating_add(1);
            self.held = None;
            self.last_motion = None;
        }
    }
    pub(crate) fn revoke(&mut self, serial: u64, lease: Option<Lease>) {
        if let Some(lease) = lease {
            if lease.stamp.generation != self.generation
                || lease.mode_epoch != self.mode_epoch
                || lease.policy_epoch != self.policy_epoch
            {
                return;
            }
            // Cancellation may reach the owner before its queued press.
            self.cancelled_through = self.cancelled_through.max(lease.transaction);
        }
        if serial > self.control_serial
            && (lease.is_none()
                || self
                    .held
                    .as_ref()
                    .is_some_and(|held| Some(held.lease) == lease))
        {
            self.control_serial = serial;
            self.policy_epoch = self.policy_epoch.saturating_add(1);
            self.held = None;
            self.last_motion = None;
        }
    }
    pub(crate) fn interrupt(&mut self) {
        self.held = None;
        self.last_motion = None;
    }
    pub(crate) fn intervene(&mut self) {
        self.held = None;
        self.last_motion = None;
        self.policy_epoch = self.policy_epoch.saturating_add(1);
    }
    pub(crate) fn active(&self) -> bool {
        self.held.is_some()
    }
    pub(crate) fn snapshot(&self, context: Context) -> Snapshot {
        Snapshot {
            enabled: self.enabled,
            policy_epoch: self.policy_epoch,
            modes: context.modes,
            ready: context.ready,
            route: if !self.enabled || context.modes.tracking() == super::Tracking::Off {
                Route::Local
            } else if context.ready && context.modes.supports_click_reports() {
                Route::Station
            } else {
                Route::Blocked
            },
            active: self.held.as_ref().map(|held| held.lease),
        }
    }
    pub(crate) fn accept(
        &mut self,
        context: Context,
        event: Event,
        now: Instant,
    ) -> Result<Vec<u8>, Error> {
        let lease = event.lease;
        self.prepare(context, lease, now)?;
        match event.report.action {
            Action::Press => {
                if lease.stamp != context.stamp
                    || lease.transaction == 0
                    || lease.transaction <= self.last_transaction.max(self.cancelled_through)
                {
                    return Err(Error::Stale);
                }
                if self.held.is_some() {
                    return Err(Error::Busy);
                }
                let bytes = encode(context.modes, &event.report, context.cols, context.rows)
                    .map_err(Error::Format)?;
                if self.window.is_none_or(|start| {
                    now.saturating_duration_since(start) >= Duration::from_secs(1)
                }) {
                    self.window = Some(now);
                    self.presses = 0;
                }
                if self.presses >= PRESSES_PER_SECOND {
                    return Err(Error::RateLimited);
                }
                self.presses += 1;
                self.last_transaction = lease.transaction;
                self.last_motion = Some((event.report.col, event.report.row));
                self.held = Some(Held {
                    lease,
                    button: event.report.button,
                    deadline: now + HOLD_LIMIT,
                });
                Ok(bytes)
            }
            Action::Release => {
                if !self
                    .held
                    .as_ref()
                    .is_some_and(|held| held.lease == lease && held.button == event.report.button)
                {
                    return Err(Error::Stale);
                }
                // Ending this transaction does not require its old frame to be
                // current. The generation, modes, policy and button still must be.
                self.held = None;
                let bytes = encode(context.modes, &event.report, context.cols, context.rows)
                    .map_err(Error::Format)?;
                self.last_motion = Some((event.report.col, event.report.row));
                Ok(bytes)
            }
        }
    }

    fn prepare(&mut self, context: Context, lease: Lease, now: Instant) -> Result<(), Error> {
        self.observe(context, now);
        if lease.stamp.generation != context.stamp.generation
            || lease.mode_epoch != context.modes.epoch()
            || lease.policy_epoch != self.policy_epoch
        {
            return Err(Error::Stale);
        }
        if !self.enabled {
            return Err(Error::Disabled);
        }
        if !context.ready {
            if self.held.as_ref().is_some_and(|held| held.lease == lease) {
                self.held = None;
                self.last_motion = None;
            }
            return Err(Error::Busy);
        }
        Ok(())
    }

    pub(crate) fn accept_sample(
        &mut self,
        context: Context,
        sample: Sample,
        now: Instant,
    ) -> Result<SampleOutcome, Error> {
        let lease = sample.lease;
        self.prepare(context, lease, now)?;
        if sample.sequence == 0 || sample.sequence <= self.last_sample {
            return Err(Error::Stale);
        }
        let held = self.held.as_ref();
        match held {
            Some(held) if held.lease != lease => return Err(Error::Stale),
            Some(_) => {}
            None => {
                if lease.stamp != context.stamp
                    || lease.transaction == 0
                    || lease.transaction <= self.last_transaction.max(self.cancelled_through)
                {
                    return Err(Error::Stale);
                }
            }
        }
        let (bytes, position) = match sample.report {
            SampleReport::Motion(report) => {
                if report.button != held.map(|held| held.button) {
                    return Err(Error::Stale);
                }
                (
                    super::encode_motion(context.modes, &report, context.cols, context.rows),
                    Some((report.col, report.row)),
                )
            }
            SampleReport::Wheel(report) => (
                super::encode_wheel(context.modes, &report, context.cols, context.rows),
                None,
            ),
        };
        let bytes = bytes.map_err(Error::Format)?;
        if held.is_none() {
            self.last_transaction = lease.transaction;
        }
        self.last_sample = sample.sequence;
        if position.is_some() && position == self.last_motion {
            return Ok(SampleOutcome::SameCell);
        }
        if self
            .sample_window
            .is_none_or(|start| now.saturating_duration_since(start) >= Duration::from_secs(1))
        {
            self.sample_window = Some(now);
            self.samples = 0;
        }
        if self.samples >= SAMPLES_PER_SECOND {
            return Ok(SampleOutcome::RateLimited);
        }
        self.samples += 1;
        if let Some(position) = position {
            self.last_motion = Some(position);
        }
        Ok(SampleOutcome::Bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mouse::Modifiers, screen::Screen};
    fn context() -> Context {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(b"\x1b[?1000;1006h");
        Context {
            stamp: Stamp {
                generation: 7,
                revision: 20,
                input_epoch: 2,
            },
            modes: screen.mouse_modes(),
            ready: true,
            cols: 80,
            rows: 24,
        }
    }
    fn event(context: Context, transaction: u64, action: Action) -> Event {
        Event {
            lease: Lease {
                stamp: context.stamp,
                mode_epoch: context.modes.epoch(),
                policy_epoch: 0,
                transaction,
            },
            report: Report {
                row: 3,
                col: 4,
                button: Button::Left,
                action,
                modifiers: Modifiers::default(),
            },
        }
    }
    fn tracking(code: usize) -> Context {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(format!("\x1b[?{code};1006h").as_bytes());
        Context {
            modes: screen.mouse_modes(),
            ..context()
        }
    }
    fn motion(context: Context, id: u64, col: usize) -> Sample {
        Sample {
            lease: event(context, id, Action::Press).lease,
            sequence: id,
            report: SampleReport::Motion(super::super::MotionReport {
                col,
                row: 3,
                button: None,
                modifiers: Modifiers::default(),
            }),
        }
    }
    fn wheel(context: Context, id: u64) -> Sample {
        Sample {
            report: SampleReport::Wheel(super::super::WheelReport {
                col: 4,
                row: 3,
                direction: super::super::WheelDirection::Down,
                modifiers: Modifiers::default(),
            }),
            ..motion(context, id, 4)
        }
    }
    #[test]
    fn unheld_samples_are_current_ordered_and_deduplicated_without_a_release_lease() {
        let mut owner = State::default();
        let context = tracking(1003);
        let now = Instant::now();
        let first = motion(context, 1, 4);
        assert_eq!(
            owner.accept_sample(context, first, now),
            Ok(SampleOutcome::Bytes(b"\x1b[<35;5;4M".to_vec()))
        );
        assert!(!owner.active());
        assert_eq!(owner.accept_sample(context, first, now), Err(Error::Stale));
        let same = motion(context, 2, 4);
        assert_eq!(
            owner.accept_sample(context, same, now),
            Ok(SampleOutcome::SameCell)
        );
        assert_eq!(owner.accept_sample(context, same, now), Err(Error::Stale));
        assert_eq!(
            owner.accept_sample(context, motion(context, 3, 5), now),
            Ok(SampleOutcome::Bytes(b"\x1b[<35;6;4M".to_vec()))
        );
        assert_eq!(
            owner.accept_sample(context, wheel(context, 4), now),
            Ok(SampleOutcome::Bytes(b"\x1b[<65;5;4M".to_vec()))
        );
        assert!(!owner.active());
        let stale = motion(context, 5, 6);
        let newer = Context {
            stamp: Stamp {
                revision: 21,
                ..context.stamp
            },
            ..context
        };
        assert_eq!(owner.accept_sample(newer, stale, now), Err(Error::Stale));
        assert_eq!(
            owner.accept_sample(newer, motion(newer, 5, 6), now),
            Ok(SampleOutcome::Bytes(b"\x1b[<35;7;4M".to_vec()))
        );
    }
    #[test]
    fn held_motion_uses_the_original_transaction_across_redraw_and_never_extends_expiry() {
        for code in [1002, 1003] {
            let mut owner = State::default();
            let context = tracking(code);
            let now = Instant::now();
            let press = event(context, 1, Action::Press);
            owner.accept(context, press, now).unwrap();
            let fresh = Context {
                stamp: Stamp {
                    revision: 40,
                    input_epoch: 3,
                    ..context.stamp
                },
                ..context
            };
            let mut sample = motion(context, 1, 5);
            if let SampleReport::Motion(report) = &mut sample.report {
                report.button = Some(Button::Left);
            }
            assert_eq!(
                owner.accept_sample(fresh, sample, now),
                Ok(SampleOutcome::Bytes(b"\x1b[<32;6;4M".to_vec()))
            );
            assert!(owner.active());
            sample.sequence = 2;
            if let SampleReport::Motion(report) = &mut sample.report {
                report.button = Some(Button::Right);
            }
            assert_eq!(owner.accept_sample(fresh, sample, now), Err(Error::Stale));
            let mut tick = wheel(context, 2);
            tick.lease = press.lease;
            assert_eq!(
                owner.accept_sample(fresh, tick, now),
                Ok(SampleOutcome::Bytes(b"\x1b[<65;5;4M".to_vec()))
            );
            assert!(owner.active());
            sample.sequence = 3;
            if let SampleReport::Motion(report) = &mut sample.report {
                report.button = Some(Button::Left);
                report.col = 6;
            }
            assert_eq!(
                owner.accept_sample(fresh, sample, now + HOLD_LIMIT),
                Err(Error::Stale)
            );
            assert!(!owner.active());
            assert_eq!(
                owner.accept(
                    fresh,
                    Event {
                        report: Report {
                            action: Action::Release,
                            ..press.report
                        },
                        ..press
                    },
                    now + HOLD_LIMIT
                ),
                Err(Error::Stale)
            );
        }
    }
    #[test]
    fn sample_rate_is_bounded_without_spending_release_capacity_or_resetting_on_toggles() {
        let mut owner = State::default();
        let context = tracking(1003);
        let now = Instant::now();
        let press = event(context, 1, Action::Press);
        owner.accept(context, press, now).unwrap();
        for id in 1..=u64::from(SAMPLES_PER_SECOND) {
            let mut tick = wheel(context, id);
            tick.lease = press.lease;
            assert!(matches!(
                owner.accept_sample(context, tick, now),
                Ok(SampleOutcome::Bytes(_))
            ));
        }
        let mut overflow = wheel(context, 31);
        overflow.lease = press.lease;
        assert_eq!(
            owner.accept_sample(context, overflow, now),
            Ok(SampleOutcome::RateLimited)
        );
        assert_eq!(
            owner.accept_sample(context, overflow, now),
            Err(Error::Stale)
        );
        assert_eq!(
            owner
                .accept(
                    context,
                    Event {
                        report: Report {
                            action: Action::Release,
                            ..press.report
                        },
                        ..press
                    },
                    now
                )
                .unwrap(),
            b"\x1b[<0;5;4m"
        );
        owner.enable(1, false);
        owner.enable(2, true);
        let mut next = wheel(context, 32);
        next.lease.policy_epoch = 2;
        assert_eq!(
            owner.accept_sample(context, next, now),
            Ok(SampleOutcome::RateLimited)
        );
        let mut next = wheel(context, 33);
        next.lease.policy_epoch = 2;
        assert_eq!(
            owner.accept_sample(context, next, now + Duration::from_secs(1)),
            Ok(SampleOutcome::Bytes(b"\x1b[<65;5;4M".to_vec()))
        );
        assert!(!owner.active());
    }
    #[test]
    fn samples_cannot_borrow_another_button_or_survive_cancellation_and_manual_input() {
        let mut owner = State::default();
        let context = tracking(1003);
        let now = Instant::now();
        owner.observe(context, now);
        let queued = motion(context, 1, 4);
        owner.revoke(1, Some(queued.lease));
        assert_eq!(owner.accept_sample(context, queued, now), Err(Error::Stale));
        let mut forged = motion(context, 2, 4);
        if let SampleReport::Motion(report) = &mut forged.report {
            report.button = Some(Button::Left);
        }
        assert_eq!(owner.accept_sample(context, forged, now), Err(Error::Stale));
        let fresh = motion(context, 2, 4);
        owner.intervene();
        assert_eq!(owner.accept_sample(context, fresh, now), Err(Error::Stale));
        let next_generation = Context {
            stamp: Stamp {
                generation: 8,
                ..context.stamp
            },
            ..context
        };
        assert_eq!(
            owner.accept_sample(next_generation, fresh, now),
            Err(Error::Stale)
        );
        let mut new = motion(next_generation, 1, 4);
        new.lease.policy_epoch = 1;
        assert!(matches!(
            owner.accept_sample(next_generation, new, now),
            Ok(SampleOutcome::Bytes(_))
        ));
    }
    #[test]
    fn one_press_and_owned_release_survive_redraw_but_replays_do_not() {
        let mut owner = State::default();
        let context = context();
        let now = Instant::now();
        let press = event(context, 1, Action::Press);
        assert_eq!(owner.accept(context, press, now).unwrap(), b"\x1b[<0;5;4M");
        assert_eq!(owner.accept(context, press, now), Err(Error::Stale));
        let fresh = Context {
            stamp: Stamp {
                revision: 40,
                input_epoch: 3,
                ..context.stamp
            },
            ..context
        };
        let release = Event {
            report: Report {
                action: Action::Release,
                ..press.report
            },
            ..press
        };
        assert_eq!(owner.accept(fresh, release, now).unwrap(), b"\x1b[<0;5;4m");
        assert_eq!(owner.accept(fresh, release, now), Err(Error::Stale));
        assert_eq!(owner.accept(fresh, press, now), Err(Error::Stale));
        assert!(!owner.active());
    }
    #[test]
    fn a_cancelled_queued_press_never_starts_and_old_cancels_do_not_revoke_new_policy() {
        let mut owner = State::default();
        let context = context();
        let now = Instant::now();
        owner.observe(context, now);
        let press = event(context, 1, Action::Press);
        owner.revoke(1, Some(press.lease));
        assert_eq!(owner.accept(context, press, now), Err(Error::Stale));
        let next = event(context, 2, Action::Press);
        owner.accept(context, next, now).unwrap();
        owner.revoke(2, Some(press.lease));
        assert!(owner.active());
        owner.enable(3, true);
        let mut fresh = event(context, 3, Action::Press);
        fresh.lease.policy_epoch = 1;
        owner.accept(context, fresh, now).unwrap();
        owner.revoke(4, Some(next.lease));
        assert!(owner.active());
    }
    #[test]
    fn wrong_button_or_old_cancel_cannot_end_a_newer_transaction() {
        let mut owner = State::default();
        let context = context();
        let now = Instant::now();
        let press = event(context, 2, Action::Press);
        owner.accept(context, press, now).unwrap();
        let wrong = Event {
            report: Report {
                button: Button::Right,
                action: Action::Release,
                ..press.report
            },
            ..press
        };
        assert_eq!(owner.accept(context, wrong, now), Err(Error::Stale));
        owner.revoke(1, Some(event(context, 1, Action::Press).lease));
        assert!(owner.active());
        assert_eq!(owner.snapshot(context).policy_epoch, 0);
        owner.revoke(2, Some(press.lease));
        assert!(!owner.active());
        assert_eq!(
            owner.accept(
                context,
                Event {
                    report: Report {
                        action: Action::Release,
                        ..press.report
                    },
                    ..press
                },
                now
            ),
            Err(Error::Stale)
        );
    }
    #[test]
    fn policy_mode_generation_and_raw_intervention_revoke_without_replay() {
        for reason in ["toggle", "revoke", "mode", "generation", "manual"] {
            let mut owner = State::default();
            let mut context = context();
            let now = Instant::now();
            let press = event(context, 1, Action::Press);
            owner.accept(context, press, now).unwrap();
            match reason {
                "toggle" => {
                    owner.enable(1, false);
                    owner.enable(2, true);
                }
                "revoke" => owner.revoke(1, None),
                "mode" => {
                    let mut screen = Screen::new(80, 24).unwrap();
                    screen.feed(b"\x1b[?1000;1006h\x1b[?1000l\x1b[?1000h");
                    context.modes = screen.mouse_modes();
                    owner.observe(context, now);
                }
                "generation" => {
                    context.stamp.generation += 1;
                    owner.observe(context, now);
                }
                "manual" => {
                    owner.intervene();
                    context.stamp.input_epoch += 1;
                }
                _ => unreachable!(),
            }
            assert!(!owner.active(), "{reason}");
            let release = Event {
                report: Report {
                    action: Action::Release,
                    ..press.report
                },
                ..press
            };
            assert_eq!(
                owner.accept(context, release, now),
                Err(Error::Stale),
                "{reason}"
            );
            assert_eq!(
                owner.accept(context, press, now),
                Err(Error::Stale),
                "{reason}"
            );
        }
    }
    #[test]
    fn busy_bad_coordinates_and_hold_expiry_never_queue_a_delayed_release() {
        let mut owner = State::default();
        let context = context();
        let now = Instant::now();
        let press = event(context, 1, Action::Press);
        assert_eq!(
            owner.accept(
                Context {
                    ready: false,
                    ..context
                },
                press,
                now
            ),
            Err(Error::Busy)
        );
        owner.accept(context, press, now).unwrap();
        let release = Event {
            report: Report {
                action: Action::Release,
                col: 80,
                ..press.report
            },
            ..press
        };
        assert_eq!(
            owner.accept(context, release, now),
            Err(Error::Format(FormatError::OutsideGrid))
        );
        assert!(!owner.active());
        let press = event(context, 2, Action::Press);
        owner.accept(context, press, now).unwrap();
        assert!(!owner.expire(now + HOLD_LIMIT - Duration::from_nanos(1)));
        assert!(owner.expire(now + HOLD_LIMIT));
        assert_eq!(
            owner.accept(
                context,
                Event {
                    report: Report {
                        action: Action::Release,
                        ..press.report
                    },
                    ..press
                },
                now + HOLD_LIMIT
            ),
            Err(Error::Stale)
        );
    }
    #[test]
    fn rate_limit_reserves_releases_and_does_not_reset_on_policy_toggles() {
        let mut owner = State::default();
        let context = context();
        let now = Instant::now();
        for id in 1..=u64::from(PRESSES_PER_SECOND) {
            let press = event(context, id, Action::Press);
            owner.accept(context, press, now).unwrap();
            owner
                .accept(
                    context,
                    Event {
                        report: Report {
                            action: Action::Release,
                            ..press.report
                        },
                        ..press
                    },
                    now,
                )
                .unwrap();
        }
        let id = u64::from(PRESSES_PER_SECOND) + 1;
        let mut press = event(context, id, Action::Press);
        assert_eq!(owner.accept(context, press, now), Err(Error::RateLimited));
        owner.enable(1, false);
        owner.enable(2, true);
        press.lease.policy_epoch = 2;
        assert_eq!(owner.accept(context, press, now), Err(Error::RateLimited));
        assert_eq!(
            owner
                .accept(context, press, now + Duration::from_secs(1))
                .unwrap(),
            b"\x1b[<0;5;4M"
        );
        let release = Event {
            report: Report {
                action: Action::Release,
                ..press.report
            },
            ..press
        };
        assert_eq!(
            owner
                .accept(context, release, now + Duration::from_secs(1))
                .unwrap(),
            b"\x1b[<0;5;4m"
        );
    }
    #[test]
    fn manual_noop_revokes_queued_leases_without_consuming_a_control_serial() {
        let mut owner = State::default();
        let context = context();
        let now = Instant::now();
        owner.observe(context, now);
        let old = event(context, 1, Action::Press);
        owner.intervene();
        assert_eq!(owner.accept(context, old, now), Err(Error::Stale));
        owner.enable(1, false);
        assert!(!owner.snapshot(context).enabled);
        assert_eq!(owner.snapshot(context).policy_epoch, 2);
        owner.enable(2, true);
        let mut next = event(context, 2, Action::Press);
        next.lease.policy_epoch = 3;
        assert_eq!(owner.accept(context, next, now).unwrap(), b"\x1b[<0;5;4M");
    }
    #[test]
    fn transport_interruption_drops_the_held_edge_and_preserves_the_user_preference() {
        let mut owner = State::default();
        let context = context();
        let now = Instant::now();
        let press = event(context, 1, Action::Press);
        owner.accept(context, press, now).unwrap();
        owner.interrupt();
        assert!(!owner.active());
        assert_eq!(owner.snapshot(context).policy_epoch, 0);
        assert!(owner.snapshot(context).enabled);
        assert_eq!(
            owner.accept(
                context,
                Event {
                    report: Report {
                        action: Action::Release,
                        ..press.report
                    },
                    ..press
                },
                now
            ),
            Err(Error::Stale)
        );
        assert_eq!(owner.accept(context, press, now), Err(Error::Stale));
    }
}
