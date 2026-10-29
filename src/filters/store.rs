//! Private local rules, merged under the shared document transaction lock.
use super::{MAX_BYTES, Rules};
use crate::storage::{
    ReadError,
    shared::{self, Document},
};
use std::path::PathBuf;

#[derive(Clone)]
pub struct Store {
    document: Document,
}
impl Store {
    pub fn at(path: PathBuf) -> Self {
        Self {
            document: Document::new(path),
        }
    }
    pub fn system() -> Result<Self, String> {
        let dirs = directories::ProjectDirs::from("", "", "PTTStella")
            .ok_or(crate::msg!("filters-directory-missing"))?;
        Ok(Self::at(dirs.config_dir().join("filters.json")))
    }
    fn read(&self) -> Result<Option<Vec<u8>>, String> {
        self.document.read(MAX_BYTES).map_err(|error| match error {
            ReadError::Io(e) => crate::msg!("filters-read-failed", e = e.to_string()),
            ReadError::TooLarge => crate::msg!("filters-too-large"),
        })
    }
    pub fn load(&self) -> Result<Rules, String> {
        self.read()?
            .map_or_else(|| Ok(Rules::default()), |b| Rules::decode(&b))
    }
    pub fn save(&self, rules: Rules) -> Result<Rules, String> {
        self.save_from(&self.document.baseline(Rules::decode)?, rules)
    }
    pub fn save_from(&self, base: &Rules, rules: Rules) -> Result<Rules, String> {
        rules.validate()?;
        self.document.update(MAX_BYTES, None, |bytes, unchanged| {
            let current = match bytes {
                None => Rules::default(),
                Some(bytes) => match Rules::decode(bytes) {
                    Ok(value) => value,
                    Err(_) if unchanged => base.clone(),
                    Err(error) => return Err(error),
                },
            };
            let merged = Rules {
                version: 4,
                allow_color: shared::field(
                    &base.allow_color,
                    &rules.allow_color,
                    &current.allow_color,
                )?,
                entries: shared::entries(
                    &base.entries,
                    &rules.entries,
                    &current.entries,
                    |a, b| a.id.eq_ignore_ascii_case(&b.id),
                )?,
            };
            merged.validate()?;
            let bytes = serde_json::to_vec_pretty(&merged).map_err(|e| e.to_string())?;
            Ok((merged, bytes))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_reload_with_backup_and_failures_preserve_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("filters.json");
        let store = Store::at(path.clone());
        let rules = store
            .load()
            .unwrap()
            .edit("Alice", "自製測試", false)
            .unwrap();
        store.save(rules.clone()).unwrap();
        let old = std::fs::read(&path).unwrap();
        let next = rules.edit("alice", "改註", false).unwrap();
        store.save(next.clone()).unwrap();
        assert_eq!(store.load().unwrap(), next);
        assert_eq!(std::fs::read(path.with_extension("json.bak")).unwrap(), old);
        let current = std::fs::read(&path).unwrap();
        std::fs::remove_file(path.with_extension("json.bak")).unwrap();
        std::fs::create_dir(path.with_extension("json.bak")).unwrap();
        assert!(store.save(Rules::default()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), current);
    }
    #[test]
    fn invalid_and_oversized_files_are_never_silently_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("filters.json");
        let store = Store::at(path.clone());
        std::fs::write(&path, b"broken").unwrap();
        assert!(store.load().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        store.save(Rules::default()).unwrap();
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            b"broken"
        );
        std::fs::write(&path, vec![b'x'; MAX_BYTES + 1]).unwrap();
        assert!(store.load().is_err());
        assert!(store.save(Rules::default()).is_err());
        assert_eq!(
            std::fs::metadata(path).unwrap().len(),
            (MAX_BYTES + 1) as u64
        );
    }
}
