//! Station mouse negotiation and bounded SGR pointer encoding.
//! The Session must still admit trusted, current, owned pointer transactions.
use serde::{Deserialize, Serialize};
pub mod input;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tracking {
    #[default]
    Off,
    X10,
    Click,
    Highlight,
    Drag,
    Motion,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoding {
    #[default]
    Legacy,
    Utf8,
    Sgr,
    Urxvt,
    Pixels,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Modes {
    tracking: Tracking,
    encoding: Encoding,
    epoch: u64,
}
impl Modes {
    pub fn tracking(self) -> Tracking {
        self.tracking
    }
    pub fn encoding(self) -> Encoding {
        self.encoding
    }
    /// Effective changes remain observable even if off/on ends in the same mode.
    pub fn epoch(self) -> u64 {
        self.epoch
    }
    /// Format capability, not input admission. Incomplete/synchronized output,
    /// old sessions and unowned releases must be rejected by the caller.
    pub fn supports_click_reports(self) -> bool {
        matches!(
            self.tracking,
            Tracking::Click | Tracking::Drag | Tracking::Motion
        ) && self.encoding == Encoding::Sgr
    }
    pub(crate) fn apply(&mut self, mode: usize, enabled: bool) -> bool {
        let tracking = match mode {
            9 => Some(Tracking::X10),
            1000 => Some(Tracking::Click),
            1001 => Some(Tracking::Highlight),
            1002 => Some(Tracking::Drag),
            1003 => Some(Tracking::Motion),
            _ => None,
        };
        let encoding = match mode {
            1005 => Some(Encoding::Utf8),
            1006 => Some(Encoding::Sgr),
            1015 => Some(Encoding::Urxvt),
            1016 => Some(Encoding::Pixels),
            _ => None,
        };
        let before = (self.tracking, self.encoding);
        if let Some(tracking) = tracking {
            self.tracking = if enabled { tracking } else { Tracking::Off };
        } else if let Some(encoding) = encoding {
            if enabled {
                self.encoding = encoding;
            } else if self.encoding == encoding {
                self.encoding = Encoding::Legacy;
            }
        } else {
            return false;
        }
        if before != (self.tracking, self.encoding) {
            self.epoch = self.epoch.saturating_add(1);
        }
        true
    }
    pub(crate) fn reset(&mut self) {
        if self.tracking != Tracking::Off || self.encoding != Encoding::Legacy {
            self.tracking = Tracking::Off;
            self.encoding = Encoding::Legacy;
            self.epoch = self.epoch.saturating_add(1);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    Left,
    Middle,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Press,
    Release,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modifiers {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Renderer grid coordinates, zero based. SGR conversion happens once here.
    pub col: usize,
    pub row: usize,
    pub button: Button,
    pub action: Action,
    pub modifiers: Modifiers,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotionReport {
    pub col: usize,
    pub row: usize,
    /// None is the 1003 no-button motion code, never a release.
    pub button: Option<Button>,
    pub modifiers: Modifiers,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WheelDirection {
    Up,
    Down,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WheelReport {
    pub col: usize,
    pub row: usize,
    pub direction: WheelDirection,
    pub modifiers: Modifiers,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NotNegotiated,
    UnsupportedTracking,
    UnsupportedEncoding,
    OutsideGrid,
}
pub fn encode(modes: Modes, report: &Report, cols: usize, rows: usize) -> Result<Vec<u8>, Error> {
    match modes.tracking {
        Tracking::Off => return Err(Error::NotNegotiated),
        Tracking::Click | Tracking::Drag | Tracking::Motion => {}
        _ => return Err(Error::UnsupportedTracking),
    }
    encode_sgr(
        modes,
        button_code(report.button),
        (report.col, report.row),
        report.modifiers,
        report.action == Action::Release,
        (cols, rows),
    )
}

/// Button motion is legal in 1002/1003; unheld motion only in 1003.
/// Deduplication, rate limits and held-button ownership belong to the Session.
pub fn encode_motion(
    modes: Modes,
    report: &MotionReport,
    cols: usize,
    rows: usize,
) -> Result<Vec<u8>, Error> {
    match (modes.tracking, report.button) {
        (Tracking::Off, _) => return Err(Error::NotNegotiated),
        (Tracking::Motion, _) | (Tracking::Drag, Some(_)) => {}
        _ => return Err(Error::UnsupportedTracking),
    }
    encode_sgr(
        modes,
        32 + report.button.map_or(3, button_code),
        (report.col, report.row),
        report.modifiers,
        false,
        (cols, rows),
    )
}

/// A wheel tick has no corresponding release edge in SGR tracking.
pub fn encode_wheel(
    modes: Modes,
    report: &WheelReport,
    cols: usize,
    rows: usize,
) -> Result<Vec<u8>, Error> {
    match modes.tracking {
        Tracking::Off => return Err(Error::NotNegotiated),
        Tracking::Click | Tracking::Drag | Tracking::Motion => {}
        _ => return Err(Error::UnsupportedTracking),
    }
    encode_sgr(
        modes,
        match report.direction {
            WheelDirection::Up => 64,
            WheelDirection::Down => 65,
        },
        (report.col, report.row),
        report.modifiers,
        false,
        (cols, rows),
    )
}

fn button_code(button: Button) -> u8 {
    match button {
        Button::Left => 0,
        Button::Middle => 1,
        Button::Right => 2,
    }
}

fn encode_sgr(
    modes: Modes,
    button: u8,
    (col, row): (usize, usize),
    modifiers: Modifiers,
    release: bool,
    (cols, rows): (usize, usize),
) -> Result<Vec<u8>, Error> {
    if modes.encoding != Encoding::Sgr {
        return Err(Error::UnsupportedEncoding);
    }
    // Same resource bound as Screen; decimal output is therefore bounded too.
    if cols == 0
        || rows == 0
        || cols
            .checked_mul(rows)
            .is_none_or(|cells| cells > crate::screen::MAX_CELLS)
        || col >= cols
        || row >= rows
    {
        return Err(Error::OutsideGrid);
    }
    let button = button
        + u8::from(modifiers.shift) * 4
        + u8::from(modifiers.alt) * 8
        + u8::from(modifiers.ctrl) * 16;
    let final_byte = if release { 'm' } else { 'M' };
    Ok(format!("\x1b[<{button};{};{}{final_byte}", col + 1, row + 1).into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::Screen;
    fn negotiated(sequence: &[u8]) -> Modes {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(sequence);
        screen.mouse_modes()
    }
    fn report() -> Report {
        Report {
            col: 0,
            row: 0,
            button: Button::Left,
            action: Action::Press,
            modifiers: Modifiers::default(),
        }
    }
    #[test]
    fn every_fragment_preserves_the_handwritten_mode_trace_and_reset_boundary() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/mouse-modes.json")).unwrap();
        let mut screen = Screen::new(80, 24).unwrap();
        for step in fixture["steps"].as_array().unwrap() {
            let input = step["input"].as_str().unwrap().as_bytes();
            let before = screen.clone();
            for cut in 0..=input.len() {
                let mut split = before.clone();
                split.feed(&input[..cut]);
                split.feed(&input[cut..]);
                assert_eq!(
                    serde_json::to_value(split.mouse_modes()).unwrap(),
                    step["state"],
                    "cut {cut}"
                );
            }
            // No mode applies before its final byte.
            screen.feed(&input[..input.len() - 1]);
            assert_eq!(screen.mouse_modes(), before.mouse_modes());
            screen.feed(&input[input.len() - 1..]);
            assert_eq!(
                serde_json::to_value(screen.mouse_modes()).unwrap(),
                step["state"]
            );
        }
        screen.reset_session();
        assert_eq!(screen.mouse_modes(), Modes::default());
        assert_eq!(screen.revision(), 0);
    }
    #[test]
    fn invalid_cancelled_strings_and_overlong_controls_do_not_change_negotiation() {
        let ready = negotiated(b"\x1b[?1000;1006h");
        let mut invalid = vec![
            b"\x1b[1006l".to_vec(),
            b"\x1b[>1006l".to_vec(),
            b"\x1b[?1006:1l".to_vec(),
            b"\x1b[?1006$l".to_vec(),
            b"\x1b[?1006\x18l".to_vec(),
            b"\x1b[?1000\x1al".to_vec(),
            b"\x1b]0;[?1000l\x07".to_vec(),
            b"\x1bP[?1006l\x1b\\".to_vec(),
            b"\x1b[?1004h".to_vec(),
        ];
        invalid.push([b"\x1b[?1006;".as_slice(), &[b'0'; 70], b"l"].concat());
        for input in invalid {
            let mut screen = Screen::new(80, 24).unwrap();
            screen.feed(b"\x1b[?1000;1006h");
            for byte in input {
                screen.feed(&[byte]);
                assert_eq!(screen.mouse_modes(), ready);
            }
        }
        // C0 executes inside CSI but does not cancel its valid parameters.
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(b"\x1b[?1000h\x1b[?100\n6h");
        assert_eq!(screen.mouse_modes(), ready);
    }
    #[test]
    fn all_tracking_and_coordinate_modes_are_retained_without_claiming_support() {
        for (code, tracking) in [
            (9, Tracking::X10),
            (1000, Tracking::Click),
            (1001, Tracking::Highlight),
            (1002, Tracking::Drag),
            (1003, Tracking::Motion),
        ] {
            let mode = negotiated(format!("\x1b[?1006;{code}h").as_bytes());
            assert_eq!(mode.tracking(), tracking);
            assert_eq!(
                mode.supports_click_reports(),
                matches!(code, 1000 | 1002 | 1003)
            );
            if matches!(code, 9 | 1001) {
                assert_eq!(
                    encode(mode, &report(), 80, 24),
                    Err(Error::UnsupportedTracking)
                );
            } else {
                assert_eq!(encode(mode, &report(), 80, 24).unwrap(), b"\x1b[<0;1;1M");
            }
        }
        for (code, encoding) in [
            (1005, Encoding::Utf8),
            (1006, Encoding::Sgr),
            (1015, Encoding::Urxvt),
            (1016, Encoding::Pixels),
        ] {
            let mode = negotiated(format!("\x1b[?1000;{code}h").as_bytes());
            assert_eq!(mode.encoding(), encoding);
            assert_eq!(mode.supports_click_reports(), code == 1006);
            if code != 1006 {
                assert_eq!(
                    encode(mode, &report(), 80, 24),
                    Err(Error::UnsupportedEncoding)
                );
            }
        }
    }
    #[test]
    fn sgr_buttons_and_one_based_coordinates_match_independent_bytes() {
        let modes = negotiated(b"\x1b[?1000;1006h");
        assert_eq!(encode(modes, &report(), 80, 24).unwrap(), b"\x1b[<0;1;1M");
        let mut event = report();
        event.col = 79;
        event.row = 23;
        event.action = Action::Release;
        assert_eq!(encode(modes, &event, 80, 24).unwrap(), b"\x1b[<0;80;24m");
        event.col = 10;
        event.row = 5;
        event.button = Button::Middle;
        event.modifiers = Modifiers {
            shift: true,
            alt: true,
            ctrl: true,
        };
        assert_eq!(encode(modes, &event, 80, 24).unwrap(), b"\x1b[<29;11;6m");
        event = report();
        event.button = Button::Right;
        event.modifiers = Modifiers {
            alt: true,
            ctrl: true,
            ..Default::default()
        };
        assert_eq!(encode(modes, &event, 80, 24).unwrap(), b"\x1b[<26;1;1M");
        for (bits, expected) in ["0", "4", "8", "12", "16", "20", "24", "28"]
            .iter()
            .enumerate()
        {
            event = report();
            event.modifiers = Modifiers {
                shift: bits & 1 != 0,
                alt: bits & 2 != 0,
                ctrl: bits & 4 != 0,
            };
            assert_eq!(
                encode(modes, &event, 80, 24).unwrap(),
                format!("\x1b[<{expected};1;1M").as_bytes()
            );
        }
    }
    #[test]
    fn absent_modes_and_bad_coordinates_never_produce_partial_bytes() {
        assert_eq!(
            encode(Modes::default(), &report(), 80, 24),
            Err(Error::NotNegotiated)
        );
        assert_eq!(
            encode(negotiated(b"\x1b[?1000h"), &report(), 80, 24),
            Err(Error::UnsupportedEncoding)
        );
        let modes = negotiated(b"\x1b[?1000;1006h");
        for (col, row, cols, rows) in [
            (80, 0, 80, 24),
            (0, 24, 80, 24),
            (usize::MAX, 0, 80, 24),
            (0, 0, 0, 24),
            (0, 0, 80, 0),
            (0, 0, usize::MAX, 2),
            (0, 0, 8001, 1),
        ] {
            let event = Report {
                col,
                row,
                ..report()
            };
            assert_eq!(encode(modes, &event, cols, rows), Err(Error::OutsideGrid));
        }
        let event = Report {
            col: 7999,
            ..report()
        };
        assert_eq!(encode(modes, &event, 8000, 1).unwrap(), b"\x1b[<0;8000;1M");
        assert!(serde_json::from_value::<Report>(serde_json::json!({"col":0,"row":0,"button":"left","action":"motion","modifiers":{"shift":false,"alt":false,"ctrl":false}})).is_err());
    }
    #[test]
    fn synchronized_output_exposes_mode_changes_separately_from_display_completion() {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(b"\x1b[?2026h\x1b[?1000;1006h");
        assert!(screen.synchronized_since().is_some());
        assert!(screen.mouse_modes().supports_click_reports());
        let before = screen.mouse_modes().epoch();
        screen.feed(b"\x1b[?1000l\x1b[?1000h");
        assert!(screen.mouse_modes().supports_click_reports());
        assert_eq!(screen.mouse_modes().epoch(), before + 2);
        screen.feed(b"\x1b[?2026l");
        assert!(screen.synchronized_since().is_none());
        assert_eq!(screen.mouse_modes().epoch(), before + 2);
    }

    #[test]
    fn motion_and_wheel_codes_match_independent_sgr_bytes_in_each_legal_mode() {
        let mut motion = MotionReport {
            col: 79,
            row: 23,
            button: None,
            modifiers: Modifiers::default(),
        };
        let mut wheel = WheelReport {
            col: 79,
            row: 23,
            direction: WheelDirection::Up,
            modifiers: Modifiers::default(),
        };
        for code in [1002, 1003] {
            let modes = negotiated(format!("\x1b[?{code};1006h").as_bytes());
            for (button, expected) in [
                (Button::Left, b"\x1b[<32;80;24M".as_slice()),
                (Button::Middle, b"\x1b[<33;80;24M"),
                (Button::Right, b"\x1b[<34;80;24M"),
            ] {
                motion.button = Some(button);
                assert_eq!(encode_motion(modes, &motion, 80, 24).unwrap(), expected);
            }
            motion.button = None;
            if code == 1003 {
                assert_eq!(
                    encode_motion(modes, &motion, 80, 24).unwrap(),
                    b"\x1b[<35;80;24M"
                );
            } else {
                assert_eq!(
                    encode_motion(modes, &motion, 80, 24),
                    Err(Error::UnsupportedTracking)
                );
            }
        }
        for code in [1000, 1002, 1003] {
            let modes = negotiated(format!("\x1b[?{code};1006h").as_bytes());
            wheel.direction = WheelDirection::Up;
            assert_eq!(
                encode_wheel(modes, &wheel, 80, 24).unwrap(),
                b"\x1b[<64;80;24M"
            );
            wheel.direction = WheelDirection::Down;
            assert_eq!(
                encode_wheel(modes, &wheel, 80, 24).unwrap(),
                b"\x1b[<65;80;24M"
            );
        }
    }

    #[test]
    fn every_modifier_combination_preserves_motion_and_wheel_bits() {
        let modes = negotiated(b"\x1b[?1003;1006h");
        // Handwritten protocol bytes, independent of the encoder's bit arithmetic.
        for (bits, (motion_code, hover_code, up_code, down_code)) in [
            ("32", "35", "64", "65"),
            ("36", "39", "68", "69"),
            ("40", "43", "72", "73"),
            ("44", "47", "76", "77"),
            ("48", "51", "80", "81"),
            ("52", "55", "84", "85"),
            ("56", "59", "88", "89"),
            ("60", "63", "92", "93"),
        ]
        .into_iter()
        .enumerate()
        {
            let modifiers = Modifiers {
                shift: bits & 1 != 0,
                alt: bits & 2 != 0,
                ctrl: bits & 4 != 0,
            };
            for (button, expected) in [(Some(Button::Left), motion_code), (None, hover_code)] {
                assert_eq!(
                    encode_motion(
                        modes,
                        &MotionReport {
                            col: 0,
                            row: 0,
                            button,
                            modifiers
                        },
                        80,
                        24
                    )
                    .unwrap(),
                    format!("\x1b[<{expected};1;1M").as_bytes()
                );
            }
            for (direction, expected) in [
                (WheelDirection::Up, up_code),
                (WheelDirection::Down, down_code),
            ] {
                assert_eq!(
                    encode_wheel(
                        modes,
                        &WheelReport {
                            col: 0,
                            row: 0,
                            direction,
                            modifiers
                        },
                        80,
                        24
                    )
                    .unwrap(),
                    format!("\x1b[<{expected};1;1M").as_bytes()
                );
            }
        }
    }

    #[test]
    fn continuous_reports_reject_unnegotiated_modes_and_invalid_grids() {
        let mut motion = MotionReport {
            col: 0,
            row: 0,
            button: Some(Button::Right),
            modifiers: Modifiers::default(),
        };
        let mut wheel = WheelReport {
            col: 0,
            row: 0,
            direction: WheelDirection::Down,
            modifiers: Modifiers::default(),
        };
        assert_eq!(
            encode_motion(Modes::default(), &motion, 80, 24),
            Err(Error::NotNegotiated)
        );
        assert_eq!(
            encode_wheel(Modes::default(), &wheel, 80, 24),
            Err(Error::NotNegotiated)
        );
        for code in [9, 1001] {
            let modes = negotiated(format!("\x1b[?{code};1006h").as_bytes());
            assert_eq!(
                encode_motion(modes, &motion, 80, 24),
                Err(Error::UnsupportedTracking)
            );
            assert_eq!(
                encode_wheel(modes, &wheel, 80, 24),
                Err(Error::UnsupportedTracking)
            );
        }
        assert_eq!(
            encode_motion(negotiated(b"\x1b[?1000;1006h"), &motion, 80, 24),
            Err(Error::UnsupportedTracking)
        );
        for sequence in [
            b"\x1b[?1003h".as_slice(),
            b"\x1b[?1003;1005h",
            b"\x1b[?1003;1015h",
            b"\x1b[?1003;1016h",
        ] {
            let modes = negotiated(sequence);
            assert_eq!(
                encode_motion(modes, &motion, 80, 24),
                Err(Error::UnsupportedEncoding)
            );
            assert_eq!(
                encode_wheel(modes, &wheel, 80, 24),
                Err(Error::UnsupportedEncoding)
            );
        }
        let modes = negotiated(b"\x1b[?1003;1006h");
        for (col, row, cols, rows) in [
            (80, 0, 80, 24),
            (0, 24, 80, 24),
            (usize::MAX, 0, 80, 24),
            (0, 0, 0, 24),
            (0, 0, 80, 0),
            (0, 0, usize::MAX, 2),
            (0, 0, 8001, 1),
        ] {
            motion.col = col;
            motion.row = row;
            wheel.col = col;
            wheel.row = row;
            assert_eq!(
                encode_motion(modes, &motion, cols, rows),
                Err(Error::OutsideGrid)
            );
            assert_eq!(
                encode_wheel(modes, &wheel, cols, rows),
                Err(Error::OutsideGrid)
            );
        }
        motion.col = 7999;
        motion.row = 0;
        wheel.col = 7999;
        wheel.row = 0;
        assert_eq!(
            encode_motion(modes, &motion, 8000, 1).unwrap(),
            b"\x1b[<34;8000;1M"
        );
        assert_eq!(
            encode_wheel(modes, &wheel, 8000, 1).unwrap(),
            b"\x1b[<65;8000;1M"
        );
        assert!(serde_json::from_value::<MotionReport>(serde_json::json!({"col":0,"row":0,"button":"wheel_up","modifiers":{"shift":false,"alt":false,"ctrl":false}})).is_err());
        assert!(serde_json::from_value::<WheelReport>(serde_json::json!({"col":0,"row":0,"direction":"left","modifiers":{"shift":false,"alt":false,"ctrl":false}})).is_err());
        assert!(serde_json::from_value::<WheelReport>(serde_json::json!({"col":0,"row":0,"direction":"up","release":true,"modifiers":{"shift":false,"alt":false,"ctrl":false}})).is_err());
    }
}
