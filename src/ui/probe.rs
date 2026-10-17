//! Synthetic fixtures and verification counters, separate from product state.
use super::protocol::ProbePixels;
use base64::Engine;
use pttstella::{encoding, settings::AppSettings};
use serde_json::Value;
use std::path::Path;

#[derive(Default)]
pub(super) struct Checks {
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
