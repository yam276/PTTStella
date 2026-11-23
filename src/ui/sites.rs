//! Dioxus packet feedback around the shared station edit owner.
use super::outbound::Feedback;
pub(super) use pttstella::sites::session::{Edit, Snapshot, Write};
use pttstella::{
    encoding::TextEncoding,
    sites::{Sites, Target, store::Store},
};
use serde::Serialize;
use std::path::Path;
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Status {
    Saved,
    Connecting,
    Rejected,
}
pub(super) struct State(pttstella::sites::session::State);
impl State {
    pub(super) fn load(probe: Option<&Path>) -> Self {
        let store = probe.map_or_else(Store::system, |path| {
            Ok(Store::at(path.join("probe-sites.json")))
        });
        Self(pttstella::sites::session::State::load(
            store,
            probe.is_some(),
        ))
    }
    pub(super) fn warning(&self) -> Option<&str> {
        self.0.warning()
    }
    pub(super) fn default_target(&self) -> Target {
        self.0.default_target()
    }
    pub(super) fn snapshot(&self, current: Target) -> Snapshot {
        self.0.snapshot(current)
    }
    pub(super) fn edit(
        &self,
        action: Edit,
        address: String,
        encoding: TextEncoding,
        name: String,
    ) -> Result<Write, String> {
        self.0.edit(action, address, encoding, name)
    }
    pub(super) fn complete(&mut self, result: Result<Sites, String>) -> Feedback<Status> {
        match self.0.complete(result) {
            Ok(()) => Feedback::new(Status::Saved, pttstella::msg!("sites-saved")),
            Err(error) => Feedback::new(Status::Rejected, error),
        }
    }
}
