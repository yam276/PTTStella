//! A human-sized synthetic gesture; independent of the production mouse encoder.
#[derive(Default)]
pub(super) struct Gesture {
    pressed: bool,
    started_a: bool,
    far: bool,
    returned: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Step {
    Continue,
    Ready,
    Redraw,
    Complete,
}
impl Gesture {
    pub(super) fn accept(&mut self, bytes: &[u8]) -> Result<Step, String> {
        let text = std::str::from_utf8(bytes).map_err(|_| "round trip requires SGR mouse bytes")?;
        let body = text
            .strip_prefix("\x1b[<")
            .ok_or("round trip requires SGR mouse bytes")?;
        if !body.is_ascii() || body.len() > 9 {
            return Err("invalid round trip SGR fields".into());
        }
        let action = body
            .as_bytes()
            .last()
            .copied()
            .ok_or("missing mouse action")?;
        let mut fields = body[..body.len() - 1].split(';');
        let mut number = || -> Option<usize> {
            let field = fields.next()?;
            if field.is_empty()
                || field.len() > 2
                || field.starts_with('0') && field != "0"
                || !field.bytes().all(|byte| byte.is_ascii_digit())
            {
                return None;
            }
            field.parse().ok()
        };
        let (Some(button), Some(col), Some(row)) = (number(), number(), number()) else {
            return Err("invalid round trip coordinates".into());
        };
        if fields.next().is_some() || !(5..=68).contains(&col) || !(6..=14).contains(&row) {
            return Err("round trip left its visible corridor".into());
        }
        let a = (9..=24).contains(&col) && (8..=12).contains(&row);
        let b = (49..=64).contains(&col) && (8..=12).contains(&row);
        if !self.pressed {
            if (button, action) != (0, b'M') {
                return Err("round trip must start with one left press in the corridor".into());
            }
            self.pressed = true;
            if a {
                self.started_a = true;
                return Ok(Step::Ready);
            }
        } else if (button, action) == (32, b'M') {
            if !self.started_a && b {
                return Err("round trip requires visiting A before B".into());
            }
            if !self.started_a && a {
                self.started_a = true;
                return Ok(Step::Ready);
            }
            if self.started_a && !self.far && b {
                self.far = true;
                return Ok(Step::Redraw);
            }
            self.returned = self.far && a;
        } else if (button, action, a, self.returned) == (0, b'm', true, true) {
            return Ok(Step::Complete);
        } else {
            return Err("round trip requires held motion A -> B -> A before its release".into());
        }
        Ok(Step::Continue)
    }
}

pub(super) const READY: &[u8] =
    b"\x1b[2;1H\x1b[2K\x1b[1;32mREADY: keep holding and move to B\x1b[0m\x1b[24;80H";

pub(super) const REDRAW: &[u8] = b"\x1b[2;1H\x1b[2K\x1b[1;32mHOLD: return to A, then release\x1b[0m\x1b[10;12H\x1b[30;43mA RETURN\x1b[0m\x1b[24;80H";

pub(in crate::connection) fn scene() -> Vec<u8> {
    let mut bytes = b"\x1b[2J\x1b[HHold LEFT and visit yellow A -> cyan B -> A; no modifiers\x1b[2;1HSTART: enter yellow A while holding; wait for READY\x1b[4;5HStay inside the dotted corridor; any point inside each colour is OK".to_vec();
    for row in 6..=14 {
        bytes.extend_from_slice(format!("\x1b[{row};5H\x1b[40;37m{}", ".".repeat(64)).as_bytes());
    }
    for row in 8..=12 {
        for (col, bg) in [(9, 43), (49, 46)] {
            bytes.extend_from_slice(
                format!("\x1b[{row};{col}H\x1b[30;{bg}m{}", " ".repeat(16)).as_bytes(),
            );
        }
    }
    bytes.extend_from_slice(
        b"\x1b[10;12H\x1b[30;43mA START\x1b[10;52H\x1b[30;46mB TURN\x1b[0m\x1b[24;80H",
    );
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mouse_regions_accept_coalesced_paths_and_different_points_within_targets() {
        let mut gesture = Gesture::default();
        for (bytes, expected) in [
            (b"\x1b[<0;9;8M".as_slice(), Step::Ready),
            (b"\x1b[<32;30;7M", Step::Continue),
            (b"\x1b[<32;64;12M", Step::Redraw),
            (b"\x1b[<32;51;10M", Step::Continue),
            (b"\x1b[<32;24;12M", Step::Continue),
            (b"\x1b[<0;10;9m", Step::Complete),
        ] {
            assert_eq!(gesture.accept(bytes).unwrap(), expected);
        }
    }
    #[test]
    fn mouse_regions_reject_missing_steps_foreign_buttons_modifiers_and_corridor_exits() {
        for sequence in [
            vec![
                b"\x1b[<0;50;9M".as_slice(),
                b"\x1b[<32;10;9M",
                b"\x1b[<0;10;9m",
            ], // starts in B
            vec![b"\x1b[<0;10;9M", b"\x1b[<0;10;9m"], // click only
            vec![b"\x1b[<0;10;9M", b"\x1b[<32;24;9M", b"\x1b[<0;10;9m"], // no B
            vec![b"\x1b[<0;10;9M", b"\x1b[<32;50;9M", b"\x1b[<0;10;9m"], // release cannot replace return motion
            vec![b"\x1b[<0;10;9M", b"\x1b[<0;50;9M"],                    // re-press
            vec![b"\x1b[<0;10;9M", b"\x1b[<35;50;9M"],                   // unheld
            vec![b"\x1b[<0;10;9M", b"\x1b[<36;50;9M"],                   // shift
            vec![b"\x1b[<0;10;9M", b"\x1b[<32;69;9M"],
            vec![b"\x1b[<0;10;9M", b"\x1b[<32;50;15M"],
            vec![
                b"\x1b[<0;10;9M",
                b"\x1b[<32;50;9M",
                b"\x1b[<32;10;9M",
                b"\x1b[<0;50;9m",
            ],
        ] {
            let mut gesture = Gesture::default();
            assert!(
                sequence.iter().any(|bytes| gesture.accept(bytes).is_err()),
                "{sequence:?}"
            );
        }
        for bytes in [
            b"\x1b[<".as_slice(),
            b"\x1b[<0;09;8M",
            b"\x1b[<0;9;8;1M",
            b"\x1b[<0;999;8M",
            "\x1b[<中".as_bytes(),
            b"\r",
        ] {
            assert!(Gesture::default().accept(bytes).is_err());
        }
    }
    #[test]
    fn mouse_corridor_press_needs_a_fresh_visit_to_a_before_b() {
        let mut gesture = Gesture::default();
        for (bytes, step) in [
            (b"\x1b[<0;8;9M".as_slice(), Step::Continue),
            (b"\x1b[<32;8;9M", Step::Continue),
            (b"\x1b[<32;10;9M", Step::Ready),
            (b"\x1b[<32;50;9M", Step::Redraw),
            (b"\x1b[<32;11;9M", Step::Continue),
            (b"\x1b[<0;11;9m", Step::Complete),
        ] {
            assert_eq!(gesture.accept(bytes).unwrap(), step);
        }
    }
}
