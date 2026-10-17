//! Station mouse negotiation and bounded SGR button encoding only.
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
        self.tracking == Tracking::Click && self.encoding == Encoding::Sgr
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
        Tracking::Click => {}
        _ => return Err(Error::UnsupportedTracking),
    }
    if modes.encoding != Encoding::Sgr {
        return Err(Error::UnsupportedEncoding);
    }
    // Same resource bound as Screen; decimal output is therefore bounded too.
    if cols == 0
        || rows == 0
        || cols
            .checked_mul(rows)
            .is_none_or(|cells| cells > crate::screen::MAX_CELLS)
        || report.col >= cols
        || report.row >= rows
    {
        return Err(Error::OutsideGrid);
    }
    let button = match report.button {
        Button::Left => 0,
        Button::Middle => 1,
        Button::Right => 2,
    } + u8::from(report.modifiers.shift) * 4
        + u8::from(report.modifiers.alt) * 8
        + u8::from(report.modifiers.ctrl) * 16;
    let final_byte = match report.action {
        Action::Press => 'M',
        Action::Release => 'm',
    };
    Ok(format!(
        "\x1b[<{button};{};{}{final_byte}",
        report.col + 1,
        report.row + 1
    )
    .into_bytes())
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
            assert_eq!(mode.supports_click_reports(), code == 1000);
            if code != 1000 {
                assert_eq!(
                    encode(mode, &report(), 80, 24),
                    Err(Error::UnsupportedTracking)
                );
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
}
