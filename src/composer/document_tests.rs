use super::*;
fn version(revision: u64) -> Version {
    Version { token: 1, revision }
}
fn document(source: &str) -> Document {
    Document::new(1, "中文標題".into(), source.into()).unwrap()
}
fn text(doc: &Document, row: usize) -> String {
    doc.rows()[row].glyphs(doc.mode()).map(|g| g.text).collect()
}
#[test]
fn three_views_keep_exact_source_trailing_lines_and_distinct_literal_escape_symbols() {
    let source = "\x1b[31mA\x1b[66;34m中\tB\x1b[m\n尾\n\x1b[ ␛😀\n";
    let mut doc = document(source);
    assert_eq!(doc.rows().len(), 4);
    assert_eq!(text(&doc, 0), "A中\tB");
    let glyphs = doc.rows()[0].glyphs(Mode::Color).collect::<Vec<_>>();
    assert_eq!((glyphs[0].col, glyphs[0].left.fg), (0, 1));
    assert_eq!(
        (
            glyphs[1].col,
            glyphs[1].width,
            glyphs[1].left.fg,
            glyphs[1].right.fg
        ),
        (1, 2, 1, 4)
    );
    assert_eq!((glyphs[2].col, glyphs[2].width), (3, 5));
    assert_eq!(glyphs[3].col, 8);
    assert_eq!(doc.rows()[1].glyphs(Mode::Color).next().unwrap().left.fg, 7);
    doc.set_mode(Mode::Plain);
    assert_eq!(text(&doc, 0), "A中\tB");
    assert!(
        doc.rows()[0]
            .glyphs(Mode::Plain)
            .all(|g| g.left == crate::screen::Style::default() && g.right == g.left)
    );
    doc.set_mode(Mode::Source);
    assert_eq!(text(&doc, 0), "␛[31mA␛[66;34m中\tB␛[m");
    assert_eq!(text(&doc, 2), "␛[ ␛😀");
    assert_eq!(doc.source(), source);
    assert_eq!(doc.revision(), 0);
    assert!(!doc.can_undo());
    let escape = doc.rows()[2].glyphs(Mode::Source).next().unwrap();
    assert_eq!(escape.raw, 0..1);
    let literal = doc.rows()[2]
        .glyphs(Mode::Source)
        .find(|g| g.text == "␛" && g.raw.start > 0)
        .unwrap();
    assert_eq!(literal.raw.len(), 3);
}
#[test]
fn wide_hits_hidden_boundary_insert_and_delete_never_split_raw_glyph_or_drop_sgr() {
    let mut doc = document("\x1b[31m中文\x1b[mZ");
    let a = doc
        .offset(Position { row: 0, col: 1 }, Affinity::Before)
        .unwrap();
    let b = doc
        .offset(Position { row: 0, col: 1 }, Affinity::After)
        .unwrap();
    assert_eq!(&doc.source()[a..b], "中");
    assert_eq!(
        doc.offset(Position { row: 0, col: 0 }, Affinity::Before)
            .unwrap(),
        0
    );
    assert_eq!(
        doc.offset(Position { row: 0, col: 0 }, Affinity::After)
            .unwrap(),
        5
    );
    // Insertion at the visible origin follows hidden initial SGR.
    doc.select(doc.version(), Selection { anchor: 0, head: 0 })
        .unwrap();
    doc.replace_selection(version(0), "A").unwrap();
    assert_eq!(doc.source(), "\x1b[31mA中文\x1b[mZ");
    let raw = doc.source().find('中').unwrap();
    doc.select(
        doc.version(),
        Selection {
            anchor: raw,
            head: raw + 3,
        },
    )
    .unwrap();
    doc.replace_selection(doc.version(), "").unwrap();
    assert_eq!(doc.source(), "\x1b[31mA文\x1b[mZ");
    assert!(
        doc.select(
            doc.version(),
            Selection {
                anchor: raw + 1,
                head: raw + 1
            }
        )
        .is_err()
    );
    // A selection consisting solely of hidden SGR has no editable visible span.
    doc.select(doc.version(), Selection { anchor: 0, head: 5 })
        .unwrap();
    let before = doc.source().to_owned();
    doc.replace_selection(doc.version(), "").unwrap();
    assert_eq!(doc.source(), before);
}
#[test]
fn line_cache_reuses_unchanged_rows_and_propagates_only_until_rendition_converges() {
    let mut doc = document("head\n\x1b[31mred\ncarry\n\x1b[mreset\nplain\ntail");
    let carry = doc.rows()[2].clone();
    let plain = doc.rows()[4].clone();
    let from = doc.source().find("red").unwrap();
    let update = doc.replace(version(0), from..from + 3, "中紅").unwrap();
    assert_eq!(update.reparsed_rows, 1);
    assert!(Arc::ptr_eq(&carry, &doc.rows()[2]));
    assert!(Arc::ptr_eq(&plain, &doc.rows()[4]));
    let from = doc.source().find("31").unwrap();
    let update = doc.replace(doc.version(), from..from + 2, "34").unwrap();
    assert_eq!(update.reparsed_rows, 3);
    assert!(!Arc::ptr_eq(&carry, &doc.rows()[2]));
    assert!(Arc::ptr_eq(&plain, &doc.rows()[4]));
    assert!(doc.rows()[2].glyphs(Mode::Color).all(|g| g.left.fg == 4));
    assert!(doc.rows()[4].glyphs(Mode::Color).all(|g| g.left.fg == 7));
    let at = doc.source().find("carry").unwrap();
    let update = doc
        .replace(doc.version(), at..at + 5, "new\n\n中文")
        .unwrap();
    assert_eq!(doc.rows().len(), 8);
    assert_eq!(text(&doc, 2), "new");
    assert_eq!(text(&doc, 3), "");
    assert_eq!(text(&doc, 4), "中文");
    assert_eq!(text(&doc, 7), "tail");
    assert!(update.reused_rows >= 3);
    doc.undo(doc.version()).unwrap();
    assert_eq!(doc.rows().len(), 6);
    assert_eq!(text(&doc, 2), "carry");
    doc.redo(doc.version()).unwrap();
    assert_eq!(doc.rows().len(), 8);
    assert_eq!(text(&doc, 7), "tail");
}
#[test]
fn one_rectangular_colour_transaction_preserves_unselected_styles_and_undo_redo() {
    let original = "\x1b[32m甲乙\nABCD\n\n末";
    let mut doc = document(original);
    let ranges = doc.rectangle(0..3, 1..3).unwrap();
    assert_eq!(ranges.len(), 2);
    assert_eq!(&doc.source()[ranges[1].clone()], "BC");
    doc.color(
        version(0),
        &ranges,
        Color {
            foreground: 9,
            background: 0,
        },
    )
    .unwrap();
    let colored = doc.source().to_owned();
    let first = doc.rows()[0].glyphs(Mode::Color).collect::<Vec<_>>();
    assert_eq!(first.len(), 2);
    assert!(first.iter().all(|g| g.left.fg == 1 && g.left.bright));
    let second = doc.rows()[1].glyphs(Mode::Color).collect::<Vec<_>>();
    assert_eq!(
        second.iter().map(|g| g.left.fg).collect::<Vec<_>>(),
        vec![2, 1, 1, 2]
    );
    assert_eq!(text(&doc, 2), "");
    assert_eq!(doc.rows()[3].glyphs(Mode::Color).next().unwrap().left.fg, 2);
    assert_eq!(doc.revision(), 1);
    assert!(doc.undo(version(1)).unwrap());
    assert_eq!(doc.source(), original);
    assert!(!doc.can_undo());
    assert!(doc.redo(version(2)).unwrap());
    assert_eq!(doc.source(), colored);
}
#[test]
fn title_and_source_share_monotonic_history_and_reject_stale_or_invalid_work_atomically() {
    let mut doc = document("初稿\n");
    doc.edit_title(version(0), "新標題".into()).unwrap();
    doc.replace(version(1), 0..6, "改稿").unwrap();
    let before = doc.source().to_owned();
    assert!(doc.replace(version(0), 0..0, "舊結果").is_err());
    assert!(doc.replace(version(2), 1..2, "").is_err());
    assert!(doc.replace(version(2), 0..0, "\x18").is_err());
    assert!(
        doc.replace(version(2), 0..0, &"x".repeat(MAX_BODY))
            .is_err()
    );
    assert!(doc.edit_title(version(2), "bad\n".into()).is_err());
    assert_eq!(doc.source(), before);
    assert_eq!(doc.revision(), 2);
    assert!(doc.undo(version(2)).unwrap());
    assert_eq!(doc.source(), "初稿\n");
    assert_eq!(doc.title(), "新標題");
    doc.set_mode(Mode::Source);
    assert!(doc.undo(version(3)).unwrap());
    assert_eq!(doc.title(), "中文標題");
    assert!(doc.redo(version(4)).unwrap());
    assert_eq!(doc.title(), "新標題");
    doc.replace(version(5), 0..0, "分歧").unwrap();
    assert!(!doc.can_redo());
    assert_eq!(doc.revision(), 6);
}
#[test]
fn history_limits_discard_only_old_undo_not_the_current_document_or_latest_transaction() {
    let mut doc = document("");
    for _ in 0..1000 {
        doc.replace(doc.version(), doc.source().len()..doc.source().len(), "中")
            .unwrap();
    }
    assert_eq!(doc.source(), "中".repeat(1000));
    assert!(doc.history_bytes() <= HISTORY_BYTES);
    let mut steps = 0;
    while doc.undo(doc.version()).unwrap() {
        steps += 1;
    }
    assert_eq!(steps, HISTORY_STEPS);
    assert_eq!(doc.source(), "中".repeat(1000 - HISTORY_STEPS));
    while doc.redo(doc.version()).unwrap() {}
    assert_eq!(doc.source(), "中".repeat(1000));
    let mut big = document(&"a".repeat(MAX_BODY));
    for i in 0..12 {
        big.replace(
            big.version(),
            0..MAX_BODY,
            &(if i % 2 == 0 { "b" } else { "a" }).repeat(MAX_BODY),
        )
        .unwrap();
    }
    assert!(big.history_bytes() <= HISTORY_BYTES);
    assert!(big.undo(big.version()).unwrap());
    assert_eq!(big.source(), "b".repeat(MAX_BODY));
}

#[test]
fn replacing_a_document_rejects_old_token_even_when_both_revisions_are_zero() {
    let old = document("舊稿");
    let old_version = old.version();
    let mut current = Document::new(2, "新標題".into(), "新稿".into()).unwrap();
    assert!(current.replace(old_version, 0..0, "晚到").is_err());
    assert!(
        current
            .select(old_version, Selection { anchor: 0, head: 0 })
            .is_err()
    );
    assert!(current.undo(old_version).is_err());
    assert_eq!(current.source(), "新稿");
    assert_eq!(current.revision(), 0);
    current.revision = u64::MAX;
    let version = current.version();
    assert!(current.replace(version, 0..0, "不能繞回").is_err());
    assert_eq!(current.source(), "新稿");
    assert!(!current.can_undo());
    assert!(Document::new(0, String::new(), String::new()).is_err());
}
#[test]
fn source_mode_colour_tracks_the_changed_raw_selection_through_history() {
    let mut doc = document("ABC");
    doc.set_mode(Mode::Source);
    doc.select(doc.version(), Selection { anchor: 1, head: 2 })
        .unwrap();
    doc.color(
        doc.version(),
        std::slice::from_ref(&(1..2)),
        Color {
            foreground: 9,
            background: 0,
        },
    )
    .unwrap();
    let selected = &doc.source()[doc.selection().range()];
    assert!(selected.contains('B'));
    assert!(selected.starts_with('\x1b'));
    assert!(!selected.contains('A') && !selected.contains('C'));
    let after = doc.selection();
    doc.undo(doc.version()).unwrap();
    assert_eq!(doc.selection(), Selection { anchor: 1, head: 2 });
    assert_eq!(doc.source(), "ABC");
    doc.redo(doc.version()).unwrap();
    assert_eq!(doc.selection(), after);
}

#[test]
fn rectangle_color_and_title_history_restore_shape_even_in_another_view() {
    let original = "A中B\nC文D\n";
    let mut doc = document(original);
    let shape = SelectionShape::Rectangle {
        anchor: Position { row: 0, col: 2 },
        head: Position { row: 1, col: 3 },
        mode: Mode::Color,
    };
    doc.select_with_shape(
        doc.version(),
        Selection {
            anchor: 1,
            head: 10,
        },
        shape,
    )
    .unwrap();
    let ranges = doc.selected_ranges().unwrap();
    assert_eq!(ranges, [1..4, 7..10]);
    doc.color(
        doc.version(),
        &ranges,
        Color {
            foreground: 10,
            background: 4,
        },
    )
    .unwrap();
    let colored = doc.source().to_owned();
    let after = doc.selected_ranges().unwrap();
    assert_eq!(doc.selection_shape(), shape);
    for (row, range) in after.iter().enumerate() {
        assert_eq!(
            doc.cache.position(Mode::Color, range.start),
            Position { row, col: 1 }
        );
        assert_eq!(
            doc.cache.position(Mode::Color, range.end),
            Position { row, col: 3 }
        );
        let glyphs = doc.rows()[row].glyphs(Mode::Color).collect::<Vec<_>>();
        assert_eq!((glyphs[0].left.fg, glyphs[0].left.bg), (7, 0));
        assert_eq!(
            (glyphs[1].left.fg, glyphs[1].left.bg, glyphs[1].left.bright),
            (2, 4, true)
        );
        assert_eq!((glyphs[2].left.fg, glyphs[2].left.bg), (7, 0));
    }
    doc.edit_title(doc.version(), "新標題".into()).unwrap();
    doc.set_mode(Mode::Source);
    assert_eq!(doc.selection_shape(), SelectionShape::Text);
    doc.undo(doc.version()).unwrap();
    assert_eq!(doc.mode(), Mode::Source);
    assert_eq!(doc.title(), "中文標題");
    assert_eq!(doc.selection_shape(), shape);
    assert_eq!(doc.selected_ranges().unwrap(), after);
    doc.undo(doc.version()).unwrap();
    assert_eq!(doc.source(), original);
    assert_eq!(doc.selected_ranges().unwrap(), [1..4, 7..10]);
    doc.redo(doc.version()).unwrap();
    assert_eq!(doc.source(), colored);
    assert_eq!(doc.selected_ranges().unwrap(), after);
}

#[test]
fn whole_source_and_line_selections_keep_hidden_boundaries_through_color_and_history() {
    let original = "\x1b[31mA\x1b[m\n中\n末";
    let mut doc = document(original);
    doc.select_with_shape(
        doc.version(),
        Selection {
            anchor: 0,
            head: original.len(),
        },
        SelectionShape::All,
    )
    .unwrap();
    doc.replace_selection(doc.version(), "X").unwrap();
    doc.undo(doc.version()).unwrap();
    assert_eq!(doc.selection_shape(), SelectionShape::All);
    doc.replace_selection(doc.version(), "Y").unwrap();
    assert_eq!(doc.source(), "Y");
    assert!(!doc.can_redo());
    let mut doc = document(original);
    let end = doc.row_start(2).unwrap();
    doc.select_with_shape(
        doc.version(),
        Selection {
            anchor: end,
            head: 0,
        },
        SelectionShape::Lines,
    )
    .unwrap();
    let ranges = doc.selected_ranges().unwrap();
    doc.color(
        doc.version(),
        &ranges,
        Color {
            foreground: 9,
            background: 0,
        },
    )
    .unwrap();
    let colored_end = doc.row_start(2).unwrap();
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: colored_end,
            head: 0
        }
    );
    let colored = doc.source().to_owned();
    doc.undo(doc.version()).unwrap();
    assert_eq!(doc.source(), original);
    assert_eq!(doc.selection_shape(), SelectionShape::Lines);
    assert_eq!(
        doc.selection(),
        Selection {
            anchor: end,
            head: 0
        }
    );
    doc.redo(doc.version()).unwrap();
    assert_eq!(doc.source(), colored);
    doc.replace_selection(doc.version(), "X\n").unwrap();
    assert_eq!(doc.source(), "X\n末");
}

#[test]
fn rectangular_insertion_and_source_color_restore_the_exact_prior_selection() {
    let mut doc = document("A中B\nC文D\n");
    let shape = SelectionShape::Rectangle {
        anchor: Position { row: 0, col: 1 },
        head: Position { row: 1, col: 1 },
        mode: Mode::Color,
    };
    doc.select_with_shape(doc.version(), Selection { anchor: 1, head: 7 }, shape)
        .unwrap();
    assert_eq!(doc.selected_ranges().unwrap(), [1..1, 7..7]);
    doc.replace_selection(doc.version(), "X").unwrap();
    assert_eq!(doc.source(), "AX中B\nCX文D\n");
    doc.undo(doc.version()).unwrap();
    assert_eq!(doc.source(), "A中B\nC文D\n");
    assert_eq!(doc.selection_shape(), shape);
    assert_eq!(doc.selected_ranges().unwrap(), [1..1, 7..7]);
    let mut doc = document("ABC\nDEF");
    doc.set_mode(Mode::Source);
    let shape = SelectionShape::Rectangle {
        anchor: Position { row: 0, col: 1 },
        head: Position { row: 1, col: 2 },
        mode: Mode::Source,
    };
    doc.select_with_shape(doc.version(), Selection { anchor: 1, head: 6 }, shape)
        .unwrap();
    let ranges = doc.selected_ranges().unwrap();
    doc.color(
        doc.version(),
        &ranges,
        Color {
            foreground: 9,
            background: 0,
        },
    )
    .unwrap();
    assert_eq!(doc.selection_shape(), SelectionShape::Text);
    let colored = doc.source().to_owned();
    doc.undo(doc.version()).unwrap();
    assert_eq!(doc.source(), "ABC\nDEF");
    assert_eq!(doc.selection_shape(), shape);
    assert_eq!(doc.selected_ranges().unwrap(), [1..2, 5..6]);
    doc.redo(doc.version()).unwrap();
    assert_eq!(doc.source(), colored);
    assert_eq!(doc.selection_shape(), SelectionShape::Text);
}

#[test]
fn stale_and_inconsistent_shapes_are_rejected_without_changing_selection_or_history() {
    let mut doc = document("ABC\n中");
    let primary = Selection { anchor: 1, head: 2 };
    doc.select(doc.version(), primary).unwrap();
    for shape in [
        SelectionShape::All,
        SelectionShape::Lines,
        SelectionShape::Rectangle {
            anchor: Position { row: 2, col: 0 },
            head: Position { row: 0, col: 1 },
            mode: Mode::Color,
        },
        SelectionShape::Rectangle {
            anchor: Position { row: 0, col: 0 },
            head: Position { row: 0, col: 1 },
            mode: Mode::Source,
        },
    ] {
        assert!(
            doc.select_with_shape(doc.version(), primary, shape)
                .is_err()
        );
        assert_eq!(doc.selection(), primary);
        assert_eq!(doc.selection_shape(), SelectionShape::Text);
        assert!(!doc.can_undo());
    }
    doc.edit_title(doc.version(), "新標題".into()).unwrap();
    assert!(
        doc.select_with_shape(version(0), primary, SelectionShape::Text)
            .is_err()
    );
    assert_eq!(doc.selection(), primary);
    assert_eq!(doc.revision(), 1);
}

#[test]
fn rectangle_beyond_short_or_empty_rows_cannot_delete_the_intervening_newline() {
    for source in ["\n\n", "A\nB"] {
        let mut doc = document(source);
        let primary = Selection {
            anchor: doc.row_start(1).unwrap() - 1,
            head: source.len(),
        };
        let shape = SelectionShape::Rectangle {
            anchor: Position { row: 0, col: 10 },
            head: Position { row: 1, col: 12 },
            mode: Mode::Color,
        };
        doc.select_with_shape(doc.version(), primary, shape)
            .unwrap();
        assert!(doc.selected_ranges().unwrap().is_empty());
        doc.replace_selection(doc.version(), "X").unwrap();
        assert_eq!(doc.source(), source);
        assert_eq!(doc.revision(), 0);
        assert_eq!(doc.selection_shape(), shape);
        assert!(!doc.can_undo());
    }
}
