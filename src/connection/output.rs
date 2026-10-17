//! Bounded presentation delay; the Screen owns the actual protocol mode.
use std::time::{Duration, Instant};

// Product recovery bound, not a protocol-mandated timeout. A slow connection
// may render a partial update after this bound rather than freeze indefinitely.
pub(super) const MAX_DELAY: Duration = Duration::from_secs(1);

#[derive(Default)]
pub(super) struct Deadline(Option<Batch>);
struct Batch {
    begin: u64,
    until: Instant,
}
impl Deadline {
    pub(super) fn track(&mut self, begin: Option<u64>, now: Instant) {
        match begin {
            Some(begin) if self.0.as_ref().is_none_or(|batch| batch.begin != begin) => {
                self.0 = Some(Batch {
                    begin,
                    until: now + MAX_DELAY,
                });
            }
            None => self.0 = None,
            _ => {}
        }
    }
    pub(super) fn active(&self) -> bool {
        self.0.is_some()
    }
    pub(super) fn expire(&mut self, now: Instant) -> bool {
        if self.0.as_ref().is_some_and(|batch| now >= batch.until) {
            self.0 = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_begin_does_not_postpone_recovery_and_new_batches_get_a_new_bound() {
        let now = Instant::now();
        let mut deadline = Deadline::default();
        deadline.track(Some(12), now);
        deadline.track(Some(12), now + MAX_DELAY / 2);
        assert!(!deadline.expire(now + MAX_DELAY - Duration::from_nanos(1)));
        assert!(deadline.expire(now + MAX_DELAY));
        assert!(!deadline.active());
        assert!(!deadline.expire(now + MAX_DELAY));
        deadline.track(Some(30), now + MAX_DELAY);
        assert!(!deadline.expire(now + MAX_DELAY));
        deadline.track(None, now + MAX_DELAY);
        assert!(!deadline.active());
        deadline.track(Some(50), now + MAX_DELAY);
        deadline.track(Some(70), now + MAX_DELAY * 2);
        assert!(!deadline.expire(now + MAX_DELAY * 2));
        assert!(deadline.expire(now + MAX_DELAY * 3));
    }
}
