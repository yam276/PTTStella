//! A single monotonic idle clock owned by the connection, never a replay queue.
use crate::navigation::Stamp;
use std::time::{Duration, Instant};

pub(super) const REFRESH: &[u8] = b"\x0c\x0c";

#[derive(Clone, Copy)]
pub(super) struct Timing {
    pub idle: Duration,
    pub poll: Duration,
    pub resume_gap: Duration,
    pub response_timeout: Duration,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            idle: Duration::from_secs(60),
            poll: Duration::from_secs(1),
            resume_gap: Duration::from_secs(5),
            response_timeout: Duration::from_secs(5),
        }
    }
}

pub(super) struct Idle {
    pub enabled: bool,
    permit: Option<Stamp>,
    last_activity: Instant,
    last_tick: Instant,
    timing: Timing,
}
impl Idle {
    pub fn new(now: Instant, timing: Timing) -> Self {
        Self {
            enabled: true,
            permit: None,
            last_activity: now,
            last_tick: now,
            timing,
        }
    }
    pub fn activity(&mut self, now: Instant) {
        self.last_activity = now;
        self.permit = None;
    }
    pub fn enable(&mut self, enabled: bool, now: Instant) {
        self.enabled = enabled;
        self.activity(now);
    }
    pub fn gate(&mut self, stamp: Stamp, ready: bool, now: Instant) {
        let permit = ready.then_some(stamp);
        if self.permit != permit {
            self.activity(now);
            self.permit = permit;
        }
    }
    pub fn ready(&self, stamp: Stamp) -> bool {
        self.enabled && self.permit == Some(stamp)
    }
    pub fn tick(&mut self, now: Instant) {
        if now.duration_since(self.last_tick) > self.timing.resume_gap {
            // Resume or a stalled owner starts a fresh idle period, with fresh UI approval.
            self.activity(now);
        }
        self.last_tick = now;
    }
    pub fn due(&self, now: Instant, stamp: Stamp) -> bool {
        self.ready(stamp) && now.duration_since(self.last_activity) >= self.timing.idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stamp() -> Stamp {
        Stamp {
            generation: 1,
            revision: 2,
            input_epoch: 3,
        }
    }
    #[test]
    fn default_on_requires_current_ui_permission_and_sixty_seconds() {
        let now = Instant::now();
        let mut idle = Idle::new(now, Timing::default());
        assert!(idle.enabled);
        assert!(!idle.due(now + Duration::from_secs(60), stamp()));
        idle.gate(stamp(), true, now);
        assert!(!idle.due(now + Duration::from_secs(59), stamp()));
        assert!(idle.due(now + Duration::from_secs(60), stamp()));
        assert!(!idle.due(
            now + Duration::from_secs(60),
            Stamp {
                input_epoch: 4,
                ..stamp()
            }
        ));
        idle.activity(now + Duration::from_secs(60));
        assert!(!idle.due(now + Duration::from_secs(600), stamp()));
    }
    #[test]
    fn pause_resume_and_sleep_do_not_accumulate_refreshes() {
        let now = Instant::now();
        let mut idle = Idle::new(now, Timing::default());
        idle.gate(stamp(), true, now);
        idle.gate(stamp(), false, now + Duration::from_secs(50));
        idle.gate(stamp(), true, now + Duration::from_secs(80));
        assert!(!idle.due(now + Duration::from_secs(100), stamp()));
        idle.tick(now + Duration::from_secs(600));
        assert!(!idle.due(now + Duration::from_secs(600), stamp()));
        idle.gate(stamp(), true, now + Duration::from_secs(600));
        assert!(!idle.due(now + Duration::from_secs(659), stamp()));
        assert!(idle.due(now + Duration::from_secs(660), stamp()));
        idle.enable(false, now);
        idle.gate(stamp(), true, now);
        assert!(!idle.due(now + Duration::from_secs(1000), stamp()));
        idle.enable(true, now);
        assert!(!idle.due(now + Duration::from_secs(1000), stamp()));
    }
}
