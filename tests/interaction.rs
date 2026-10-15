use pttstella::{
    screen::Screen,
    selection::{self, Position, Selection, Span},
};

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
