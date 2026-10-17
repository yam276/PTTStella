use super::*;
use pttstella::{screen::Screen, selection::Selection};

fn view() -> View {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/filters.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&pttstella::encoding::encode(fixture["reading"].as_str().unwrap()).unwrap());
    View {
        output_pending: false,
        frame: screen.frame(),
        generation: 7,
        input_epoch: 0,
        input_pending: false,
        login_pending: false,
        phase: Phase::Connected,
        message: String::new(),
        received: 1,
        keep_alive: false,
        keep_alive_ready: false,
        keep_alive_sent: 0,
        keep_alive_failure: None,
    }
}

#[test]
fn retained_frame_cannot_authorize_interactions_during_an_output_batch() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut view = view();
    let identity = displayed(&mut state, &view);
    hover(&mut state, identity, &view);
    state.select(identity, 5, &Selection::All, &view).unwrap();
    let lease = state.lease_export(identity, None, view.input_epoch);
    assert!(lease.check(&view).is_ok());
    // Direct worker/watch observation must revoke a write even before update().
    view.output_pending = true;
    assert!(lease.check(&view).is_err());
    assert!(!state.painted(&view));
    assert!(state.current(identity, &view).is_none());
    assert!(state.export_rows(identity, None, &view).is_err());
    assert!(!state.copied(identity, 5, &view));
    assert!(state.update(&view));
    let packet = state
        .queue(&view, false, &DisplaySettings::default())
        .unwrap();
    assert!(packet.output_pending && packet.input_pending);
    assert_eq!(packet.frame, view.frame); // Never the half-written source.
    assert!(packet.links.is_empty() && packet.hover.is_none());
    assert_eq!(packet.navigation.kind(), "unknown");
    state.acknowledge(identity).unwrap();
    assert!(!state.navigation_shown(view.stamp()));
    assert!(!state.browse_shown(view.generation, &view.context()));
    view.output_pending = false;
    view.frame.revision += 1;
    state.update(&view);
    let resumed = displayed(&mut state, &view);
    assert!(state.current(resumed, &view).is_some());
    assert!(state.export_rows(resumed, None, &view).is_ok());
    assert!(lease.check(&view).is_err());
}
fn displayed(state: &mut SurfaceState, view: &View) -> Identity {
    let identity = Identity::of(
        state
            .queue(view, false, &DisplaySettings::default())
            .unwrap(),
    );
    state.acknowledge(identity).unwrap();
    identity
}
fn hover(state: &mut SurfaceState, identity: Identity, view: &View) {
    let link = state.current(identity, view).unwrap().links[0].clone();
    state.begin_hover(Lease {
        sequence: 8,
        stamp: view.stamp(),
        index: 0,
        link,
    });
}

#[test]
fn only_one_paint_can_be_in_flight_and_unknown_ack_preserves_the_waiter() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut latest = view();
    let initial = Identity::of(
        state
            .queue(&latest, false, &DisplaySettings::default())
            .unwrap(),
    );
    assert!(!state.painted(&latest));
    for revision in [100, 101, 102] {
        latest.frame.revision = revision;
        state.update(&latest);
        assert!(
            state
                .queue(&latest, false, &DisplaySettings::default())
                .is_none()
        );
    }
    assert!(
        state
            .acknowledge(Identity {
                revision: 999,
                ..initial
            })
            .is_err()
    );
    assert!(state.has_in_flight());
    state.acknowledge(initial).unwrap();
    assert!(state.acknowledge(initial).is_err());
    let newest = Identity::of(
        state
            .queue(&latest, false, &DisplaySettings::default())
            .unwrap(),
    );
    assert_eq!(newest.revision, 102);
    state.acknowledge(newest).unwrap();
    assert!(state.painted(&latest));
    assert!(
        state
            .queue(&latest, false, &DisplaySettings::default())
            .is_none()
    );
}

#[test]
fn rule_changes_invalidate_selection_hover_copy_and_png_until_the_new_ack() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let view = view();
    let old = displayed(&mut state, &view);
    hover(&mut state, old, &view);
    let text = state
        .select(old, 11, &Selection::Line { row: 5 }, &view)
        .unwrap();
    assert_eq!(text.rows, 5..6);
    assert!(state.copied(old, 11, &view));
    assert_eq!(state.export_rows(old, Some(11), &view).unwrap(), 5..6);
    state.replace_rules(filters::Rules::default().edit("Alice", "", false).unwrap());
    assert!(!state.clear_hover());
    assert!(!state.copied(old, 11, &view));
    assert!(state.export_rows(old, None, &view).is_err());
    let masked = displayed(&mut state, &view);
    assert_eq!(state.current(masked, &view).unwrap().masked_pushes, 2);
    assert!(state.select(masked, 12, &Selection::All, &view).is_some());
    assert!(state.reveal(masked, true, &view));
    assert!(!state.copied(masked, 12, &view));
    assert!(state.export_rows(masked, Some(12), &view).is_err());
    assert!(!state.reveal(old, false, &view));
    let revealed = displayed(&mut state, &view);
    assert!(state.current(revealed, &view).unwrap().revealed);
    assert_eq!(state.export_rows(revealed, None, &view).unwrap(), 0..24);
}

#[test]
fn session_frame_and_input_transitions_revoke_only_the_appropriate_interactions() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let original = view();
    let identity = displayed(&mut state, &original);
    hover(&mut state, identity, &original);
    state
        .select(identity, 5, &Selection::All, &original)
        .unwrap();
    assert!(!state.cancel_hover(99));
    let mut input = original.clone();
    input.input_epoch += 1;
    assert!(state.update(&input)); // Hover cannot survive new input.
    assert!(!state.navigation_shown(input.stamp()));
    assert!(state.browse_shown(
        input.generation,
        &pttstella::navigation::analyze(&input.frame)
    ));
    let mut next = original.clone();
    next.frame.revision += 1;
    state.update(&next);
    assert!(!state.copied(identity, 5, &next));
    assert!(state.export_rows(identity, Some(5), &next).is_err());
    let next_id = displayed(&mut state, &next);
    hover(&mut state, next_id, &next);
    let mut session = next.clone();
    session.generation += 1;
    assert!(state.update(&session));
    assert!(state.current(next_id, &session).is_none());
    assert!(!state.browse_shown(
        session.generation,
        &pttstella::navigation::analyze(&session.frame)
    ));
}

#[test]
fn a_display_change_during_paint_retains_the_requested_redraw() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let view = view();
    let old = Identity::of(
        state
            .queue(&view, false, &DisplaySettings::default())
            .unwrap(),
    );
    state.display_changed();
    assert!(state.dirty());
    state.acknowledge(old).unwrap();
    let settings = DisplaySettings {
        font_size: 18.0,
        baseline: 19.0,
        ..Default::default()
    };
    let packet = state.queue(&view, false, &settings).unwrap();
    assert_eq!(packet.settings.font_size, 18.0);
    assert_eq!(packet.settings.baseline, 19.0);
}

#[test]
fn selection_clear_is_scoped_and_does_not_cancel_a_full_page_export() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let view = view();
    let identity = displayed(&mut state, &view);
    state.select(identity, 11, &Selection::All, &view).unwrap();
    let selected = state.lease_export(identity, Some(11), view.input_epoch);
    state.clear_selection(identity, 10);
    assert!(selected.check(&view).is_ok());
    assert!(state.copied(identity, 11, &view));
    state.clear_selection(identity, 11);
    assert!(selected.check(&view).is_err());
    assert!(!state.copied(identity, 11, &view));
    state.select(identity, 12, &Selection::All, &view).unwrap();
    let full = state.lease_export(identity, None, view.input_epoch);
    state.clear_selection(identity, 12);
    assert!(full.check(&view).is_ok());
}
