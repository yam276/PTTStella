use super::{
    geometry::{Point, Viewport},
    state::State,
};
use std::time::{Duration, Instant};
#[test]
fn five_category_targets_remain_separate_from_trigger_and_each_other_through_4k() {
    for (width, height) in [
        (580., 360.),
        (680., 520.),
        (1280., 800.),
        (1920., 1080.),
        (3840., 2160.),
    ] {
        let view = Viewport::new(width, height).unwrap();
        for left in [false, true] {
            for fraction in [0., 0.35, 0.65, 1.] {
                let point = view.dock(
                    Point {
                        x: width / 2.,
                        y: height * fraction,
                    },
                    left,
                );
                let layout = view.layout(point);
                assert!((1. ..=1.75).contains(&layout.scale));
                if layout.compact {
                    continue;
                }
                let overlap = |a: super::geometry::Rect, b: super::geometry::Rect| {
                    a.x < b.x + b.width
                        && a.x + a.width > b.x
                        && a.y < b.y + b.height
                        && a.y + a.height > b.y
                };
                for (i, r) in layout.petals.iter().enumerate() {
                    assert!(
                        r.x >= 0.
                            && r.y >= 0.
                            && r.x + r.width <= width
                            && r.y + r.height <= height
                    );
                    assert!(!overlap(*r, layout.ball));
                    for other in &layout.petals[i + 1..] {
                        assert!(!overlap(*r, *other));
                    }
                }
            }
        }
    }
    assert!(Viewport::new(f32::NAN, 800.).is_none());
}
#[test]
fn right_click_parking_and_category_back_restore_the_original_point() {
    let mut state = State::new(Viewport::new(1280., 800.).unwrap());
    let parked = state.point;
    state.show_at(Point { x: 200., y: 300. });
    assert_ne!(state.point, parked);
    assert!(state.open);
    state.category = Some(2);
    state.toggle();
    assert!(state.open);
    assert!(state.category.is_none());
    assert_eq!(state.selected, 2);
    state.toggle();
    assert!(!state.open);
    assert_eq!(state.point, parked);
}
#[test]
fn drag_release_includes_endpoint_and_reduced_motion_docks_without_a_timer() {
    let viewport = Viewport::new(1280., 800.).unwrap();
    let now = Instant::now();
    let mut state = State::new(viewport);
    let origin = state.point;
    state.down(
        Point {
            x: origin.x + 20.,
            y: origin.y + 20.,
        },
        now,
    );
    state.motion(
        Point {
            x: origin.x - 200.,
            y: origin.y - 50.,
        },
        now + Duration::from_millis(30),
        true,
    );
    assert!(!state.up(
        Point {
            x: origin.x - 230.,
            y: origin.y - 60.
        },
        now + Duration::from_millis(60),
        true
    ));
    assert_eq!(state.point.x, viewport.width - 72.);
    assert!(!state.animating());
    assert!(!state.open);
}
#[test]
fn coast_is_bounded_and_lost_button_cancel_never_opens_tools() {
    let now = Instant::now();
    let mut state = State::new(Viewport::new(1280., 800.).unwrap());
    let p = state.point;
    state.down(p, now);
    state.motion(
        Point {
            x: p.x - 150.,
            y: p.y,
        },
        now + Duration::from_millis(40),
        true,
    );
    state.up(
        Point {
            x: p.x - 200.,
            y: p.y,
        },
        now + Duration::from_millis(80),
        false,
    );
    assert!(state.animating());
    for i in 1..=200 {
        state.tick(now + Duration::from_millis(80 + i * 16));
    }
    assert!(!state.animating());
    assert!(!state.open);
    state.down(p, now);
    state.motion(
        Point { x: 100., y: 100. },
        now + Duration::from_millis(20),
        false,
    );
    assert!(!state.dragging());
    assert!(!state.animating());
    assert!(!state.open);
    state.down(p, now);
    state.motion(
        Point {
            x: p.x - 100.,
            y: p.y,
        },
        now + Duration::from_millis(20),
        true,
    );
    state.up(
        Point {
            x: p.x - 150.,
            y: p.y,
        },
        now + Duration::from_millis(40),
        false,
    );
    state.tick(now + Duration::from_secs(10));
    assert!(!state.animating());
}

#[test]
fn suspend_wall_clock_jump_stops_coast_even_when_monotonic_time_barely_moves() {
    let now = Instant::now();
    let mut state = State::new(Viewport::new(1280., 800.).unwrap());
    let p = state.point;
    state.down(p, now);
    state.motion(
        Point {
            x: p.x - 100.,
            y: p.y,
        },
        now + Duration::from_millis(20),
        true,
    );
    state.up(
        Point {
            x: p.x - 150.,
            y: p.y,
        },
        now + Duration::from_millis(40),
        false,
    );
    assert!(state.animating());
    state.tick_with_clock(
        now + Duration::from_millis(56),
        std::time::SystemTime::now() + Duration::from_secs(600),
    );
    assert!(!state.animating());
    assert!(!state.open);
}
