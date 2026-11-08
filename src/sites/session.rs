//! Shared station edit baseline and successful file-result publication.
//! Transport switching is a separate user intent; this type never dials.
use super::{Sites, Target, store::Store};
use crate::encoding::TextEncoding;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Edit {
    Favourite,
    Remove,
    Default,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub sites: Sites,
    pub current: Target,
    pub writable: bool,
    pub warning: Option<String>,
}
pub struct Write {
    pub base: Sites,
    pub action: Edit,
    pub store: Store,
    pub draft: Sites,
}
pub struct State {
    store: Result<Store, String>,
    sites: Sites,
    warning: Option<String>,
    local_probe: bool,
}
impl State {
    pub fn load(store: Result<Store, String>, local_probe: bool) -> Self {
        let (sites, warning) = match store.as_ref().map_err(Clone::clone).and_then(Store::load) {
            Ok(sites) => (sites, None),
            Err(error) => (Sites::default(), Some(error)),
        };
        Self {
            store,
            sites,
            warning,
            local_probe,
        }
    }
    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }
    pub fn default_target(&self) -> Target {
        self.sites.default.clone()
    }
    pub fn snapshot(&self, current: Target) -> Snapshot {
        Snapshot {
            sites: self.sites.clone(),
            current,
            writable: self.warning.is_none(),
            warning: self.warning.clone(),
        }
    }
    pub fn edit(
        &self,
        action: Edit,
        address: String,
        encoding: TextEncoding,
        name: String,
    ) -> Result<Write, String> {
        if let Some(warning) = &self.warning {
            return Err(warning.clone());
        }
        let target = Target::new(address, encoding)?;
        if !matches!(action, Edit::Remove) {
            target.validate_connection(self.local_probe)?;
        }
        let mut draft = self.sites.clone();
        match action {
            Edit::Favourite => draft.save_favourite(name, target)?,
            Edit::Remove => draft.remove(&target),
            Edit::Default => draft.default = target,
        }
        draft.validate()?;
        Ok(Write {
            base: self.sites.clone(),
            action,
            store: self.store.clone()?,
            draft,
        })
    }
    pub fn complete(&mut self, result: Result<Sites, String>) -> Result<(), String> {
        let sites = result?;
        sites.validate()?;
        self.sites = sites;
        Ok(())
    }
    pub fn sites(&self) -> &Sites {
        &self.sites
    }
    pub fn store(&self) -> Result<Store, String> {
        self.store.clone()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_and_default_do_not_commit_on_preview_or_failed_persistence() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = State::load(
            Ok(Store::at(directory.path().join("probe-sites.json"))),
            true,
        );
        let original = state.sites.clone();
        let write = state
            .edit(
                Edit::Default,
                "wss://example.com/bbs".into(),
                TextEncoding::Gbk,
                String::new(),
            )
            .unwrap();
        assert_eq!(state.sites, original);
        assert!(state.complete(Err("failed".into())).is_err());
        assert_eq!(state.sites, original);
        state
            .complete(write.store.save_from(&write.base, write.draft))
            .unwrap();
        assert_eq!(
            State::load(
                Ok(Store::at(directory.path().join("probe-sites.json"))),
                true
            )
            .sites,
            state.sites
        );
        assert_eq!(state.sites.default.encoding, TextEncoding::Gbk);
        assert!(
            state
                .edit(
                    Edit::Favourite,
                    "https://example.com".into(),
                    TextEncoding::Uao,
                    "Invalid".into()
                )
                .is_err()
        );
    }
    #[test]
    fn corrupt_station_file_is_read_only_but_retains_a_manual_connection_fallback() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("probe-sites.json");
        std::fs::write(&path, b"broken").unwrap();
        let state = State::load(
            Ok(Store::at(directory.path().join("probe-sites.json"))),
            true,
        );
        assert!(state.warning.is_some());
        assert_eq!(state.default_target(), Target::default());
        assert!(!state.snapshot(Target::default()).writable);
        assert!(
            state
                .edit(
                    Edit::Default,
                    "telnet://example.com".into(),
                    TextEncoding::Gbk,
                    String::new()
                )
                .is_err()
        );
        assert_eq!(std::fs::read(path).unwrap(), b"broken");
    }
}
