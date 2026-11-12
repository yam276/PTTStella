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
