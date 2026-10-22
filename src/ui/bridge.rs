//! One typed sending boundary; raw Eval ownership stays with lifecycle cleanup.
use super::{outbound::Packet, protocol::SurfaceEvent};
use dioxus::prelude::document;

pub(super) struct Bridge<'a> {
    settings: &'a mut super::settings_finish::RetainedEdits,
    eval: document::Eval,
    diagnostics: super::diagnostics::Handle,
}
impl<'a> Bridge<'a> {
    pub(super) fn new(
        script: &str,
        settings: &'a mut super::settings_finish::RetainedEdits,
        diagnostics: super::diagnostics::Handle,
    ) -> Self {
        Self {
            settings,
            eval: document::eval(script),
            diagnostics,
        }
    }
    pub(super) fn owner(&self) -> document::Eval {
        self.eval
    }
    pub(super) fn send(&self, packet: Packet<'_>) -> Result<(), document::EvalError> {
        let trace = match &packet {
            Packet::Paint(p) => Some((
                "paint",
                p.generation,
                p.frame.revision,
                p.presentation,
                true,
            )),
            Packet::InputRejected(_) => Some(("input_rejected", 0, 0, 0, false)),
            Packet::NavigationResult(result) => Some((
                "navigation_result",
                0,
                0,
                0,
                matches!(result.status, super::outbound::NavigationStatus::Sent),
            )),
            Packet::ReadingMove {
                generation,
                article,
                ..
            } => Some(("reading_move", *generation, 0, *article, true)),
            Packet::Stop => Some(("stop", 0, 0, 0, true)),
            _ => None,
        };
        let result = self.eval.send(packet);
        if let Some((kind, generation, revision, presentation, ok)) = trace {
            self.diagnostics.bridge(
                "send",
                kind,
                result.is_ok() && ok,
                generation,
                revision,
                presentation,
            );
        }
        result
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
        let result = self.eval.recv().await;
        match &result {
            Ok(event) => {
                let trace = match event {
                    SurfaceEvent::Painted {
                        generation,
                        revision,
                        presentation,
                    } => Some(("painted", *generation, *revision, *presentation)),
                    SurfaceEvent::Key { generation, .. } => Some(("key", *generation, 0, 0)),
                    SurfaceEvent::Text { generation, .. } => Some(("text", *generation, 0, 0)),
                    SurfaceEvent::Navigate { stamp, .. } => {
                        Some(("navigate", stamp.generation, stamp.revision, 0))
                    }
                    SurfaceEvent::InlineExpand { stamp, .. } => {
                        Some(("inline_expand", stamp.generation, stamp.revision, 0))
                    }
                    SurfaceEvent::Error { .. } => Some(("renderer_error", 0, 0, 0)),
                    _ => None,
                };
                if let Some((kind, generation, revision, presentation)) = trace {
                    self.diagnostics
                        .bridge("recv", kind, true, generation, revision, presentation);
                }
            }
            Err(_) => self
                .diagnostics
                .bridge("recv", "bridge_error", false, 0, 0, 0),
        }
        result
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
