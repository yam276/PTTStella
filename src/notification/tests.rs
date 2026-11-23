use super::*;
fn state(reduced: bool) -> State {
    State::new(Policy::embedded().unwrap(), reduced)
}
fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}
#[test]
fn semantic_deadlines_and_error_priority() {
    for (kind, wait) in [(Kind::Copy, 2000), (Kind::Success, 3000)] {
        let mut s = state(false);
        assert!(s.show(ms(0), kind));
        s.expire(ms(wait - 1));
        assert!(s.visible() && !s.fading());
        s.expire(ms(wait));
        assert!(s.fading());
        s.expire(ms(wait + 499));
        assert!(s.visible());
        s.expire(ms(wait + 500));
        assert!(!s.visible());
    }
    let mut s = state(false);
    assert!(s.show(ms(0), Kind::Error));
    for kind in [Kind::Copy, Kind::Success] {
        assert!(!s.show(ms(600_000), kind));
        assert!(s.error());
        assert_eq!(s.delay(), None);
    }
    s.clear();
    assert!(s.show(ms(600_000), Kind::Copy));
    assert!(!s.error());
}
#[test]
fn persistent_messages_and_replacement_reset_the_clock() {
    let mut s = state(false);
    s.show(ms(0), Kind::Persistent);
    s.expire(ms(600_000));
    assert!(s.visible() && s.delay().is_none());
    s.show(ms(600_000), Kind::Success);
    s.expire(ms(602_900));
    s.show(ms(602_900), Kind::Success);
    s.expire(ms(605_899));
    assert!(s.visible() && !s.fading());
    s.expire(ms(605_900));
    assert!(s.fading());
    s.show(ms(606_000), Kind::Error);
    s.expire(ms(900_000));
    assert!(s.error());
}
#[test]
fn hover_and_focus_pause_wait_and_fade_without_consuming_paused_time() {
    let mut s = state(false);
    s.show(ms(0), Kind::Copy);
    s.interaction(ms(1000), true, false, false);
    s.interaction(ms(61_000), true, true, false);
    s.interaction(ms(61_000), false, true, false);
    assert!(s.delay().is_none());
    s.interaction(ms(62_000), false, false, false);
    assert_eq!(s.delay(), Some(ms(1000)));
    s.expire(ms(63_000));
    assert!(s.fading());
    s.interaction(ms(63_200), true, false, false);
    assert!(!s.fading() && s.visible());
    s.interaction(ms(123_200), false, false, false);
    assert_eq!(s.delay(), Some(ms(300)));
    s.expire(ms(123_499));
    assert!(s.visible());
    s.expire(ms(123_500));
    assert!(!s.visible());
}
#[test]
fn reduced_motion_skips_fade_and_can_change_while_paused() {
    let mut s = state(true);
    s.show(ms(0), Kind::Copy);
    s.expire(ms(2000));
    assert!(!s.visible());
    let mut s = state(false);
    s.show(ms(0), Kind::Success);
    s.expire(ms(3000));
    s.interaction(ms(3100), true, false, false);
    s.interaction(ms(3200), true, false, true);
    assert!(s.visible() && !s.fading() && s.delay().is_none());
    s.interaction(ms(4000), false, false, true);
    assert!(!s.visible());
}
#[test]
fn stop_revokes_future_work_and_clear_removes_old_interaction() {
    let mut s = state(false);
    s.show(ms(0), Kind::Copy);
    s.interaction(ms(1000), true, true, false);
    s.clear();
    s.show(ms(1000), Kind::Copy);
    assert_eq!(s.delay(), Some(ms(2000)));
    s.stop();
    s.expire(ms(4000));
    assert!(!s.show(ms(5000), Kind::Error));
    assert!(!s.visible() && s.delay().is_none());
}
