//! One pending import preview; files are never applied without confirmation.
use pttstella::filters::{Entry, Policy, Rules};

#[derive(Default)]
pub(super) struct Import {
    request: Option<u64>,
    preview: Option<(Rules, Rules)>,
}
impl Import {
    pub fn begin(&mut self, request: u64) {
        self.request = Some(request);
        self.preview = None;
    }
    pub fn cancel(&mut self, request: u64) {
        if self.request == Some(request) {
            self.clear();
        }
    }
    pub fn clear(&mut self) {
        self.request = None;
        self.preview = None;
    }
    pub fn accepts(&self, request: u64) -> bool {
        self.request == Some(request)
    }
    pub fn preview(
        &mut self,
        request: u64,
        current: &Rules,
        entries: &[Entry],
    ) -> Result<Rules, String> {
        if !self.accepts(request) {
            return Err(pttstella::msg!("filters-import-stale"));
        }
        let mut merged = current.clone();
        for entry in entries {
            // An existing explicit exception survives imported masks.
            if entry.policy == Policy::Mask
                && merged
                    .entries
                    .iter()
                    .any(|e| e.id.eq_ignore_ascii_case(&entry.id) && e.policy == Policy::Allow)
            {
                continue;
            }
            merged = merged.edit_policy(&entry.id, &entry.note, entry.policy, false)?;
        }
        self.preview = Some((current.clone(), merged.clone()));
        Ok(merged)
    }
    pub fn apply(&mut self, request: u64, current: &Rules) -> Result<Rules, String> {
        if !self.accepts(request) {
            return Err(pttstella::msg!("filters-import-stale"));
        }
        let Some((base, next)) = self.preview.as_ref() else {
            return Err(pttstella::msg!("filters-import-stale"));
        };
        if base != current {
            self.clear();
            return Err(pttstella::msg!("filters-import-stale"));
        }
        let next = next.clone();
        self.clear();
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_is_explicit_white_wins_and_cancelled_or_changed_sources_cannot_apply() {
        let current = Rules::default()
            .edit_policy("Alice", "local exception", Policy::Allow, false)
            .unwrap();
        let imported = vec![
            Entry {
                id: "ALICE".into(),
                note: "mask".into(),
                policy: Policy::Mask,
            },
            Entry {
                id: "Bob".into(),
                note: "".into(),
                policy: Policy::Mask,
            },
        ];
        let mut owner = Import::default();
        owner.begin(1);
        let result = owner.preview(1, &current, &imported).unwrap();
        assert_eq!(current.entries.len(), 1);
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].policy, Policy::Allow);
        assert_eq!(owner.apply(1, &current).unwrap(), result);
        assert!(owner.apply(1, &current).is_err());
        owner.begin(2);
        owner.cancel(2);
        assert!(owner.preview(2, &current, &imported).is_err());
        owner.begin(3);
        owner.preview(3, &current, &imported).unwrap();
        owner.begin(4);
        assert!(owner.apply(3, &current).is_err());
        owner.preview(4, &current, &imported).unwrap();
        let changed = current.edit("Carol", "", false).unwrap();
        assert!(owner.apply(4, &changed).is_err());
    }
}
