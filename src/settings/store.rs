//! User configuration paths and bounded, backed-up persistence.
use super::{AppSettings, MAX_SETTINGS_BYTES};
use crate::storage;
use std::{
    fs::File,
    io::{ErrorKind, Read},
    path::PathBuf,
};

pub struct Store {
    path: PathBuf,
}
impl Store {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn system() -> Result<Self, String> {
        let dirs =
            directories::ProjectDirs::from("", "", "PTTStella").ok_or("無法找出使用者設定目錄")?;
        Ok(Self::at(dirs.config_dir().join("settings.json")))
    }
    pub fn read(&self) -> Result<Option<Vec<u8>>, String> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("無法讀取設定：{error}")),
        };
        let mut bytes = Vec::new();
        file.take((MAX_SETTINGS_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("無法讀取設定：{e}"))?;
        if bytes.len() > MAX_SETTINGS_BYTES {
            return Err("設定檔超過 8 KiB；原檔保留".into());
        }
        Ok(Some(bytes))
    }
    pub fn load(&self) -> Result<AppSettings, String> {
        match self.read()? {
            None => Ok(AppSettings::default()),
            Some(bytes) => AppSettings::decode(&bytes),
        }
    }
    pub fn save(&self, draft: AppSettings) -> Result<AppSettings, String> {
        let settings = draft.validate().map_err(str::to_owned)?;
        let bytes = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_SETTINGS_BYTES {
            return Err("設定超過 8 KiB".into());
        }
        let previous = self.read()?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("無法建立設定目錄：{e}"))?;
        }
        if let Some(previous) = previous {
            storage::replace(&self.path.with_extension("json.bak"), &previous)?;
        }
        storage::replace(&self.path, &bytes)?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loading_v1_preserves_source_until_explicit_save_and_backs_up_verbatim() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let old = serde_json::to_vec_pretty(&super::super::DisplaySettings::default()).unwrap();
        std::fs::write(&path, &old).unwrap();
        let store = Store::at(path.clone());
        let loaded = store.load().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), old);
        assert!(matches!(
            loaded.display.fonts,
            super::super::Fonts::Custom { .. }
        ));
        store.save(loaded.clone()).unwrap();
        assert_eq!(store.load().unwrap(), loaded);
        assert_eq!(std::fs::read(path.with_extension("json.bak")).unwrap(), old);
    }
    #[test]
    fn saves_reload_and_keep_the_previous_bytes_as_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = Store::at(path.clone());
        assert_eq!(store.load().unwrap(), AppSettings::default());
        store.save(AppSettings::default()).unwrap();
        let previous = std::fs::read(&path).unwrap();
        let mut draft = AppSettings::default();
        draft.display.font_size = 18.0;
        let saved = store.save(draft).unwrap();
        assert_eq!(store.load().unwrap(), saved);
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            previous
        );
    }
    #[test]
    fn corrupt_and_oversized_settings_are_not_silently_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = Store::at(path.clone());
        std::fs::write(&path, b"broken").unwrap();
        assert!(store.load().is_err());
        store.save(AppSettings::default()).unwrap();
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            b"broken"
        );
        std::fs::write(&path, vec![b'X'; MAX_SETTINGS_BYTES + 1]).unwrap();
        assert!(store.load().is_err());
        assert!(store.save(AppSettings::default()).is_err());
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            (MAX_SETTINGS_BYTES + 1) as u64
        );
    }
    #[test]
    fn invalid_or_failed_save_preserves_existing_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = Store::at(path.clone());
        store.save(AppSettings::default()).unwrap();
        let previous = std::fs::read(&path).unwrap();
        assert!(
            store
                .save(AppSettings {
                    display: super::super::DisplayProfile {
                        font_size: 100.0,
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .is_err()
        );
        std::fs::create_dir(path.with_extension("json.bak")).unwrap();
        assert!(
            store
                .save(AppSettings {
                    display: super::super::DisplayProfile {
                        font_size: 18.0,
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), previous);
    }
}
