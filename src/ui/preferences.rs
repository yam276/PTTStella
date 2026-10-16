//! Effective preferences change only after persistence succeeds; previews are drafts.
use super::outbound::{FontPreview, ImportedProfile, Packet, ProfileResult, SettingsOpen};
use super::presentation::{font_sample, sample_packet};
use pttstella::{
    screen::Frame,
    settings::{
        AppSettings, DisplayProfile, DisplaySettings, HoverPosition, PortableProfile, store::Store,
    },
    storage,
};
use std::path::{Path, PathBuf};

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
                Some(format!("{error}；目前使用預設顯示")),
            ),
        };
        let display = app.display.resolve().map_err(str::to_owned)?;
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

    pub(super) fn set_hover(&mut self, position: HoverPosition) -> Result<(), String> {
        let mut draft = self.app.clone();
        draft.hover = position;
        self.app = self.store.as_ref().map_err(Clone::clone)?.save(draft)?;
        Ok(())
    }

    pub(super) fn save_display(&mut self, profile: DisplayProfile) -> Result<(), String> {
        let mut draft = self.app.clone();
        draft.display = profile;
        let saved = self.store.as_ref().map_err(Clone::clone)?.save(draft)?;
        let display = saved.display.resolve().map_err(str::to_owned)?;
        self.app = saved;
        self.display = display;
        self.warning = None;
        Ok(())
    }

    pub(super) fn open_packet(&self) -> Packet<'static> {
        Packet::SettingsOpen(Box::new(SettingsOpen {
            profile: self.app.display.clone(),
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
            Err(error) => FontPreview::Rejected {
                request,
                error: error.into(),
            },
        })
    }

    /// Import only builds a preview packet. Export reads the effective profile.
    pub(super) fn profile_packet(
        &self,
        action: &str,
        name: String,
        probe: Option<&Path>,
    ) -> Packet<'static> {
        Packet::ProfileResult(
            self.profile(action, name, probe)
                .unwrap_or_else(|notice| ProfileResult::Rejected { notice }),
        )
    }

    fn profile(
        &self,
        action: &str,
        name: String,
        probe: Option<&Path>,
    ) -> Result<ProfileResult, String> {
        if !matches!(action, "import" | "export") {
            return Err("未知設定檔操作".into());
        }
        let path = match probe {
            Some(directory) => Some(directory.join("display.pttstella.json")),
            None => Self::pick_profile(action),
        };
        let Some(path) = path else {
            return Ok(ProfileResult::Cancelled);
        };
        if action == "export" {
            if !path.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .to_ascii_lowercase()
                    .ends_with(".pttstella.json")
            }) {
                return Err("請使用 .pttstella.json 副檔名".into());
            }
            let bytes = PortableProfile::new(name, self.app.display.clone())?.encode()?;
            storage::replace(&path, &bytes)?;
            Ok(ProfileResult::Exported {
                notice: "已匯出目前生效設定；未套用的草稿不包含在內".into(),
            })
        } else {
            let bytes = Store::at(path).read()?.ok_or("找不到顯示設定檔")?;
            let profile = PortableProfile::decode(&bytes)?;
            let values = profile.display.resolve().map_err(str::to_owned)?;
            Ok(ProfileResult::Imported {
                profile: Box::new(ImportedProfile {
                    profile: profile.display,
                    name: profile.name,
                    sample: sample_packet(&self.sample, &values),
                    values,
                }),
                notice: "設定已載入預覽；套用並儲存後才會生效".into(),
            })
        }
    }

    fn pick_profile(action: &str) -> Option<PathBuf> {
        let window = dioxus::desktop::window();
        let dialog = rfd::FileDialog::new()
            .set_parent(window.window.as_ref())
            .add_filter("PTTStella 顯示設定", &["json"]);
        if action == "import" {
            dialog.set_title("載入顯示設定至預覽").pick_file()
        } else {
            dialog
                .set_title("匯出目前生效的顯示設定")
                .set_file_name("display.pttstella.json")
                .save_file()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            serde_json::to_value(preferences.profile_packet(
                "export",
                "effective".into(),
                Some(directory.path())
            ))
            .unwrap()["payload"]["status"],
            "exported"
        );
        let exported = std::fs::read(directory.path().join("display.pttstella.json")).unwrap();
        assert_eq!(
            PortableProfile::decode(&exported).unwrap().display,
            original.display
        );
        assert_eq!(
            serde_json::to_value(preferences.profile_packet(
                "import",
                "".into(),
                Some(directory.path())
            ))
            .unwrap()["payload"]["status"],
            "imported"
        );
        assert_eq!(preferences.app(), &original);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::create_dir(path.with_extension("json.bak")).unwrap();
        assert!(preferences.save_display(profile.clone()).is_err());
        assert_eq!(preferences.app(), &original);
        assert_eq!(preferences.display().font_size, original.display.font_size);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_dir(path.with_extension("json.bak")).unwrap();
        preferences.save_display(profile).unwrap();
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
        preferences.save_display(DisplayProfile::default()).unwrap();
        assert!(preferences.warning().is_none());
        assert_eq!(
            std::fs::read(path.with_extension("json.bak")).unwrap(),
            b"broken"
        );
    }
}
