//! Structured events from the renderer. No UI or transport side effects.
use pttstella::{
    navigation::{Intent, Stamp},
    selection::Selection,
    settings::{DisplayProfile, HoverPosition},
    vim,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum MediaAction {
    Close,
    Retry,
    External,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(super) enum SurfaceEvent {
    RecentBoardsClear {
        trusted: bool,
    },
    FiltersOpen {
        trusted: bool,
    },
    FiltersEdit {
        id: String,
        note: String,
        remove: bool,
        trusted: bool,
    },
    FiltersReveal {
        presentation: u64,
        generation: u64,
        revision: u64,
        reveal: bool,
        trusted: bool,
    },
    Hover {
        presentation: u64,
        sequence: u64,
        stamp: Stamp,
        index: Option<usize>,
        trusted: bool,
    },
    HoverPreference {
        position: HoverPosition,
        trusted: bool,
    },
    KeyboardState {
        action: String,
        #[serde(default)]
        code: String,
        generation: u64,
        trusted: bool,
    },
    MediaDecoded {
        request: u64,
        success: bool,
    },
    Media {
        request: u64,
        action: MediaAction,
        trusted: bool,
    },
    KeepAlive {
        enabled: bool,
        trusted: bool,
    },
    KeepAliveGate {
        stamp: Stamp,
        ready: bool,
    },
    Navigate {
        stamp: Stamp,
        intent: Intent,
        trusted: bool,
    },
    DestinationSubmit {
        request: u64,
        stamp: Stamp,
        value: String,
        trusted: bool,
    },
    DestinationCancel {
        operation: Stamp,
        trusted: bool,
    },
    SettingsOpen {
        trusted: bool,
    },
    LoginOpen {
        trusted: bool,
    },
    LoginSubmit {
        account: String,
        password: String,
        remember: bool,
        stamp: Stamp,
        trusted: bool,
    },
    LoginForget {
        trusted: bool,
    },
    LoginCancel {
        trusted: bool,
    },
    SettingsFonts {
        request: u64,
    },
    FontSource {
        source: crate::fonts::Source,
        trusted: bool,
    },
    SettingsPreview {
        request: u64,
        values: DisplayProfile,
    },
    SettingsSave {
        values: DisplayProfile,
        #[serde(default)]
        language: pttstella::i18n::Language,
        trusted: bool,
    },
    SettingsProfile {
        action: String,
        name: String,
        trusted: bool,
    },
    ExportPng {
        presentation: u64,
        generation: u64,
        revision: u64,
        selection_request: Option<u64>,
        png: String,
        trusted: bool,
    },
    OpenLink {
        presentation: u64,
        generation: u64,
        revision: u64,
        index: usize,
        trusted: bool,
    },
    Paste {
        text: String,
        generation: u64,
        trusted: bool,
    },
    PasteConfirm {
        trusted: bool,
    },
    PasteCancel {
        trusted: bool,
    },
    Select {
        presentation: u64,
        generation: u64,
        revision: u64,
        request: u64,
        selection: Selection,
    },
    Copied {
        presentation: u64,
        generation: u64,
        revision: u64,
        request: u64,
        trusted: bool,
    },
    SelectionCleared {
        presentation: u64,
        generation: u64,
        revision: u64,
        request: u64,
    },
    Key {
        key: vim::Key,
        generation: u64,
        trusted: bool,
    },
    Text {
        text: String,
        generation: u64,
        trusted: bool,
        #[serde(default)]
        composition_input: bool,
        #[serde(default)]
        composed: bool,
    },
    Painted {
        presentation: u64,
        revision: u64,
        generation: u64,
    },
    Probe(Box<ProbePixels>),
    Error {
        message: String,
    },
    Stopped {
        clean: bool,
    },
    SurfaceProbe {
        clean: bool,
    },
}

impl SurfaceEvent {
    /// Admit user actions before any state change, encoding or station enqueue.
    /// This is event provenance, not a security boundary against injected code.
    /// Frame/revision and feature-specific checks still belong to their owners.
    pub(super) fn admitted(&self, generation: u64) -> bool {
        use SurfaceEvent::*;
        match self {
            Key {
                trusted,
                generation: source,
                ..
            }
            | Paste {
                trusted,
                generation: source,
                ..
            } => *trusted && *source == generation,
            Text {
                trusted,
                composition_input,
                generation: source,
                ..
            } => {
                // Legacy WebKit/CDP: JS requires matching trusted start/input.
                (*trusted || *composition_input) && *source == generation
            }
            KeyboardState {
                trusted,
                action,
                generation: source,
                ..
            } => {
                // Ending composition only revokes an edit; it grants no input.
                (*trusted || action == "composition_end") && *source == generation
            }
            Navigate { trusted, stamp, .. }
            | LoginSubmit { trusted, stamp, .. }
            | DestinationSubmit { trusted, stamp, .. } => {
                *trusted && stamp.generation == generation
            }
            DestinationCancel { trusted, operation } => {
                *trusted && operation.generation == generation
            }
            FiltersOpen { trusted }
            | RecentBoardsClear { trusted }
            | FiltersEdit { trusted, .. }
            | FiltersReveal { trusted, .. }
            | Hover { trusted, .. }
            | HoverPreference { trusted, .. }
            | Media { trusted, .. }
            | KeepAlive { trusted, .. }
            | SettingsOpen { trusted }
            | FontSource { trusted, .. }
            | LoginOpen { trusted }
            | LoginForget { trusted }
            | LoginCancel { trusted }
            | SettingsSave { trusted, .. }
            | SettingsProfile { trusted, .. }
            | ExportPng { trusted, .. }
            | OpenLink { trusted, .. }
            | PasteConfirm { trusted }
            | PasteCancel { trusted }
            | Copied { trusted, .. } => *trusted,
            // Internal notifications and projections do not submit user input.
            MediaDecoded { .. }
            | KeepAliveGate { .. }
            | SettingsFonts { .. }
            | SettingsPreview { .. }
            | Select { .. }
            | SelectionCleared { .. }
            | Painted { .. }
            | Probe(_)
            | Error { .. }
            | Stopped { .. }
            | SurfaceProbe { .. } => true,
        }
    }

    pub(super) fn discard(mut self) {
        if let Self::LoginSubmit { password, .. } = &mut self {
            zeroize::Zeroize::zeroize(password);
        }
    }
}

#[cfg(test)]
mod admission_tests {
    use super::*;
    use serde_json::{Value, json};

    fn admits(mut value: Value, trusted: bool, generation: u64) -> bool {
        value["trusted"] = trusted.into();
        serde_json::from_value::<SurfaceEvent>(value)
            .unwrap()
            .admitted(generation)
    }

    #[test]
    fn all_station_input_paths_reject_synthetic_or_stale_events() {
        let stamp = json!({"generation":7,"revision":4,"input_epoch":2});
        let events = [
            json!({"kind":"key","key":{"key":"Enter","code":"Enter","ctrl":false,"alt":false,"shift":false,"repeat":false},"generation":7}),
            json!({"kind":"text","text":"a","generation":7}),
            json!({"kind":"paste","text":"a\nb","generation":7}),
            json!({"kind":"navigate","stamp":stamp,"intent":{"action":"back"}}),
            json!({"kind":"destination_submit","request":1,"stamp":stamp,"value":"Stella"}),
            json!({"kind":"destination_cancel","operation":stamp}),
            json!({"kind":"login_submit","account":"Synthetic","password":"unused","remember":false,"stamp":stamp}),
        ];
        for event in events {
            assert!(!admits(event.clone(), false, 7), "{event}");
            assert!(!admits(event.clone(), true, 8), "{event}");
            assert!(admits(event.clone(), true, 7), "{event}");
        }
        assert!(!admits(
            json!({"kind":"font_source","source":"microsoft_jhenghei"}),
            false,
            7
        ));
        assert!(admits(
            json!({"kind":"font_source","source":"microsoft_jhenghei"}),
            true,
            7
        ));
        for kind in ["paste_confirm", "paste_cancel", "recent_boards_clear"] {
            assert!(!admits(json!({"kind":kind}), false, 7));
            assert!(admits(json!({"kind":kind}), true, 7));
        }
    }

    #[test]
    fn composition_exception_and_revocation_never_admit_an_old_session() {
        let legacy = json!({"kind":"text","text":"中","generation":7,"composition_input":true,"composed":true});
        assert!(admits(legacy.clone(), false, 7));
        assert!(!admits(legacy, false, 8));
        let end = json!({"kind":"keyboard_state","action":"composition_end","generation":7});
        assert!(admits(end.clone(), false, 7));
        assert!(!admits(end, false, 8));
        for action in ["composition_start", "toggle", "once", "release", "reset"] {
            assert!(!admits(
                json!({"kind":"keyboard_state","action":action,"generation":7}),
                false,
                7
            ));
        }
    }
}

#[derive(Deserialize)]
pub(super) struct ProbePixels {
    pub(super) png: String,
    pub(super) red_pixels: usize,
    pub(super) green_pixels: usize,
    pub(super) left_bg: Vec<u8>,
    pub(super) right_bg: Vec<u8>,
    pub(super) fonts: String,
    pub(super) dpr: f64,
    pub(super) pixel_size: [u32; 2],
    pub(super) font_stack: String,
    pub(super) fills_body: bool,
    pub(super) colour_samples: Vec<Vec<u8>>,
    pub(super) art_seam_pixels: usize,
    pub(super) art_zoom_seams: Vec<usize>,
    pub(super) art_geometry: bool,
    pub(super) hover_controls: bool,
    pub(super) proportional_zoom: bool,
    pub(super) zoom_samples: Vec<[u32; 2]>,
}
