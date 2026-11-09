//! Candidate stores use the existing formats and locks under a separate identity.
use pttstella::{settings::store::Store as SettingsStore, sites::store::Store as SiteStore};
use std::path::PathBuf;

pub const APPLICATION: &str = "PTTStellaSlintResearch";
pub struct Profile {
    config: PathBuf,
    data: PathBuf,
}
impl Profile {
    pub fn system() -> Result<Self, String> {
        let dirs = directories::ProjectDirs::from("", "", APPLICATION)
            .ok_or_else(|| pttstella::msg!("settings-directory-missing"))?;
        Ok(Self {
            config: dirs.config_dir().to_owned(),
            data: dirs.data_local_dir().to_owned(),
        })
    }
    /// Report paths without reading stores or opening an OS credential provider.
    pub fn describe(&self) -> Result<serde_json::Value, String> {
        let path = |path: PathBuf| serde_json::to_value(path).map_err(|e| e.to_string());
        Ok(serde_json::json!({
            "application": APPLICATION,
            "config_directory": path(self.config.clone())?,
            "settings": path(self.config.join("settings.json"))?,
            "recent_boards":path(self.data.join("recent-boards.json"))?,
            "bookmarks": path(self.data.join("bookmarks.json"))?,
            "sites": path(self.config.join("sites.json"))?,
            "filters": path(self.config.join("filters.json"))?,
            "credential_scope": "SlintResearch",
            "probe_credential_scope": "SlintSyntheticProbe",
            "stores_read": false,
            "vault_opened": false,
        }))
    }
    pub fn settings(&self) -> SettingsStore {
        SettingsStore::at(self.config.join("settings.json"))
    }
    pub fn load_preferences(
        &self,
    ) -> Result<pttstella::settings::preferences::Preferences, String> {
        pttstella::settings::preferences::Preferences::load(Ok(self.settings()))
    }
    pub fn sites(&self) -> SiteStore {
        SiteStore::at(self.config.join("sites.json"))
    }
    pub fn drafts(&self) -> pttstella::drafts::store::Store {
        pttstella::drafts::store::Store::at(self.config.join("drafts.json"))
    }
    pub fn recent(&self) -> pttstella::recent_boards::store::Store {
        pttstella::recent_boards::store::Store::at(self.data.join("recent-boards.json"))
    }
    pub fn bookmarks(&self) -> pttstella::bookmarks::store::Store {
        pttstella::bookmarks::store::Store::at(self.data.join("bookmarks.json"))
    }
    pub fn filters(&self) -> pttstella::filters::store::Store {
        pttstella::filters::store::Store::at(self.config.join("filters.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::{
        settings::{AppSettings, ToolOpacity},
        sites::Sites,
    };

    #[test]
    fn research_namespace_is_distinct_from_the_product_without_opening_either() {
        let product = directories::ProjectDirs::from("", "", "PTTStella").unwrap();
        let research = Profile::system().unwrap();
        assert_ne!(research.config, product.config_dir());
        assert!(!research.config.starts_with(product.config_dir()));
        assert!(!product.config_dir().starts_with(&research.config));
        assert_ne!(research.data, product.data_local_dir());
    }

    #[test]
    fn candidate_defaults_and_edits_do_not_read_or_modify_product_copies() {
        let temp = tempfile::tempdir().unwrap();
        let product = temp.path().join("product-copy");
        std::fs::create_dir(&product).unwrap();
        let settings = AppSettings {
            tools_opacity: ToolOpacity::try_from(23).unwrap(),
            ..Default::default()
        };
        let original = serde_json::to_vec_pretty(&settings).unwrap();
        std::fs::write(product.join("settings.json"), &original).unwrap();
        // Use the existing schema rather than depending on an external profile.
        let choices = Sites {
            default: pttstella::sites::Target::new(
                "wss://example.org/bbs".into(),
                pttstella::encoding::TextEncoding::Gbk,
            )
            .unwrap(),
            ..Default::default()
        };
        let sites = choices.encode().unwrap();
        std::fs::write(product.join("sites.json"), &sites).unwrap();
        let filters = pttstella::filters::Rules::default()
            .edit("Alice", "product-copy", false)
            .unwrap();
        let filter_bytes = serde_json::to_vec(&filters).unwrap();
        std::fs::write(product.join("filters.json"), &filter_bytes).unwrap();
        let candidate = Profile {
            config: temp.path().join("candidate"),
            data: temp.path().join("candidate-data"),
        };
        assert_eq!(candidate.settings().load().unwrap(), AppSettings::default());
        assert_eq!(candidate.sites().load().unwrap(), Sites::default());
        assert_eq!(
            candidate.filters().load().unwrap(),
            pttstella::filters::Rules::default()
        );
        let preferences = candidate.load_preferences().unwrap();
        let loaded = preferences.app().clone();
        let warning = preferences.warning();
        let stations = crate::stations::Stations::load(&candidate);
        assert_eq!(loaded, AppSettings::default());
        assert_eq!(stations.entries[0].1, pttstella::sites::Target::default());
        assert!(warning.is_none());
        assert!(stations.warnings.is_empty());
        candidate.settings().save(AppSettings::default()).unwrap();
        candidate.sites().save(Sites::default()).unwrap();
        candidate
            .filters()
            .save(pttstella::filters::Rules::default())
            .unwrap();
        assert_eq!(
            std::fs::read(product.join("settings.json")).unwrap(),
            original
        );
        assert_eq!(std::fs::read(product.join("sites.json")).unwrap(), sites);
        assert_eq!(
            std::fs::read(product.join("filters.json")).unwrap(),
            filter_bytes
        );
        assert!(!product.join("settings.json.bak").exists());
        assert!(!product.join("sites.json.bak").exists());
    }

    #[test]
    fn explicit_old_data_copy_uses_common_decode_and_backups_without_touching_source() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("old-settings-copy.json");
        // v1 is the existing DisplaySettings representation.
        let old = serde_json::to_vec(&pttstella::settings::DisplaySettings::default()).unwrap();
        std::fs::write(&source, &old).unwrap();
        let candidate = Profile {
            config: temp.path().join("candidate"),
            data: temp.path().join("candidate-data"),
        };
        std::fs::create_dir(&candidate.config).unwrap();
        std::fs::copy(&source, candidate.config.join("settings.json")).unwrap();
        let loaded = candidate.settings().load().unwrap();
        assert_eq!(
            std::fs::read(candidate.config.join("settings.json")).unwrap(),
            old
        );
        let mut draft = loaded.clone();
        draft.tools_opacity = ToolOpacity::try_from(45).unwrap();
        let saved = candidate.settings().save_from(&loaded, draft).unwrap();
        assert_eq!(candidate.settings().load().unwrap(), saved);
        assert_eq!(
            std::fs::read(candidate.config.join("settings.json.bak")).unwrap(),
            old
        );
        assert_eq!(std::fs::read(&source).unwrap(), old);
        std::fs::write(candidate.config.join("settings.json"), b"broken").unwrap();
        assert!(candidate.settings().load().is_err());
        let preferences = candidate.load_preferences().unwrap();
        let loaded = preferences.app().clone();
        let warning = preferences.warning();
        assert_eq!(loaded, AppSettings::default());
        assert!(warning.is_some());
        assert_eq!(
            std::fs::read(candidate.config.join("settings.json")).unwrap(),
            b"broken"
        );
        assert_eq!(std::fs::read(&source).unwrap(), old);
        // The same loaded store must retain its observation of the corrupt
        // bytes. A new Store here would reject this explicit recovery.
        let mut recovered = loaded;
        recovered.hover = pttstella::settings::HoverPosition::Off;
        let saved = preferences
            .store()
            .unwrap()
            .save_from(preferences.edit_base(), recovered)
            .unwrap();
        assert_eq!(saved.hover, pttstella::settings::HoverPosition::Off);
        assert_eq!(
            std::fs::read(candidate.config.join("settings.json.bak")).unwrap(),
            b"broken"
        );
    }
}
