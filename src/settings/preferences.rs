//! Effective settings and editor baselines shared by native and web adapters.
//! The frontend's existing bounded file owner performs I/O. Only its successful
//! result may commit effective values; previews never call `commit`.
use super::{AppSettings, DisplaySettings, HoverPosition, store::Store};

pub struct Preferences {
    store: Result<Store, String>,
    app: AppSettings,
    edit_base: AppSettings,
    display: DisplaySettings,
    warning: Option<String>,
}
impl Preferences {
    pub fn load(store: Result<Store, String>) -> Result<Self, String> {
        let (app, warning) = match store.as_ref().map_err(Clone::clone).and_then(Store::load) {
            Ok(app) => (app, None),
            Err(error) => (
                AppSettings::default(),
                Some(crate::msg!("settings-load-default", error = error)),
            ),
        };
        Self::loaded(store, app, warning)
    }
    /// The application startup already read this store. Do not read it again on
    /// the UI thread when another consumer attaches to the same settings owner.
    pub fn loaded(
        store: Result<Store, String>,
        app: AppSettings,
        warning: Option<String>,
    ) -> Result<Self, String> {
        let app = app.validate()?;
        let display = app.display.resolve()?;
        Ok(Self {
            store,
            edit_base: app.clone(),
            app,
            display,
            warning,
        })
    }
    pub fn app(&self) -> &AppSettings {
        &self.app
    }
    pub fn display(&self) -> &DisplaySettings {
        &self.display
    }
    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }
    pub fn store(&self) -> Result<Store, String> {
        self.store.clone()
    }
    pub fn edit_base(&self) -> &AppSettings {
        &self.edit_base
    }
    pub fn open(&mut self) {
        self.edit_base = self.app.clone();
    }
    /// Keep the fields the editor actually submitted, rather than a merged disk
    /// result containing external changes that the still-open editor never saw.
    pub fn submitted(&mut self, draft: AppSettings) {
        self.edit_base = draft;
    }
    pub fn hover_draft(&self, hover: HoverPosition) -> AppSettings {
        AppSettings {
            hover,
            ..self.app.clone()
        }
    }
    pub fn commit(&mut self, saved: AppSettings) -> Result<(), String> {
        let saved = saved.validate()?;
        let display = saved.display.resolve()?;
        self.app = saved;
        self.display = display;
        self.warning = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn open_editor_keeps_submitted_baseline_while_external_fields_merge_and_undo() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::at(dir.path().join("settings.json"));
        let mut preferences = Preferences::load(Ok(store.clone())).unwrap();
        let original = preferences.app().clone();
        let mut external = original.clone();
        external.language = crate::i18n::Language::English;
        store.save_from(&original, external).unwrap();
        for size in [18., 19., original.display.font_size] {
            let mut submitted = original.clone();
            submitted.display.font_size = size;
            let saved = store
                .save_from(preferences.edit_base(), submitted.clone())
                .unwrap();
            preferences.commit(saved).unwrap();
            preferences.submitted(submitted);
            assert_eq!(preferences.app().language, crate::i18n::Language::English);
            assert_eq!(store.load().unwrap(), *preferences.app());
            assert_eq!(preferences.edit_base().language, original.language);
            assert_eq!(preferences.display().font_size, size);
        }
        preferences.open();
        assert_eq!(preferences.edit_base(), preferences.app());
    }
    #[test]
    fn failed_load_and_failed_write_preserve_warning_effective_values_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, b"broken").unwrap();
        let store = Store::at(path.clone());
        let mut preferences = Preferences::load(Ok(store.clone())).unwrap();
        let original = preferences.app().clone();
        assert!(preferences.warning().is_some());
        let draft = preferences.hover_draft(HoverPosition::Off);
        std::fs::create_dir(path.with_extension("json.bak")).unwrap();
        assert!(
            store
                .save_from(preferences.edit_base(), draft.clone())
                .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        assert_eq!(preferences.app(), &original);
        assert!(preferences.warning().is_some());
        std::fs::remove_dir(path.with_extension("json.bak")).unwrap();
        preferences
            .commit(store.save_from(preferences.edit_base(), draft).unwrap())
            .unwrap();
        assert_eq!(preferences.app().hover, HoverPosition::Off);
        assert!(preferences.warning().is_none());
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            b"broken"
        );
        let mut invalid = preferences.app().clone();
        invalid.display.font_width = 0.;
        assert!(preferences.commit(invalid).is_err());
        assert_eq!(store.load().unwrap(), *preferences.app());
    }
}
