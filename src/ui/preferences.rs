//! Effective preferences change only after persistence succeeds; previews are drafts.
use super::outbound::{FontPreview, Packet, SettingsOpen};
use super::presentation::{font_sample, sample_packet};
use pttstella::{
    screen::Frame,
    settings::{AppSettings, DisplayProfile, DisplaySettings, HoverPosition, store::Store},
};
use std::path::Path;

/// Fields editable by the settings window. Session controls and secrets are absent.
#[derive(serde::Deserialize)]
pub(super) struct Change {
    pub values: DisplayProfile,
    #[serde(default)]
    pub language: pttstella::i18n::Language,
    pub tools_opacity: pttstella::settings::ToolOpacity,
    pub tools_access: pttstella::settings::ToolAccess,
    #[serde(default)]
    pub hover: Option<HoverPosition>,
}
impl Change {
    pub(super) fn apply(self, mut app: AppSettings) -> Result<AppSettings, String> {
        app.display = self.values.validate()?;
        app.language = self.language;
        app.tools_opacity = self.tools_opacity;
        app.tools_access = self.tools_access;
        if let Some(hover) = self.hover {
            app.hover = hover;
        }
        Ok(app)
    }
}

pub(super) struct Preferences {
    store: Result<Store, String>,
    app: AppSettings,
    display: DisplaySettings,
    warning: Option<String>,
    sample: Frame,
}

impl Preferences {
    pub(super) fn load(probe: Option<&Path>) -> Result<Self, String> {
        let store = probe.map_or_else(Store::system, |path| {
            Ok(Store::at(path.join("probe-settings.json")))
        });
        Self::from_store(store)
    }

    fn from_store(store: Result<Store, String>) -> Result<Self, String> {
        let (app, warning) = match store.as_ref().map_err(Clone::clone).and_then(Store::load) {
            Ok(settings) => (settings, None),
            Err(error) => (
                AppSettings::default(),
                Some(pttstella::msg!(
                    "settings-load-default",
                    error = error.to_string()
                )),
            ),
        };
        let display = app.display.resolve().map_err(|error| error.to_string())?;
        Ok(Self {
            store,
            app,
            display,
            warning,
            sample: font_sample()?,
        })
    }

    pub(super) fn app(&self) -> &AppSettings {
        &self.app
    }
    pub(super) fn display(&self) -> &DisplaySettings {
        &self.display
    }
    pub(super) fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }
    pub(super) fn hover(&self) -> HoverPosition {
        self.app.hover
    }

    pub(super) fn hover_draft(&self, position: HoverPosition) -> AppSettings {
        let mut draft = self.app.clone();
        draft.hover = position;
        draft
    }

    pub(super) fn store(&self) -> Result<Store, String> {
        self.store.clone()
    }

    /// Only a successful owned disk operation may commit an effective draft.
    pub(super) fn commit(&mut self, saved: AppSettings) -> Result<(), String> {
        let display = saved.display.resolve().map_err(|error| error.to_string())?;
        self.app = saved;
        self.display = display;
        self.warning = None;
        Ok(())
    }

    pub(super) fn open_packet(&self) -> Packet<'static> {
        Packet::SettingsOpen(Box::new(SettingsOpen {
            hover: self.app.hover,
            profile: self.app.display.clone(),
            tools_opacity: self.app.tools_opacity,
            tools_access: self.app.tools_access,
            language: self.app.language,
            values: self.display.clone(),
            defaults: DisplaySettings::default(),
            presets: DisplaySettings::presets(),
            warning: self.warning.clone(),
            sample: sample_packet(&self.sample, &self.display),
        }))
    }

    pub(super) fn preview_packet(&self, request: u64, profile: DisplayProfile) -> Packet<'static> {
        Packet::FontPreview(match profile.resolve() {
            Ok(display) => FontPreview::Ready {
                request,
                packet: Box::new(sample_packet(&self.sample, &display)),
            },
            Err(error) => FontPreview::Rejected { request, error },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::settings::PortableProfile;
    fn save(preferences: &mut Preferences, profile: DisplayProfile) -> Result<(), String> {
        let mut draft = preferences.app().clone();
        draft.display = profile;
        let saved = preferences.store()?.save(draft)?;
        preferences.commit(saved)
    }

    #[test]
    fn previews_imports_and_failed_saves_preserve_the_effective_preferences() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let store = Store::at(path.clone());
        store.save(AppSettings::default()).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let mut preferences = Preferences::from_store(Ok(store)).unwrap();
        let original = preferences.app().clone();
        let profile = DisplayProfile {
            font_size: 18.0,
            baseline: 19.0,
            ..Default::default()
        };
        let packet = serde_json::to_value(preferences.preview_packet(7, profile.clone())).unwrap();
        assert_eq!(packet["payload"]["packet"]["settings"]["font_size"], 18.0);
        assert_eq!(preferences.app(), &original);
        let path_profile = directory.path().join("display.pttstella.json");
        let request = super::super::files::ProfileRequest::Export {
            name: "effective".into(),
            display: preferences.app().display.clone(),
        };
        let exported =
            super::super::files::profile_at(request, Some(path_profile.clone())).unwrap();
        assert!(matches!(
            exported,
            super::super::outbound::ProfileResult::Exported { .. }
        ));
        let bytes_profile = std::fs::read(&path_profile).unwrap();
        assert_eq!(
            PortableProfile::decode(&bytes_profile).unwrap().display,
            original.display
        );
        let imported = super::super::files::profile_at(
            super::super::files::ProfileRequest::Import,
            Some(path_profile),
        )
        .unwrap();
        assert!(matches!(
            imported,
            super::super::outbound::ProfileResult::Imported { .. }
        ));
        assert_eq!(preferences.app(), &original);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::create_dir(path.with_extension("json.bak")).unwrap();
        assert!(save(&mut preferences, profile.clone()).is_err());
        assert_eq!(preferences.app(), &original);
        assert_eq!(preferences.display().font_size, original.display.font_size);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_dir(path.with_extension("json.bak")).unwrap();
        save(&mut preferences, profile).unwrap();
        assert_eq!(preferences.display().font_size, 18.0);
        assert_eq!(Store::at(path).load().unwrap(), *preferences.app());
    }

    #[test]
    fn failed_load_keeps_warning_and_source_until_explicit_successful_save() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        std::fs::write(&path, b"broken").unwrap();
        let mut preferences = Preferences::from_store(Ok(Store::at(path.clone()))).unwrap();
        assert!(preferences.warning().is_some());
        preferences.preview_packet(1, DisplayProfile::default());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
        assert!(preferences.warning().is_some());
        save(&mut preferences, DisplayProfile::default()).unwrap();
        assert!(preferences.warning().is_none());
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            b"broken"
        );
    }
}
