//! Read-only OS animation preferences. One latest-value channel, no polling.
//! This service has no UI dependency and never changes the user's settings.
use serde::Serialize;
use tokio::sync::watch;

#[cfg(target_os = "macos")]
#[path = "motion_preferences/macos.rs"]
mod provider;
#[cfg(target_os = "windows")]
#[path = "motion_preferences/windows.rs"]
mod provider;
#[cfg(target_os = "linux")]
#[path = "motion_preferences/linux.rs"]
mod provider;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Preference {
    Pending,
    Known {
        reduced: bool,
        source: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        raw_value: Option<u32>,
    },
    Unavailable {
        stage: &'static str,
        detail: String,
    },
}
impl Preference {
    pub fn reduced(&self) -> bool {
        match self {
            Self::Known { reduced, .. } => *reduced,
            Self::Pending | Self::Unavailable { .. } => true,
        }
    }
    pub fn failed(stage: &'static str, error: impl std::fmt::Display) -> Self {
        let mut detail = error.to_string();
        if detail.len() > 1024 {
            let mut end = 1024;
            while !detail.is_char_boundary(end) {
                end -= 1;
            }
            detail.truncate(end);
            detail.push('…');
        }
        Self::Unavailable { stage, detail }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Audit {
    pub listener_removed: bool,
    pub worker_joined: Option<bool>,
}

/// Must be stopped and joined before retiring the UI event loop/runtime.
pub struct Subscription {
    provider: provider::Provider,
}
impl Subscription {
    pub fn start() -> Result<(Self, watch::Receiver<Preference>), String> {
        let (sender, receiver) = watch::channel(Preference::Pending);
        let provider = provider::Provider::start(sender)?;
        Ok((Self { provider }, receiver))
    }
    pub fn cancel(&mut self) -> Result<(), String> {
        self.provider.cancel()
    }
    pub async fn shutdown(&mut self) -> Result<Audit, String> {
        self.provider.shutdown().await
    }
    /// Only the macOS conformance probe posts a process-local workspace
    /// notification; it never changes the system preference or another app.
    #[cfg(target_os = "macos")]
    pub fn verify_notification(&self) -> Result<(), String> {
        self.provider.verify_notification()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_is_conservative_without_claiming_a_known_preference() {
        assert!(Preference::Pending.reduced());
        let unavailable = Preference::failed("read", "missing service");
        assert!(unavailable.reduced());
        assert!(matches!(unavailable, Preference::Unavailable { .. }));
        for reduced in [true, false] {
            assert_eq!(
                Preference::Known {
                    reduced,
                    source: "fixture",
                    raw_value: None,
                }
                .reduced(),
                reduced
            );
        }
    }
    #[test]
    fn backend_error_text_is_utf8_bounded_with_an_explicit_truncation_marker() {
        let Preference::Unavailable { detail, .. } = Preference::failed("read", "繁".repeat(1000))
        else {
            panic!()
        };
        assert!(detail.len() <= 1027 && detail.ends_with('…'));
    }
}

// Exercise the actual Linux D-Bus consumer against an owned Unix socket on the
// Mac development host as well; this is protocol evidence, not a Linux desktop.
#[cfg(all(test, target_os = "macos"))]
#[allow(dead_code)]
#[path = "motion_preferences/linux.rs"]
mod linux_protocol;
