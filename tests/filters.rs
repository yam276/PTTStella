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
    assert!(Rules::decode(br#"{"version":3,"entries":[]}"#).is_err());
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
    assert_eq!(mask.version, 2);
    assert_eq!(mask.entries[0].policy, filters::Policy::Mask);
    let allow = mask
        .edit_policy("ALICE", "exception", filters::Policy::Allow, false)
        .unwrap();
    assert_eq!(allow.entries.len(), 1);
    let source = frame();
    let projected = filters::project(&source, &allow, false);
    assert!(projected.matches.is_empty());
    assert_eq!(projected.frame, source);
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
