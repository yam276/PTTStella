//! Candidate stores use the existing formats and locks under a separate identity.
use pttstella::{settings::store::Store as SettingsStore, sites::store::Store as SiteStore};
use std::path::PathBuf;

pub const APPLICATION: &str = "PTTStellaSlintResearch";
pub struct Profile {
    config: PathBuf,
    data: PathBuf,
    prefix: &'static str,
}
impl Profile {
    pub fn system() -> Result<Self, String> {
        let dirs = directories::ProjectDirs::from("", "", APPLICATION)
            .ok_or_else(|| pttstella::msg!("settings-directory-missing"))?;
        Ok(Self {
            config: dirs.config_dir().to_owned(),
            data: dirs.data_local_dir().to_owned(),
            prefix: "",
        })
    }
    /// Explicit, marked copies only. Match the original Dioxus probe filenames
    /// so both real frontends can reopen the same files without relocating data.
    pub fn fixture(root: PathBuf) -> Result<Self, String> {
        use std::io::Read;
        let root_metadata = std::fs::symlink_metadata(&root).map_err(|e| e.to_string())?;
        if !root_metadata.is_dir() {
            return Err("store fixture root must be a regular directory".into());
        }
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        let marker = root.join(".store-compatibility.json");
        let marker_metadata = std::fs::symlink_metadata(&marker).map_err(|e| e.to_string())?;
        if !marker_metadata.is_file() {
            return Err("store fixture marker must be a regular file".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(marker)
            .map_err(|e| e.to_string())?
            .take(1025)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > 1024
            || serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|e| e.to_string())?
                != serde_json::json!({"format":"PTTStella.StoreCompatibility","version":1,"synthetic":true})
        {
            return Err("explicit synthetic store fixture marker required".into());
        }
        for name in [
            "settings",
            "sites",
            "filters",
            "bookmarks",
            "recent-boards",
            "drafts",
        ] {
            for suffix in [".json", ".json.bak", ".json.lock"] {
                let path = root.join(format!("probe-{name}{suffix}"));
                match std::fs::symlink_metadata(path) {
                    Ok(metadata) if !metadata.is_file() => {
                        return Err("store fixture files must be regular files".into());
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        Ok(Self {
            config: root.clone(),
            data: root,
            prefix: "probe-",
        })
    }
    fn config_file(&self, name: &str) -> PathBuf {
        self.config.join(format!("{}{name}.json", self.prefix))
    }
    fn data_file(&self, name: &str) -> PathBuf {
        self.data.join(format!("{}{name}.json", self.prefix))
    }
    /// Report paths without reading stores or opening an OS credential provider.
    pub fn describe(&self) -> Result<serde_json::Value, String> {
        let path = |path: PathBuf| serde_json::to_value(path).map_err(|e| e.to_string());
        Ok(serde_json::json!({
            "application": if self.prefix.is_empty() { APPLICATION } else { "SlintStoreCompatibilityProbe" },
            "config_directory": path(self.config.clone())?,
            "settings": path(self.config_file("settings"))?,
            "recent_boards":path(self.data_file("recent-boards"))?,
            "bookmarks": path(self.data_file("bookmarks"))?,
            "sites": path(self.config_file("sites"))?,
            "filters": path(self.config_file("filters"))?,
            "credential_scope": if self.prefix.is_empty() { "SlintResearch" } else { "SlintSyntheticProbe" },
            "probe_credential_scope": "SlintSyntheticProbe",
            "stores_read": false,
            "vault_opened": false,
        }))
    }
    pub fn settings(&self) -> SettingsStore {
        SettingsStore::at(self.config_file("settings"))
    }
    pub fn load_preferences(
        &self,
    ) -> Result<pttstella::settings::preferences::Preferences, String> {
        pttstella::settings::preferences::Preferences::load(Ok(self.settings()))
    }
    pub fn sites(&self) -> SiteStore {
        SiteStore::at(self.config_file("sites"))
    }
    pub fn drafts(&self) -> pttstella::drafts::store::Store {
        pttstella::drafts::store::Store::at(self.config_file("drafts"))
    }
    pub fn recent(&self) -> pttstella::recent_boards::store::Store {
        pttstella::recent_boards::store::Store::at(self.data_file("recent-boards"))
    }
    pub fn bookmarks(&self) -> pttstella::bookmarks::store::Store {
        pttstella::bookmarks::store::Store::at(self.data_file("bookmarks"))
    }
    pub fn filters(&self) -> pttstella::filters::store::Store {
        pttstella::filters::store::Store::at(self.config_file("filters"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::{
        settings::{AppSettings, ToolOpacity},
        sites::Sites,
    };

    fn mark(root: &std::path::Path) {
        std::fs::write(
            root.join(".store-compatibility.json"),
            br#"{"format":"PTTStella.StoreCompatibility","version":1,"synthetic":true}"#,
        )
        .unwrap();
    }

    #[test]
    fn fixture_requires_a_bounded_exact_marker_without_creating_stores() {
        let root = tempfile::tempdir().unwrap();
        assert!(Profile::fixture(root.path().to_owned()).is_err());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        for bytes in [
            b"broken".to_vec(),
            vec![b' '; 1025],
            br#"{"format":"PTTStella.StoreCompatibility","version":1,"synthetic":false}"#.to_vec(),
            br#"{"format":"PTTStella.StoreCompatibility","version":1,"synthetic":true,"extra":1}"#
                .to_vec(),
        ] {
            std::fs::write(root.path().join(".store-compatibility.json"), &bytes).unwrap();
            assert!(Profile::fixture(root.path().to_owned()).is_err());
            assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
        }
    }

    #[test]
    fn marked_fixture_reopens_legacy_settings_without_reseeding_or_backup() {
        let root = tempfile::tempdir().unwrap();
        mark(root.path());
        let path = root.path().join("probe-settings.json");
        let old = br#"{"version":1,"ascii_font":"Courier New","cjk_font":"PingFang TC","font_size":20,"baseline":19,"font_width":100}"#;
        std::fs::write(&path, old).unwrap();
        let fixture = Profile::fixture(root.path().to_owned()).unwrap();
        let preferences = fixture.load_preferences().unwrap();
        assert_eq!(preferences.app().display.font_size, 20.0);
        assert!(preferences.warning().is_none());
        assert_eq!(std::fs::read(&path).unwrap(), old);
        assert!(!path.with_extension("json.bak").exists());
        let description = fixture.describe().unwrap();
        assert_eq!(description["application"], "SlintStoreCompatibilityProbe");
        assert_eq!(description["credential_scope"], "SlintSyntheticProbe");
        assert_eq!(
            description["settings"],
            serde_json::to_value(path.canonicalize().unwrap()).unwrap()
        );
        assert!(!root.path().join("settings.json").exists());
    }

    #[test]
    fn unavailable_schema_keeps_original_bytes_and_warning_on_fixture_startup() {
        let root = tempfile::tempdir().unwrap();
        mark(root.path());
        let path = root.path().join("probe-settings.json");
        for bytes in [b"broken".as_slice(), br#"{"version":999}"#.as_slice()] {
            std::fs::write(&path, bytes).unwrap();
            let fixture = Profile::fixture(root.path().to_owned()).unwrap();
            let preferences = fixture.load_preferences().unwrap();
            assert!(preferences.warning().is_some());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert!(!root.path().join("probe-settings.json.bak").exists());
        }
    }

    #[test]
    fn fixture_rejects_nonregular_store_and_backup_before_a_window_opens() {
        let root = tempfile::tempdir().unwrap();
        mark(root.path());
        for name in [
            "probe-settings.json",
            "probe-drafts.json.bak",
            "probe-bookmarks.json.lock",
        ] {
            let path = root.path().join(name);
            std::fs::create_dir(&path).unwrap();
            assert!(Profile::fixture(root.path().to_owned()).is_err());
            std::fs::remove_dir(path).unwrap();
        }
        assert!(Profile::fixture(root.path().to_owned()).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn fixture_rejects_links_to_unrelated_data() {
        let root = tempfile::tempdir().unwrap();
        let unrelated = tempfile::tempdir().unwrap();
        let source = unrelated.path().join("keep.json");
        std::fs::write(&source, b"unrelated data").unwrap();
        mark(root.path());
        let link = root.path().join("probe-filters.json");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        assert!(Profile::fixture(root.path().to_owned()).is_err());
        assert_eq!(std::fs::read(source).unwrap(), b"unrelated data");
        std::fs::remove_file(link).unwrap();
        let alias = unrelated.path().join("alias");
        std::os::unix::fs::symlink(root.path(), &alias).unwrap();
        assert!(Profile::fixture(alias).is_err());
    }

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
            prefix: "",
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
            prefix: "",
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
