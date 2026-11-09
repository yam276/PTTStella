//! Notification semantics shared by the native host and the DOM policy adapter.
//! Message text never decides whether a failure can disappear.
use serde::Deserialize;
use std::time::Duration;

pub const POLICY: &str = include_str!("../assets/notification-policy.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Persistent,
    Error,
    Copy,
    Success,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    wait_ms: Option<u64>,
    blocks_timed: bool,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    schema: u32,
    fade_ms: u64,
    persistent: Rule,
    error: Rule,
    copy: Rule,
    success: Rule,
}

impl Policy {
    pub fn embedded() -> Result<Self, String> {
        let policy: Self = serde_json::from_str(POLICY).map_err(|e| e.to_string())?;
        if policy.schema != 1
            || policy.fade_ms == 0
            || policy.persistent.wait_ms.is_some()
            || policy.error.wait_ms.is_some()
            || !policy.error.blocks_timed
            || policy.copy.wait_ms.is_none_or(|ms| ms == 0)
            || policy.success.wait_ms.is_none_or(|ms| ms == 0)
        {
            return Err("invalid embedded notification policy".into());
        }
        Ok(policy)
    }
    fn rule(self, kind: Kind) -> Rule {
        match kind {
            Kind::Persistent => self.persistent,
            Kind::Error => self.error,
            Kind::Copy => self.copy,
            Kind::Success => self.success,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Persistent,
    Wait,
    Fade,
}

/// One outlet, no queue and no background polling. The adapter owns one timer
/// and feeds a monotonic elapsed clock; paused time is never consumed.
pub struct State {
    policy: Policy,
    kind: Kind,
    phase: Phase,
    remaining: Duration,
    started: Option<Duration>,
    hovered: bool,
    focused: bool,
    reduced: bool,
    stopped: bool,
}

impl State {
    pub fn new(policy: Policy, reduced: bool) -> Self {
        Self {
            policy,
            kind: Kind::Persistent,
            phase: Phase::Idle,
            remaining: Duration::ZERO,
            started: None,
            hovered: false,
            focused: false,
            reduced,
            stopped: false,
        }
    }
    pub fn visible(&self) -> bool {
        self.phase != Phase::Idle
    }
    pub fn error(&self) -> bool {
        self.kind == Kind::Error && self.visible()
    }
    pub fn fading(&self) -> bool {
        self.phase == Phase::Fade && !self.hovered && !self.focused
    }
    pub fn delay(&self) -> Option<Duration> {
        self.started.map(|_| self.remaining)
    }
    fn settle(&mut self, now: Duration) {
        if let Some(started) = self.started.take() {
            self.remaining = self.remaining.saturating_sub(now.saturating_sub(started));
        }
    }
    fn resume(&mut self, now: Duration) {
        if self.stopped || self.hovered || self.focused {
            return;
        }
        if self.phase == Phase::Fade && self.reduced {
            self.clear();
        } else if matches!(self.phase, Phase::Wait | Phase::Fade) {
            self.started = Some(now);
        }
    }
    pub fn show(&mut self, now: Duration, kind: Kind) -> bool {
        let rule = self.policy.rule(kind);
        if self.stopped
            || self.visible() && self.policy.rule(self.kind).blocks_timed && rule.wait_ms.is_some()
        {
            return false;
        }
        self.kind = kind;
        self.started = None;
        self.phase = if let Some(ms) = rule.wait_ms {
            self.remaining = Duration::from_millis(ms);
            Phase::Wait
        } else {
            self.remaining = Duration::ZERO;
            Phase::Persistent
        };
        self.resume(now);
        true
    }
    pub fn interaction(&mut self, now: Duration, hovered: bool, focused: bool, reduced: bool) {
        self.settle(now);
        self.hovered = hovered;
        self.focused = focused;
        self.reduced = reduced;
        self.resume(now);
    }
    pub fn expire(&mut self, now: Duration) {
        if self.started.is_none() {
            return;
        }
        self.settle(now);
        if !self.remaining.is_zero() {
            self.resume(now);
            return;
        }
        if self.phase == Phase::Wait && !self.reduced {
            self.phase = Phase::Fade;
            self.remaining = Duration::from_millis(self.policy.fade_ms);
            self.resume(now);
        } else {
            self.clear();
        }
    }
    pub fn clear(&mut self) {
        self.phase = Phase::Idle;
        self.started = None;
        self.remaining = Duration::ZERO;
        self.hovered = false;
        self.focused = false;
    }
    pub fn stop(&mut self) {
        self.clear();
        self.stopped = true;
    }
}

#[cfg(test)]
mod tests;
