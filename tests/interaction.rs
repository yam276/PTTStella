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

#[test]
fn ansi_clipboard_preserves_both_halves_styles_spaces_and_resets_each_line() {
    let mut source = Screen::new(5, 2).unwrap();
    source.feed(b"\x1b[1;31;44m\xa4\x1b[7;92;103m\xa4Z\r\n\x1b[0m A");
    let frame = source.frame();
    let selected = selection::extract(&frame, &Selection::All).unwrap();
    let ansi = selection::ansi(&frame, &selected.spans, selected.rows);
    assert!(ansi.contains(";66;"));
    assert!(ansi.starts_with("\x1b[0m"));
    assert!(ansi.ends_with("\x1b[0m"));
    let bytes = pttstella::encoding::encode(&ansi.replace('\n', "\r\n")).unwrap();
    let mut restored = Screen::new(5, 2).unwrap();
    restored.feed(&bytes);
    assert_eq!(restored.frame().glyphs, frame.glyphs);
    let partial = selection::extract(&frame, &range((1, 0), (1, 0))).unwrap();
    let ansi = selection::ansi(&frame, &partial.spans, partial.rows);
    assert!(ansi.contains('中'));
    assert!(!ansi.contains('Z'));
}

#[test]
fn editor_paste_accepts_only_bounded_sgr_and_types_insert_escape_not_terminal_commands() {
    use pttstella::{
        encoding::TextEncoding,
        input::{prepare_editor_paste, prepare_paste},
    };
    let text = "\x1b[1;31m中\x1b[m\r\nB";
    let prepared = prepare_editor_paste(text, TextEncoding::Uao).unwrap();
    assert_eq!(prepared.bytes, b"\x15[1;31m\xa4\xa4\x15[m\rB");
    assert_eq!(prepared.lines, 2);
    assert!(
        prepare_paste(text).is_err(),
        "ordinary paste must continue rejecting ESC"
    );
    for text in [
        "\x1b[2J",
        "\x1b[H",
        "\x1b]8;;https://example.com\x07",
        "\x1b[31mA\x18",
        "\x15[31m",
        "\x1b[?31m",
        "\x1b[38;5;1m",
        "\x1b[9999999999m",
        "\x1b[31",
        "\x1b[31\nm",
        "\x1b[31mA\x7f",
    ] {
        assert!(
            prepare_editor_paste(text, TextEncoding::Uao).is_err(),
            "{text:?}"
        );
    }
    assert!(
        prepare_editor_paste(&format!("\x1b[{}mA", "0;".repeat(33)), TextEncoding::Uao).is_err()
    );
    assert!(prepare_editor_paste(&"中".repeat(2049), TextEncoding::Uao).is_err());
}

#[test]
fn station_editor_recognition_requires_complete_ptt_footer_and_body_cursor() {
    use pttstella::navigation::article_editor;
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(
        &pttstella::encoding::encode(
            "\x1b[24;1H\x1b[30;47m編輯文章 ^X存檔 ^C色碼 ^V彩色 ^Q放棄\x1b[0m\x1b[3;1H",
        )
        .unwrap(),
    );
    assert!(article_editor(&screen.frame()));
    let mut bad = screen.frame();
    bad.cursor.1 = 23;
    assert!(!article_editor(&bad));
    screen.feed(b"\x1b[");
    assert!(!article_editor(&screen.frame()));
    for footer in [
        "編輯文章 ^X存檔",
        "文章中引用 ^X ^V 編輯文章",
        "編輯文章 ^V彩色",
    ] {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(
            &pttstella::encoding::encode(&format!("\x1b[24;1H\x1b[30;47m{footer}\x1b[0m\x1b[3;1H"))
                .unwrap(),
        );
        assert!(!article_editor(&screen.frame()));
    }
}

#[test]
fn station_editor_accepts_colour_code_help_without_listing_ctrl_v() {
    use pttstella::navigation::article_editor;
    for footer in [
        " (^X)存檔 (^C)色碼 (Esc-h)按鍵 (^Z)說明 插入 | user | 1:1",
        " ^X存檔 ^C色碼 ^Z說明 覆寫",
    ] {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(
            &pttstella::encoding::encode(&format!(
                "\x1b[24;1H\x1b[34;46m 編輯文章 \x1b[30;47m{footer}\x1b[0m\x1b[1;1H"
            ))
            .unwrap(),
        );
        assert!(article_editor(&screen.frame()));
    }
    for footer in [
        "編輯文章 ^X存檔 ^C複製",
        "編輯文章 ^C色碼 ^Z說明",
        "編輯文章 色碼 ^X存檔",
    ] {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(
            &pttstella::encoding::encode(&format!("\x1b[24;1H\x1b[30;47m{footer}\x1b[0m\x1b[1;1H"))
                .unwrap(),
        );
        assert!(!article_editor(&screen.frame()));
    }
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
        assert_eq!(result.rows, 0..1);
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
            selection::extract(&screen.frame(), &request).unwrap().rows,
            0..2
        );
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
    assert!(check_export_header(&header, &(0..24)).is_ok());
    header[20..24].copy_from_slice(&575u32.to_be_bytes());
    assert!(check_export_header(&header, &(0..24)).is_err());
    assert!(check_export_header(b"not an image", &(0..24)).is_err());
    assert!(check_export_header(&vec![0; MAX_PNG_BYTES + 1], &(0..24)).is_err());
    for rows in [0..1, 3..4, 19..23, 23..24] {
        header[20..24].copy_from_slice(&((rows.end - rows.start) as u32 * 24).to_be_bytes());
        assert!(check_export_header(&header, &rows).is_ok());
        assert!(check_export_header(&header, &(0..24)).is_err());
    }
    for rows in [0..0, 24..25, usize::MAX..usize::MAX] {
        assert!(check_export_header(&header, &rows).is_err());
    }
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

fn hyperlink(uri: &str, label: &str, terminator: &str) -> Vec<u8> {
    let mut bytes = format!("\x1b]8;;{uri}{terminator}").into_bytes();
    bytes.extend(pttstella::encoding::encode(label).unwrap());
    bytes.extend(format!("\x1b]8;;{terminator}").as_bytes());
    bytes
}

#[test]
fn osc8_wrapping_keeps_the_complete_target_for_click_and_copy_across_all_splits() {
    use pttstella::links::{self, Link};
    let uri = "https://example.com/a?complete=destination&tail=1";
    let expected = vec![
        Link {
            row: 0,
            start: 0,
            end: 10,
            url: uri.into(),
        },
        Link {
            row: 1,
            start: 0,
            end: 3,
            url: uri.into(),
        },
    ];
    for ending in ["\x07", "\x1b\\"] {
        let bytes = hyperlink(uri, "ABCDEFGH中文Z", ending);
        for split in 0..=bytes.len() {
            let mut screen = Screen::new(10, 3).unwrap();
            screen.feed(&bytes[..split]);
            screen.feed(&bytes[split..]);
            assert_eq!(screen.cursor(), (3, 1));
            assert_eq!(links::detect(&screen.frame()), expected);
            assert_eq!(
                pttstella::text::rows(&screen.frame())
                    .unwrap()
                    .iter()
                    .map(|row| row.text.as_str())
                    .collect::<Vec<_>>(),
                ["ABCDEFGH中", "文Z       ", "          "]
            );
            for index in 0..2 {
                let selected =
                    selection::extract(&screen.frame(), &Selection::Link { index }).unwrap();
                assert_eq!(selected.text, uri);
                assert_eq!(selected.rows, index..index + 1);
            }
            assert!(selection::extract(&screen.frame(), &Selection::Link { index: 2 }).is_err());
        }
        let mut screen = Screen::new(10, 3).unwrap();
        for byte in bytes {
            screen.feed(&[byte]);
        }
        assert_eq!(links::detect(&screen.frame()), expected);
    }
}

#[test]
fn osc8_replaces_targets_without_duplicate_guesses_or_exposing_invalid_schemes() {
    use pttstella::links;
    let mut screen = Screen::new(80, 3).unwrap();
    screen.feed(b"\x1b]8;id=first;https://example.com/actual\x07https://example.com/shown \x1b]8;;https://example.org/next\x1b\\NEXT\x1b]8;;\x07 https://example.net/plain ");
    let found = links::detect(&screen.frame());
    assert_eq!(
        found.iter().map(|l| l.url.as_str()).collect::<Vec<_>>(),
        [
            "https://example.com/actual",
            "https://example.org/next",
            "https://example.net/plain"
        ]
    );
    assert_eq!((found[0].start, found[0].end), (0, 26));
    for uri in [
        "javascript:alert(1)",
        "file:///secret",
        "https://example.com/a\nb",
        "https://example.com/a b",
    ] {
        let mut screen = Screen::new(80, 2).unwrap();
        screen.feed(&hyperlink(uri, "https://example.com/shown ", "\x07"));
        assert!(links::detect(&screen.frame()).is_empty(), "{uri:?}");
        assert!(
            !serde_json::to_string(&screen.frame())
                .unwrap()
                .contains(uri)
        );
    }
    for bad in [
        "https://example.com/a\nb",
        "https://example.com/a\tb",
        "https://example.com/\u{85}",
    ] {
        assert!(links::validate(bad).is_err());
    }
}

#[test]
fn osc8_attributes_follow_cell_edits_and_never_attach_to_erased_cells() {
    use pttstella::links;
    let mut screen = Screen::new(8, 3).unwrap();
    screen.feed(&hyperlink("https://example.com/a", "ABC", "\x07"));
    screen.feed(b"\x1b[1;2H\x1b[@");
    let found = links::detect(&screen.frame());
    assert_eq!(
        found
            .iter()
            .map(|l| (l.row, l.start, l.end))
            .collect::<Vec<_>>(),
        [(0, 0, 1), (0, 2, 4)]
    );
    screen.feed(b"\x1b[P");
    assert_eq!(
        links::detect(&screen.frame())
            .iter()
            .map(|l| (l.start, l.end))
            .collect::<Vec<_>>(),
        [(0, 3)]
    );
    screen.feed(b"\x1b[X");
    assert_eq!(
        links::detect(&screen.frame())
            .iter()
            .map(|l| (l.start, l.end))
            .collect::<Vec<_>>(),
        [(0, 1), (2, 3)]
    );
    screen.feed(b"\x1b[K");
    assert_eq!(
        links::detect(&screen.frame())
            .iter()
            .map(|l| (l.start, l.end))
            .collect::<Vec<_>>(),
        [(0, 1)]
    );
    screen.feed(b"\x1b[2K");
    assert!(links::detect(&screen.frame()).is_empty());
    screen.feed(b"\x1b]8;;https://example.com/b\x07\x1b[2J");
    assert!(links::detect(&screen.frame()).is_empty());
    screen.feed(b"X");
    assert_eq!(
        links::detect(&screen.frame())[0].url,
        "https://example.com/b"
    );
    screen.reset_session();
    screen.feed(b"RESET");
    assert!(screen.frame().hyperlinks.is_empty());
}

#[test]
fn osc8_scroll_regions_overwrites_and_mixed_double_width_cells_have_no_stale_targets() {
    use pttstella::links;
    let mut screen = Screen::new(4, 3).unwrap();
    for (row, name) in [(1, "a"), (2, "b"), (3, "c")] {
        screen.feed(format!("\x1b[{row};1H").as_bytes());
        screen.feed(&hyperlink(
            &format!("https://example.com/{name}"),
            name,
            "\x07",
        ));
    }
    screen.feed(b"\x1b[S");
    assert_eq!(
        links::detect(&screen.frame())
            .iter()
            .map(|l| (l.row, l.url.as_str()))
            .collect::<Vec<_>>(),
        [(0, "https://example.com/b"), (1, "https://example.com/c")]
    );
    screen.feed(b"\x1b[2;3r\x1b[T");
    assert_eq!(
        links::detect(&screen.frame())
            .iter()
            .map(|l| (l.row, l.url.as_str()))
            .collect::<Vec<_>>(),
        [(0, "https://example.com/b"), (2, "https://example.com/c")]
    );
    screen.feed(b"\x1b[3;1HX");
    assert_eq!(links::detect(&screen.frame()).len(), 1);
    screen.feed(b"\x1b[1;1HX");
    assert!(screen.frame().hyperlinks.is_empty());
    screen.feed(b"\x1b[1;1H\x1b]8;;https://example.com/half\x07\xa4\x1b]8;;\x07\xa4");
    assert_eq!(
        pttstella::text::rows(&screen.frame()).unwrap()[0].cells(0, 2),
        Some("中")
    );
    assert!(links::detect(&screen.frame()).is_empty());
    screen.feed(
        b"\x1b[1;1H\x1b]8;;https://example.com/whole\x07\x1b[31m\xa4\x1b[32m\xa4\x1b]8;;\x07",
    );
    assert_eq!(
        (
            links::detect(&screen.frame())[0].start,
            links::detect(&screen.frame())[0].end
        ),
        (0, 2)
    );
}

#[test]
fn osc8_limits_discard_the_control_tail_and_reuse_only_unreferenced_targets() {
    use pttstella::{
        links,
        screen::hyperlinks::{MAX_PARAMETERS, MAX_TARGETS, MAX_URL_BYTES},
    };
    for size in [MAX_URL_BYTES, MAX_URL_BYTES + 1, MAX_URL_BYTES * 8] {
        let uri = format!("https://example.com/{}", "x".repeat(size - 20));
        assert_eq!(uri.len(), size);
        let mut screen = Screen::new(80, 2).unwrap();
        screen.feed(&hyperlink(&uri, "visible", "\x1b\\"));
        screen.feed(b" DONE");
        assert!(screen.frame().control_complete);
        assert_eq!(
            pttstella::text::rows(&screen.frame()).unwrap()[0]
                .text
                .trim_end(),
            "visible DONE"
        );
        assert_eq!(
            links::detect(&screen.frame()).len(),
            usize::from(size == MAX_URL_BYTES)
        );
    }
    for size in [MAX_PARAMETERS, MAX_PARAMETERS + 1] {
        let mut screen = Screen::new(80, 2).unwrap();
        screen.feed(
            format!(
                "\x1b]8;{};https://example.com/a\x07X\x1b]8;;\x07",
                "p".repeat(size)
            )
            .as_bytes(),
        );
        assert_eq!(
            links::detect(&screen.frame()).len(),
            usize::from(size == MAX_PARAMETERS)
        );
    }
    let mut screen = Screen::new(80, 2).unwrap();
    for n in 0..=MAX_TARGETS {
        screen.feed(&hyperlink(&format!("https://example.com/{n}"), "X", "\x07"));
    }
    let old = screen.frame();
    assert_eq!(links::detect(&old).len(), MAX_TARGETS);
    screen.feed(b"\x1b[1;1H ");
    screen.feed(&hyperlink("https://example.com/reused", "NEW", "\x07"));
    let found = links::detect(&screen.frame());
    assert!(found.iter().any(|l| l.url.ends_with("/reused")));
    assert!(!found.iter().any(|l| l.url.ends_with("/0")));
    assert_eq!(links::detect(&old).len(), MAX_TARGETS);
    let mut screen = Screen::new(80, 2).unwrap();
    for n in 0..17 {
        let prefix = format!("https://example.com/{n}/");
        let uri = format!("{prefix}{}", "a".repeat(MAX_URL_BYTES - prefix.len()));
        screen.feed(&hyperlink(&uri, "X", "\x07"));
    }
    assert_eq!(links::detect(&screen.frame()).len(), 16);
}

#[test]
fn osc8_incomplete_cancelled_and_repeated_controls_remain_bounded_and_recoverable() {
    use pttstella::links;
    let mut screen = Screen::new(80, 2).unwrap();
    screen.feed(&hyperlink("https://example.com/a", "A", "\x07"));
    screen.feed(b"\x1b]8;;https://incomplete.example/");
    assert!(links::detect(&screen.frame()).is_empty());
    screen.feed(b"\x18B");
    assert!(screen.frame().control_complete);
    assert_eq!(links::detect(&screen.frame()).len(), 1);
    for n in 0..512 {
        screen.feed(b"\x1b[H");
        screen.feed(&hyperlink(&format!("https://example.com/{n}"), "X", "\x07"));
    }
    assert_eq!(screen.frame().hyperlinks.len(), 1);
    assert_eq!(
        links::detect(&screen.frame())[0].url,
        "https://example.com/511"
    );
    screen.feed(b"\x1bc\x1b[HRESET");
    assert!(screen.frame().hyperlinks.is_empty());
}

#[test]
fn osc8_projected_link_count_and_url_bytes_are_admitted_by_complete_target() {
    use pttstella::{links, screen::hyperlinks::MAX_URL_BYTES};
    for count in [64, 65] {
        let uri = format!("https://example.com/{}", "x".repeat(MAX_URL_BYTES - 20));
        let mut screen = Screen::new(80, 2).unwrap();
        for _ in 0..count {
            screen.feed(&hyperlink(&uri, "X", "\x07"));
            screen.feed(b" ");
        }
        let found = links::detect(&screen.frame());
        assert_eq!(found.len(), if count == 64 { 64 } else { 0 });
        assert!(
            found.iter().map(|link| link.url.len()).sum::<usize>()
                <= links::MAX_PROJECTED_URL_BYTES
        );
    }
    for count in [links::MAX_LINKS, links::MAX_LINKS + 1] {
        let mut screen = Screen::new(80, 20).unwrap();
        for _ in 0..count {
            screen.feed(&hyperlink("https://example.com/a", "X", "\x07"));
            screen.feed(b" ");
        }
        assert_eq!(
            links::detect(&screen.frame()).len(),
            if count == links::MAX_LINKS { count } else { 0 }
        );
    }
}
