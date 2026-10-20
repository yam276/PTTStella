use pttstella::{
    encoding,
    filters::{self, Rules, State},
    links,
    screen::Screen,
    selection::{self, Selection},
    text,
};
fn frame() -> pttstella::screen::Frame {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/filters.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&encoding::encode(data["reading"].as_str().unwrap()).unwrap());
    screen.frame()
}
#[test]
fn styled_push_ranges_require_complete_ids_and_timestamps_not_body_lookalikes() {
    let found = filters::pushes(&frame());
    assert_eq!(
        found
            .iter()
            .map(|p| (p.row, p.id.as_str(), p.start, p.end))
            .collect::<Vec<_>>(),
        vec![
            (5, "Alice", 9, 66),
            (6, "ALICE", 9, 66),
            (7, "Alice2", 10, 66),
            (8, "Bob", 7, 66)
        ]
    );
}
#[test]
fn one_projection_preserves_source_and_cells_for_copy_links_and_renderer() {
    let source = frame();
    let original = source.clone();
    let rules = Rules::default().edit("alice", "自製備註", false).unwrap();
    let result = filters::project(&source, &rules, false);
    assert_eq!(source, original);
    assert_eq!(result.frame.revision, source.revision);
    assert_eq!(result.frame.cursor, source.cursor);
    assert_eq!(result.matches.len(), 2);
    assert!(
        result
            .frame
            .glyphs
            .iter()
            .filter(|g| g.row == 5 && (9..66).contains(&g.col))
            .all(|g| g.width == 1 && g.left.fg == 0 && g.left.bg == 0)
    );
    let rows = text::rows(&result.frame).unwrap();
    assert_eq!(rows[5].cells(9, 66), Some("█".repeat(57).as_str()));
    assert_eq!(rows[5].cells(0, 9), Some("推 Alice:"));
    assert_eq!(rows[5].cells(67, 78), Some("10/16 12:34"));
    assert!(rows[3].text.contains("本文中的同名文字必須保留"));
    assert!(rows[7].text.contains("不同 ID"));
    assert!(rows[10].text.contains("截斷推文"));
    let copied = selection::extract(&result.frame, &Selection::All)
        .unwrap()
        .text;
    assert!(!copied.contains("機密測試內容"));
    assert!(!copied.contains("hidden.png"));
    assert!(copied.contains("visible.png"));
    let urls = links::detect(&result.frame);
    assert_eq!(urls.len(), 1);
    assert!(urls[0].url.ends_with("visible.png"));
    let shown = filters::project(&source, &rules, true);
    assert_eq!(shown.frame, source);
    assert_eq!(links::detect(&shown.frame).len(), 2);
}

#[test]
fn hidden_pushes_remove_osc8_destinations_from_every_projected_consumer() {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/filters.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&encoding::encode(data["reading"].as_str().unwrap()).unwrap());
    let secret_target = "https://example.com/hidden-metadata.png?complete=1";
    screen.feed(
        format!("\x1b[6;11H\x1b[0m\x1b]8;;{secret_target}\x07LINK\x1b]8;;\x07\x1b[24;80H")
            .as_bytes(),
    );
    let source = screen.frame();
    assert!(
        links::detect(&source)
            .iter()
            .any(|link| link.url == secret_target)
    );
    let rules = Rules::default().edit("Alice", "", false).unwrap();
    let hidden = filters::project(&source, &rules, false).frame;
    assert!(hidden.hyperlinks.iter().all(|link| link.target.is_none()));
    assert!(
        !links::detect(&hidden)
            .iter()
            .any(|link| link.url == secret_target)
    );
    assert!(
        !selection::extract(&hidden, &Selection::All)
            .unwrap()
            .text
            .contains("LINK")
    );
    let shown = filters::project(&source, &rules, true).frame;
    assert!(
        links::detect(&shown)
            .iter()
            .any(|link| link.url == secret_target)
    );
    assert_eq!(shown, source);
}
#[test]
fn unknown_unmapped_partial_and_wrapped_records_are_not_guessed() {
    let source = frame();
    let rules = Rules::default().edit("Alice", "", false).unwrap();
    let mut changed = source.clone();
    changed.control_complete = false;
    assert!(filters::project(&changed, &rules, false).matches.is_empty());
    let mut unmapped = source.clone();
    unmapped.glyphs[0].unmapped = true;
    assert!(filters::pushes(&unmapped).is_empty());
    let mut wrong = source.clone();
    wrong.cursor = (0, 0);
    assert!(filters::pushes(&wrong).is_empty());
    let mut forged = source.clone();
    for g in forged.glyphs.iter_mut().filter(|g| g.row == 5) {
        g.left = Default::default();
        g.right = Default::default();
    }
    assert!(!filters::pushes(&forged).iter().any(|p| p.row == 5));
}
#[test]
fn editing_is_case_insensitive_bounded_and_explicit() {
    let a = Rules::default().edit("Alice", "one", false).unwrap();
    let b = a.edit("ALICE", "two", false).unwrap();
    assert_eq!(b.entries.len(), 1);
    assert_eq!(b.entries[0].note, "two");
    assert!(b.edit("Alice2", "", true).is_err());
    assert!(b.edit("Alice", "bad\nnote", false).is_err());
    assert!(b.edit("tooLongId12345", "", false).is_err());
    assert!(b.edit("x", &"字".repeat(129), false).is_err());
    assert!(b.edit("alice", "", true).unwrap().entries.is_empty());
    let mut duplicate = b.clone();
    duplicate.entries.push(a.entries[0].clone());
    assert!(duplicate.validate().is_err());
    assert!(Rules::decode(br#"{"version":4,"entries":[]}"#).is_err());
    assert!(Rules::decode(&vec![0; filters::MAX_BYTES + 1]).is_err());
    let entries = (0..filters::MAX_ENTRIES)
        .map(|i| filters::Entry {
            id: format!("Test{i}"),
            note: String::new(),
            policy: filters::Policy::Mask,
        })
        .collect();
    let full = Rules {
        version: 1,
        entries,
    };
    assert!(full.validate().is_ok());
    assert!(full.edit("Extra", "", false).is_err());
    assert!(Rules::decode(br#"{"version":1,"entries":[],"unexpected":true}"#).is_err());
}
#[test]
fn reveal_and_policy_changes_invalidate_same_frame_and_reset_on_new_source() {
    let source = frame();
    let mut state = State::new(Rules::default().edit("Alice", "", false).unwrap());
    let old = state.version();
    state.reveal(1, source.revision, true);
    assert!(state.version() > old);
    assert_eq!(state.project(1, &source).frame, source);
    state.sync(2, source.revision);
    assert!(!state.project(2, &source).revealed);
    assert_ne!(state.project(2, &source).frame, source);
    state.reveal(2, source.revision, true);
    let old = state.version();
    state.replace(state.rules().clone());
    assert!(state.version() > old);
    assert!(!state.project(2, &source).revealed);
    state.reveal(2, source.revision, true);
    state.sync(2, source.revision + 1);
    assert!(!state.project(2, &source).revealed);
}

#[test]
fn allow_policy_preserves_all_consumers_and_old_rules_migrate() {
    let old = br#"{"version":1,"entries":[{"id":"Alice","note":"old"}]}"#;
    let mask = Rules::decode(old).unwrap();
    assert_eq!(mask.version, 3);
    assert_eq!(mask.entries[0].policy, filters::Policy::Mask);
    let allow = mask
        .edit_policy("ALICE", "exception", filters::Policy::Allow, false)
        .unwrap();
    assert_eq!(allow.entries.len(), 1);
    let source = frame();
    let projected = filters::project(&source, &allow, false);
    assert!(projected.matches.is_empty());
    assert_ne!(projected.frame, source);
    for (before, after) in source.glyphs.iter().zip(&projected.frame.glyphs) {
        if [5, 6].contains(&before.row) && (3..8).contains(&before.col) {
            assert_eq!(after.left.fg, 6);
            assert!(after.left.bright);
            assert_eq!(after.text, before.text);
        } else {
            assert_eq!(before, after);
        }
    }
    assert_eq!(links::detect(&projected.frame), links::detect(&source));
    assert_eq!(
        selection::extract(&projected.frame, &Selection::All)
            .unwrap()
            .text,
        selection::extract(&source, &Selection::All).unwrap().text
    );
    assert_eq!(
        Rules::decode(&serde_json::to_vec(&allow).unwrap()).unwrap(),
        allow
    );
    assert!(
        Rules::decode(br#"{"version":1,"entries":[{"id":"Alice","note":"","policy":"allow"}]}"#)
            .is_err()
    );
    assert!(
        Rules::decode(br#"{"version":2,"entries":[{"id":"Alice","note":"","policy":"unknown"}]}"#)
            .is_err()
    );
}

fn article_list() -> pttstella::screen::Frame {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/navigation.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&encoding::encode(data["list"].as_str().unwrap()).unwrap());
    screen.frame()
}

#[test]
fn article_titles_full_rows_and_only_allow_ids_share_one_projection() {
    use filters::Policy;
    let source = article_list();
    let rules = Rules::default()
        .edit_policy("ALICE", "", Policy::Mask, false)
        .unwrap()
        .edit_policy("bob", "", Policy::MaskRow, false)
        .unwrap()
        .edit_policy("carol", "", Policy::Allow, false)
        .unwrap();
    let result = filters::project(&source, &rules, false);
    assert_eq!(
        result
            .matches
            .iter()
            .map(|p| (p.row, p.start, p.end))
            .collect::<Vec<_>>(),
        vec![(3, 30, 80), (4, 0, 80)]
    );
    let rows = text::rows(&result.frame).unwrap();
    let original = text::rows(&source).unwrap();
    assert_eq!(rows[3].cells(0, 30), original[3].cells(0, 30));
    assert_eq!(rows[3].cells(30, 80), Some("█".repeat(50).as_str()));
    assert_eq!(rows[4].text, "█".repeat(80));
    assert_eq!(rows[5].text, original[5].text);
    for g in &result.frame.glyphs {
        if g.row == 5 && (17..22).contains(&g.col) {
            assert_eq!((g.left.fg, g.left.bg, g.left.bright), (6, 0, true));
        } else if g.row != 4 && !(g.row == 3 && g.col >= 30) {
            assert!(source.glyphs.contains(g));
        }
    }
    assert_eq!(
        (result.frame.cols, result.frame.rows, result.frame.cursor),
        (80, 24, source.cursor)
    );
    let copied = selection::extract(&result.frame, &Selection::All)
        .unwrap()
        .text;
    assert!(copied.contains("alice") && copied.contains("carol"));
    assert!(
        !copied.contains("bob") && !copied.contains("合成文章一") && !copied.contains("合成文章二")
    );
    let revealed = filters::project(&source, &rules, true);
    assert_eq!(
        selection::extract(&revealed.frame, &Selection::All)
            .unwrap()
            .text,
        selection::extract(&source, &Selection::All).unwrap().text
    );
    assert!(
        revealed
            .frame
            .glyphs
            .iter()
            .filter(|g| g.row == 5 && (17..22).contains(&g.col))
            .all(|g| g.left.fg == 6)
    );
}

#[test]
fn entire_push_row_removes_identity_time_content_and_links_without_moving_rows() {
    let source = frame();
    let rules = Rules::default()
        .edit_policy("Alice", "", filters::Policy::MaskRow, false)
        .unwrap();
    let result = filters::project(&source, &rules, false);
    for row in [5, 6] {
        assert_eq!(text::rows(&result.frame).unwrap()[row].text, "█".repeat(80));
        assert!(
            result
                .frame
                .glyphs
                .iter()
                .filter(|g| g.row == row)
                .all(|g| g.left.fg == 0 && g.left.bg == 0 && !g.left.inverse)
        );
    }
    assert_eq!(
        result
            .frame
            .glyphs
            .iter()
            .filter(|g| ![5, 6].contains(&g.row))
            .collect::<Vec<_>>(),
        source
            .glyphs
            .iter()
            .filter(|g| ![5, 6].contains(&g.row))
            .collect::<Vec<_>>()
    );
    assert_eq!(links::detect(&result.frame).len(), 1);
    assert_eq!(filters::project(&source, &rules, true).frame, source);
}

#[test]
fn v2_migrates_without_reinterpreting_masks_and_old_schemas_reject_row_policy() {
    let old = br#"{"version":2,"entries":[{"id":"Alice","note":"keep","policy":"mask"},{"id":"Bob","note":"","policy":"allow"}]}"#;
    let migrated = Rules::decode(old).unwrap();
    assert_eq!(migrated.version, 3);
    assert_eq!(migrated.entries[0].policy, filters::Policy::Mask);
    assert_eq!(migrated.entries[1].policy, filters::Policy::Allow);
    for version in [1, 2] {
        assert!(Rules::decode(format!(r#"{{"version":{version},"entries":[{{"id":"Alice","note":"","policy":"mask_row"}}]}}"#).as_bytes()).is_err());
    }
    let row = migrated
        .edit_policy("Alice", "", filters::Policy::MaskRow, false)
        .unwrap();
    assert_eq!(
        Rules::decode(&serde_json::to_vec(&row).unwrap()).unwrap(),
        row
    );
}

#[test]
fn article_identity_is_never_inferred_from_mentions_or_deleted_rows() {
    let mut source = article_list();
    let rules = Rules::default().edit("Alice2", "", false).unwrap();
    assert_eq!(filters::project(&source, &rules, false).frame, source);
    let rules = Rules::default().edit("Alice", "", false).unwrap();
    source.control_complete = false;
    assert_eq!(filters::project(&source, &rules, false).frame, source);
    source.control_complete = true;
    source.cursor = (0, 0);
    assert_eq!(filters::project(&source, &rules, false).frame, source);
}

#[test]
fn consecutive_reply_ids_are_masked_without_changing_type_colon_content_or_time() {
    let source = frame();
    let mut merged = source.clone();
    filters::merge_push_ids(&mut merged, &filters::pushes(&source));
    let rows = text::rows(&merged).unwrap();
    assert_eq!(rows[5].cells(0, 9), Some("推 Alice:"));
    assert_eq!(rows[6].cells(0, 9), Some("→ █████:"));
    assert_eq!(rows[7].cells(0, 10), Some("推 Alice2:"));
    for glyph in &merged.glyphs {
        if glyph.row == 6 && (3..8).contains(&glyph.col) {
            assert_eq!(
                (glyph.text.as_str(), glyph.left.fg, glyph.left.bg),
                ("█", 0, 0)
            );
        } else {
            assert!(source.glyphs.contains(glyph));
        }
    }
    assert_eq!(links::detect(&merged), links::detect(&source));
    assert_eq!(merged.cursor, source.cursor);
    assert!(
        selection::extract(&merged, &Selection::All)
            .unwrap()
            .text
            .contains("→ █████:")
    );
}

#[test]
fn merging_never_bridges_a_gap_different_id_unknown_row_or_list() {
    let source = frame();
    let mut replies = filters::pushes(&source);
    replies[1].row = 10;
    let mut merged = source.clone();
    filters::merge_push_ids(&mut merged, &replies);
    assert_eq!(merged, source);
    let list = article_list();
    assert!(filters::pushes(&list).is_empty());
    let mut unknown = source.clone();
    unknown.control_complete = false;
    assert!(filters::pushes(&unknown).is_empty());
}

#[test]
fn merging_follows_account_policies_even_when_revealed_and_removes_id_hyperlinks() {
    let mut source = frame();
    source
        .hyperlinks
        .push(pttstella::screen::hyperlinks::Hyperlink {
            target: Some("https://example.com/identity".into()),
            spans: vec![pttstella::screen::hyperlinks::Span {
                row: 6,
                start: 3,
                end: 8,
            }],
        });
    let replies = filters::pushes(&source);
    for policy in [
        filters::Policy::Mask,
        filters::Policy::MaskRow,
        filters::Policy::Allow,
    ] {
        for revealed in [false, true] {
            let rules = Rules::default()
                .edit_policy("Alice", "", policy, false)
                .unwrap();
            let mut projected = filters::project(&source, &rules, revealed).frame;
            filters::merge_push_ids(&mut projected, &replies);
            assert_eq!(
                text::rows(&projected).unwrap()[6].cells(3, 8),
                Some("█████")
            );
            assert!(
                !links::detect(&projected)
                    .iter()
                    .any(|link| link.url.ends_with("/identity"))
            );
            if policy == filters::Policy::Allow {
                assert!(
                    projected
                        .glyphs
                        .iter()
                        .filter(|g| g.row == 5 && (3..8).contains(&g.col))
                        .all(|g| g.left.fg == 6)
                );
            }
        }
    }
}
