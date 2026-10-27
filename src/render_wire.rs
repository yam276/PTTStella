//! Lossless renderer-only encoding. The screen and export representation stays
//! unchanged; runs group only contiguous glyphs with identical geometry/style.
use crate::screen::{Frame, Glyph, Style};
use serde::{Serialize, Serializer, ser::SerializeSeq};
use std::io::{self, Write};

pub const MAX_PACKET_BYTES: usize = 64 * 1024 * 1024;

/// Serialize directly, without serde_json::Value's per-field allocation tree.
/// Dioxus transports the result as a string, never as executable source.
pub fn packet_json(packet: &impl Serialize) -> Result<String, serde_json::Error> {
    packet_json_limit(packet, MAX_PACKET_BYTES)
}
fn packet_json_limit(packet: &impl Serialize, limit: usize) -> Result<String, serde_json::Error> {
    struct Bounded(Vec<u8>, usize);
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.1.saturating_sub(self.0.len()) {
                return Err(io::Error::other("renderer packet exceeds byte limit"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut bytes = Bounded(Vec::new(), limit);
    serde_json::to_writer(&mut bytes, packet)?;
    String::from_utf8(bytes.0).map_err(|error| serde_json::Error::io(io::Error::other(error)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_limit_counts_json_escaping_and_returns_no_partial_packet() {
        // Two quotes plus the six-byte escaped control character.
        assert_eq!(packet_json_limit(&"\0", 8).unwrap(), "\"\\u0000\"");
        assert!(packet_json_limit(&"\0", 7).unwrap_err().is_io());
        assert!(packet_json_limit(&serde_json::json!({"a":["safe","\0"]}), 12).is_err());
        assert_eq!(packet_json_limit(&"中文", 8).unwrap(), "\"中文\"");
        assert!(packet_json_limit(&"中文", 7).is_err());
    }
}

fn style(style: Style) -> u32 {
    u32::from(style.fg)
        | (u32::from(style.bg) << 8)
        | (u32::from(style.bright) << 16)
        | (u32::from(style.inverse) << 17)
}

struct Texts<'a>(&'a [Glyph]);
impl Serialize for Texts<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for glyph in self.0 {
            seq.serialize_element(&glyph.text)?;
        }
        seq.end()
    }
}
struct Runs<'a>(&'a [Glyph]);
impl Serialize for Runs<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        let mut start = 0;
        while start < self.0.len() {
            let first = &self.0[start];
            let mut end = start + 1;
            while let Some(next) = self.0.get(end) {
                let previous = &self.0[end - 1];
                if next.row != first.row
                    || next.col != previous.col + previous.width
                    || next.width != first.width
                    || next.left != first.left
                    || next.right != first.right
                    || next.unmapped != first.unmapped
                {
                    break;
                }
                end += 1;
            }
            seq.serialize_element(&(
                first.row,
                first.col,
                first.width,
                style(first.left),
                style(first.right),
                first.unmapped,
                Texts(&self.0[start..end]),
            ))?;
            start = end;
        }
        seq.end()
    }
}

#[derive(Serialize)]
pub struct CompactFrame<'a> {
    glyph_encoding: &'static str,
    control_complete: bool,
    cols: usize,
    rows: usize,
    cursor: (usize, usize),
    cursor_visible: bool,
    revision: u64,
    unsupported: u64,
    runs: Runs<'a>,
}
impl<'a> From<&'a Frame> for CompactFrame<'a> {
    fn from(frame: &'a Frame) -> Self {
        Self {
            glyph_encoding: "runs_v1",
            control_complete: frame.control_complete,
            cols: frame.cols,
            rows: frame.rows,
            cursor: frame.cursor,
            cursor_visible: frame.cursor_visible,
            revision: frame.revision,
            unsupported: frame.unsupported,
            runs: Runs(&frame.glyphs),
        }
    }
}
pub fn serialize_frame<S: Serializer>(frame: &Frame, serializer: S) -> Result<S::Ok, S::Error> {
    CompactFrame::from(frame).serialize(serializer)
}
pub fn serialize_optional_frame<S: Serializer>(
    frame: &Option<Frame>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    frame.as_ref().map(CompactFrame::from).serialize(serializer)
}
