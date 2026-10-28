use pttstella::{
    render_wire::{CompactFrame, packet_json},
    screen::{Frame, Glyph, Style},
};

#[test]
fn compact_runs_match_independent_dual_colour_unicode_and_escape_example() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/render-wire.json")).unwrap();
    let a = Style {
        fg: 1,
        bg: 4,
        bright: true,
        inverse: false,
    };
    let b = Style {
        fg: 2,
        bg: 5,
        bright: false,
        inverse: true,
    };
    let glyph = |row, col, width, text: &str, left, right, unmapped| Glyph {
        row,
        col,
        width,
        text: text.into(),
        left,
        right,
        unmapped,
    };
    let s = Style::default();
    let frame = Frame {
        control_complete: true,
        cols: 4,
        rows: 2,
        cursor: (3, 1),
        cursor_visible: true,
        revision: 9,
        unsupported: 3,
        hyperlinks: Vec::new(),
        glyphs: vec![
            glyph(0, 0, 1, "a", s, s, false),
            glyph(0, 1, 1, "\"\\\n", s, s, false),
            glyph(0, 2, 2, "中", a, b, true),
            glyph(1, 0, 1, "<img>", s, s, false),
            glyph(1, 1, 1, "e\u{301}", s, s, false),
        ],
    };
    assert_eq!(serde_json::to_value(&frame).unwrap(), expected["expanded"]);
    let encoded = packet_json(&CompactFrame::from(&frame)).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&encoded).unwrap(),
        expected["compact"]
    );
    // Screen/export serialization remains separate from the renderer codec.
    assert_eq!(serde_json::to_value(&frame).unwrap(), expected["expanded"]);
}

#[test]
fn runs_preserve_glyph_order_gaps_and_empty_frames() {
    let mut frame = pttstella::screen::Screen::new(4, 2).unwrap().frame();
    frame.glyphs.reverse();
    frame.glyphs.retain(|g| g.col != 2);
    let compact = serde_json::to_value(CompactFrame::from(&frame)).unwrap();
    assert_eq!(compact["runs"].as_array().unwrap().len(), 6);
    assert_eq!(compact["runs"][0][0], 1);
    assert_eq!(compact["runs"][0][1], 3);
    frame.glyphs.clear();
    assert_eq!(
        serde_json::to_value(CompactFrame::from(&frame)).unwrap()["runs"],
        serde_json::json!([])
    );
}
