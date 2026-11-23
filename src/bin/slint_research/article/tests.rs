use super::*;
use pttstella::{
    replay,
    selection::{self, Selection},
};

#[test]
fn viewport_footer_uses_local_position_and_keeps_station_glyphs_intact() {
    let display = replay::parity_display_profile().resolve().unwrap();
    let (cache, source) = fixture(&display).unwrap();
    let window = cache.window(&source, 0..1).unwrap();
    let mut reading = window.paint.article.unwrap();
    let flow = Flow::new(window.body_rows, 20, [(10, 60)]).unwrap();
    let midpoint = flow.height() as f64 / 2.;
    let position = footer::display(&reading, &flow, 20, 0., midpoint, None);
    assert_eq!(position.percent, Some(footer::Percent::Known(50)));
    assert_eq!(
        footer::display(&reading, &flow, 20, midpoint, flow.height() as f64, None).percent,
        Some(footer::Percent::Known(100))
    );
    let original = source.frame.clone();
    let rendered = footer::frame(&source.frame, &reading, position);
    let extract = |frame: &pttstella::screen::Frame, cells: &Range<usize>| {
        frame
            .glyphs
            .iter()
            .filter(|g| cells.contains(&g.col))
            .map(|g| g.text.as_str())
            .collect::<String>()
    };
    assert_eq!(
        extract(&rendered, reading.percent_cells.as_ref().unwrap()),
        " 50"
    );
    let range = reading.range_cells.as_ref().unwrap();
    assert_eq!(extract(&rendered, &range.first).trim(), "1");
    assert_eq!(extract(&rendered, &range.last).trim(), "417");
    assert_eq!(
        extract(&rendered, reading.page_cells.as_ref().unwrap()).trim(),
        "1"
    );
    assert_eq!(source.frame, original);

    reading.end = false;
    let pending = footer::display(&reading, &flow, 20, 0., midpoint, None);
    assert_eq!(pending.percent, Some(footer::Percent::Unknown));
    let rendered = footer::frame(&source.frame, &reading, pending);
    assert_eq!(
        extract(&rendered, reading.percent_cells.as_ref().unwrap()),
        " --"
    );
}

#[test]
fn footer_colours_follow_the_local_end_state_not_the_nearest_page() {
    use pttstella::screen::Style;
    let display = replay::parity_display_profile().resolve().unwrap();
    let (cache, source) = fixture(&display).unwrap();
    let window = cache.window(&source, 0..1).unwrap();
    let mut reading = window.paint.article.unwrap();
    // Distinct synthetic colours: the station's end page versus earlier pages.
    let end_style = Style {
        fg: 7,
        bg: 4,
        ..Style::default()
    };
    let middle_style = Style {
        fg: 0,
        bg: 6,
        ..Style::default()
    };
    for page in &mut reading.pages {
        page.styles = vec![if page.end { end_style } else { middle_style }; source.frame.cols];
    }
    assert_eq!(reading.pages.iter().filter(|page| page.end).count(), 1);
    let end = reading.pages.iter().find(|page| page.end).unwrap().clone();
    let flow = Flow::new(window.body_rows, 20, [(10, 60)]).unwrap();
    let cells = |frame: &pttstella::screen::Frame, cells: &Range<usize>| {
        frame
            .glyphs
            .iter()
            .filter(|g| cells.contains(&g.col))
            .map(|g| g.text.as_str())
            .collect::<String>()
    };
    let colour =
        |frame: &pttstella::screen::Frame| frame.glyphs.iter().find(|g| g.col == 0).map(|g| g.left);

    // A viewport taller than one station page reaches 100 percent while its
    // first row is still nearest an earlier page.
    let whole = footer::display(&reading, &flow, 20, 0., flow.height() as f64, None);
    assert_eq!(whole.percent, Some(footer::Percent::Known(100)));
    let rendered = footer::frame(&source.frame, &reading, whole);
    assert_eq!(colour(&rendered), Some(end_style));
    assert_eq!(
        cells(&rendered, reading.page_cells.as_ref().unwrap()).trim(),
        end.number.to_string()
    );

    // A short viewport on the end page's first rows is not at 100 percent yet.
    let top = flow.top(end.line - reading.first_line) as f64;
    let early = footer::display(&reading, &flow, 20, top, top + 100., None);
    assert!(matches!(
        early.percent,
        Some(footer::Percent::Known(0..=99))
    ));
    let rendered = footer::frame(&source.frame, &reading, early);
    assert_eq!(colour(&rendered), Some(middle_style));
    assert_eq!(
        cells(&rendered, reading.page_cells.as_ref().unwrap()).trim(),
        end.number.to_string()
    );

    // A cache without an earlier page keeps the only colours it has.
    reading.pages.retain(|page| page.end);
    let rendered = footer::frame(
        &source.frame,
        &reading,
        footer::display(&reading, &flow, 20, top, top + 100., None),
    );
    assert_eq!(colour(&rendered), Some(end_style));
}

#[test]
fn shared_long_workload_keeps_837_rows_and_historical_leading_ink_reset() {
    let display = replay::parity_display_profile().resolve().unwrap();
    let (cache, source) = fixture(&display).unwrap();
    let window = cache.window(&source, 648..676).unwrap();
    assert_eq!(window.body_rows, 837);
    let rows = (0..28)
        .map(|row| {
            selection::extract(&window.paint.frame, &Selection::Line { row })
                .unwrap()
                .text
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        (649..=676)
            .map(|n| format!("Full article synthetic line {n:04}"))
            .collect::<Vec<_>>()
    );
    let historical = window
        .paint
        .frame
        .glyphs
        .iter()
        .find(|g| g.row == 12 && g.col == 0)
        .unwrap();
    assert_eq!((historical.left.fg, historical.left.bright), (3, true));
    let tail = cache.window(&source, 819..837).unwrap();
    assert_eq!(
        selection::extract(&tail.paint.frame, &Selection::Line { row: 17 })
            .unwrap()
            .text
            .trim_end(),
        "Full article synthetic line 0837"
    );
    assert_eq!(source.frame.rows, 24);
    assert!(cache.archive_snapshot(&source.frame).is_some());
    assert!(replay::long_load_page(0, Some("http://127.0.0.1:9")).is_err());
    assert!(replay::full_load_page(21, Some("http://127.0.0.1:9")).is_err());
}
