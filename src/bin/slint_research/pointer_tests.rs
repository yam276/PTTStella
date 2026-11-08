use super::*;
use pttstella::{encoding, navigation, screen::Screen};
fn fixture(name: &str) -> Frame {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../../tests/fixtures/navigation.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&encoding::encode(data[name].as_str().unwrap()).unwrap());
    screen.frame()
}
fn geometry() -> Geometry {
    Geometry {
        size: Size {
            width: 960,
            height: 576,
        },
        left: 70.,
        top: 30.,
        width: 1100.,
        height: 680.,
        dpi: 1.,
    }
}
fn point(col: usize, row: usize) -> (f32, f32) {
    (70. + col as f32 * 12. + 6., 30. + row as f32 * 24. + 12.)
}
fn target(name: &str, point: (f32, f32)) -> Option<Hit> {
    let frame = fixture(name);
    hit(
        &navigation::analyze(&frame),
        &[],
        &frame,
        geometry(),
        point,
        true,
    )
}
#[test]
fn row_open_back_and_headers_follow_the_original_profiles() {
    for (name, col, row, expected) in [
        ("list", 16, 4, Some(Intent::Back)),
        ("list", 17, 4, Some(Intent::Open { row: 4 })),
        ("list", 30, 2, None),
        ("list", 0, 23, Some(Intent::Last)),
        ("boards_caption", 9, 6, Some(Intent::Back)),
        ("boards_caption", 10, 6, Some(Intent::Open { row: 6 })),
        ("main_menu", 19, 13, None),
        ("main_menu", 20, 13, Some(Intent::Open { row: 13 })),
        ("reading_middle", 1, 0, Some(Intent::Back)),
        ("reading_middle", 30, 0, Some(Intent::First)),
        ("reading_middle", 30, 6, Some(Intent::PreviousPage)),
        ("reading_middle", 30, 16, Some(Intent::NextPage)),
        ("reading_middle", 30, 22, Some(Intent::Last)),
        ("reading_middle", 30, 23, None),
    ] {
        assert_eq!(
            target(name, point(col, row)),
            expected.map(Hit::Navigate),
            "{name} {col}/{row}"
        );
    }
}
#[test]
fn visible_black_margins_and_status_row_do_not_invent_cells() {
    for (name, point, expected) in [
        ("list", (3., 1.), Some(Intent::First)),
        ("list", (3., 679.), Some(Intent::Last)),
        ("list", (500., 26.), Some(Intent::PreviousPage)),
        ("list", (500., 640.), Some(Intent::NextPage)),
        ("list", (3., 150.), Some(Intent::Back)),
        ("list", (1090., 150.), Some(Intent::Forward)),
        ("main_menu", (3., 150.), None),
        ("reading_middle", (1090., 150.), Some(Intent::Forward)),
    ] {
        let frame = fixture(name);
        assert!(geometry().cell(point, &frame).is_none());
        assert_eq!(target(name, point), expected.map(Hit::Navigate));
    }
    assert_eq!(target("list", (1100., 100.)), None);
    assert_eq!(target("list", (100., f32::NAN)), None);
    let frame = fixture("list");
    let unknown = Context::Unknown {
        reason: navigation::UnknownReason::Unrecognized,
    };
    for context in [&unknown, &Context::EmptyList] {
        assert_eq!(hit(context, &[], &frame, geometry(), (1., 1.), true), None);
    }
}
#[test]
fn links_precede_back_and_page_but_continue_consumes_the_entire_grid() {
    let frame = fixture("reading_middle");
    let context = navigation::analyze(&frame);
    let link = Link {
        row: 6,
        start: 1,
        end: 9,
        url: "https://example.test/one".into(),
    };
    for ready in [false, true] {
        assert_eq!(
            hit(
                &context,
                std::slice::from_ref(&link),
                &frame,
                geometry(),
                point(2, 6),
                ready
            ),
            Some(Hit::Link(0))
        );
        assert_eq!(
            hit(
                &Context::ContinuePrompt,
                std::slice::from_ref(&link),
                &frame,
                geometry(),
                point(2, 6),
                ready
            ),
            ready.then_some(Hit::Navigate(Intent::Continue))
        );
    }
    assert_eq!(
        hit(
            &Context::ContinuePrompt,
            &[],
            &frame,
            geometry(),
            (1., 1.),
            true
        ),
        None
    );
}
#[test]
fn fractional_clipped_geometry_uses_the_same_physical_cell_boundaries() {
    let frame = fixture("list");
    let context = navigation::analyze(&frame);
    let g = Geometry {
        size: Size {
            width: 943,
            height: 565,
        },
        left: 82.5,
        top: 55.,
        width: 1200.,
        height: 700.,
        dpi: 1.25,
    };
    let boundary = g.size.cell(17, 4, 1, &frame);
    let y = g.top + boundary.center().y;
    assert_eq!(
        hit(
            &context,
            &[],
            &frame,
            g,
            (g.left + boundary.left - 0.1, y),
            true
        ),
        Some(Hit::Navigate(Intent::Back))
    );
    assert_eq!(
        hit(&context, &[], &frame, g, (g.left + boundary.left, y), true),
        Some(Hit::Navigate(Intent::Open { row: 4 }))
    );
    let clipped = Geometry {
        left: -100.,
        top: -100.,
        width: 600.,
        height: 400.,
        ..g
    };
    assert!(clipped.cell((-1., 100.), &frame).is_none());
    assert!(clipped.cell((600., 100.), &frame).is_none());
    assert_eq!(hit(&context, &[], &frame, clipped, (601., 1.), true), None);
}
#[test]
fn wheel_only_pages_recognized_profiles_and_has_no_zero_or_nan_fallback() {
    let list = navigation::analyze(&fixture("list"));
    assert_eq!(wheel(&list, 40.), Some(Intent::PreviousPage));
    assert_eq!(wheel(&list, -40.), Some(Intent::NextPage));
    assert_eq!(wheel(&list, 0.), None);
    assert_eq!(wheel(&list, f32::NAN), None);
    assert_eq!(wheel(&Context::ContinuePrompt, 40.), None);
    assert_eq!(wheel(&Context::EmptyList, 40.), None);
}

#[test]
fn article_hit_and_visible_spans_share_odd_pixel_columns_and_pinned_footer() {
    let mut frame = Screen::new(80, 24).unwrap().frame();
    frame.rows = 838;
    let geometry = SurfaceGeometry::Article(ArticleGeometry {
        left: -10.,
        width: 700.,
        body_height: 287.5,
        offset: 6256.25,
        line_width: 1000,
        row: 25,
        body_rows: 837,
        dpi: 1.25,
    });
    assert_eq!(
        geometry.cell((53., 0.), &frame),
        Some(Position { col: 5, row: 250 })
    );
    assert_eq!(
        geometry.cell((53., 287.49), &frame),
        Some(Position { col: 5, row: 261 })
    );
    assert_eq!(
        geometry.cell((53., 287.5), &frame),
        Some(Position { col: 5, row: 837 })
    );
    assert!(geometry.cell((53., 312.5), &frame).is_none());
    let span = |row| pttstella::selection::Span {
        row,
        start: 4,
        end: 6,
    };
    assert_eq!(
        geometry.span(&span(250), &frame),
        Some(skia_safe::Rect::new(40., 0., 65., 18.75))
    );
    assert_eq!(
        geometry.span(&span(837), &frame),
        Some(skia_safe::Rect::new(40., 287.5, 65., 312.5))
    );
    assert!(geometry.span(&span(100), &frame).is_none());
    assert!(geometry.span(&span(262), &frame).is_none());
}
