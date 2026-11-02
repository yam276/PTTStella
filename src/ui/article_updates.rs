//! One opt-in refresh deadline, owned by the surface loop; never a sender task.
use pttstella::{
    connection::View,
    navigation::Stamp,
    navigation_job::{Destination, Progress},
};
use serde::Serialize;
use tokio::time::{Duration, Instant};
const INTERVAL: Duration = Duration::from_secs(5);
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct Status {
    pub generation: u64,
    pub article: u64,
    pub enabled: bool,
    pub available: bool,
    pub busy: bool,
    pub failed: bool,
    pub recovery: bool,
}
#[derive(Default)]
pub(super) struct Updates {
    scope: Option<(u64, u64)>,
    enabled: bool,
    failed: bool,
    next: Option<Instant>,
    operation: Option<(Stamp, Option<u64>, Instant)>,
    fetching: bool,
    cancelled: Option<Stamp>,
    preserved: bool,
}
impl Updates {
    pub fn sync(&mut self, scope: Option<(u64, u64)>) {
        if self.scope != scope {
            self.stop(false);
            self.scope = scope;
            self.preserved = false;
        }
    }
    pub fn status(&self) -> Option<Status> {
        let (generation, article) = self.scope?;
        Some(Status {
            generation,
            article,
            enabled: self.enabled,
            available: false,
            busy: self.operation.is_some() || self.fetching,
            failed: self.failed,
            recovery: self.preserved,
        })
    }
    pub fn action(
        &mut self,
        generation: u64,
        article: u64,
        enabled: bool,
        available: bool,
        now: Instant,
    ) -> bool {
        if self.scope != Some((generation, article))
            || enabled
                && (!available || self.operation.is_some() || self.fetching || self.preserved)
        {
            return false;
        }
        if enabled {
            self.enabled = true;
            self.failed = false;
            self.next = Some(now);
        } else {
            self.stop(false);
        }
        true
    }
    pub fn stop(&mut self, failed: bool) {
        if let Some((stamp, _, _)) = self.operation.take() {
            self.cancelled = Some(stamp);
            self.preserved = true;
        }
        self.enabled = false;
        self.failed = failed;
        self.next = None;
        self.fetching = false;
    }
    pub fn take_cancelled(&mut self) -> Option<Stamp> {
        self.cancelled.take()
    }
    pub fn holding(&self) -> bool {
        self.operation.is_some() || self.preserved
    }
    pub fn fetching(&self) -> bool {
        self.fetching
    }
    pub fn deadline(&self, allowed: bool) -> Option<Instant> {
        self.operation
            .map(|v| v.2)
            .or_else(|| allowed.then_some(self.next).flatten())
    }
    pub fn due(&self, now: Instant) -> bool {
        self.enabled
            && !self.fetching
            && self.operation.is_none()
            && self.next.is_some_and(|at| now >= at)
    }
    pub fn begin(&mut self, stamp: Stamp, now: Instant) {
        self.failed = false;
        self.preserved = false;
        self.next = None;
        self.operation = Some((stamp, None, now + Duration::from_secs(30)));
    }
    /// True means keep the acknowledged article projection during re-entry.
    pub fn observe(&mut self, view: &View, now: Instant) -> bool {
        let valid = self
            .scope
            .is_some_and(|(generation, _)| generation == view.generation)
            && view.phase == pttstella::connection::Phase::Connected
            && view.site.ptt_features()
            && view.mouse.route == pttstella::mouse::input::Route::Local;
        if !valid {
            self.stop(true);
            self.preserved = false;
            return false;
        }
        if self.preserved {
            return true;
        }
        let Some((stamp, serial, deadline)) = self.operation else {
            return false;
        };
        if now >= deadline {
            self.stop(true);
            return true;
        }
        match &view.destination {
            Progress::Active {
                operation, serial, ..
            } if *operation == stamp => {
                self.operation = Some((stamp, Some(*serial), deadline));
                true
            }
            Progress::Reached {
                serial: done,
                target: Destination::Article(_),
                operation,
            } if *operation == stamp && serial.is_none_or(|seen| seen == *done) => {
                self.operation = None;
                self.fetching = true;
                false
            }
            _ if serial.is_none() && view.input_epoch <= stamp.input_epoch => true,
            _ => {
                self.stop(true);
                true
            }
        }
    }
    pub fn finish(&mut self, now: Instant) {
        self.fetching = false;
        if self.enabled {
            self.next = Some(now + INTERVAL);
        }
    }
    pub fn defer(&mut self, now: Instant) {
        if self.enabled && !self.holding() && !self.fetching {
            self.next = Some(now + INTERVAL);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ready() -> (Updates, Instant) {
        let mut owner = Updates::default();
        let now = Instant::now();
        owner.sync(Some((7, 1)));
        assert!(!owner.due(now));
        assert!(!owner.action(6, 1, true, true, now));
        assert!(!owner.action(7, 2, true, true, now));
        assert!(!owner.action(7, 1, true, false, now));
        assert!(owner.action(7, 1, true, true, now));
        (owner, now)
    }
    fn view() -> View {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/filters.json")).unwrap();
        let mut screen = pttstella::screen::Screen::new(80, 24).unwrap();
        screen.feed(&pttstella::encoding::encode(fixture["reading"].as_str().unwrap()).unwrap());
        View {
            site: Default::default(),
            mouse: Default::default(),
            output_pending: false,
            frame: screen.frame(),
            generation: 7,
            input_epoch: 2,
            input_pending: false,
            login_pending: false,
            login_guidance: pttstella::login::Guidance::Idle,
            destination: Progress::Idle,
            board_visit: None,
            phase: pttstella::connection::Phase::Connected,
            message: String::new(),
            received: 1,
            keep_alive: false,
            keep_alive_ready: false,
            keep_alive_sent: 0,
            keep_alive_failure: None,
        }
    }
    #[test]
    fn one_deadline_defers_without_catchup_and_scope_change_revokes_once() {
        let (mut owner, now) = ready();
        assert!(owner.due(now));
        owner.defer(now);
        assert!(!owner.due(now + Duration::from_secs(4)));
        assert!(owner.deadline(false).is_none());
        assert!(owner.due(now + INTERVAL));
        let stamp = Stamp {
            generation: 7,
            revision: 3,
            input_epoch: 2,
        };
        owner.begin(stamp, now);
        assert!(!owner.due(now + INTERVAL));
        assert_eq!(owner.deadline(false), Some(now + Duration::from_secs(30)));
        owner.sync(Some((7, 2)));
        assert_eq!(owner.take_cancelled(), Some(stamp));
        assert!(owner.take_cancelled().is_none());
        assert!(!owner.status().unwrap().enabled);
        owner.finish(now);
        assert!(owner.deadline(true).is_none());
    }
    #[test]
    fn cancellation_during_fetch_never_reschedules_and_timeout_is_bounded() {
        let (mut owner, now) = ready();
        owner.fetching = true;
        owner.next = None;
        assert!(owner.action(7, 1, false, false, now));
        owner.finish(now);
        assert!(!owner.due(now + INTERVAL));
        assert!(!owner.status().unwrap().busy);
        let (mut owner, now) = ready();
        owner.begin(
            Stamp {
                generation: 7,
                revision: 3,
                input_epoch: 2,
            },
            now,
        );
        let view = view();
        assert!(owner.observe(&view, now));
        assert!(owner.observe(&view, now + Duration::from_secs(30)));
        assert!(owner.status().unwrap().failed);
        assert!(owner.take_cancelled().is_some());
    }
    #[test]
    fn exact_operation_completion_survives_coalesced_watch_but_foreign_completion_never_attaches() {
        for foreign in [false, true] {
            let (mut owner, now) = ready();
            let stamp = Stamp {
                generation: 7,
                revision: 3,
                input_epoch: 2,
            };
            owner.begin(stamp, now);
            let mut done = view();
            done.input_epoch = 4;
            done.destination = Progress::Reached {
                serial: 9,
                operation: if foreign {
                    Stamp {
                        input_epoch: 3,
                        ..stamp
                    }
                } else {
                    stamp
                },
                target: Destination::Article(
                    pttstella::article_target::ArticleTarget::new("Stella", "00000101").unwrap(),
                ),
            };
            assert_eq!(owner.observe(&done, now), foreign);
            assert_eq!(owner.fetching(), !foreign);
            assert_eq!(owner.status().unwrap().recovery, foreign);
            if !foreign {
                owner.finish(now);
                assert!(!owner.due(now + Duration::from_millis(4999)));
                assert!(owner.due(now + Duration::from_secs(5)));
            }
        }
    }
}
