use pttstella::{
    screen::Screen,
    selection::{self, Position, Selection, Span},
};

#[test]
fn paste_line_endings_are_normalized_once_without_adding_enter() {
    use pttstella::input::prepare_paste;
    let single = prepare_paste("中").unwrap();
    assert_eq!(single.bytes, [0xa4, 0xa4]);
    assert_eq!(single.lines, 1);
    let multi = prepare_paste("A\r\nB\rC\n").unwrap();
    assert_eq!(multi.text, "A\nB\nC\n");
    assert_eq!(multi.bytes, b"A\rB\rC\r");
    assert_eq!(multi.lines, 4);
}

#[test]
fn paste_limits_count_encoded_bytes_and_reject_the_complete_payload() {
    use pttstella::input::{InputError, prepare_paste};
    assert_eq!(prepare_paste(&"中".repeat(2048)).unwrap().bytes.len(), 4096);
    assert_eq!(
        prepare_paste(&"中".repeat(2049)).unwrap_err(),
        InputError::TooLong
    );
    assert_eq!(
        prepare_paste("valid🚀").unwrap_err(),
        InputError::Unencodable
    );
}

#[test]
fn paste_rejects_control_sequences_but_preserves_tabs_and_text() {
    use pttstella::input::{InputError, prepare_paste};
    for ch in ['\0', '\x1b', '\x03', '\x7f', '\u{85}'] {
        assert_eq!(
            prepare_paste(&format!("A{ch}B")).unwrap_err(),
            InputError::Control(ch)
        );
    }
    assert_eq!(prepare_paste("A\tB").unwrap().bytes, b"A\tB");
    assert_eq!(prepare_paste("").unwrap_err(), InputError::Empty);
}

fn range(a: (usize, usize), b: (usize, usize)) -> Selection {
    Selection::Range {
        anchor: Position { col: a.0, row: a.1 },
        focus: Position { col: b.0, row: b.1 },
    }
}

#[test]
fn either_half_of_a_dual_colour_glyph_copies_the_complete_character_once() {
    let mut screen = Screen::new(4, 2).unwrap();
    screen.feed(b"\x1b[31m\xa4\x1b[32m\xa4!");
    for col in [0, 1] {
        let result = selection::extract(&screen.frame(), &range((col, 0), (col, 0))).unwrap();
        assert_eq!(result.text, "中");
        assert_eq!(
            result.spans,
            [Span {
                row: 0,
                start: 0,
                end: 2
            }]
        );
    }
}

#[test]
fn reversed_selection_retains_spaces_and_row_boundaries() {
    let mut screen = Screen::new(4, 2).unwrap();
    screen.feed(b" A B\r\n C");
    for request in [range((1, 0), (2, 1)), range((2, 1), (1, 0))] {
        assert_eq!(
            selection::extract(&screen.frame(), &request).unwrap().text,
            "A B\n C "
        );
    }
}

#[test]
fn line_and_all_selection_preserve_blank_cells_without_ansi_sequences() {
    let mut screen = Screen::new(4, 2).unwrap();
    screen.feed(b"\x1b[31mAB");
    assert_eq!(
        selection::extract(&screen.frame(), &Selection::Line { row: 0 })
            .unwrap()
            .text,
        "AB  "
    );
    assert_eq!(
        selection::extract(&screen.frame(), &Selection::All)
            .unwrap()
            .text,
        "AB  \n    "
    );
}

#[test]
fn unmapped_cells_remain_visible_in_copied_text() {
    let mut screen = Screen::new(2, 1).unwrap();
    screen.feed(b"A\x81");
    assert_eq!(
        selection::extract(&screen.frame(), &Selection::All)
            .unwrap()
            .text,
        "A\u{fffd}"
    );
}

#[test]
fn out_of_bounds_selection_is_rejected() {
    let screen = Screen::new(4, 2).unwrap();
    for request in [
        range((4, 0), (0, 0)),
        range((0, 2), (0, 0)),
        Selection::Line { row: usize::MAX },
    ] {
        assert!(selection::extract(&screen.frame(), &request).is_err());
    }
}
