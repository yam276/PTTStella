//! A bounded control snapshot bypasses the input queue without losing an off/on
//! cycle or a queued-press cancellation to watch coalescing.
use crate::mouse::input::Lease;
#[derive(Clone, Copy)]
pub(super) struct Preference {
    pub generation: u64,
    pub serial: u64,
    pub enabled: bool,
}
#[derive(Clone, Copy)]
pub(super) struct Revoke {
    pub generation: u64,
    pub serial: u64,
}
#[derive(Clone, Copy)]
pub(super) struct Cancel {
    pub lease: Lease,
    pub serial: u64,
}
#[derive(Clone, Copy, Default)]
pub(super) struct Controls {
    serial: u64,
    pub preference: Option<Preference>,
    pub revoke: Option<Revoke>,
    pub cancel: Option<Cancel>,
}
impl Controls {
    pub fn enable(&mut self, generation: u64, enabled: bool) {
        if self
            .preference
            .is_some_and(|old| old.generation > generation)
        {
            return;
        }
        self.serial = self.serial.saturating_add(1);
        self.preference = Some(Preference {
            generation,
            serial: self.serial,
            enabled,
        });
    }
    pub fn revoke(&mut self, generation: u64) {
        if self.revoke.is_some_and(|old| old.generation > generation) {
            return;
        }
        self.serial = self.serial.saturating_add(1);
        self.revoke = Some(Revoke {
            generation,
            serial: self.serial,
        });
    }
    pub fn cancel(&mut self, lease: Lease) {
        if self.cancel.is_some_and(|old| {
            old.lease.stamp.generation > lease.stamp.generation
                || old.lease.stamp.generation == lease.stamp.generation
                    && old.lease.transaction > lease.transaction
        }) {
            return;
        }
        self.serial = self.serial.saturating_add(1);
        self.cancel = Some(Cancel {
            lease,
            serial: self.serial,
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coalesced_controls_keep_preferences_global_revoke_and_newest_pointer_separate() {
        let mut controls = Controls::default();
        controls.enable(7, false);
        controls.enable(7, true);
        controls.revoke(7);
        let lease = Lease {
            stamp: crate::navigation::Stamp {
                generation: 7,
                revision: 1,
                input_epoch: 0,
            },
            mode_epoch: 2,
            policy_epoch: 0,
            transaction: 4,
        };
        controls.cancel(lease);
        controls.cancel(Lease {
            transaction: 3,
            ..lease
        });
        controls.enable(6, false);
        controls.revoke(6);
        assert!(controls.preference.unwrap().enabled);
        assert_eq!(controls.preference.unwrap().serial, 2);
        assert_eq!(controls.revoke.unwrap().serial, 3);
        assert_eq!(controls.cancel.unwrap().lease.transaction, 4);
        assert_eq!(controls.cancel.unwrap().serial, 4);
    }
}
