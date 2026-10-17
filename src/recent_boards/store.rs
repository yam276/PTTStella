//! Independent, versioned local history. Invalid data requires explicit recovery.
use super::{MAX_BYTES, RecentBoards};
use std::path::PathBuf;

#[derive(Clone)]
pub struct Store {
    path: PathBuf,
}
impl Store {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn system() -> Result<Self, String> {
        let dirs = directories::ProjectDirs::from("", "", "PTTStella")
            .ok_or(crate::msg!("recent-directory-missing"))?;
        Ok(Self::at(dirs.data_local_dir().join("recent-boards.json")))
    }
    fn read(&self) -> Result<Option<Vec<u8>>, String> {
        crate::storage::read_bounded(&self.path, MAX_BYTES).map_err(|error| match error {
            crate::storage::ReadError::Io(error) => error.to_string(),
            crate::storage::ReadError::TooLarge => crate::msg!("recent-invalid"),
        })
    }
    pub fn load(&self) -> Result<RecentBoards, String> {
        self.read()?.map_or_else(
            || Ok(RecentBoards::default()),
            |bytes| RecentBoards::decode(&bytes),
        )
    }
    /// An explicit clear may recover corrupt data, but never copies it into a
    /// backup. Both files become empty before the UI claims a successful clear.
    pub fn save(&self, history: &RecentBoards, clear: bool) -> Result<(), String> {
        let bytes = history.encode()?;
        let previous = if clear { None } else { self.read()? };
        if let Some(bytes) = &previous {
            RecentBoards::decode(bytes)?;
        }
        let backup = if clear {
            Some(RecentBoards::default().encode()?)
        } else {
            previous
        };
        crate::storage::replace_backed_up(&self.path, &bytes, backup.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn history(name: &str) -> RecentBoards {
        let mut result = RecentBoards::default();
        result.visit(crate::board_navigation::BoardName::parse(name).unwrap());
        result
    }
    #[test]
    fn restart_preserves_confirmed_order_backup_and_clear_erases_both_copies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recent.json");
        let store = Store::at(path.clone());
        assert!(store.load().unwrap().boards().is_empty());
        store.save(&history("Stella"), false).unwrap();
        let old = std::fs::read(&path).unwrap();
        let mut next = history("Stella");
        next.visit(crate::board_navigation::BoardName::parse("C_Chat").unwrap());
        store.save(&next, false).unwrap();
        assert_eq!(Store::at(path.clone()).load().unwrap(), next);
        assert_eq!(std::fs::read(path.with_extension("json.bak")).unwrap(), old);
        store.save(&RecentBoards::default(), true).unwrap();
        for file in [&path, &path.with_extension("json.bak")] {
            assert!(
                RecentBoards::decode(&std::fs::read(file).unwrap())
                    .unwrap()
                    .boards()
                    .is_empty()
            );
        }
    }
    #[test]
    fn corrupt_or_oversized_history_is_preserved_until_an_explicit_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recent.json");
        let store = Store::at(path.clone());
        for bad in [b"broken".to_vec(), vec![b'X'; MAX_BYTES + 1]] {
            std::fs::write(&path, &bad).unwrap();
            assert!(store.load().is_err());
            assert!(store.save(&history("Stella"), false).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bad);
            assert!(!path.with_extension("json.bak").exists());
            store.save(&RecentBoards::default(), true).unwrap();
            assert!(store.load().unwrap().boards().is_empty());
            assert!(
                Store::at(path.with_extension("json.bak"))
                    .load()
                    .unwrap()
                    .boards()
                    .is_empty()
            );
            std::fs::remove_file(path.with_extension("json.bak")).unwrap();
        }
    }
    #[test]
    fn backup_failure_preserves_current_history_and_failed_clear_is_not_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("recent.json");
        let store = Store::at(path.clone());
        store.save(&history("Stella"), false).unwrap();
        let old = std::fs::read(&path).unwrap();
        std::fs::create_dir(path.with_extension("json.bak")).unwrap();
        assert!(store.save(&history("Other"), false).is_err());
        assert!(store.save(&RecentBoards::default(), true).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), old);
        // A failed final replacement leaves an error even if the backup cleared.
        std::fs::remove_dir(path.with_extension("json.bak")).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(store.save(&RecentBoards::default(), true).is_err());
        assert!(path.is_dir());
        assert!(
            Store::at(path.with_extension("json.bak"))
                .load()
                .unwrap()
                .boards()
                .is_empty()
        );
    }
}
