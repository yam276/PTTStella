//! Bounded local article references; no page contents or account data.
use super::{Bookmarks, MAX_BYTES};
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
            .ok_or(crate::msg!("bookmarks-directory-missing"))?;
        Ok(Self::at(dirs.data_local_dir().join("bookmarks.json")))
    }
    fn read(&self) -> Result<Option<Vec<u8>>, String> {
        self.document.read(MAX_BYTES).map_err(|error| match error {
            ReadError::Io(error) => crate::msg!("bookmarks-read-failed", error = error.to_string()),
            ReadError::TooLarge => crate::msg!("bookmarks-invalid"),
        })
    }
    pub fn load(&self) -> Result<Bookmarks, String> {
        self.read()?.map_or_else(
            || Ok(Bookmarks::default()),
            |bytes| Bookmarks::decode(&bytes),
        )
    }
    /// A bad existing document is never replaced by an empty fallback or edit.
    pub fn save(&self, bookmarks: Bookmarks) -> Result<Bookmarks, String> {
        self.save_from(&self.document.baseline(Bookmarks::decode)?, bookmarks)
    }
    pub fn save_from(&self, base: &Bookmarks, draft: Bookmarks) -> Result<Bookmarks, String> {
        draft.encode()?;
        self.document.update(MAX_BYTES, None, |bytes, _| {
            let current = bytes.map_or_else(|| Ok(Bookmarks::default()), Bookmarks::decode)?;
            let merged = Bookmarks {
                entries: shared::entries(
                    &base.entries,
                    &draft.entries,
                    &current.entries,
                    |a, b| a.matches(&b.target),
                )?,
            };
            let bytes = merged.encode()?;
            Ok((merged, bytes))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::article_target::ArticleTarget;
    fn one() -> Bookmarks {
        Bookmarks::default()
            .upsert(
                ArticleTarget::new("Stella", "00000101").unwrap(),
                "自製合成書籤",
                100,
            )
            .unwrap()
    }
    #[test]
    fn restart_and_deletion_keep_an_exact_backup_of_the_previous_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/bookmarks.json");
        let store = Store::at(path.clone());
        assert!(store.load().unwrap().entries().is_empty());
        store.save(one()).unwrap();
        let previous = std::fs::read(&path).unwrap();
        let second = one()
            .upsert(ArticleTarget::new("Other", "1AbCdE-_").unwrap(), "", 200)
            .unwrap();
        store.save(second.clone()).unwrap();
        assert_eq!(Store::at(path.clone()).load().unwrap(), second);
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            previous
        );
        let before_delete = std::fs::read(&path).unwrap();
        let remaining = second
            .remove(&ArticleTarget::new("STELLA", "00000101").unwrap())
            .unwrap();
        store.save(remaining.clone()).unwrap();
        assert_eq!(store.load().unwrap(), remaining);
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            before_delete
        );
        assert_eq!(remaining.entries()[0].target().board().as_str(), "Other");
    }
    #[test]
    fn invalid_and_oversized_documents_preserve_both_copies_on_every_edit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bookmarks.json");
        let backup = path.with_extension("json.bak");
        let store = Store::at(path.clone());
        std::fs::write(&backup, b"prior backup").unwrap();
        for bytes in [
            b"broken".to_vec(),
            vec![b'x'; MAX_BYTES + 1],
            br#"{"version":2,"entries":[]}"#.to_vec(),
        ] {
            std::fs::write(&path, &bytes).unwrap();
            assert!(store.load().is_err());
            assert!(store.save(one()).is_err());
            assert!(store.save(Bookmarks::default()).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert_eq!(std::fs::read(&backup).unwrap(), b"prior backup");
        }
    }
    #[test]
    fn failed_backup_or_main_replacement_is_not_a_successful_edit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bookmarks.json");
        let backup = path.with_extension("json.bak");
        let store = Store::at(path.clone());
        store.save(one()).unwrap();
        let previous = std::fs::read(&path).unwrap();
        std::fs::create_dir(&backup).unwrap();
        assert!(store.save(Bookmarks::default()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), previous);
        let mut names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "bookmarks.json",
                "bookmarks.json.bak",
                "bookmarks.json.lock"
            ]
        );
        assert_eq!(
            std::fs::metadata(path.with_extension("json.lock"))
                .unwrap()
                .len(),
            0
        );
        std::fs::remove_dir(&backup).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(store.load().is_err());
        assert!(store.save(one()).is_err());
        assert!(path.is_dir());
        assert!(!backup.exists());
    }
}
