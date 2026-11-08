use super::*;
use pttstella::{
    replay,
    selection::{self, Selection},
};

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
