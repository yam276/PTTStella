//! User configuration paths and bounded, backed-up persistence.
use super::{AppSettings, MAX_SETTINGS_BYTES};
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
            .ok_or(crate::msg!("settings-directory-missing"))?;
        Ok(Self::at(dirs.config_dir().join("settings.json")))
    }
    pub fn read(&self) -> Result<Option<Vec<u8>>, String> {
        self.document
            .read(MAX_SETTINGS_BYTES)
            .map_err(|error| match error {
                ReadError::Io(error) => {
                    crate::msg!("settings-read-failed", error = error.to_string())
                }
                ReadError::TooLarge => crate::msg!("settings-too-large"),
            })
    }
    pub fn load(&self) -> Result<AppSettings, String> {
        match self.read()? {
            None => Ok(AppSettings::default()),
            Some(bytes) => AppSettings::decode(&bytes),
        }
    }
    pub fn save(&self, draft: AppSettings) -> Result<AppSettings, String> {
        self.save_from(&self.document.baseline(AppSettings::decode)?, draft)
    }
    pub fn save_from(&self, base: &AppSettings, draft: AppSettings) -> Result<AppSettings, String> {
        let draft = draft.validate()?;
        self.document
            .update(MAX_SETTINGS_BYTES, None, |bytes, unchanged| {
                let current = match bytes {
                    None => AppSettings::default(),
                    Some(bytes) => match AppSettings::decode(bytes) {
                        Ok(value) => value,
                        // Explicit recovery of the exact corrupt file observed at load.
                        Err(_) if unchanged => base.clone(),
                        Err(error) => return Err(error),
                    },
                };
                let mut merged = current.clone();
                merged.display.fonts = shared::field(
                    &base.display.fonts,
                    &draft.display.fonts,
                    &current.display.fonts,
                )?;
                merged.display.font_size = shared::field(
                    &base.display.font_size,
                    &draft.display.font_size,
                    &current.display.font_size,
                )?;
                merged.display.baseline = shared::field(
                    &base.display.baseline,
                    &draft.display.baseline,
                    &current.display.baseline,
                )?;
                merged.language =
                    shared::field(&base.language, &draft.language, &current.language)?;
                merged.article_mode = shared::field(
                    &base.article_mode,
                    &draft.article_mode,
                    &current.article_mode,
                )?;
                merged.article_boundary_navigation = shared::field(
                    &base.article_boundary_navigation,
                    &draft.article_boundary_navigation,
                    &current.article_boundary_navigation,
                )?;
                merged.tools_opacity = shared::field(
                    &base.tools_opacity,
                    &draft.tools_opacity,
                    &current.tools_opacity,
                )?;
                merged.tools_access = shared::field(
                    &base.tools_access,
                    &draft.tools_access,
                    &current.tools_access,
                )?;
                merged.hover = shared::field(&base.hover, &draft.hover, &current.hover)?;
                let merged = merged.validate()?;
                let bytes = serde_json::to_vec_pretty(&merged).map_err(|e| e.to_string())?;
                Ok((merged, bytes))
            })
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
    fn v2_migrates_language_only_on_save_and_unknown_languages_preserve_the_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let store = Store::at(path.clone());
        let old=br#"{"version":2,"display":{"fonts":{"kind":"platform_default"},"font_size":20,"baseline":20},"hover":"off"}"#;
        std::fs::write(&path, old).unwrap();
        let mut draft = store.load().unwrap();
        assert_eq!(draft.version, 6);
        assert_eq!(draft.tools_access, crate::settings::ToolAccess::default());
        assert_eq!(draft.language, crate::i18n::Language::TraditionalChinese);
        assert_eq!(std::fs::read(&path).unwrap(), old);
        draft.language = crate::i18n::Language::English;
        store.save(draft).unwrap();
        assert_eq!(
            store.load().unwrap().language,
            crate::i18n::Language::English
        );
        assert_eq!(std::fs::read(path.with_extension("json.bak")).unwrap(), old);
        let invalid=br#"{"version":3,"display":{"fonts":{"kind":"platform_default"},"font_size":20,"baseline":20},"hover":"off","language":"xx"}"#;
        std::fs::write(&path, invalid).unwrap();
        assert!(store.load().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), invalid);
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
