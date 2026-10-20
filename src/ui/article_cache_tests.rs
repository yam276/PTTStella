use super::*;
use pttstella::{
    encoding,
    navigation::Intent,
    screen::Screen,
    selection::{self, Selection},
    settings::DisplaySettings,
};

fn page(first: usize, last: usize, percent: u8) -> Paint {
    let mut screen = Screen::new(80, 24).unwrap();
    for (row, line) in (first..=last).enumerate() {
        screen.feed(format!("\x1b[{};1Hline {line:04}", row + 1).as_bytes());
    }
    screen.feed(b"\x1b[24;1H");
    screen.feed(&encoding::encode(&format!("瀏覽 第 {first}~{last} 行 ({percent}%)")).unwrap());
    let mut paint =
        super::super::presentation::sample_packet(&screen.frame(), &DisplaySettings::default());
    paint.connected = true;
    paint.ptt_features = true;
    paint.navigation = Context::ArticleReading {
        first_line: first,
        last_line: last,
        percent,
    };
    paint.frame.revision = first as u64;
    paint.generation = 7;
    paint
}
fn lines(p: &Paint) -> Vec<String> {
    (0..p.frame.rows - 1)
        .map(|row| {
            selection::extract(&p.frame, &Selection::Line { row })
                .unwrap()
                .text
                .trim_end()
                .to_owned()
        })
        .collect()
}

fn reply_page(first: usize, row: usize) -> Paint {
    let mut p = page(first, first + 22, 50);
    for (col, ch) in (3..8).zip("Alice".chars()) {
        p.frame
            .glyphs
            .iter_mut()
            .find(|g| g.row == row && g.col == col)
            .unwrap()
            .text = ch.to_string();
    }
    p.replies = vec![pttstella::filters::Push {
        row,
        id: "Alice".into(),
        start: 9,
        end: 66,
    }];
    p
}

#[test]
fn adjacent_cached_pages_merge_ids_but_standalone_png_and_new_segments_keep_the_first() {
    let mut cache = ArticleCache::default();
    let mut first = reply_page(1, 22);
    let first_ids = first.replies.clone();
    cache.project(&mut first);
    super::super::presentation::merge_replies(&mut first, &first_ids);
    assert_eq!(
        pttstella::text::rows(&first.frame).unwrap()[22].cells(3, 8),
        Some("Alice")
    );
    let mut next = reply_page(24, 0);
    let next_ids = next.replies.clone();
    cache.project(&mut next);
    super::super::presentation::merge_replies(&mut next, &next_ids);
    assert_eq!(
        pttstella::text::rows(&next.frame).unwrap()[23].cells(3, 8),
        Some("█████")
    );
    assert_eq!(
        pttstella::text::rows(next.source_frame.as_ref().unwrap()).unwrap()[0].cells(3, 8),
        Some("Alice")
    );
    assert!(
        !serde_json::to_value(&next)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("replies")
    );
    // The cache retains the unmerged filtered row, so clearing history restores
    // its first ID instead of persisting a one-way mask.
    cache.clear();
    let mut fresh = reply_page(24, 0);
    cache.project(&mut fresh);
    super::super::presentation::merge_replies(&mut fresh, &next_ids);
    assert_eq!(
        pttstella::text::rows(&fresh.frame).unwrap()[0].cells(3, 8),
        Some("Alice")
    );
    let mut gap = reply_page(100, 0);
    cache.project(&mut gap);
    super::super::presentation::merge_replies(&mut gap, &next_ids);
    assert_eq!(
        pttstella::text::rows(&gap.frame).unwrap()[0].cells(3, 8),
        Some("Alice")
    );
}
#[test]
fn overlaps_are_merged_by_line_and_the_original_png_source_is_untouched() {
    let mut cache = ArticleCache::default();
    let mut first = page(1, 23, 40);
    let source = first.frame.clone();
    cache.project(&mut first);
    let id = first.article.as_ref().unwrap().id;
    assert_eq!(first.source_frame.as_ref(), Some(&source));
    let mut next = page(22, 44, 80);
    cache.project(&mut next);
    assert_eq!(next.article.as_ref().unwrap().id, id);
    assert_eq!(
        lines(&next),
        (1..=44)
            .map(|line| format!("line {line:04}"))
            .collect::<Vec<_>>()
    );
    let mut previous = page(1, 23, 40);
    cache.project(&mut previous);
    assert_eq!(lines(&previous), lines(&next));
    assert_eq!(previous.frame.rows, 45); // 44 body lines and one footer.
}
#[test]
fn incomplete_frames_cannot_replace_cached_content_or_claim_end_of_article() {
    let mut cache = ArticleCache::default();
    let mut first = page(1, 23, 40);
    cache.project(&mut first);
    let expected = lines(&first);
    for (input, output, complete) in [
        (true, false, true),
        (false, true, true),
        (false, false, false),
    ] {
        let mut fragment = page(22, 44, 100);
        fragment.input_pending = input;
        fragment.output_pending = output;
        fragment.frame.control_complete = complete;
        cache.project(&mut fragment);
        assert_eq!(lines(&fragment), expected);
        assert!(!fragment.article.unwrap().end);
    }
}
#[test]
fn gaps_generation_filter_policy_and_nonreading_input_reset_the_scope() {
    let mut cache = ArticleCache::default();
    let mut first = page(1, 23, 10);
    cache.project(&mut first);
    let mut jump = page(100, 122, 100);
    cache.project(&mut jump);
    assert!(jump.article.as_ref().unwrap().id > first.article.unwrap().id);
    assert_eq!(lines(&jump).len(), 23);
    let mut changed = page(110, 132, 100);
    changed.presentation = 1;
    cache.project(&mut changed);
    assert_eq!(lines(&changed).len(), 23);
    let id = changed.article.unwrap().id;
    cache.input(&changed.navigation, b"\x1b[D");
    let mut other = page(1, 23, 100);
    cache.project(&mut other);
    assert!(other.article.as_ref().unwrap().id > id);
    other.generation += 1;
    other.source_frame = None;
    other.article = None;
    other.frame = page(50, 72, 90).frame;
    other.navigation = page(50, 72, 90).navigation;
    cache.project(&mut other);
    assert_eq!(other.article.unwrap().first_line, 50);
}
#[test]
fn both_directions_evict_only_the_far_edge_and_never_exceed_the_bound() {
    let mut cache = ArticleCache::default();
    for first in (1..=441).step_by(22) {
        let mut p = page(first, first + 22, 50);
        cache.project(&mut p);
        assert!(p.frame.rows <= MAX_LINES + 1);
    }
    assert_eq!(cache.rows.len(), MAX_LINES);
    assert_eq!(*cache.rows.last_key_value().unwrap().0, 463);
    for first in (0..=20).rev().map(|i| 1 + i * 22) {
        let mut p = page(first, first + 22, 50);
        cache.project(&mut p);
        assert!(p.frame.rows <= MAX_LINES + 1);
    }
    assert_eq!(*cache.rows.first_key_value().unwrap().0, 1);
    assert_eq!(*cache.rows.last_key_value().unwrap().0, 256);
}
#[test]
fn cached_historical_links_and_dual_colour_glyphs_keep_their_exact_row_owner() {
    let mut cache = ArticleCache::default();
    let mut p = page(1, 23, 50);
    let mut source = Screen::new(80, 24).unwrap();
    source.feed(b"\x1b[31m\xa4\x1b[32m\xa4\x1b[0m https://example.com/p.png");
    p.frame = source.frame();
    cache.project(&mut p);
    let glyph = p.frame.glyphs[0].clone();
    assert_ne!(glyph.left, glyph.right);
    let mut next = page(22, 44, 100);
    cache.project(&mut next);
    assert_eq!(next.frame.glyphs[0], glyph);
    assert_eq!(next.links[0].url, "https://example.com/p.png");
    assert_eq!(next.links[0].row, 0);
    assert_eq!(
        selection::extract(&next.frame, &Selection::Link { index: 0 })
            .unwrap()
            .text,
        "https://example.com/p.png"
    );
}
#[test]
fn boundary_fetch_waits_for_new_revision_and_stops_after_a_target_or_no_progress() {
    let mut cache = ArticleCache::default();
    let mut first = page(50, 72, 30);
    cache.project(&mut first);
    let mut last = page(72, 94, 40);
    cache.project(&mut last);
    cache.begin_fetch(Intent::PreviousPage, &last.navigation, 72);
    assert!(cache.continue_fetch(&last.navigation, 72).is_none()); // ACK is not a station response.
    let mut transit = page(50, 72, 30);
    cache.project(&mut transit);
    assert!(matches!(
        cache.continue_fetch(&transit.navigation, 73),
        Some(Intent::PreviousPage)
    ));
    assert!(cache.continue_fetch(&transit.navigation, 74).is_none()); // redraw with no movement.
    assert!(cache.fetch.is_none());
    cache.begin_fetch(Intent::PreviousPage, &transit.navigation, 74);
    let mut loaded = page(28, 50, 20);
    cache.project(&mut loaded);
    assert!(cache.continue_fetch(&loaded.navigation, 75).is_none());
    assert!(cache.fetch.is_none());
    assert_eq!(loaded.article.unwrap().first_line, 28);
}
#[test]
fn reading_counts_preserve_scope_but_editor_and_unrecognized_commands_do_not() {
    let context = page(1, 23, 50).navigation;
    assert!(reading_input(&context, &[b'j'; 100]));
    assert!(reading_input(&context, b"\x1b[5~"));
    for bytes in [b"jk".as_slice(), b"r", b"\r", b"\x1b[D", &[b'k'; 101]] {
        assert!(!reading_input(&context, bytes));
    }
    assert!(!reading_input(&Context::ContinuePrompt, b" "));
}

#[test]
fn changed_opening_header_and_a_shorter_confirmed_end_cannot_leave_an_old_tail() {
    let mut cache = ArticleCache::default();
    let mut first = page(1, 23, 50);
    cache.project(&mut first);
    let mut second = page(22, 44, 100);
    cache.project(&mut second);
    let mut shorter = page(1, 23, 100);
    cache.project(&mut shorter);
    assert_eq!(lines(&shorter).len(), 23);
    assert!(shorter.article.unwrap().end);
    let mut second = page(22, 44, 100);
    cache.project(&mut second);
    let id = second.article.unwrap().id;
    let mut changed = page(1, 23, 50);
    changed.frame.glyphs[0].text = "X".into();
    cache.project(&mut changed);
    assert!(changed.article.as_ref().unwrap().id > id);
    assert_eq!(lines(&changed).len(), 23);
}

#[test]
fn a_page_turn_does_not_relabel_partial_new_text_with_the_previous_footer_range() {
    let mut cache = ArticleCache::default();
    let mut initial = page(1, 23, 50);
    cache.project(&mut initial);
    let expected = lines(&initial);
    cache.input(&initial.navigation, b"\x1b[6~");
    let mut fragment = page(1, 23, 50);
    fragment.frame.glyphs[0].text = "X".into();
    cache.project(&mut fragment);
    assert_eq!(lines(&fragment), expected);
    let mut complete = page(22, 44, 100);
    cache.project(&mut complete);
    assert_eq!(
        lines(&complete),
        (1..=44)
            .map(|line| format!("line {line:04}"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn short_articles_keep_the_source_grid_size_and_footer_row() {
    for last in [1, 3, 22, 23] {
        let mut cache = ArticleCache::default();
        let mut short = page(1, last, 100);
        let source = short.frame.clone();
        cache.project(&mut short);
        assert_eq!((short.frame.cols, short.frame.rows), (80, 24));
        assert_eq!(short.source_frame.as_ref(), Some(&source));
        assert_eq!(short.article.as_ref().unwrap().last_line, last);
        assert!(short.frame.glyphs.iter().any(|g| g.row == 23));
        assert!(
            pttstella::text::rows(&short.frame).is_some(),
            "short article with {last} lines must retain a complete cell grid"
        );
        assert!(
            short
                .frame
                .glyphs
                .iter()
                .filter(|g| g.row >= last && g.row < 23)
                .all(|g| g.text.trim().is_empty())
        );
        assert_eq!(short.settings, DisplaySettings::default());
    }
}

#[test]
fn short_article_padding_preserves_plain_and_explicit_image_links() {
    for last in [1, 3, 22, 23] {
        let mut cache = ArticleCache::default();
        let mut short = page(1, last, 100);
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(b"https://example.com/plain.png \x1b]8;;https://example.com/explicit.png\x1b\\image\x1b]8;;\x1b\\");
        short.frame = screen.frame();
        let source = short.frame.clone();
        cache.project(&mut short);
        assert_eq!(short.source_frame.as_ref(), Some(&source));
        assert_eq!(short.image_links.len(), 2, "article length {last}");
        assert_eq!(
            short
                .links
                .iter()
                .map(|link| link.url.as_str())
                .collect::<Vec<_>>(),
            vec![
                "https://example.com/explicit.png",
                "https://example.com/plain.png"
            ]
        );
        let selected = selection::extract(&short.frame, &Selection::Link { index: 1 }).unwrap();
        assert_eq!(selected.text, "https://example.com/plain.png");
        assert!(
            pttstella::filters::account_words(&short.frame)
                .iter()
                .any(|word| word.id == "image")
        );
    }
}
