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
            if entry.policy != Policy::Allow
                && merged
                    .entries
                    .iter()
                    .any(|e| e.id.eq_ignore_ascii_case(&entry.id) && e.policy == Policy::Allow)
            {
                continue;
            }
            // A supplied marker colour replaces the old override. Unmarked
            // imports preserve a local override rather than silently reset it.
            let color = entry.color.or_else(|| {
                merged
                    .entries
                    .iter()
                    .find(|e| e.id.eq_ignore_ascii_case(&entry.id))
                    .filter(|e| e.policy == Policy::Allow && entry.policy == Policy::Allow)
                    .and_then(|e| e.color)
            });
            merged = merged.edit_color(&entry.id, &entry.note, entry.policy, color, false)?;
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
    fn imported_colours_survive_preview_apply_and_serialization_without_resetting_local_overrides()
    {
        let current = Rules::default()
            .with_allow_color(11)
            .unwrap()
            .edit_color("Alice", "local", Policy::Allow, Some(13), false)
            .unwrap()
            .edit_color("Bob", "local", Policy::Allow, Some(12), false)
            .unwrap();
        let imported = pttstella::filters::import::parse(
            b"alice=W2:imported\nBob=W:no marker\nCarol=W:shared\nDave=C9:row\nEve=B2:content",
        )
        .unwrap();
        let mut owner = Import::default();
        owner.begin(1);
        let preview = owner.preview(1, &current, &imported.entries).unwrap();
        assert_eq!(current.entries[0].color, Some(13));
        assert_eq!(preview.allow_color, 11);
        assert_eq!(
            preview
                .entries
                .iter()
                .map(|e| (e.policy, e.color))
                .collect::<Vec<_>>(),
            vec![
                (Policy::Allow, Some(10)),
                (Policy::Allow, Some(12)),
                (Policy::Allow, None),
                (Policy::MaskRow, None),
                (Policy::Mask, None)
            ]
        );
        let saved = owner.apply(1, &current).unwrap();
        assert_eq!(saved, preview);
        assert_eq!(
            Rules::decode(&serde_json::to_vec(&saved).unwrap()).unwrap(),
            preview
        );
        owner.begin(2);
        owner.preview(2, &current, &imported.entries).unwrap();
        let externally_changed = current.with_allow_color(14).unwrap();
        assert!(owner.apply(2, &externally_changed).is_err());
    }
    #[test]
    fn preview_is_explicit_white_wins_and_cancelled_or_changed_sources_cannot_apply() {
        let current = Rules::default()
            .edit_policy("Alice", "local exception", Policy::Allow, false)
            .unwrap();
        let imported = vec![
            Entry {
                id: "ALICE".into(),
                note: "mask row".into(),
                policy: Policy::MaskRow,
                color: None,
            },
            Entry {
                id: "Bob".into(),
                note: "".into(),
                policy: Policy::Mask,
                color: None,
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
