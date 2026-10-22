//! One typed sending boundary; raw Eval ownership stays with lifecycle cleanup.
use super::{outbound::Packet, protocol::SurfaceEvent};
use dioxus::prelude::document;

pub(super) struct Bridge<'a> {
    settings: &'a mut super::settings_finish::RetainedEdits,
    eval: document::Eval,
}
impl<'a> Bridge<'a> {
    pub(super) fn new(
        script: &str,
        settings: &'a mut super::settings_finish::RetainedEdits,
    ) -> Self {
        Self {
            settings,
            eval: document::eval(script),
        }
    }
    pub(super) fn owner(&self) -> document::Eval {
        self.eval
    }
    pub(super) fn send(&self, packet: Packet<'_>) -> Result<(), document::EvalError> {
        self.eval.send(packet)
    }
    pub(super) fn open_settings(
        &mut self,
        store: Result<pttstella::settings::store::Store, String>,
    ) {
        self.settings.open(store);
    }
    pub(super) fn settings_notice(&self, message: &str) -> Result<(), document::EvalError> {
        self.eval.send(Packet::Notice(message.into()))
    }
    pub(super) async fn recv(&mut self) -> Result<SurfaceEvent, document::EvalError> {
        self.eval.recv().await
    }
    /// Only the explicit local fault probe calls this serialization failure.
    pub(super) fn probe_send_failure(&self) -> Result<(), document::EvalError> {
        struct Rejected;
        impl serde::Serialize for Rejected {
            fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
                Err(serde::ser::Error::custom(
                    "synthetic packet serialization failure",
                ))
            }
        }
        self.eval.send(Rejected)
    }
}
