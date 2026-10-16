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
    SettingsPreview {
        request: u64,
        values: DisplayProfile,
    },
    SettingsSave {
        values: DisplayProfile,
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
