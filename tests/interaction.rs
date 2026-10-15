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

#[test]
fn urls_use_glyph_columns_and_keep_balanced_parentheses() {
    use pttstella::{encoding, links};
    let mut screen = Screen::new(80, 2).unwrap();
    screen.feed(&encoding::encode("中 https://example.com/a(b)). ").unwrap());
    let links = links::detect(&screen.frame());
    assert_eq!(links.len(), 1);
    assert_eq!((links[0].row, links[0].start, links[0].end), (0, 3, 27));
    assert_eq!(links[0].url, "https://example.com/a(b)");
}

#[test]
fn urls_do_not_join_rows_or_accept_other_schemes() {
    use pttstella::links;
    let value = "https://example.com/";
    let mut screen = Screen::new(value.len(), 2).unwrap();
    screen.feed(value.as_bytes());
    assert!(links::detect(&screen.frame()).is_empty());
    for value in [
        "javascript:alert(1)",
        "file:///secret",
        "mailto:a@example.com",
        "https://",
    ] {
        assert!(links::validate(value).is_err());
    }
    assert!(links::validate("HTTPS://example.com/path").is_ok());
}

#[test]
fn url_detection_handles_case_ports_and_multiple_destinations() {
    use pttstella::links;
    let mut screen = Screen::new(80, 2).unwrap();
    screen.feed(b"HTTP://example.com:8080/a, https://[::1]/b! ");
    let detected = links::detect(&screen.frame());
    assert_eq!(
        detected
            .iter()
            .map(|link| link.url.as_str())
            .collect::<Vec<_>>(),
        ["http://example.com:8080/a", "https://[::1]/b"]
    );
}

#[test]
fn url_detection_does_not_open_a_destination_with_unmapped_cells() {
    let mut screen = Screen::new(80, 2).unwrap();
    screen.feed(b"https://example.com/\x81 ");
    assert!(pttstella::links::detect(&screen.frame()).is_empty());
}

#[test]
fn png_bridge_budget_and_geometry_are_checked_before_saving() {
    use pttstella::export::{MAX_PNG_BYTES, check_export_header};
    let mut header = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    header.extend(960u32.to_be_bytes());
    header.extend(576u32.to_be_bytes());
    header.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
    assert!(check_export_header(&header).is_ok());
    header[20..24].copy_from_slice(&575u32.to_be_bytes());
    assert!(check_export_header(&header).is_err());
    assert!(check_export_header(b"not an image").is_err());
    assert!(check_export_header(&vec![0; MAX_PNG_BYTES + 1]).is_err());
}

#[test]
fn display_preferences_validate_names_geometry_and_version() {
    use pttstella::settings::DisplaySettings;
    assert!(DisplaySettings::default().validate().is_ok());
    assert!(
        DisplaySettings {
            font_size: f64::NAN,
            ..DisplaySettings::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        DisplaySettings {
            baseline: 25.0,
            ..DisplaySettings::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        DisplaySettings {
            version: 2,
            ..DisplaySettings::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        DisplaySettings {
            ascii_font: "bad\nfont".into(),
            ..DisplaySettings::default()
        }
        .validate()
        .is_err()
    );
    assert_eq!(
        DisplaySettings {
            ascii_font: "  Mono Font  ".into(),
            ..DisplaySettings::default()
        }
        .validate()
        .unwrap()
        .ascii_font,
        "Mono Font"
    );
}
