//! Structured events from the renderer. No UI or transport side effects.
use pttstella::{
    navigation::{Intent, Stamp},
    selection::Selection,
    settings::{DisplayProfile, HoverPosition},
    vim,
};
use serde::{Deserialize, Serialize};

/// A renderer wait is scoped independently of source revisions and station bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NavigationTicket {
    pub request: u64,
    pub generation: u64,
}

/// Bounded, chrome-only observations returned by synthetic probes at shutdown.
#[derive(Deserialize, Serialize)]
pub(super) struct ToolsObservation {
    presses: u16,
    moves: u16,
    drag_releases: u16,
    coast_frames: u16,
    max_coast_distance: f64,
    last_release: Option<ToolsRelease>,
}
#[derive(Deserialize, Serialize)]
struct ToolsRelease {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    reduced: bool,
    samples: u8,
}

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
    Diagnostics {
        trace: super::diagnostics::WebTrace,
    },
    StationMouse {
        event: pttstella::mouse::input::Event,
        trusted: bool,
    },
    StationMouseSample {
        sample: pttstella::mouse::input::Sample,
        trusted: bool,
    },
    StationMouseCancel {
        lease: pttstella::mouse::input::Lease,
    },
    StationMouseRevoke {
        generation: u64,
    },
    StationMouseEnabled {
        generation: u64,
        enabled: bool,
        trusted: bool,
    },
    BookmarksOpen {
        request: u64,
        trusted: bool,
    },
    BookmarksEdit {
        request: u64,
        value: String,
        label: String,
        remove: bool,
        trusted: bool,
    },
    RecentBoardsClear {
        trusted: bool,
    },
    FiltersOpen {
        trusted: bool,
    },
    AccountRule {
        presentation: u64,
        stamp: Stamp,
        row: usize,
        col: usize,
        policy: pttstella::filters::Policy,
        #[serde(default)]
        remove: bool,
        trusted: bool,
    },
    ReplyFocus {
        presentation: u64,
        stamp: Stamp,
        row: Option<usize>,
        trusted: bool,
    },
    FiltersDefaultColor {
        color: u8,
        trusted: bool,
    },
    FiltersEdit {
        id: String,
        note: String,
        #[serde(default)]
        policy: pttstella::filters::Policy,
        #[serde(default)]
        color: Option<u8>,
        remove: bool,
        trusted: bool,
    },
    FiltersImport {
        request: u64,
        trusted: bool,
    },
    FiltersImportApply {
        request: u64,
        trusted: bool,
    },
    FiltersImportCancel {
        request: u64,
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
    InlineExpand {
        presentation: u64,
        stamp: Stamp,
        key: vim::Key,
        #[serde(default)]
        target: super::inline_images::Target,
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
        #[serde(default)]
        reading: bool,
        #[serde(default)]
        article: Option<u64>,
    },
    Navigate {
        request: u64,
        stamp: Stamp,
        intent: Intent,
        source: pttstella::navigation::Source,
        trusted: bool,
    },
    NavigationCancel {
        request: u64,
        generation: u64,
    },
    ArticleLoad {
        generation: u64,
        article: u64,
        resume: bool,
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
    SitesOpen {
        request: u64,
        trusted: bool,
    },
    SitesEdit {
        request: u64,
        action: super::sites::Edit,
        address: String,
        encoding: pttstella::encoding::TextEncoding,
        name: String,
        trusted: bool,
    },
    SitesConnect {
        request: u64,
        address: String,
        encoding: pttstella::encoding::TextEncoding,
        trusted: bool,
    },
    SettingsVisibility {
        open: bool,
    },
    AboutRepository {
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
        #[serde(flatten)]
        change: super::preferences::Change,
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
        #[serde(default)]
        destination: super::files::ExportDestination,
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
    CopyText {
        presentation: u64,
        generation: u64,
        revision: u64,
        target: CopyTarget,
        trusted: bool,
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
        #[serde(default)]
        tools: Option<ToolsObservation>,
    },
    SurfaceProbe {
        clean: bool,
    },
}

impl SurfaceEvent {
    pub(super) fn navigation_ticket(&self) -> Option<NavigationTicket> {
        match self {
            Self::Navigate { request, stamp, .. } => Some(NavigationTicket {
                request: *request,
                generation: stamp.generation,
            }),
            _ => None,
        }
    }
    /// Site capabilities are checked before credential access or local PTT data edits.
    pub(super) fn requires_ptt(&self) -> bool {
        matches!(
            self,
            Self::LoginOpen { .. }
                | Self::LoginSubmit { .. }
                | Self::LoginForget { .. }
                | Self::DestinationSubmit { .. }
                | Self::BookmarksOpen { .. }
                | Self::BookmarksEdit { .. }
                | Self::RecentBoardsClear { .. }
                | Self::FiltersOpen { .. }
                | Self::FiltersEdit { .. }
                | Self::FiltersDefaultColor { .. }
                | Self::AccountRule { .. }
                | Self::ReplyFocus { .. }
                | Self::FiltersImport { .. }
                | Self::FiltersImportApply { .. }
                | Self::FiltersImportCancel { .. }
                | Self::FiltersReveal { .. }
                | Self::Navigate { .. }
                | Self::ArticleLoad { .. }
                | Self::KeepAlive { .. }
        )
    }
    /// Admit user actions before any state change, encoding or station enqueue.
    /// This is event provenance, not a security boundary against injected code.
    /// Frame/revision and feature-specific checks still belong to their owners.
    pub(super) fn admitted(&self, generation: u64) -> bool {
        use SurfaceEvent::*;
        match self {
            StationMouse { event, trusted } => {
                *trusted && event.lease.stamp.generation == generation
            }
            StationMouseSample { sample, trusted } => {
                *trusted && sample.lease.stamp.generation == generation
            }
            StationMouseEnabled {
                generation: source,
                trusted,
                ..
            } => *trusted && *source == generation,
            // Revocation grants no input; it still cannot cross a session.
            StationMouseCancel { lease } => lease.stamp.generation == generation,
            StationMouseRevoke { generation: source } => *source == generation,
            AccountRule { trusted, stamp, .. }
            | ReplyFocus { trusted, stamp, .. }
            | InlineExpand { trusted, stamp, .. } => *trusted && stamp.generation == generation,
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
            Navigate {
                request,
                trusted,
                stamp,
                source,
                intent,
            } => {
                *trusted
                    && (1..=9_007_199_254_740_991).contains(request)
                    && stamp.generation == generation
                    && (*source != pttstella::navigation::Source::Keyboard
                        || matches!(intent, Intent::AdjacentArticle { .. }))
            }
            NavigationCancel {
                generation: source, ..
            } => *source == generation,
            ArticleLoad {
                trusted,
                generation: source,
                ..
            } => *trusted && *source == generation,
            LoginSubmit { trusted, stamp, .. } | DestinationSubmit { trusted, stamp, .. } => {
                *trusted && stamp.generation == generation
            }
            DestinationCancel { trusted, operation } => {
                *trusted && operation.generation == generation
            }
            SitesOpen { trusted, .. }
            | SitesEdit { trusted, .. }
            | SitesConnect { trusted, .. }
            | BookmarksOpen { trusted, .. }
            | BookmarksEdit { trusted, .. }
            | FiltersOpen { trusted }
            | RecentBoardsClear { trusted }
            | FiltersEdit { trusted, .. }
            | FiltersDefaultColor { trusted, .. }
            | FiltersImport { trusted, .. }
            | FiltersImportApply { trusted, .. }
            | FiltersImportCancel { trusted, .. }
            | FiltersReveal { trusted, .. }
            | Hover { trusted, .. }
            | HoverPreference { trusted, .. }
            | Media { trusted, .. }
            | KeepAlive { trusted, .. }
            | AboutRepository { trusted }
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
            | Copied { trusted, .. }
            | CopyText { trusted, .. } => *trusted,
            // Internal notifications and projections do not submit user input.
            MediaDecoded { .. }
            | Diagnostics { .. }
            | KeepAliveGate { .. }
            | SettingsVisibility { .. }
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
    #[test]
    fn filter_color_edits_are_typed_trusted_ptt_actions() {
        use serde_json::json;
        for (value, expected) in [
            (
                json!({"kind":"filters_default_color","color":10,"trusted":true}),
                true,
            ),
            (
                json!({"kind":"filters_default_color","color":10,"trusted":false}),
                false,
            ),
        ] {
            let event: super::SurfaceEvent = serde_json::from_value(value).unwrap();
            assert!(event.requires_ptt());
            assert_eq!(event.admitted(7), expected);
        }
        assert!(
            serde_json::from_value::<super::SurfaceEvent>(
                json!({"kind":"filters_default_color","color":"red","trusted":true})
            )
            .is_err()
        );
    }

    #[test]
    fn context_copy_requires_trust_and_closed_targets() {
        let event = json!({"kind":"copy_text","presentation":3,"generation":7,"revision":4,"target":{"kind":"link","index":0}});
        assert!(admits(event.clone(), true, 7));
        assert!(!admits(event.clone(), false, 7));
        for target in [
            json!({"kind":"text","text":"arbitrary"}),
            json!({"kind":"link","index":0,"text":"override"}),
            json!({"kind":"link","index":-1}),
        ] {
            let mut invalid = event.clone();
            invalid["trusted"] = true.into();
            invalid["target"] = target;
            assert!(serde_json::from_value::<SurfaceEvent>(invalid).is_err());
        }
    }

    use super::*;
    use serde_json::{Value, json};
    #[test]
    fn shutdown_probe_observations_are_optional_typed_and_bounded() {
        assert!(matches!(
            serde_json::from_value::<SurfaceEvent>(json!({"kind":"stopped","clean":true})).unwrap(),
            SurfaceEvent::Stopped { tools: None, .. }
        ));
        let tools = json!({"presses":2,"moves":10,"drag_releases":1,"coast_frames":8,"max_coast_distance":72.5,
            "last_release":{"x":200.0,"y":300.0,"vx":-0.6,"vy":0.2,"reduced":false,"samples":8}});
        let event = serde_json::from_value::<SurfaceEvent>(
            json!({"kind":"stopped","clean":true,"tools":tools}),
        )
        .unwrap();
        let SurfaceEvent::Stopped {
            tools: Some(observed),
            ..
        } = event
        else {
            panic!("missing observations")
        };
        assert_eq!(serde_json::to_value(observed).unwrap(), tools);
        let mut oversized = tools;
        oversized["presses"] = json!(65536);
        assert!(
            serde_json::from_value::<SurfaceEvent>(
                json!({"kind":"stopped","clean":true,"tools":oversized})
            )
            .is_err()
        );
    }

    fn admits(mut value: Value, trusted: bool, generation: u64) -> bool {
        value["trusted"] = trusted.into();
        serde_json::from_value::<SurfaceEvent>(value)
            .unwrap()
            .admitted(generation)
    }

    #[test]
    fn navigation_requests_match_shared_javascript_lifecycle_examples() {
        use super::super::outbound::{Feedback, NavigationStatus};
        let cases: Vec<Value> = serde_json::from_str(include_str!(
            "../../tests/fixtures/navigation-requests.json"
        ))
        .unwrap();
        for case in cases {
            let event: SurfaceEvent = serde_json::from_value(case["event"].clone()).unwrap();
            let admitted = event.admitted(case["current_generation"].as_u64().unwrap());
            assert_eq!(
                admitted,
                case["admitted"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
            let result = event
                .navigation_ticket()
                .unwrap()
                .finish(Feedback::quiet(if admitted {
                    NavigationStatus::Sent
                } else {
                    NavigationStatus::Rejected
                }));
            assert_eq!(
                serde_json::to_value(result).unwrap(),
                case["result"],
                "{}",
                case["name"]
            );
        }
    }

    #[test]
    fn keyboard_origin_is_narrowly_admitted_only_for_adjacent_article_and_valid_ticket() {
        let mut event = json!({"kind":"navigate","request":1,"stamp":{"generation":7,"revision":4,"input_epoch":2},"intent":{"action":"adjacent_article","down":true},"source":"keyboard"});
        assert!(admits(event.clone(), true, 7));
        assert!(!admits(event.clone(), false, 7));
        assert!(!admits(event.clone(), true, 8));
        for intent in [
            json!({"action":"back"}),
            json!({"action":"next_page"}),
            json!({"action":"lines","down":true,"count":1}),
        ] {
            event["intent"] = intent;
            assert!(!admits(event.clone(), true, 7));
        }
        event["source"] = json!("button");
        event["intent"] = json!({"action":"next_page"});
        for request in [0, 9_007_199_254_740_992u64] {
            event["request"] = json!(request);
            assert!(!admits(event.clone(), true, 7));
        }
        assert!(admits(
            json!({"kind":"navigation_cancel","request":1,"generation":7}),
            false,
            7
        ));
        assert!(!admits(
            json!({"kind":"navigation_cancel","request":1,"generation":7}),
            false,
            8
        ));
    }

    #[test]
    fn station_actions_require_trusted_closed_typed_intents() {
        for value in [
            json!({"kind":"sites_open","request":1}),
            json!({"kind":"sites_edit","request":2,"action":"favourite","address":"telnet://example.com","encoding":"gbk","name":"Example"}),
            json!({"kind":"sites_connect","request":3,"address":"wss://example.com/bbs","encoding":"big5"}),
        ] {
            assert!(admits(value.clone(), true, 7));
            assert!(!admits(value, false, 7));
        }
        for value in [
            json!({"kind":"sites_connect","request":1,"address":"example.com","encoding":"guess","trusted":true}),
            json!({"kind":"sites_edit","request":1,"action":"execute","address":"example.com","encoding":"gbk","name":"","trusted":true}),
        ] {
            assert!(serde_json::from_value::<SurfaceEvent>(value).is_err());
        }
    }

    #[test]
    fn settings_opacity_requires_a_bounded_integer_and_trusted_save() {
        let valid = json!({"kind":"settings_save","values":{"fonts":{"kind":"platform_default"},"font_size":20,"baseline":20},"language":"en-US","tools_opacity":65,"tools_access":{"right_click_menu":true,"show_floating_button":true},"trusted":true});
        assert!(admits(valid.clone(), true, 7));
        assert!(!admits(valid.clone(), false, 7));
        for value in [json!(19), json!(101), json!(65.5), json!("65"), Value::Null] {
            let mut invalid = valid.clone();
            invalid["tools_opacity"] = value;
            assert!(serde_json::from_value::<SurfaceEvent>(invalid).is_err());
        }
        for access in [
            json!({"right_click_menu":"true","show_floating_button":true}),
            json!({"right_click_menu":true}),
            json!({"right_click_menu":true,"show_floating_button":true,"unknown":false}),
        ] {
            let mut invalid = valid.clone();
            invalid["tools_access"] = access;
            assert!(serde_json::from_value::<SurfaceEvent>(invalid).is_err());
        }
    }

    #[test]
    fn all_station_input_paths_reject_synthetic_or_stale_events() {
        let stamp = json!({"generation":7,"revision":4,"input_epoch":2});
        let events = [
            json!({"kind":"inline_expand","presentation":3,"stamp":stamp,"key":{"key":"Enter","code":"Enter","ctrl":false,"alt":false,"shift":false,"repeat":false}}),
            json!({"kind":"key","key":{"key":"Enter","code":"Enter","ctrl":false,"alt":false,"shift":false,"repeat":false},"generation":7}),
            json!({"kind":"text","text":"a","generation":7}),
            json!({"kind":"paste","text":"a\nb","generation":7}),
            json!({"kind":"navigate","request":1,"stamp":stamp,"intent":{"action":"back"},"source":"pointer"}),
            json!({"kind":"navigate","request":1,"stamp":stamp,"intent":{"action":"back"},"source":"button"}),
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
        for event in [
            json!({"kind":"bookmarks_open","request":1}),
            json!({"kind":"bookmarks_edit","request":2,"value":"#00000101@Stella","label":"","remove":false}),
        ] {
            assert!(!admits(event.clone(), false, 7));
            assert!(admits(event.clone(), true, 7));
            assert!(admits(event, true, 8)); // Local data survives session changes.
        }
    }

    #[test]
    fn station_mouse_requires_trusted_current_reports_and_closed_structured_types() {
        let lease = json!({"stamp":{"generation":7,"revision":4,"input_epoch":2},"mode_epoch":5,"policy_epoch":3,"transaction":9});
        let event = json!({"kind":"station_mouse","event":{"lease":lease,"report":{"col":4,"row":3,"button":"right","action":"press","modifiers":{"shift":false,"alt":true,"ctrl":false}}}});
        assert!(admits(event.clone(), true, 7));
        assert!(!admits(event.clone(), false, 7));
        assert!(!admits(event.clone(), true, 8));
        for report in [
            json!({"kind":"motion","col":4,"row":3,"button":null,"modifiers":{"shift":false,"alt":true,"ctrl":false}}),
            json!({"kind":"wheel","col":4,"row":3,"direction":"up","modifiers":{"shift":false,"alt":false,"ctrl":false}}),
        ] {
            let sample = json!({"kind":"station_mouse_sample","sample":{"lease":lease,"sequence":10,"report":report}});
            assert!(admits(sample.clone(), true, 7));
            assert!(!admits(sample.clone(), false, 7));
            assert!(!admits(sample.clone(), true, 8));
            let mut invalid = sample.clone();
            invalid["trusted"] = true.into();
            invalid["sample"]["report"]["raw"] = json!("\u{001b}[B");
            assert!(serde_json::from_value::<SurfaceEvent>(invalid).is_err());
        }
        for (field, value) in [
            ("action", json!("wheel")),
            ("button", json!("unknown")),
            ("col", json!(-1)),
            ("raw", json!("\u{001b}[B")),
        ] {
            let mut invalid = event.clone();
            invalid["trusted"] = true.into();
            invalid["event"]["report"][field] = value;
            assert!(serde_json::from_value::<SurfaceEvent>(invalid).is_err());
        }
        let toggle = json!({"kind":"station_mouse_enabled","generation":7,"enabled":false});
        assert!(admits(toggle.clone(), true, 7));
        assert!(!admits(toggle.clone(), false, 7));
        assert!(!admits(toggle, true, 8));
        for value in [
            json!({"kind":"station_mouse_cancel","lease":lease}),
            json!({"kind":"station_mouse_revoke","generation":7}),
        ] {
            // Revocations grant no input and are allowed for internal cleanup.
            assert!(admits(value.clone(), false, 7));
            assert!(!admits(value, false, 8));
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

    #[test]
    fn local_navigation_requires_a_closed_source_and_cannot_impersonate_vim() {
        let event = json!({"kind":"navigate","request":1,"stamp":{"generation":7,"revision":4,"input_epoch":2},"intent":{"action":"back"},"trusted":true});
        assert!(serde_json::from_value::<SurfaceEvent>(event.clone()).is_err());
        let mut invalid = event.clone();
        invalid["source"] = "unspecified".into();
        assert!(serde_json::from_value::<SurfaceEvent>(invalid).is_err());
        for source in ["pointer", "button", "keyboard"] {
            let mut value = event.clone();
            value["source"] = source.into();
            assert_eq!(admits(value.clone(), true, 7), source != "keyboard");
            assert!(!admits(value.clone(), false, 7));
            assert!(!admits(value, true, 8));
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
    pub(super) floating_tools: bool,
    pub(super) proportional_zoom: bool,
    pub(super) zoom_samples: Vec<[u32; 2]>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum CopyTarget {
    Selection { request: u64 },
    Link { index: usize },
}

#[cfg(test)]
mod article_load_tests {
    use super::*;
    #[test]
    fn loading_actions_are_typed_ptt_actions_and_reject_foreign_sessions_or_synthetic_clicks() {
        for (trusted, generation, expected) in
            [(true, 7, true), (false, 7, false), (true, 8, false)]
        {
            let event = SurfaceEvent::ArticleLoad {
                generation,
                article: 42,
                resume: false,
                trusted,
            };
            assert!(event.requires_ptt());
            assert_eq!(event.admitted(7), expected);
        }
    }
}

#[cfg(test)]
mod image_clipboard_tests {
    use super::*;
    #[test]
    fn image_export_destinations_are_closed_and_require_trusted_events() {
        let mut value = serde_json::json!({"kind":"export_png","presentation":3,"generation":7,"revision":4,"selection_request":null,"destination":"clipboard","png":"data:image/png;base64,AA==","trusted":true});
        let event: SurfaceEvent = serde_json::from_value(value.clone()).unwrap();
        assert!(event.admitted(7));
        // SurfaceState and OutputLease own source freshness for export events.
        assert!(matches!(
            event,
            SurfaceEvent::ExportPng {
                destination: super::super::files::ExportDestination::Clipboard,
                ..
            }
        ));
        value["trusted"] = false.into();
        assert!(
            !serde_json::from_value::<SurfaceEvent>(value.clone())
                .unwrap()
                .admitted(7)
        );
        value["destination"] = "custom".into();
        assert!(serde_json::from_value::<SurfaceEvent>(value).is_err());
    }
}
