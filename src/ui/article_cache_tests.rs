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
    cache.stop_load(LoadState::Cancelled);
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
    cache.stop_load(LoadState::Cancelled);
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
fn both_directions_revisit_known_pages_without_discarding_full_history() {
    let mut cache = ArticleCache::default();
    for first in (1..=441).step_by(22) {
        let mut p = page(first, first + 22, 50);
        cache.project(&mut p);
    }
    assert_eq!(cache.rows.len(), 463);
    cache.stop_load(LoadState::Cancelled);
    for first in (0..=20).rev().map(|i| 1 + i * 22) {
        let mut p = page(first, first + 22, 50);
        cache.project(&mut p);
        assert_eq!(
            lines(&p),
            (1..=463)
                .map(|n| format!("line {n:04}"))
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(*cache.rows.first_key_value().unwrap().0, 1);
    assert_eq!(*cache.rows.last_key_value().unwrap().0, 463);
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
    let id = second.article.as_ref().unwrap().id;
    let expected = lines(&second);
    let mut changed = page(1, 23, 50);
    changed.frame.glyphs[0].text = "X".into();
    cache.project(&mut changed);
    assert_eq!(changed.article.as_ref().unwrap().id, id);
    assert_eq!(
        changed.article.as_ref().unwrap().load,
        Some(LoadState::Changed)
    );
    assert_eq!(lines(&changed), expected);
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

#[test]
fn cached_footer_keeps_its_explicit_link_at_the_live_footer_row() {
    for last in [3, 23] {
        let mut cache = ArticleCache::default();
        let mut first = page(1, last, 50);
        let mut screen = Screen::new(80, 24).unwrap();
        screen
            .feed(b"\x1b[24;1H\x1b]8;;https://example.com/footer.png\x07footer image\x1b]8;;\x07");
        first.frame = screen.frame();
        cache.project(&mut first);
        assert_eq!(first.links.len(), 1);
        assert_eq!(
            (first.links[0].row, first.links[0].start, first.links[0].end),
            (23, 0, 12)
        );
        assert_eq!(first.links[0].url, "https://example.com/footer.png");
        assert_eq!(first.image_links, vec![0]);
        assert_eq!(
            selection::extract(&first.frame, &Selection::Link { index: 0 })
                .unwrap()
                .text,
            "https://example.com/footer.png"
        );

        let mut next = page(last + 1, last + 23, 100);
        next.frame = screen.frame();
        cache.project(&mut next);
        assert_eq!(next.links.len(), 1); // Never retain duplicate historical footers.
        assert_eq!(next.links[0].row, last + 23);
    }
}

#[test]
fn full_loading_retains_more_than_the_old_window_and_completes_only_with_both_ends() {
    let mut cache = ArticleCache::default();
    let now = Instant::now();
    let mut first = page(1, 23, 1);
    cache.project(&mut first);
    let id = first.article.unwrap().id;
    for index in 1..=20 {
        let start = 1 + (index - 1) * 22;
        let context = Context::ArticleReading {
            first_line: start,
            last_line: start + 22,
            percent: if index == 20 { 100 } else { 50 },
        };
        if index > 1 {
            let mut next = page(start, start + 22, if index == 20 { 100 } else { 50 });
            cache.project(&mut next);
            assert_eq!(next.article.as_ref().unwrap().id, id);
        }
        let result = cache.next_full_page(&context, start as u64, now);
        if index < 20 {
            assert_eq!(result, Some(Intent::NextPage));
            assert!(cache.next_full_page(&context, start as u64, now).is_none());
        } else {
            assert!(result.is_none());
            assert_eq!(cache.load_state(), Some(LoadState::Complete));
        }
    }
    assert_eq!(cache.rows.len(), 441);
    let mut final_page = page(419, 441, 100);
    cache.project(&mut final_page);
    assert_eq!(
        lines(&final_page),
        (1..=441)
            .map(|n| format!("line {n:04}"))
            .collect::<Vec<_>>()
    );
}
#[test]
fn full_loading_cancel_keeps_late_page_but_never_restarts_without_a_current_resume() {
    let mut cache = ArticleCache::default();
    let now = Instant::now();
    let mut first = page(1, 23, 10);
    cache.project(&mut first);
    let id = first.article.unwrap().id;
    assert_eq!(
        cache.next_full_page(&first.navigation, 1, now),
        Some(Intent::NextPage)
    );
    assert!(!cache.load_action(id + 1, false));
    assert!(cache.load_action(id, false));
    let mut late = page(22, 44, 20);
    cache.project(&mut late);
    assert_eq!(late.article.unwrap().load, Some(LoadState::Cancelled));
    assert_eq!(cache.rows.len(), 44);
    assert!(cache.next_full_page(&late.navigation, 22, now).is_none());
    assert!(cache.load_deadline().is_none());
    assert!(cache.load_action(id, true));
    assert_eq!(
        cache.next_full_page(&late.navigation, 22, now),
        Some(Intent::NextPage)
    );
    cache.input(&late.navigation, b"\x1b[D");
    assert!(!cache.load_action(id, true));
    assert!(cache.load_deadline().is_none());
}
#[test]
fn full_loading_waits_for_a_real_range_and_times_out_without_retries() {
    let mut cache = ArticleCache::default();
    let now = Instant::now();
    let mut first = page(1, 23, 10);
    cache.project(&mut first);
    assert_eq!(
        cache.next_full_page(&first.navigation, 1, now),
        Some(Intent::NextPage)
    );
    assert!(cache.next_full_page(&first.navigation, 2, now).is_none());
    assert!(!cache.expire_load(now + Duration::from_secs(5)));
    assert!(cache.expire_load(now + LOAD_RESPONSE_TIMEOUT));
    assert_eq!(cache.load_state(), Some(LoadState::Stopped));
    assert!(cache.next_full_page(&first.navigation, 3, now).is_none());
    assert!(!cache.expire_load(now + Duration::from_secs(60)));
}
#[test]
fn full_loading_observed_response_clears_network_deadline_even_while_ui_is_paused() {
    let mut cache = ArticleCache::default();
    let now = Instant::now();
    let mut first = page(50, 72, 50);
    cache.project(&mut first);
    assert_eq!(
        cache.next_full_page(&first.navigation, 50, now),
        Some(Intent::First)
    );
    let mut head = page(1, 23, 10);
    cache.project(&mut head);
    assert!(cache.load_deadline().is_none());
    assert!(!cache.expire_load(now + Duration::from_secs(60)));
    assert_eq!(
        cache.next_full_page(&head.navigation, 1, now),
        Some(Intent::NextPage)
    );
    assert_eq!(cache.rows.len(), 23);
}

#[test]
fn full_loading_changed_overlap_and_capacity_stop_without_claiming_complete() {
    let mut cache = ArticleCache::default();
    let mut first = page(1, 23, 10);
    cache.project(&mut first);
    let now = Instant::now();
    cache.next_full_page(&first.navigation, 1, now);
    let mut changed = page(22, 44, 100);
    changed.frame.glyphs[0].text = "X".into();
    cache.project(&mut changed);
    assert_eq!(cache.load_state(), Some(LoadState::Changed));
    assert_eq!(cache.rows.len(), 23);
    assert!(!changed.article.unwrap().end);
    assert!(cache.next_full_page(&changed.navigation, 22, now).is_none());
    // A narrow but complete synthetic grid exercises the production row limit
    // without allocating thousands of full-width text frames in the test.
    let mut cache = ArticleCache::default();
    for start in (1..=MAX_FULL_LINES + 22).step_by(22) {
        let mut p = page(start, start + 22, 50);
        p.frame.cols = 1;
        p.frame.glyphs.retain(|g| g.col == 0);
        cache.project(&mut p);
        assert!(cache.rows.len() <= MAX_FULL_LINES);
    }
    assert_eq!(cache.load_state(), Some(LoadState::Limit));
    assert_eq!(*cache.rows.first_key_value().unwrap().0, 1);
    assert!(cache.load_deadline().is_none());
}

#[test]
fn full_loading_matches_wrapped_physical_rows_instead_of_losing_them_to_logical_line_numbers() {
    fn wrapped(first: usize, last: usize, percent: u8, body: &[String]) -> Paint {
        let mut p = page(first, last, percent);
        let mut screen = Screen::new(80, 24).unwrap();
        for (row, line) in body.iter().enumerate() {
            screen.feed(format!("\x1b[{};1H{line}", row + 1).as_bytes());
        }
        p.frame = screen.frame();
        p.frame.revision = first as u64;
        p
    }
    let mut initial = (1..=21).map(|n| format!("row {n:04}")).collect::<Vec<_>>();
    initial.extend(["wrapped first part".into(), "wrapped second part".into()]);
    let mut next = vec!["wrapped first part".into(), "wrapped second part".into()];
    next.extend((23..=43).map(|n| format!("row {n:04}")));
    let mut cache = ArticleCache::default();
    let mut first = wrapped(1, 22, 50, &initial);
    cache.project(&mut first);
    cache.next_full_page(&first.navigation, 1, Instant::now());
    let mut last = wrapped(22, 43, 100, &next);
    cache.project(&mut last);
    initial.extend(next.iter().skip(2).cloned());
    assert_eq!(lines(&last), initial);
    assert_eq!(last.article.unwrap().load, Some(LoadState::Complete));
    let mut historic = wrapped(1, 22, 50, &initial[..23]);
    cache.project(&mut historic);
    assert_eq!(lines(&historic), initial);
    let mut tail = wrapped(22, 43, 100, &next);
    cache.project(&mut tail);
    assert_eq!(lines(&tail), initial);
}

#[test]
fn clamped_short_last_page_can_start_inside_a_wrapped_logical_line() {
    let mut cache = ArticleCache::default();
    let mut first = page(1, 22, 46);
    cache.project(&mut first);
    cache.next_full_page(&first.navigation, 1, Instant::now());
    // A last-page clamp can report logical line 9 while the first physical
    // row is the continuation of that line (display row 10).
    let mut last = page(10, 32, 100);
    last.navigation = Context::ArticleReading {
        first_line: 9,
        last_line: 31,
        percent: 100,
    };
    cache.project(&mut last);
    assert_eq!(
        lines(&last),
        (1..=32).map(|n| format!("line {n:04}")).collect::<Vec<_>>()
    );
    assert_eq!(last.article.unwrap().load, Some(LoadState::Complete));
}

#[test]
fn overlap_ignores_invisible_blank_foreground_but_preserves_visible_changes() {
    for change in ["blank_ink", "text_ink", "blank_background"] {
        let mut cache = ArticleCache::default();
        let mut first = page(1, 23, 50);
        cache.project(&mut first);
        let mut last = page(22, 44, 100);
        for glyph in last.frame.glyphs.iter_mut().filter(|g| g.row == 0) {
            if change == "text_ink" || glyph.text == " " {
                if change == "blank_background" {
                    glyph.left.bg = 4;
                    glyph.right.bg = 4;
                } else {
                    glyph.left.fg = 2;
                    glyph.right.fg = 2;
                }
            }
        }
        cache.project(&mut last);
        assert_eq!(
            last.article.as_ref().unwrap().load,
            Some(if change == "blank_ink" {
                LoadState::Complete
            } else {
                LoadState::Changed
            })
        );
        if change == "blank_ink" {
            assert_eq!(lines(&last).len(), 44);
        }
    }
}

#[test]
fn local_percent_metadata_identifies_only_ascii_status_cells_and_preserves_source() {
    let mut cache = ArticleCache::default();
    let mut p = page(1, 3, 100);
    let original = p.frame.clone();
    cache.project(&mut p);
    assert_eq!(p.article.as_ref().unwrap().percent_cells, Some(16..19));
    assert_eq!(p.source_frame.as_ref().unwrap(), &original);
    let rows = pttstella::text::rows(&p.frame).unwrap();
    assert_eq!(rows.last().unwrap().cells(16, 19), Some("100"));
    for text in [
        "(101%)",
        "(URL%)",
        "(1a%)",
        "(1%)bad",
        "(1000%)",
        "(１００%)",
    ] {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(b"\x1b[24;1H");
        screen.feed(&encoding::encode(text).unwrap());
        if text == "(1%)bad" {
            assert_eq!(footer_percent_cells(&screen.frame()), Some(1..2));
        } else {
            assert_eq!(footer_percent_cells(&screen.frame()), None, "{text}");
        }
    }
}

#[test]
fn visible_row_range_metadata_validates_both_fields_without_changing_source_cells() {
    let mut cache = ArticleCache::default();
    let mut p = page(1, 3, 100);
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(b"\x1b[24;1H");
    screen.feed(&encoding::encode("目前顯示: 第 01~03 行").unwrap());
    p.frame.glyphs.retain(|g| g.row != 23);
    p.frame
        .glyphs
        .extend(screen.frame().glyphs.into_iter().filter(|g| g.row == 23));
    let source = p.frame.clone();
    cache.project(&mut p);
    let range = p.article.as_ref().unwrap().range_cells.as_ref().unwrap();
    let rows = pttstella::text::rows(p.source_frame.as_ref().unwrap()).unwrap();
    assert_eq!(
        rows.last()
            .unwrap()
            .cells(range.first.start, range.first.end)
            .unwrap()
            .trim(),
        "01"
    );
    assert_eq!(
        rows.last()
            .unwrap()
            .cells(range.last.start, range.last.end)
            .unwrap()
            .trim(),
        "03"
    );
    assert_eq!(p.source_frame.as_ref().unwrap(), &source);
    for text in [
        "目前顯示: 第 0~3 行",
        "目前顯示: 第 9~3 行",
        "目前顯示: 第 1~x 行",
        "目前顯示: 第 １~3 行",
        "第 1~3 行",
        "目前顯示: 第 1-3 行",
    ] {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(b"\x1b[24;1H");
        screen.feed(&encoding::encode(text).unwrap());
        assert!(footer_range_cells(&screen.frame()).is_none(), "{text}");
    }
}

#[test]
fn source_page_numbers_keep_their_physical_starts_across_a_clamped_tail_and_revisit() {
    fn numbered(mut packet: Paint, page: usize, percent: usize) -> Paint {
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(b"\x1b[24;1H");
        screen.feed(
            &encoding::encode(&format!(
                " 瀏覽 第 {page}/2 頁 ({percent}%) 目前顯示: 第 1~22 行 (y)回應(X)推文(h)說明"
            ))
            .unwrap(),
        );
        packet.frame.glyphs.retain(|g| g.row != 23);
        packet
            .frame
            .glyphs
            .extend(screen.frame().glyphs.into_iter().filter(|g| g.row == 23));
        let style = Style {
            fg: if page == 1 { 7 } else { 0 },
            bg: if page == 1 { 4 } else { 6 },
            ..Style::default()
        };
        for glyph in packet.frame.glyphs.iter_mut().filter(|g| g.row == 23) {
            glyph.left = style;
            glyph.right = style;
            if glyph.col == 1 {
                glyph.right = Style {
                    fg: 3,
                    bg: 5,
                    bright: true,
                    inverse: true,
                };
            }
        }
        packet
    }
    let mut cache = ArticleCache::default();
    let original = numbered(page(1, 22, 46), 1, 46);
    let mut first = original.clone();
    cache.project(&mut first);
    let mut last = numbered(page(10, 32, 100), 2, 100);
    last.navigation = Context::ArticleReading {
        first_line: 9,
        last_line: 31,
        percent: 100,
    };
    cache.project(&mut last);
    let reading = last.article.as_ref().unwrap();
    assert_eq!(reading.page_cells, Some(8..10));
    assert_eq!(
        reading
            .pages
            .iter()
            .map(|p| (p.line, p.number))
            .collect::<Vec<_>>(),
        vec![(1, 1), (10, 2)]
    );
    assert_eq!(reading.pages[0].styles.len(), 80);
    assert_eq!(
        reading.pages[0].styles[0],
        Style {
            fg: 7,
            bg: 4,
            ..Style::default()
        }
    );
    assert_eq!(
        reading.pages[1].styles[0],
        Style {
            fg: 0,
            bg: 6,
            ..Style::default()
        }
    );
    assert_eq!(
        reading.pages[0].styles[2],
        Style {
            fg: 3,
            bg: 5,
            bright: true,
            inverse: true
        }
    );
    assert_eq!(
        footer_styles(last.source_frame.as_ref().unwrap()),
        reading.pages[1].styles
    );
    assert_eq!(reading.last_line, 32);
    let mut revisit = original;
    revisit.frame.revision += 100;
    cache.project(&mut revisit);
    assert_eq!(
        revisit
            .article
            .as_ref()
            .unwrap()
            .pages
            .iter()
            .map(|p| (p.line, p.number))
            .collect::<Vec<_>>(),
        vec![(1, 1), (10, 2)]
    );
    assert_eq!(revisit.article.as_ref().unwrap().last_line, 32);
}

#[test]
fn long_full_load_accepts_default_ink_reset_only_at_a_new_pages_leading_overlap() {
    for change in ["reset", "text", "background", "new_ink", "second_row"] {
        let mut cache = ArticleCache::default();
        for index in 0..38 {
            let start = 1 + index * 22;
            let mut packet = page(start, start + 22, if index == 37 { 100 } else { 50 });
            // A colour span starts late on page 30 and crosses its last row.
            // Page 31 redraws the same text with default ink at its first row.
            if index == 29 {
                for g in packet
                    .frame
                    .glyphs
                    .iter_mut()
                    .filter(|g| g.row == 22 || change == "second_row" && g.row == 21)
                {
                    g.left.fg = 3;
                    g.left.bright = true;
                    g.right = g.left;
                }
            }
            if index == 30 {
                if change == "text" {
                    packet.frame.glyphs[0].text = "X".into();
                }
                if change == "background" {
                    packet.frame.glyphs[0].left.bg = 4;
                }
                if change == "new_ink" {
                    packet.frame.glyphs[0].left.fg = 2;
                }
                if change == "second_row" {
                    // Two rows overlap, and a reset on the second remains invalid.
                    packet = page(start - 1, start + 21, 50);
                }
            }
            let context = packet.navigation.clone();
            let revision = packet.frame.revision;
            cache.project(&mut packet);
            if index == 30 && change != "reset" {
                assert_eq!(cache.load_state(), Some(LoadState::Changed), "{change}");
                break;
            }
            assert_ne!(
                cache.load_state(),
                Some(LoadState::Changed),
                "{change} page {}",
                index + 1
            );
            if index == 30 {
                let projected = &packet.frame.glyphs;
                let g = projected
                    .iter()
                    .find(|g| g.row == 660 && g.col == 0)
                    .unwrap();
                assert_eq!(
                    (g.left.fg, g.left.bright),
                    (3, true),
                    "retain known full-article colour"
                );
                assert_eq!(
                    packet.source_frame.as_ref().unwrap().glyphs[0].left,
                    Style::default()
                );
            }
            cache.next_full_page(&context, revision, Instant::now());
        }
        if change == "reset" {
            assert_eq!(cache.load_state(), Some(LoadState::Complete));
            assert_eq!(cache.rows.len(), 837);
        }
    }
}

#[test]
fn blank_screen_tail_is_preserved_when_ptt_repeats_its_logical_line() {
    let mut cache = ArticleCache::default();
    let mut first = page(1, 22, 30);
    cache.project(&mut first);
    assert_eq!(first.article.as_ref().unwrap().last_line, 22);
    let mut second = page(23, 45, 60);
    for g in second.frame.glyphs.iter_mut().filter(|g| g.row == 0) {
        g.text = " ".into();
    }
    second.navigation = Context::ArticleReading {
        first_line: 22,
        last_line: 44,
        percent: 60,
    };
    cache.project(&mut second);
    assert_eq!(
        second.article.as_ref().unwrap().load,
        Some(LoadState::Loading)
    );
    let mut last = page(45, 67, 100);
    last.navigation = Context::ArticleReading {
        first_line: 44,
        last_line: 66,
        percent: 100,
    };
    cache.project(&mut last);
    let mut expected: Vec<_> = (1..=22).map(|n| format!("line {n:04}")).collect();
    expected.push(String::new());
    expected.extend((24..=67).map(|n| format!("line {n:04}")));
    assert_eq!(lines(&last), expected);
    assert_eq!(
        last.article.as_ref().unwrap().load,
        Some(LoadState::Complete)
    );
}

#[test]
fn retained_blank_tail_does_not_accept_changed_text_or_background() {
    for background in [false, true] {
        let mut cache = ArticleCache::default();
        cache.project(&mut page(1, 22, 30));
        let mut changed = page(23, 45, 60);
        changed.navigation = Context::ArticleReading {
            first_line: 22,
            last_line: 44,
            percent: 60,
        };
        if background {
            for g in changed.frame.glyphs.iter_mut().filter(|g| g.row == 0) {
                g.text = " ".into();
                g.left.bg = 1;
                g.right.bg = 1;
            }
        }
        cache.project(&mut changed);
        assert_eq!(
            changed.article.as_ref().unwrap().load,
            Some(LoadState::Changed)
        );
        assert_eq!(changed.article.as_ref().unwrap().last_line, 22);
    }
}

#[test]
fn terminal_article_edges_only_switch_on_fresh_arrows_when_enabled() {
    let context = Context::ArticleReading {
        first_line: 1,
        last_line: 16,
        percent: 100,
    };
    for (name, bytes) in [
        ("ArrowDown", b"\x1b[F\x1b[B".as_slice()),
        ("ArrowUp", b"\x1b[H\x1b[A".as_slice()),
    ] {
        let mut key = pttstella::vim::Key {
            key: name.into(),
            code: name.into(),
            ctrl: false,
            alt: false,
            shift: false,
            repeat: false,
        };
        assert_eq!(
            terminal_arrow_boundary(&key, &context, false),
            Some(Vec::new())
        );
        assert_eq!(
            terminal_arrow_boundary(&key, &context, true).as_deref(),
            Some(bytes)
        );
        key.repeat = true;
        assert_eq!(
            terminal_arrow_boundary(&key, &context, true),
            Some(Vec::new())
        );
        key.repeat = false;
        key.shift = true;
        assert_eq!(terminal_arrow_boundary(&key, &context, true), None);
        key.shift = false;
        let middle = Context::ArticleReading {
            first_line: 24,
            last_line: 46,
            percent: 50,
        };
        assert_eq!(terminal_arrow_boundary(&key, &middle, true), None);
    }
    for name in ["PageUp", "PageDown", "j", "k", "End", "Home"] {
        let key = pttstella::vim::Key {
            key: name.into(),
            code: name.into(),
            ctrl: false,
            alt: false,
            shift: false,
            repeat: false,
        };
        assert_eq!(terminal_arrow_boundary(&key, &context, true), None);
    }
}

#[test]
fn refresh_appends_overlapping_tail_without_replacing_scope_and_rejects_tail_edits() {
    for changed in [false, true] {
        let mut cache = ArticleCache::default();
        cache.project(&mut page(1, 23, 51));
        let mut old = page(23, 45, 100);
        cache.project(&mut old);
        let id = old.article.as_ref().unwrap().id;
        assert_eq!(
            old.article.as_ref().unwrap().load,
            Some(LoadState::Complete)
        );
        cache.begin_refresh();
        let mut refreshed = page(23, 45, 93);
        if changed {
            refreshed
                .frame
                .glyphs
                .iter_mut()
                .find(|g| g.row == 4 && g.col == 0)
                .unwrap()
                .text = "X".into();
        }
        cache.project(&mut refreshed);
        assert_eq!(refreshed.article.as_ref().unwrap().id, id);
        if changed {
            assert_eq!(
                refreshed.article.as_ref().unwrap().load,
                Some(LoadState::Changed)
            );
            assert_eq!(lines(&refreshed), lines(&old));
        } else {
            assert_eq!(
                refreshed.article.as_ref().unwrap().load,
                Some(LoadState::Loading)
            );
            let mut appended = page(26, 48, 100);
            cache.project(&mut appended);
            assert_eq!(appended.article.as_ref().unwrap().id, id);
            assert_eq!(
                appended.article.as_ref().unwrap().load,
                Some(LoadState::Complete)
            );
            assert_eq!(
                lines(&appended),
                (1..=48).map(|n| format!("line {n:04}")).collect::<Vec<_>>()
            );
            cache.begin_refresh();
            let mut unchanged = page(26, 48, 100);
            cache.project(&mut unchanged);
            assert_eq!(
                unchanged.article.as_ref().unwrap().load,
                Some(LoadState::Complete)
            );
            assert_eq!(lines(&unchanged), lines(&appended));
        }
    }
}
