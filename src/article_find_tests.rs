use super::*;
use crate::{
    article_cache::Reading,
    presentation,
    screen::{Glyph, Screen},
    settings::DisplaySettings,
};
fn glyph(row: usize, col: usize, text: &str, width: usize) -> Glyph {
    Glyph {
        row,
        col,
        text: text.into(),
        width,
        left: Style::default(),
        right: Style::default(),
        unmapped: false,
    }
}
fn packet(glyphs: Vec<Glyph>, rows: usize) -> Paint {
    let mut frame = Screen::new(80, 24).unwrap().frame();
    frame.rows = rows + 1;
    frame.revision = 1;
    frame.glyphs = glyphs;
    let mut p = presentation::sample_packet(&frame, &DisplaySettings::default());
    p.ptt_features = true;
    p.generation = 1;
    p.navigation = Context::ArticleReading {
        first_line: 1,
        last_line: rows,
        percent: 100,
    };
    p.article = Some(Reading {
        id: 1,
        first_line: 1,
        last_line: rows,
        end: true,
        load: None,
        next_segment: false,
        reply_count: 0,
        percent_cells: None,
        page_cells: None,
        range_cells: None,
        pages: Vec::new(),
    });
    p
}
fn search(find: &mut Find, p: &Paint, query: &str) {
    if !find.opened() {
        assert!(find.action(p, find.request + 1, Action::Open));
    }
    assert!(find.action(
        p,
        find.request + 1,
        Action::Edit {
            query: query.into()
        }
    ));
}
#[test]
fn literal_case_insensitive_hits_wrap_without_mutating_source() {
    let p = packet(
        vec![
            glyph(0, 0, "Hello", 1),
            glyph(0, 10, "hello", 1),
            glyph(39, 0, "HELLO", 1),
        ],
        40,
    );
    let original = p.frame.clone();
    let mut f = Find::default();
    search(&mut f, &p, "hElLo");
    assert_eq!(
        f.hits,
        vec![
            Hit {
                row: 0,
                start: 0,
                end: 1,
                index: 0
            },
            Hit {
                row: 0,
                start: 10,
                end: 11,
                index: 1
            },
            Hit {
                row: 39,
                start: 0,
                end: 1,
                index: 2
            }
        ]
    );
    assert_eq!(f.style(0, 0, 1), Some(SELECTED));
    assert_eq!(f.style(0, 10, 1), Some(NORMAL));
    assert_eq!(f.style(0, 7, 1), None);
    assert!(f.action(&p, 3, Action::Step { back: true }));
    assert_eq!(f.active, Some(2));
    assert!(f.action(&p, 4, Action::Step { back: false }));
    assert_eq!(f.active, Some(0));
    search(&mut f, &p, ".*");
    assert!(f.hits.is_empty());
    assert_eq!(p.frame, original);
}

#[test]
fn sparse_space_matches_map_to_each_exact_cell_and_reject_overlapping_glyphs() {
    let p = packet(vec![glyph(0, 0, "A", 1), glyph(0, 4, "B", 1)], 1);
    let mut f = Find::default();
    search(&mut f, &p, " ");
    assert_eq!(
        f.hits.iter().map(|h| (h.start, h.end)).collect::<Vec<_>>(),
        [(1, 2), (2, 3), (3, 4)]
    );
    let bad = packet(vec![glyph(0, 0, "中", 2), glyph(0, 1, "x", 1)], 1);
    let mut f = Find::default();
    search(&mut f, &bad, "中");
    assert!(!f.opened() && f.indexed_rows() == 0);
}
#[test]
fn unicode_expansion_combining_and_dual_cells_keep_whole_original_glyphs() {
    let p = packet(
        vec![
            glyph(0, 0, "中", 2),
            glyph(0, 2, "文", 2),
            glyph(0, 4, "😀", 2),
            glyph(0, 6, "İ", 1),
            glyph(1, 0, "e\u{301}\u{301}", 1),
        ],
        2,
    );
    let mut f = Find::default();
    search(&mut f, &p, "中文😀");
    assert_eq!(
        f.active(),
        Some(Hit {
            row: 0,
            start: 0,
            end: 6,
            index: 0
        })
    );
    search(&mut f, &p, "i");
    assert_eq!(f.active().unwrap().start, 6);
    search(&mut f, &p, "\u{301}");
    assert_eq!(f.hits.len(), 2);
    assert!(f.action(&p, f.request + 1, Action::Step { back: false }));
    assert_eq!(f.style(1, 0, 1), Some(SELECTED));
}
#[test]
fn either_concealed_half_breaks_search_and_footer_is_excluded() {
    let mut secret = glyph(0, 0, "secret", 1);
    secret.left.fg = 0;
    let mut half = glyph(0, 2, "word", 2);
    half.right.fg = 0;
    let p = packet(
        vec![secret, glyph(0, 1, "x", 1), half, glyph(1, 0, "footer", 1)],
        1,
    );
    let mut f = Find::default();
    for q in ["secret", "word", "footer", "\0"] {
        search(&mut f, &p, q);
        assert!(f.hits.is_empty(), "{q}");
    }
    search(&mut f, &p, "x");
    assert_eq!(f.hits.len(), 1);
    assert_eq!(f.style(0, 0, 1), None);
}
#[test]
fn unqueried_closed_and_cleared_search_retains_no_index_and_same_source_reuses_it() {
    let glyphs = (0..10000).map(|i| glyph(i / 80, i % 80, "a", 1)).collect();
    let p = packet(glyphs, 125);
    let mut f = Find::default();
    for _ in 0..10 {
        f.sync(&p);
    }
    assert_eq!(f.builds(), 0);
    search(&mut f, &p, "a");
    assert_eq!(f.hits.len(), 4096);
    assert!(f.limited);
    assert_eq!(f.builds(), 1);
    search(&mut f, &p, "aa");
    for _ in 0..10 {
        f.sync(&p);
    }
    assert_eq!(f.builds(), 1);
    search(&mut f, &p, "");
    assert_eq!(f.indexed_rows(), 0);
    search(&mut f, &p, "aa");
    assert_eq!(f.builds(), 2);
    f.close(f.request + 1);
    assert_eq!(f.indexed_rows(), 0);
    assert!(!f.opened());
    assert!(f.hits.is_empty());
}
#[test]
fn same_article_reprojection_preserves_anchor_and_foreign_scope_closes() {
    let mut p = packet(
        vec![
            glyph(0, 0, "a", 1),
            glyph(0, 10, "a", 1),
            glyph(1, 0, "a", 1),
        ],
        2,
    );
    let mut f = Find::default();
    search(&mut f, &p, "a");
    f.action(&p, 3, Action::Step { back: false });
    f.suspend();
    assert!(f.hits.is_empty());
    p.frame.revision += 1;
    f.sync(&p);
    assert_eq!(f.active().unwrap().start, 10);
    assert_eq!(f.builds(), 2);
    p.article.as_mut().unwrap().id = 2;
    f.sync(&p);
    assert!(!f.opened());
    assert!(f.query().is_empty());
    search(&mut f, &p, "a");
    p.generation += 1;
    f.sync(&p);
    assert!(!f.opened());
}
#[test]
fn pending_ime_and_edits_clear_matches_without_rebuilding_old_queries() {
    let mut p = packet(vec![glyph(0, 0, "a", 1)], 1);
    let mut f = Find::default();
    search(&mut f, &p, "a");
    f.action(&p, 3, Action::Pending);
    assert!(f.hits.is_empty());
    assert_eq!(f.indexed_rows(), 1);
    p.frame.revision += 1;
    f.sync(&p);
    assert_eq!(f.builds(), 1);
    assert!(f.hits.is_empty());
    assert!(!f.action(&p, 4, Action::Step { back: false }));
    search(&mut f, &p, "a");
    assert_eq!(f.builds(), 2);
}
#[test]
fn terminal_new_page_closes_and_utf16_query_limit_never_splits_unicode() {
    let mut p = packet(vec![glyph(0, 0, "term", 1), glyph(23, 0, "term", 1)], 23);
    p.article = None;
    let mut f = Find::default();
    search(&mut f, &p, "term");
    assert_eq!(f.hits.len(), 1);
    p.frame.revision += 1;
    f.sync(&p);
    assert!(!f.opened());
    assert_eq!(bounded_query(&("x".repeat(255) + "😀")), "x".repeat(255));
    assert_eq!(bounded_query(&"😀".repeat(129)), "😀".repeat(128));
}
#[test]
fn display_override_has_contrast_and_never_alters_source_or_a_stale_projection() {
    let mut p = packet(vec![glyph(0, 0, "中", 2)], 1);
    p.frame.glyphs[0].left = Style {
        fg: 1,
        bg: 4,
        ..Style::default()
    };
    p.frame.glyphs[0].right = Style {
        fg: 2,
        bg: 5,
        ..Style::default()
    };
    let original = p.frame.clone();
    let mut f = Find::default();
    search(&mut f, &p, "中");
    let mut display = original.clone();
    f.decorate(Source::of(&p), &mut display, 0);
    assert_eq!(display.glyphs[0].left, SELECTED);
    assert_eq!(display.glyphs[0].right, SELECTED);
    assert_eq!(p.frame, original);
    let mut stale = Source::of(&p);
    stale.presentation += 1;
    let mut export = original.clone();
    f.decorate(stale, &mut export, 0);
    assert_eq!(export, original);
    let data = f.data_version();
    f.action(&p, f.request + 1, Action::Step { back: false });
    assert!(f.reply(data).hits.is_none());
}
