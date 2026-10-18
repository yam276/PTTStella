//! Synthetic fixtures and verification counters, separate from product state.
use super::protocol::ProbePixels;
use base64::Engine;
use pttstella::{encoding, settings::AppSettings};
use serde_json::Value;
use std::path::Path;

/// Cleanup errors preserve observations without asserting unconfirmed joins.
pub(super) fn finish_report(
    report: &mut Value,
    disconnect: Option<pttstella::connection::CloseOutcome>,
) {
    use pttstella::connection::CloseOutcome;
    let joined = disconnect.map(|_| true);
    report["cleanup_completed"] = disconnect.is_some().into();
    report["listeners_released"] = joined.into();
    report["connection_worker_joined"] = joined.into();
    report["credential_worker_joined"] = joined.into();
    report["disconnect"] = serde_json::json!(disconnect);
    if disconnect != Some(CloseOutcome::Acknowledged) {
        report["status"] = "failed".into();
    }
}

pub(super) fn mouse_case(case: Option<&str>) -> bool {
    matches!(
        case,
        Some(
            "mouse"
                | "mouse_boundary"
                | "mouse_drag"
                | "mouse_round_trip"
                | "mouse_motion"
                | "mouse_wheel"
        )
    )
}

#[derive(Default)]
pub(super) struct Checks {
    pub(super) mouse_edges: Vec<pttstella::mouse::Report>,
    pub(super) mouse_samples: Vec<pttstella::mouse::input::SampleReport>,
    pub(super) mouse_sample_overflow: bool,
    pub(super) mouse_return_painted: Option<u64>,
    pub(super) mouse_cancels: usize,
    pub(super) mouse_copies: usize,
    pub(super) mouse_toggles: Vec<bool>,
    pub(super) bookmark_opens: usize,
    pub(super) bookmark_saves: usize,
    pub(super) bookmark_rejected: usize,
    pub(super) article_reached: Vec<(u64, String)>,
    pub(super) article_rejected: usize,
    pub(super) article_missing: bool,
    pub(super) article_cancelled: bool,
    pub(super) board_reached: Vec<(u64, String)>,
    pub(super) board_rejected: usize,
    pub(super) board_cancelled: bool,
    pub(super) osc8_open_rows: Vec<usize>,
    pub(super) osc8_copy_rows: Vec<usize>,
    pub(super) osc8_selected: Option<(u64, u64, u64, usize)>,
    pub(super) osc8_dragged: bool,
    pub(super) osc8_media_decoded: usize,
    pub(super) verification_required: bool,
    pub(super) verification_rejected: bool,
    pub(super) verification_browsing: bool,
    pub(super) filter_edits: usize,
    pub(super) filter_reveals: usize,
    pub(super) filter_copies: usize,
    pub(super) filter_hover_requests: usize,
    pub(super) media_external: usize,
    pub(super) arrow_checked: bool,
    pub(super) input_bytes: Vec<Vec<u8>>,
    pub(super) expected_input: Vec<Vec<u8>>,
    pub(super) expected_enter_count: usize,
    pub(super) enter_count: usize,
    pub(super) closed_checked: bool,
    pub(super) probe_started: bool,
    pub(super) copy_checked: bool,
    pub(super) composition_checked: bool,
    pub(super) link_requests: usize,
    pub(super) font_sources: Vec<String>,
    pub(super) font_catalog_reads: usize,
    pub(super) export_checked: bool,
    pub(super) rows_export_checked: bool,
    pub(super) settings_saved: bool,
    pub(super) navigation_requests: usize,
    pub(super) paste_single: bool,
    pub(super) paste_confirm: bool,
    pub(super) paste_cancel: bool,
    pub(super) paste_rejected: usize,
    pub(super) copied_selection: Option<(u64, u64, u64)>,
}
impl Checks {
    fn mouse_reports(&self) -> Vec<(pttstella::mouse::Button, pttstella::mouse::Action)> {
        self.mouse_edges
            .iter()
            .map(|edge| (edge.button, edge.action))
            .collect()
    }
    pub(super) fn mouse_observations(&self) -> Value {
        serde_json::json!({"reports":self.mouse_reports(),"edges":self.mouse_edges,
            "samples":self.mouse_samples,"sample_overflow":self.mouse_sample_overflow,
            "return_after_redraw":self.mouse_return_painted,
            "cancels":self.mouse_cancels,"copies":self.mouse_copies,
            "toggles":self.mouse_toggles,"navigation_requests":self.navigation_requests})
    }
    pub(super) fn mouse_samples_passed(&self, case: &str) -> bool {
        use pttstella::mouse::{
            Action::{Press, Release},
            Button::Left,
            Modifiers, WheelDirection,
            input::SampleReport,
        };
        if self.mouse_samples.is_empty()
            || self.mouse_sample_overflow
            || self.mouse_cancels != 0
            || !self.mouse_toggles.is_empty()
            || self.navigation_requests != 0
            || self.link_requests != 0
            || !self.export_checked
        {
            return false;
        }
        if case == "mouse_wheel" {
            return self.mouse_edges.is_empty()
                && self.mouse_samples.iter().all(|sample|matches!(sample,SampleReport::Wheel(r) if r.col==4 && r.row==4 && r.modifiers==Modifiers::default()))
                && self.mouse_samples.iter().any(|sample|matches!(sample,SampleReport::Wheel(r) if r.direction==WheelDirection::Up))
                && self.mouse_samples.iter().any(|sample|matches!(sample,SampleReport::Wheel(r) if r.direction==WheelDirection::Down));
        }
        let button = match case {
            "mouse_drag" | "mouse_round_trip" => Some(Left),
            "mouse_motion" => None,
            _ => return false,
        };
        if case == "mouse_round_trip" {
            let in_a = |col, row| (8..=23).contains(&col) && (7..=11).contains(&row);
            return self.mouse_return_painted.is_some()
                && self.mouse_reports()==[(Left,Press),(Left,Release)]
                && self.mouse_edges.iter().all(|edge| in_a(edge.col,edge.row) && edge.modifiers==Modifiers::default())
                && self.mouse_samples.iter().all(|sample| matches!(sample,SampleReport::Motion(r) if (4..=67).contains(&r.col) && (5..=13).contains(&r.row) && r.button==Some(Left) && r.modifiers==Modifiers::default()))
                && self.mouse_samples.iter().position(|sample| matches!(sample,SampleReport::Motion(r) if (48..=63).contains(&r.col) && (7..=11).contains(&r.row)))
                    .is_some_and(|at| self.mouse_samples[at+1..].iter().any(|sample| matches!(sample,SampleReport::Motion(r) if in_a(r.col,r.row))))
                && matches!(self.mouse_samples.last(),Some(SampleReport::Motion(r)) if in_a(r.col,r.row));
        }
        self.mouse_reports()==[(Left,Press),(Left,Release)]
            && self.mouse_samples.iter().all(|sample|matches!(sample,SampleReport::Motion(r) if (4..=10).contains(&r.col) && r.row==4 && r.button==button && r.modifiers==Modifiers::default()))
            && self.mouse_samples.iter().any(|sample|matches!(sample,SampleReport::Motion(r) if r.col==10))
    }
    pub(super) fn mouse_passed(&self, boundary: bool) -> bool {
        use pttstella::mouse::{
            Action::{Press, Release},
            Button::{Left, Middle, Right},
        };
        if boundary {
            return self.mouse_reports() == [(Left, Press)]
                && self.mouse_cancels >= 1
                && self.mouse_toggles.is_empty()
                && self.navigation_requests == 0
                && self.link_requests == 0
                && self.export_checked;
        }
        self.mouse_reports()
            == [
                (Left, Press),
                (Left, Release),
                (Right, Press),
                (Right, Release),
                (Middle, Press),
            ]
            && self.mouse_toggles == [false, true]
            && self.mouse_cancels >= 1
            && self.mouse_copies >= 1
            && self.link_requests == 0
            && self.navigation_requests == 1
            && self.export_checked
    }
    pub(super) fn bookmarks_passed(
        &self,
        case: &str,
        initial: &super::bookmarks::Snapshot,
        final_state: &super::bookmarks::Snapshot,
        destination: &pttstella::navigation_job::Progress,
    ) -> bool {
        if self.bookmark_opens == 0 {
            return false;
        }
        match case {
            "bookmarks" => {
                initial.entries.is_empty()
                    && final_state.entries.len() == 1
                    && self.bookmark_saves >= 4
                    && self.bookmark_rejected >= 1
                    && final_state.entries[0].target().board().as_str() == "Stella"
                    && final_state.entries[0].target().aid().as_str() == "00000101"
                    && final_state.entries[0].label() == Some("自製更新")
                    && matches!(destination,pttstella::navigation_job::Progress::Reached {target:pttstella::navigation_job::Destination::Article(target),..} if target.board().as_str()=="Stella" && target.aid().as_str()=="00000101")
            }
            "bookmarks_restart" => {
                initial.entries.len() == 1
                    && final_state.entries.is_empty()
                    && self.bookmark_saves >= 1
            }
            "bookmarks_empty" => {
                initial.entries.is_empty()
                    && final_state.entries.is_empty()
                    && self.bookmark_saves == 0
                    && final_state.writable
            }
            "bookmarks_corrupt" => {
                initial.entries.is_empty()
                    && final_state.entries.is_empty()
                    && self.bookmark_saves == 0
                    && !initial.writable
                    && !final_state.writable
            }
            _ => false,
        }
    }
    pub(super) fn destination_rejected(&mut self, case: Option<&str>) {
        match case {
            Some("boards" | "boards_close") => {
                self.board_rejected = self.board_rejected.saturating_add(1)
            }
            Some("articles" | "articles_close") => {
                self.article_rejected = self.article_rejected.saturating_add(1)
            }
            _ => {}
        }
    }
    pub(super) fn new() -> Self {
        Self {
            expected_input: vec![
                vec![97],
                vec![98],
                vec![99],
                vec![0xa4, 0xa4],
                vec![0xa4, 0xa4],
                vec![110],
            ],
            expected_enter_count: 3,
            ..Self::default()
        }
    }
    pub(super) fn interactions_complete(&self, complete: bool) -> bool {
        self.arrow_checked
            && self.enter_count == self.expected_enter_count
            && self.input_bytes == self.expected_input
            && complete
            && self.closed_checked
            && self.copy_checked
            && self.composition_checked
            && self.link_requests == 1
            && self.export_checked
            && self.rows_export_checked
            && self.settings_saved
            && self.navigation_requests == 8
            && self.paste_single
            && self.paste_confirm
            && self.paste_cancel
            && self.paste_rejected == 3
    }
    pub(super) fn augment_report(&self, report: &mut Value, complete: bool) {
        report["arrow_input"] = self.arrow_checked.into();
        report["enter_input"] = (self.enter_count == self.expected_enter_count).into();
        report["text_input"] = (self.input_bytes == self.expected_input).into();
        report["trusted_webview_input"] = (self.arrow_checked
            && self.enter_count == self.expected_enter_count
            && self.input_bytes == self.expected_input)
            .into();
        report["loopback_input_roundtrip"] = complete.into();
        report["closed_after_roundtrip"] = self.closed_checked.into();
        report["selection_copy"] = self.copy_checked.into();
        report["ime_composition"] = self.composition_checked.into();
        report["url_open"] = (self.link_requests == 1).into();
        report["png_export"] = self.export_checked.into();
        report["rows_png_export"] = self.rows_export_checked.into();
        report["settings_saved"] = self.settings_saved.into();
        report["mouse_navigation"] = (self.navigation_requests == 8).into();
        report["paste_single"] = self.paste_single.into();
        report["paste_confirm"] = self.paste_confirm.into();
        report["paste_cancel"] = self.paste_cancel.into();
        report["paste_rejected"] = (self.paste_rejected == 3).into();
    }
}

pub(super) fn pixels_report(
    pixels: ProbePixels,
    path: &Path,
    reload_settings: bool,
    settings: &AppSettings,
) -> Result<Value, String> {
    let ProbePixels {
        png,
        red_pixels,
        green_pixels,
        left_bg,
        right_bg,
        fonts,
        dpr,
        pixel_size,
        font_stack,
        fills_body,
        colour_samples,
        art_seam_pixels,
        art_zoom_seams,
        art_geometry,
        hover_controls,
        proportional_zoom,
        zoom_samples,
    } = pixels;
    let expected: Vec<Vec<u8>> = [
        [0, 0, 0],
        [128, 0, 0],
        [0, 128, 0],
        [128, 128, 0],
        [0, 0, 128],
        [128, 0, 128],
        [0, 128, 128],
        [192, 192, 192],
        [128, 128, 128],
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [0, 0, 255],
        [255, 0, 255],
        [0, 255, 255],
        [255, 255, 255],
        [128, 0, 0],
        [255, 0, 0],
        [128, 0, 0],
        [192, 192, 192],
    ]
    .into_iter()
    .map(Vec::from)
    .collect();
    let ansi_colours = colour_samples == expected;
    let valid = red_pixels > 5
        && green_pixels > 5
        && left_bg == [0, 0, 128, 255]
        && right_bg == [128, 0, 128, 255]
        && fonts == "loaded"
        && fills_body
        && ansi_colours
        && art_seam_pixels == 0
        && art_zoom_seams == [0, 0, 0]
        && art_geometry
        && hover_controls
        && proportional_zoom;
    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
    let encoded = png
        .strip_prefix("data:image/png;base64,")
        .ok_or("invalid PNG response")?;
    if encoded.len() > 4 * 1024 * 1024 {
        return Err("probe image exceeds limit".into());
    }
    let pixels = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| e.to_string())?;
    std::fs::write(
        path.join(if reload_settings {
            "reloaded-screen.png"
        } else {
            "screen.png"
        }),
        pixels,
    )
    .map_err(|e| e.to_string())?;
    let report = serde_json::json!({"status":if valid {"passed"} else {"failed"},
    "scope":"mounted WebView canvas and input bridge; synthetic data only",
    "red_pixels":red_pixels,"green_pixels":green_pixels,"left_bg":left_bg,"right_bg":right_bg,
    "fonts":fonts,
    "dpr":dpr,"pixel_size":pixel_size,"font_stack":font_stack,"grid":[80,24],
    "fills_body":fills_body,"ansi_colours":ansi_colours,"colour_samples":colour_samples,
    "art_seam_pixels":art_seam_pixels,
    "art_zoom_seams":art_zoom_seams,"art_geometry":art_geometry,
    "hover_controls":hover_controls,"proportional_zoom":proportional_zoom,"zoom_samples":zoom_samples,
    "settings_loaded":settings,
    "font_fallback_per_glyph":"not measured"});
    Ok(report)
}

pub(super) fn preview(probe: bool, media_origin: Option<&str>) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let text = "PTTStella\r\n\r\n尚未連線，歡迎使用 PTTStella。";
    bytes.extend_from_slice(&encoding::encode(text).map_err(|e| e.to_string())?);
    bytes.extend_from_slice(b"\x1b[4;1H\x1b[31;44m\xa4\x1b[32;45m\xa4\x1b[0m  ");
    bytes.extend_from_slice(&encoding::encode("雙色字顯示預覽").map_err(|e| e.to_string())?);
    bytes.extend_from_slice(b"\x1b[7;1H");
    bytes.extend_from_slice(&encoding::encode("正在連線 PTT…\r\n\r\n連線後，點一下畫面即可輸入帳號密碼。\r\n滑鼠移至上緣可顯示連線與等比例縮放工具。").map_err(|e| e.to_string())?);
    if probe {
        if let Some(origin) = media_origin {
            for (row, path) in [
                (1, "hover.png"),
                (2, "hover-slow.png"),
                (5, "colours.jpg"),
                (6, "colours.webp"),
                (8, "colours.png"),
                (9, "motion.gif"),
                (10, "movie.webm"),
                (11, "broken.png"),
                (14, "slow.png"),
            ] {
                bytes.extend_from_slice(format!("\x1b[{row};1H\x1b[2K{origin}/{path} ").as_bytes());
            }
            bytes.extend_from_slice(b"\x1b[18;1Hhttps://youtu.be/Abc_12-dEF0 ");
            bytes.extend_from_slice(format!("\x1b[23;45H{origin}/hover.png ").as_bytes());
        }
        let solid = encoding::encode("█").map_err(|e| e.to_string())?;
        bytes.extend_from_slice(b"\x1b[12;1Hhttps://example.com/pttstella");
        bytes.extend_from_slice(b"\x1b[13;1H");
        bytes.extend_from_slice(&encoding::encode("●○◎⊙").map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[15;1H");
        bytes.extend_from_slice(&encoding::encode("一般色 / 亮色").map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[16;1H");
        for colour in 0..16 {
            let fg = if colour < 8 {
                30 + colour
            } else {
                90 + colour - 8
            };
            let bg = if colour == 0 { 47 } else { 40 };
            bytes.extend_from_slice(format!("\x1b[0;{fg};{bg}m").as_bytes());
            bytes.extend_from_slice(&solid);
        }
        // Also exercise intensity-on, intensity-off and full reset independently.
        bytes.extend_from_slice(b"\x1b[17;1H");
        for sgr in ["0;31", "1", "22", "0"] {
            bytes.extend_from_slice(format!("\x1b[{sgr}m").as_bytes());
            bytes.extend_from_slice(&solid);
        }
        bytes.extend_from_slice(b"\x1b[0;31;44m\x1b[20;1H");
        bytes.extend_from_slice(&encoding::encode(&"█".repeat(8)).map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[21;1H");
        bytes.extend_from_slice(&encoding::encode(&"█".repeat(8)).map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[22;1H");
        bytes.extend_from_slice(&encoding::encode("▄▌◢◣◤◥").map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[23;1H");
        bytes.extend_from_slice(&encoding::encode("▁▂▃▄▅▆▇█").map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[0m\x1b[19;1H");
    }
    Ok(bytes)
}

pub(super) const OSC8_URL: &str = "https://example.com/pttstella?complete=one-two&tail=both";

pub(super) fn surface_sample(
    case: Option<&str>,
    probe: bool,
    media_origin: Option<&str>,
) -> Result<Vec<u8>, String> {
    match case {
        Some("sgr66") => sgr66_sample(true),
        Some("sgr66_legacy") => sgr66_sample(false),
        Some("osc8") => osc8_sample(media_origin),
        _ => preview(probe, media_origin),
    }
}

pub(super) fn osc8_sample(media_origin: Option<&str>) -> Result<Vec<u8>, String> {
    let mut bytes = encoding::encode("\x1b[2J\x1b[HOSC 8 synthetic wrapped destinations\x1b[4;4HClick each fragment; right-click and copy the complete destination.").map_err(|e|e.to_string())?;
    for (row, label) in [
        (9, "https://example.com/pttstella?complete="),
        (10, "one-two&tail=both"),
    ] {
        bytes.extend_from_slice(format!("\x1b[{row};4H\x1b[36m\x1b]8;id=wrapped;{OSC8_URL}\x1b\\{label}\x1b]8;;\x1b\\\x1b[0m").as_bytes());
    }
    if let Some(origin) = media_origin {
        for (row, label) in [
            (15, "Local image: first fragment"),
            (16, "image: second fragment"),
        ] {
            bytes.extend_from_slice(format!("\x1b[{row};4H\x1b[32m\x1b]8;;{origin}/colours.png?complete=osc8\x07{label}\x1b]8;;\x07\x1b[0m").as_bytes());
        }
    }
    bytes.extend_from_slice(b"\x1b[24;80H\x1b[?25l");
    Ok(bytes)
}

/// Equivalent self-authored streams for separate native pixel captures.
pub(super) fn sgr66_sample(modern: bool) -> Result<Vec<u8>, String> {
    let mut bytes = preview(true, None)?;
    for (row, legacy, modern_bytes) in [
        (
            4,
            b"\x1b[0;31;44m\xa4\x1b[32;45m\xa4".as_slice(),
            b"\x1b[0;31;44;66;32;45m\xa4\xa4".as_slice(),
        ),
        (
            18,
            b"\x1b[0;31;44mA\x1b[32;45mB",
            b"\x1b[0;31;44;66;32;45mAB",
        ),
        (
            19,
            b"\x1b[1;7;31;44m\xa4\x1b[22;27;32;45m\xa4",
            b"\x1b[1;7;31;44;66;22;27;32;45m\xa4\xa4",
        ),
    ] {
        bytes.extend_from_slice(format!("\x1b[{row};1H").as_bytes());
        bytes.extend_from_slice(if modern { modern_bytes } else { legacy });
        bytes.extend_from_slice(b"\x1b[0m");
    }
    bytes.extend_from_slice(b"\x1b[24;80H\x1b[?25l");
    Ok(bytes)
}

#[cfg(test)]
mod sample_tests {
    use super::*;
    #[test]
    fn round_trip_requires_regions_in_order_painted_return_and_both_edges() {
        use pttstella::mouse::{
            Action, Button, Modifiers, MotionReport, Report, input::SampleReport,
        };
        let mut checks = Checks {
            export_checked: true,
            ..Checks::default()
        };
        checks.mouse_edges = vec![
            Report {
                col: 9,
                row: 8,
                button: Button::Left,
                action: Action::Press,
                modifiers: Modifiers::default(),
            },
            Report {
                col: 22,
                row: 10,
                button: Button::Left,
                action: Action::Release,
                modifiers: Modifiers::default(),
            },
        ];
        let motion = |col, row| {
            SampleReport::Motion(MotionReport {
                col,
                row,
                button: Some(Button::Left),
                modifiers: Modifiers::default(),
            })
        };
        checks.mouse_samples = vec![motion(30, 6), motion(63, 11), motion(23, 11)];
        assert!(!checks.mouse_samples_passed("mouse_round_trip")); // no painted return
        checks.mouse_return_painted = Some(42);
        assert!(checks.mouse_samples_passed("mouse_round_trip"));
        checks.mouse_samples.pop();
        assert!(!checks.mouse_samples_passed("mouse_round_trip")); // release alone is insufficient
        checks.mouse_samples = vec![motion(23, 11), motion(63, 11)];
        assert!(!checks.mouse_samples_passed("mouse_round_trip")); // reversed order
        checks.mouse_samples = vec![motion(63, 11), motion(23, 11)];
        checks.mouse_edges[1].col = 48;
        assert!(!checks.mouse_samples_passed("mouse_round_trip")); // release outside A
        checks.mouse_edges[1].col = 22;
        checks.mouse_cancels = 1;
        assert!(!checks.mouse_samples_passed("mouse_round_trip"));
        checks.mouse_cancels = 0;
        checks.mouse_sample_overflow = true;
        assert!(!checks.mouse_samples_passed("mouse_round_trip"));
    }
    #[test]
    fn mouse_observations_preserve_grid_coordinates_and_modifiers_in_failed_reports() {
        use pttstella::mouse::{Action, Button, Modifiers, Report};
        let mut checks = Checks::default();
        checks.mouse_edges.push(Report {
            col: 17,
            row: 8,
            button: Button::Left,
            action: Action::Press,
            modifiers: Modifiers {
                shift: false,
                alt: true,
                ctrl: false,
            },
        });
        let mut report = serde_json::json!({"status":"passed","mouse":checks.mouse_observations()});
        finish_report(&mut report, None);
        assert_eq!(report["status"], "failed");
        assert_eq!(
            report["mouse"],
            serde_json::json!({
                "reports":[["left","press"]],
                "edges":[{"col":17,"row":8,"button":"left","action":"press","modifiers":{"shift":false,"alt":true,"ctrl":false}}],
                "samples":[],"sample_overflow":false,"return_after_redraw":null,"cancels":0,"copies":0,"toggles":[],"navigation_requests":0
            })
        );
    }
    #[test]
    fn failed_cleanup_keeps_observations_and_cannot_claim_a_pass_or_join() {
        let mut report = serde_json::json!({"status":"passed","mouse":{"reports":[["left","press"]],"cancels":1},"listeners_released":true});
        finish_report(&mut report, None);
        assert_eq!(
            report,
            serde_json::json!({
                "status":"failed","mouse":{"reports":[["left","press"]],"cancels":1},
                "cleanup_completed":false,"listeners_released":null,
                "connection_worker_joined":null,"credential_worker_joined":null,"disconnect":null
            })
        );
    }
    #[test]
    fn acknowledged_cleanup_preserves_interaction_failure_and_other_outcomes_fail() {
        use pttstella::connection::CloseOutcome;
        let mut report = serde_json::json!({"status":"failed","case":"mouse_drag"});
        finish_report(&mut report, Some(CloseOutcome::Acknowledged));
        assert_eq!(
            report,
            serde_json::json!({
                "status":"failed","case":"mouse_drag","cleanup_completed":true,
                "listeners_released":true,"connection_worker_joined":true,
                "credential_worker_joined":true,"disconnect":"acknowledged"
            })
        );
        for outcome in [
            CloseOutcome::NoConnection,
            CloseOutcome::PeerClosedWithoutReply,
            CloseOutcome::Unconfirmed,
        ] {
            let mut report = serde_json::json!({"status":"passed"});
            finish_report(&mut report, Some(outcome));
            assert_eq!(report["status"], "failed");
            assert_eq!(report["cleanup_completed"], true);
        }
    }
    #[test]
    fn osc8_sample_has_independent_fragment_ranges_and_media_destinations() {
        let mut screen = pttstella::screen::Screen::new(80, 24).unwrap();
        screen.feed(&surface_sample(Some("osc8"), true, Some("http://127.0.0.1:9876")).unwrap());
        let links = pttstella::links::detect(&screen.frame());
        assert_eq!(
            links
                .iter()
                .map(|link| (link.row, link.start, link.end, link.url.as_str()))
                .collect::<Vec<_>>(),
            [
                (8, 3, 42, OSC8_URL),
                (9, 3, 20, OSC8_URL),
                (14, 3, 30, "http://127.0.0.1:9876/colours.png?complete=osc8"),
                (15, 3, 25, "http://127.0.0.1:9876/colours.png?complete=osc8"),
            ]
        );
        assert!(links[2..].iter().all(|link| {
            pttstella::media::classify(&link.url).unwrap().unwrap().kind
                == pttstella::media::Kind::Image
        }));
    }
    #[test]
    fn sgr66_native_samples_have_independent_cell_expectations_and_equal_glyphs() {
        let mut frames = Vec::new();
        for modern in [false, true] {
            let mut screen = pttstella::screen::Screen::new(80, 24).unwrap();
            screen.feed(&sgr66_sample(modern).unwrap());
            let left = screen.cell(0, 3).unwrap();
            let right = screen.cell(1, 3).unwrap();
            assert_eq!(
                (left.style.fg, left.style.bg, right.style.fg, right.style.bg),
                (1, 4, 2, 5)
            );
            assert_eq!(
                (
                    screen.cell(0, 17).unwrap().byte,
                    screen.cell(1, 17).unwrap().byte
                ),
                (b'A', b'B')
            );
            assert!(
                screen.cell(0, 18).unwrap().style.inverse
                    && screen.cell(0, 18).unwrap().style.bright
            );
            assert!(
                !screen.cell(1, 18).unwrap().style.inverse
                    && !screen.cell(1, 18).unwrap().style.bright
            );
            assert_eq!(screen.frame().unsupported, 0);
            frames.push(screen.frame());
        }
        assert_eq!(frames[0].glyphs, frames[1].glyphs);
    }
}
