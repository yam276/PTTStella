use super::*;
use pttstella::{
    connection::{CloseOutcome, View},
    mouse::{
        self, Action,
        input::{Lease, SampleReport},
    },
    screen::Screen,
};
use state::{CADENCE, Context, Effect, State};
use std::time::{Duration, SystemTime};
async fn scene(tracking: u16) -> (Connection, View) {
    let connection = Connection::start_ui_probe(
        format!("\x1b[2J\x1b[HSELF AUTHORED MOUSE SCENE\x1b[?{tracking};1006h").into_bytes(),
    )
    .unwrap();
    connection.connect().unwrap();
    let mut updates = connection.updates.clone();
    let view = tokio::time::timeout(Duration::from_secs(4), updates.wait_for(|v| v.received > 0))
        .await
        .unwrap()
        .unwrap()
        .clone();
    (connection, view)
}
fn context(view: &View, now: Instant, buttons: u8) -> Context<'_> {
    Context {
        view,
        geometry: Some(Geometry {
            size: Size {
                width: 960,
                height: 576,
            },
            left: 0.,
            top: 0.,
            width: 960.,
            height: 576.,
            dpi: 1.,
        }),
        presented: true,
        blocked: false,
        selected: false,
        buttons,
        now,
        wall: SystemTime::now(),
    }
}
fn point(col: usize, row: usize) -> (f32, f32) {
    (col as f32 * 12. + 6., row as f32 * 24. + 12.)
}
fn bytes(effects: &[Effect], view: &View) -> Vec<Vec<u8>> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::Edge(e) => Some(mouse::encode(view.mouse.modes, &e.report, 80, 24).unwrap()),
            Effect::Sample(s) => Some(match s.report {
                SampleReport::Motion(r) => {
                    mouse::encode_motion(view.mouse.modes, &r, 80, 24).unwrap()
                }
                SampleReport::Wheel(r) => {
                    mouse::encode_wheel(view.mouse.modes, &r, 80, 24).unwrap()
                }
            }),
            _ => None,
        })
        .collect()
}
fn lease(effects: &[Effect]) -> Lease {
    effects
        .iter()
        .find_map(|e| {
            if let Effect::Edge(e) = e {
                Some(e.lease)
            } else {
                None
            }
        })
        .unwrap()
}
#[tokio::test]
async fn original_press_lease_survives_a_normal_redraw_before_release() {
    let (connection, mut view) = scene(1002).await;
    let now = Instant::now();
    let mut state = State::default();
    let (consumed, effects) = state.pointer(
        &context(&view, now, 1),
        0,
        Some(Button::Left),
        point(4, 4),
        Modifiers::default(),
        false,
    );
    assert!(consumed);
    assert_eq!(bytes(&effects, &view), [b"\x1b[<0;5;5M".to_vec()]);
    let source = lease(&effects);
    view.input_epoch += 1;
    view.frame.revision += 1;
    view.mouse.active = Some(source);
    let mut c = context(&view, now + Duration::from_millis(1), 0);
    c.presented = false;
    assert!(state.observe(&c).is_empty());
    assert!(state.active());
    let (_, effects) = state.pointer(
        &c,
        2,
        Some(Button::Left),
        point(10, 4),
        Modifiers::default(),
        false,
    );
    assert_eq!(bytes(&effects, &view), [b"\x1b[<0;11;5m".to_vec()]);
    assert_eq!(lease(&effects), source);
    assert!(!state.active());
    assert!(state.wake_after(now).is_none());
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
}
#[tokio::test]
async fn bounded_latest_samples_prioritize_wheel_and_discard_stale_passive_tail() {
    let (connection, mut view) = scene(1003).await;
    let now = Instant::now();
    let mut state = State::default();
    state.pointer(
        &context(&view, now, 0),
        1,
        None,
        point(3, 4),
        Modifiers::default(),
        false,
    );
    for _ in 0..4 {
        state.wheel(
            &context(&view, now, 0),
            point(4, 4),
            10.,
            Modifiers::default(),
            false,
        );
    }
    let effects = state.flush(&context(&view, now, 0), false);
    assert_eq!(bytes(&effects, &view), [b"\x1b[<64;5;5M".to_vec()]);
    assert!(state.flush(&context(&view, now, 0), false).is_empty());
    for col in 5..=14 {
        state.pointer(
            &context(&view, now + Duration::from_millis(10), 0),
            1,
            None,
            point(col, 4),
            Modifiers::default(),
            false,
        );
    }
    assert!(
        state
            .flush(&context(&view, now + Duration::from_millis(10), 0), false)
            .is_empty()
    );
    assert_eq!(
        state.wake_after(now + Duration::from_millis(10)),
        Some(Duration::from_millis(24))
    );
    let effects = state.flush(&context(&view, now + CADENCE, 0), false);
    assert_eq!(bytes(&effects, &view), [b"\x1b[<35;15;5M".to_vec()]);
    state.wheel(
        &context(&view, now + CADENCE, 0),
        point(4, 4),
        -8000.,
        Modifiers::default(),
        false,
    );
    view.input_epoch += 1;
    assert!(state.observe(&context(&view, now + CADENCE, 0)).is_empty());
    assert!(state.wake_after(now).is_none());
    assert!(
        state
            .flush(&context(&view, now + Duration::from_secs(10), 0), false)
            .is_empty()
    );
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
}
#[tokio::test]
async fn shift_at_down_keeps_local_selection_and_context_menu_when_modifier_is_released() {
    let (connection, mut view) = scene(1002).await;
    let now = Instant::now();
    for (button, mask) in [(Button::Left, 1), (Button::Right, 2)] {
        let mut state = State::default();
        let shift = Modifiers {
            shift: true,
            ..Default::default()
        };
        let (consumed, effects) = state.pointer(
            &context(&view, now, mask),
            0,
            Some(button),
            point(4, 4),
            shift,
            false,
        );
        assert!(!consumed);
        assert!(bytes(&effects, &view).is_empty());
        view.mouse.policy_epoch += 1;
        assert!(
            !state
                .pointer(
                    &context(&view, now, mask),
                    1,
                    None,
                    point(10, 4),
                    Modifiers::default(),
                    false
                )
                .0
        );
        assert!(
            !state
                .pointer(
                    &context(&view, now, 0),
                    2,
                    Some(button),
                    point(10, 4),
                    Modifiers::default(),
                    false
                )
                .0
        );
        assert!(!state.active());
        assert!(state.wake_after(now).is_none());
    }
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
}
#[tokio::test]
async fn lost_button_meta_release_geometry_policy_and_timeout_cancel_without_release() {
    let (connection, view) = scene(1002).await;
    let now = Instant::now();
    for case in 0..5 {
        let mut state = State::default();
        let (_, press) = state.pointer(
            &context(&view, now, 1),
            0,
            Some(Button::Left),
            point(4, 4),
            Modifiers::default(),
            false,
        );
        let source = lease(&press);
        let mut changed = view.clone();
        let mut c = context(&view, now, 1);
        let effects = match case {
            0 => {
                c.buttons = 0;
                state
                    .pointer(&c, 1, None, point(5, 4), Modifiers::default(), false)
                    .1
            }
            1 => {
                state
                    .pointer(
                        &c,
                        2,
                        Some(Button::Left),
                        point(5, 4),
                        Modifiers::default(),
                        true,
                    )
                    .1
            }
            2 => {
                c.geometry.as_mut().unwrap().left = 1.;
                state.observe(&c)
            }
            3 => {
                changed.mouse.policy_epoch += 1;
                state.observe(&context(&changed, now, 1))
            }
            _ => {
                c.now += mouse::input::HOLD_LIMIT;
                state.observe(&c)
            }
        };
        assert!(
            effects
                .iter()
                .any(|e| matches!(e,Effect::Cancel(lease) if *lease==source)),
            "case {case}"
        );
        assert!(bytes(&effects, &view).is_empty());
        assert!(!state.active());
        assert!(state.wake_after(now).is_none());
    }
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
}
#[tokio::test]
async fn blocked_and_cancel_revoke_once_and_clear_even_an_unheld_sample() {
    let (connection, view) = scene(1003).await;
    let now = Instant::now();
    let mut state = State::default();
    state.pointer(
        &context(&view, now, 0),
        1,
        None,
        point(4, 4),
        Modifiers::default(),
        false,
    );
    let mut c = context(&view, now, 0);
    c.blocked = true;
    let effects = state.observe(&c);
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Revoke(_)))
            .count(),
        1
    );
    assert!(state.observe(&c).is_empty());
    assert!(state.wake_after(now).is_none());
    c.blocked = false;
    state.pointer(&c, 1, None, point(4, 4), Modifiers::default(), false);
    assert!(
        state
            .revoke(view.generation)
            .iter()
            .any(|e| matches!(e, Effect::Revoke(_)))
    );
    assert!(state.flush(&c, false).is_empty());
    assert!(state.wake_after(now).is_none());
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
}
#[tokio::test]
async fn unsupported_mode_and_unpresented_scene_never_send_or_fall_back() {
    let (connection, view) = scene(1000).await;
    let now = Instant::now();
    let mut state = State::default();
    let mut c = context(&view, now, 1);
    c.presented = false;
    let (consumed, effects) = state.pointer(
        &c,
        0,
        Some(Button::Right),
        point(4, 4),
        Modifiers::default(),
        false,
    );
    assert!(consumed);
    assert!(bytes(&effects, &view).is_empty());
    let mut changed = view.clone();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(b"\x1b[?1001;1006h");
    changed.mouse.modes = screen.mouse_modes();
    changed.mouse.route = Route::Blocked;
    let (consumed, effects) = state.pointer(
        &context(&changed, now, 1),
        0,
        Some(Button::Left),
        point(4, 4),
        Modifiers::default(),
        false,
    );
    assert!(consumed);
    assert!(bytes(&effects, &view).is_empty());
    let shift = Modifiers {
        shift: true,
        ..Default::default()
    };
    assert!(
        !state
            .pointer(
                &context(&changed, now, 1),
                0,
                Some(Button::Left),
                point(4, 4),
                shift,
                false
            )
            .0
    );
    assert!(!state.active());
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
}
#[tokio::test]
async fn release_flushes_only_latest_owned_motion_before_its_edge() {
    let (connection, view) = scene(1002).await;
    let now = Instant::now();
    let mut state = State::default();
    let (_, press) = state.pointer(
        &context(&view, now, 1),
        0,
        Some(Button::Left),
        point(4, 4),
        Modifiers::default(),
        false,
    );
    let source = lease(&press);
    state.pointer(
        &context(&view, now, 1),
        1,
        None,
        point(6, 4),
        Modifiers::default(),
        false,
    );
    state.pointer(
        &context(&view, now, 1),
        1,
        None,
        point(10, 4),
        Modifiers::default(),
        false,
    );
    let (_, effects) = state.pointer(
        &context(&view, now, 0),
        2,
        Some(Button::Left),
        point(10, 4),
        Modifiers::default(),
        false,
    );
    assert_eq!(
        bytes(&effects, &view),
        [b"\x1b[<32;11;5M".to_vec(), b"\x1b[<0;11;5m".to_vec()]
    );
    assert!(effects.iter().all(|e| match e {
        Effect::Edge(e) => e.lease == source && e.report.action == Action::Release,
        Effect::Sample(s) => s.lease == source,
        _ => false,
    }));
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
}

#[tokio::test]
async fn stalled_or_sleeping_ui_discards_pending_samples_with_unchanged_source() {
    let (connection, view) = scene(1003).await;
    let now = Instant::now();
    for wall_only in [false, true] {
        let mut state = State::default();
        let c = context(&view, now, 0);
        state.pointer(&c, 1, None, point(4, 4), Modifiers::default(), false);
        let mut later = context(&view, now, 0);
        later.wall = c.wall + Duration::from_secs(1);
        if !wall_only {
            later.now += Duration::from_secs(1);
        }
        assert!(state.flush(&later, false).is_empty());
        assert!(state.wake_after(later.now).is_none());
    }
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
}
