//! Kernel-locked merge transactions. A stale editor gets a separate copy.
use super::{Drafts, Entry, MAX_BYTES};
use crate::storage::shared::Document;
use std::path::PathBuf;
#[derive(Clone)]
pub struct Store {
    document: Document,
}
#[derive(Clone, serde::Serialize)]
pub struct Saved {
    pub entry: Entry,
    pub conflicted: bool,
}
impl Store {
    pub fn at(path: PathBuf) -> Self {
        Self {
            document: Document::new(path),
        }
    }
    pub fn system() -> Result<Self, String> {
        let dirs = directories::ProjectDirs::from("", "", "PTTStella")
            .ok_or(crate::msg!("draft-invalid"))?;
        Ok(Self::at(dirs.data_local_dir().join("drafts.json")))
    }
    pub fn load(&self) -> Result<Drafts, String> {
        self.document
            .read(MAX_BYTES)
            .map_err(|_| crate::msg!("draft-read-failed"))?
            .map_or_else(|| Ok(Drafts::empty()), |b| Drafts::decode(&b))
    }
    pub fn save(
        &self,
        base: Option<Entry>,
        mut entry: Entry,
        conflict_id: String,
    ) -> Result<Saved, String> {
        entry.validate()?;
        self.document.update(MAX_BYTES, None, |bytes, _| {
            let mut data = bytes.map_or_else(|| Ok(Drafts::empty()), Drafts::decode)?;
            let index = data.entries.iter().position(|e| e.id == entry.id);
            let current = index.map(|i| &data.entries[i]);
            let conflict = current != base.as_ref();
            if conflict {
                entry.id = conflict_id;
                entry.revision = 1;
                entry.conflict = true;
                if data.entries.iter().any(|e| e.id == entry.id) {
                    return Err(crate::msg!("draft-invalid"));
                }
                data.entries.push(entry.clone());
            } else {
                entry.revision = base.as_ref().map_or(Ok(1), |b| {
                    b.revision.checked_add(1).ok_or(crate::msg!("draft-limit"))
                })?;
                if let Some(index) = index {
                    data.entries[index] = entry.clone();
                } else {
                    data.entries.push(entry.clone());
                }
            }
            let bytes = data.encode()?;
            Ok((
                Saved {
                    entry,
                    conflicted: conflict,
                },
                bytes,
            ))
        })
    }
    pub fn remove(&self, base: Entry) -> Result<(), String> {
        self.document.update(MAX_BYTES, None, |bytes, _| {
            let mut data = bytes.map_or_else(|| Ok(Drafts::empty()), Drafts::decode)?;
            if data.entries.iter().find(|e| e.id == base.id) != Some(&base) {
                return Err(crate::storage::shared::conflict());
            }
            data.entries.retain(|e| e.id != base.id);
            let bytes = data.encode()?;
            Ok(((), bytes))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    pub fn entry() -> Entry {
        Entry {
            id: "one".into(),
            scope: super::super::Scope {
                site: Default::default(),
                account: "StellaTest".into(),
                board: crate::board_navigation::BoardName::parse("Stella").unwrap(),
            },
            revision: 0,
            title: "中文標題".into(),
            body: "第一行\n\x1b[1;31m紅色中文\x1b[m\n第三行".into(),
            saved_at: 1,
            conflict: false,
        }
    }
    #[test]
    fn restart_preserves_full_unicode_and_exact_ansi_and_backup() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("drafts.json");
        let store = Store::at(path.clone());
        let first = store.save(None, entry(), "unused".into()).unwrap().entry;
        assert_eq!(
            Store::at(path.clone()).load().unwrap().entries,
            vec![first.clone()]
        );
        let before = std::fs::read(&path).unwrap();
        let mut next = first.clone();
        next.body.push_str("\n離線修改");
        let saved = store.save(Some(first), next, "unused".into()).unwrap();
        assert_eq!(
            Store::at(path.clone()).load().unwrap().entries,
            vec![saved.entry]
        );
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            before
        );
    }
    #[test]
    fn conflict_keeps_both_versions_and_future_saves_use_the_copy() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::at(root.path().join("drafts.json"));
        let base = store.save(None, entry(), "unused".into()).unwrap().entry;
        let mut a = base.clone();
        a.body = "視窗 A".into();
        let mut b = base.clone();
        b.body = "視窗 B\x1b[32m綠色\x1b[m".into();
        store.save(Some(base.clone()), a, "unused".into()).unwrap();
        let copy = store.save(Some(base), b, "copy".into()).unwrap();
        assert!(copy.conflicted);
        let mut changed = copy.entry.clone();
        changed.body.push('中');
        store
            .save(Some(copy.entry), changed, "unused".into())
            .unwrap();
        let all = store.load().unwrap().entries;
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].body, "視窗 A");
        assert_eq!(all[1].body, "視窗 B\x1b[32m綠色\x1b[m中");
    }
    #[test]
    fn corrupt_and_full_store_are_not_overwritten() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("drafts.json");
        let store = Store::at(path.clone());
        std::fs::write(&path, b"broken").unwrap();
        assert!(store.save(None, entry(), "copy".into()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        let all = Drafts {
            version: 1,
            entries: (0..super::super::MAX_DRAFTS)
                .map(|n| Entry {
                    id: format!("id{n}"),
                    ..entry()
                })
                .collect(),
        };
        std::fs::write(&path, all.encode().unwrap()).unwrap();
        assert!(store.save(None, entry(), "copy".into()).is_err());
        assert_eq!(store.load().unwrap(), all);
    }
    #[test]
    fn deleting_stale_source_cannot_remove_another_windows_edit() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::at(root.path().join("drafts.json"));
        let base = store.save(None, entry(), "unused".into()).unwrap().entry;
        let mut next = base.clone();
        next.body = "new".into();
        store
            .save(Some(base.clone()), next, "unused".into())
            .unwrap();
        assert!(store.remove(base).is_err());
        assert_eq!(store.load().unwrap().entries[0].body, "new");
    }
}
