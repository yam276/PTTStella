//! Station choices commit only completed file results; a connection is a separate intent.
use super::outbound::Feedback;
use pttstella::{
    encoding::TextEncoding,
    sites::{Sites, Target, store::Store},
};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Edit {
    Favourite,
    Remove,
    Default,
}
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Status {
    Saved,
    Connecting,
    Rejected,
}
#[derive(Serialize)]
pub(super) struct Snapshot {
    pub sites: Sites,
    pub current: Target,
    pub writable: bool,
    pub warning: Option<String>,
}
pub(super) struct Write {
    pub base: Sites,
    pub action: Edit,
    pub store: Store,
    pub draft: Sites,
}
pub(super) struct State {
    store: Result<Store, String>,
    sites: Sites,
    warning: Option<String>,
}
impl State {
    pub(super) fn load(probe: Option<&Path>) -> Self {
        let store = probe.map_or_else(Store::system, |path| {
            Ok(Store::at(path.join("probe-sites.json")))
        });
        let (sites, warning) = match store.as_ref().map_err(Clone::clone).and_then(Store::load) {
            Ok(sites) => (sites, None),
            Err(error) => (Sites::default(), Some(error)),
        };
        Self {
            store,
            sites,
            warning,
        }
    }
    pub(super) fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }
    pub(super) fn default_target(&self) -> Target {
        self.sites.default.clone()
    }
    pub(super) fn snapshot(&self, current: Target) -> Snapshot {
        Snapshot {
            sites: self.sites.clone(),
            current,
            writable: self.warning.is_none(),
            warning: self.warning.clone(),
        }
    }
    pub(super) fn edit(
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
    pub(super) fn complete(&mut self, result: Result<Sites, String>) -> Feedback<Status> {
        match result {
            Ok(sites) => {
                self.sites = sites;
                Feedback::new(Status::Saved, pttstella::msg!("sites-saved"))
            }
            Err(error) => Feedback::new(Status::Rejected, error),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edits_and_default_do_not_commit_on_preview_or_failed_persistence() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = State::load(Some(directory.path()));
        let original = state.sites.clone();
        let write = state
            .edit(
                Edit::Default,
                "telnet://example.com".into(),
                TextEncoding::Gbk,
                String::new(),
            )
            .unwrap();
        assert_eq!(state.sites, original);
        state.complete(Err("failed".into()));
        assert_eq!(state.sites, original);
        state.complete(write.store.save_from(&write.base, write.draft));
        assert_eq!(State::load(Some(directory.path())).sites, state.sites);
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
        let state = State::load(Some(directory.path()));
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
