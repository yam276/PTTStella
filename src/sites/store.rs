//! Local station choices, using the shared bounded read and atomic backup writer.
use super::{MAX_BYTES, Sites};
use crate::storage::{self, ReadError};
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
            .ok_or_else(|| crate::msg!("settings-directory-missing"))?;
        Ok(Self::at(dirs.config_dir().join("sites.json")))
    }
    fn read(&self) -> Result<Option<Vec<u8>>, String> {
        storage::read_bounded(&self.path, MAX_BYTES).map_err(|e| match e {
            ReadError::TooLarge => crate::msg!("sites-invalid"),
            ReadError::Io(e) => crate::msg!("sites-read-failed", error = e.to_string()),
        })
    }
    pub fn load(&self) -> Result<Sites, String> {
        self.read()?
            .map_or_else(|| Ok(Sites::default()), |bytes| Sites::decode(&bytes))
    }
    pub fn save(&self, sites: Sites) -> Result<Sites, String> {
        let bytes = sites.encode()?;
        let previous = self.read()?;
        if let Some(bytes) = &previous {
            Sites::decode(bytes)?;
        }
        storage::replace_backed_up(&self.path, &bytes, previous.as_deref())?;
        Ok(sites)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encoding::TextEncoding, sites::Target};
    #[test]
    fn favourites_and_default_survive_restart_with_exact_backup_and_failed_writes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sites.json");
        let store = Store::at(path.clone());
        let mut sites = store.load().unwrap();
        store.save(sites.clone()).unwrap();
        let previous = std::fs::read(&path).unwrap();
        let target = Target::new("telnet://example.com:2323".into(), TextEncoding::Gbk).unwrap();
        sites
            .save_favourite("合成站點".into(), target.clone())
            .unwrap();
        sites.default = target;
        store.save(sites.clone()).unwrap();
        assert_eq!(Store::at(path.clone()).load().unwrap(), sites);
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            previous
        );
        let saved = std::fs::read(&path).unwrap();
        std::fs::remove_file(path.with_extension("json.bak")).unwrap();
        std::fs::create_dir(path.with_extension("json.bak")).unwrap();
        sites.favourites.clear();
        assert!(store.save(sites).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), saved);
    }
    #[test]
    fn corrupt_source_is_retained_when_an_edit_tries_to_save() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sites.json");
        std::fs::write(&path, b"broken").unwrap();
        let store = Store::at(path.clone());
        assert!(store.load().is_err());
        assert!(store.save(Sites::default()).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"broken");
    }
}
