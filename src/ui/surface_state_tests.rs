use super::*;
use pttstella::{screen::Screen, selection::Selection};

fn view() -> View {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/filters.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&pttstella::encoding::encode(fixture["reading"].as_str().unwrap()).unwrap());
    View {
        site: pttstella::sites::Target::default(),
        mouse: pttstella::mouse::input::Snapshot::default(),
        output_pending: false,
        frame: screen.frame(),
        generation: 7,
        input_epoch: 0,
        input_pending: false,
        login_pending: false,
        login_guidance: pttstella::login::Guidance::Idle,
        destination: pttstella::navigation_job::Progress::Idle,
        board_visit: None,
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
fn cached_history_is_an_acknowledged_filtered_projection_with_its_own_link_indices() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut view = view();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(
        &super::super::probe::surface_sample(
            Some("inline_pages"),
            true,
            Some("http://127.0.0.1:1234"),
        )
        .unwrap(),
    );
    view.frame = screen.frame();
    let old = displayed(&mut state, &view);
    state.article_input(&view.context(), b"\x1b[6~");
    screen.feed(&super::super::probe::inline_second_page(Some("http://127.0.0.1:1234")).unwrap());
    view.frame = screen.frame();
    state.update(&view);
    assert!(state.current(old, &view).is_none());
    let fresh = displayed(&mut state, &view);
    let cached = state.current(fresh, &view).unwrap();
    assert_eq!(cached.frame.rows, 47);
    assert_eq!(cached.article.as_ref().unwrap().first_line, 1);
    assert_eq!(cached.source_frame.as_ref().unwrap().rows, 24);
    let historical_link = cached.links.iter().position(|link| link.row == 2).unwrap();
    assert_eq!(
        state
            .copy_text(
                fresh,
                &super::super::protocol::CopyTarget::Link {
                    index: historical_link
                },
                &view
            )
            .unwrap(),
        "http://127.0.0.1:1234/colours.png"
    );
    assert!(
        state
            .select(fresh, 42, &Selection::Line { row: 0 }, &view)
            .unwrap()
            .text
            .starts_with("Inline image synthetic reading page")
    );
    assert_eq!(state.export_rows(fresh, Some(42), &view).unwrap(), 0..1);
    state
        .select(fresh, 43, &Selection::Line { row: 32 }, &view)
        .unwrap();
    assert_eq!(state.export_rows(fresh, Some(43), &view).unwrap(), 32..33);
    state.select(fresh, 44, &Selection::All, &view).unwrap();
    assert!(state.export_rows(fresh, Some(44), &view).is_err());
    state.replace_rules(filters::Rules::default());
    assert!(
        state
            .copy_text(
                fresh,
                &super::super::protocol::CopyTarget::Link {
                    index: historical_link
                },
                &view
            )
            .is_none()
    );
    let filtered = displayed(&mut state, &view);
    assert_eq!(
        state
            .current(filtered, &view)
            .unwrap()
            .article
            .as_ref()
            .unwrap()
            .first_line,
        24
    );
    assert!(state.set_raw_reading(true));
    let raw = displayed(&mut state, &view);
    let raw = state.current(raw, &view).unwrap();
    assert!(raw.article.is_none() && raw.source_frame.is_none());
    assert_eq!(raw.frame, view.frame);
}

#[test]
fn retained_frame_cannot_authorize_interactions_during_an_output_batch() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut view = view();
    let identity = displayed(&mut state, &view);
    hover(&mut state, identity, &view);
    state.select(identity, 5, &Selection::All, &view).unwrap();
    let lease = state.lease_output(identity, None, view.input_epoch);
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
    assert_eq!(packet.source_frame.as_ref(), Some(&view.frame)); // Original source stays retained.
    assert_eq!(packet.frame.glyphs, view.frame.glyphs); // Cached body never reads partial source bytes.
    assert!(!packet.frame.cursor_visible);
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
fn row_masks_and_id_highlights_rebuild_cached_and_terminal_outputs() {
    use pttstella::{filters::Policy, settings::ArticleMode, text};
    for mode in [ArticleMode::Web, ArticleMode::Terminal] {
        let mut state = SurfaceState::new(filters::Rules::default());
        let view = view();
        let settings = DisplaySettings::default();
        state.set_article_mode(mode);
        let old = displayed(&mut state, &view);
        let rules = filters::Rules::default()
            .edit_policy("Alice", "", Policy::MaskRow, false)
            .unwrap()
            .edit_policy("Bob", "", Policy::Allow, false)
            .unwrap();
        state.replace_rules(rules);
        assert!(state.current(old, &view).is_none());
        let fresh = Identity::of(state.queue(&view, false, &settings).unwrap());
        state.acknowledge(fresh).unwrap();
        let packet = state.current(fresh, &view).unwrap();
        for frame in [
            &packet.frame,
            packet.source_frame.as_ref().unwrap_or(&packet.frame),
        ] {
            assert_eq!(text::rows(frame).unwrap()[5].text, "█".repeat(80));
            assert_eq!(text::rows(frame).unwrap()[6].text, "█".repeat(80));
            assert!(
                frame
                    .glyphs
                    .iter()
                    .filter(|g| g.row == 8 && (3..6).contains(&g.col))
                    .all(|g| g.left.fg == 6 && g.left.bright)
            );
        }
        assert!(!packet.links.iter().any(|l| l.url.contains("hidden.png")));
        assert!(state.reveal(fresh, true, &view));
        let revealed = displayed(&mut state, &view);
        assert!(
            text::rows(&state.current(revealed, &view).unwrap().frame).unwrap()[5]
                .text
                .contains("Alice")
        );
    }
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
    let selected = state.lease_output(identity, Some(11), view.input_epoch);
    state.clear_selection(identity, 10);
    assert!(selected.check(&view).is_ok());
    assert!(state.copied(identity, 11, &view));
    state.clear_selection(identity, 11);
    assert!(selected.check(&view).is_err());
    assert!(!state.copied(identity, 11, &view));
    state.select(identity, 12, &Selection::All, &view).unwrap();
    let full = state.lease_output(identity, None, view.input_epoch);
    state.clear_selection(identity, 12);
    assert!(full.check(&view).is_ok());
}

#[test]
fn mouse_press_consumes_the_acknowledged_mode_but_owned_release_can_cross_paints() {
    use pttstella::mouse::{
        Action, Button, Modifiers, Report,
        input::{Event, Lease, Route},
    };
    let mut view = view();
    let mut modes = pttstella::screen::Screen::new(80, 24).unwrap();
    modes.feed(b"\x1b[?1000;1006h");
    view.mouse.modes = modes.mouse_modes();
    view.mouse.route = Route::Station;
    view.mouse.ready = true;
    let event = Event {
        lease: Lease {
            stamp: view.stamp(),
            mode_epoch: view.mouse.modes.epoch(),
            policy_epoch: 0,
            transaction: 1,
        },
        report: Report {
            col: 4,
            row: 3,
            button: Button::Left,
            action: Action::Press,
            modifiers: Modifiers::default(),
        },
    };
    let mut state = SurfaceState::new(filters::Rules::default());
    assert!(!state.mouse_shown(&event));
    let sample = pttstella::mouse::input::Sample {
        lease: event.lease,
        sequence: 1,
        report: pttstella::mouse::input::SampleReport::Wheel(pttstella::mouse::WheelReport {
            col: 4,
            row: 3,
            direction: pttstella::mouse::WheelDirection::Up,
            modifiers: Modifiers::default(),
        }),
    };
    assert!(!state.mouse_sample_shown(&sample));
    displayed(&mut state, &view);
    assert!(state.mouse_shown(&event));
    assert!(state.mouse_sample_shown(&sample));
    let mut invalid = event;
    invalid.lease.mode_epoch += 1;
    assert!(!state.mouse_shown(&invalid));
    invalid = event;
    invalid.lease.policy_epoch += 1;
    assert!(!state.mouse_shown(&invalid));
    invalid = event;
    invalid.lease.stamp.input_epoch += 1;
    assert!(!state.mouse_shown(&invalid));
    let mut next = view.clone();
    next.input_epoch += 1;
    next.frame.revision += 1;
    state.update(&next);
    displayed(&mut state, &next);
    assert!(!state.mouse_shown(&event));
    assert!(!state.mouse_sample_shown(&sample));
    next.mouse.active = Some(event.lease);
    state.update(&next);
    displayed(&mut state, &next);
    assert!(state.mouse_sample_shown(&sample));
    let mut invalid_sample = sample;
    invalid_sample.lease.mode_epoch += 1;
    assert!(!state.mouse_sample_shown(&invalid_sample));
    next.output_pending = true;
    state.update(&next);
    displayed(&mut state, &next);
    assert!(!state.mouse_sample_shown(&sample));
    next.output_pending = false;
    let release = Event {
        report: Report {
            action: Action::Release,
            ..event.report
        },
        ..event
    };
    assert!(state.mouse_shown(&release)); // The Session still verifies this original lease.
    next.mouse.route = Route::Blocked;
    state.update(&next);
    displayed(&mut state, &next);
    let mut fresh = event;
    fresh.lease.stamp = next.stamp();
    assert!(!state.mouse_shown(&fresh));
}

#[test]
fn inline_expansion_requires_current_filtered_acknowledgement_and_input_epoch() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut view = view();
    let id = displayed(&mut state, &view);
    assert!(state.inline_page(id, view.stamp(), &view).is_some());
    view.input_pending = true;
    assert!(state.inline_page(id, view.stamp(), &view).is_none());
    view.input_pending = false;
    view.phase = Phase::Disconnected;
    assert!(state.inline_page(id, view.stamp(), &view).is_none());
    view.phase = Phase::Connected;
    let old = view.stamp();
    view.input_epoch += 1;
    assert!(state.inline_page(id, old, &view).is_none());
    assert!(state.inline_page(id, view.stamp(), &view).is_none());
    state.update(&view);
    let id = displayed(&mut state, &view);
    state.replace_rules(filters::Rules::default());
    assert!(state.inline_page(id, view.stamp(), &view).is_none());
}

#[test]
fn context_copy_uses_acknowledged_filtered_text_and_exact_selection_request() {
    use super::super::protocol::CopyTarget;
    let mut state = SurfaceState::new(filters::Rules::default().edit("Alice", "", false).unwrap());
    let mut view = view();
    let identity = displayed(&mut state, &view);
    state
        .select(identity, 41, &Selection::Line { row: 5 }, &view)
        .unwrap();
    let selected = CopyTarget::Selection { request: 41 };
    assert!(
        !state
            .copy_text(identity, &selected, &view)
            .unwrap()
            .contains("機密測試內容")
    );
    assert!(
        state
            .copy_text(identity, &CopyTarget::Selection { request: 40 }, &view)
            .is_none()
    );
    assert_eq!(
        state
            .copy_text(identity, &CopyTarget::Link { index: 0 }, &view)
            .as_deref(),
        Some("https://example.com/visible.png")
    );
    assert!(
        state
            .copy_text(identity, &CopyTarget::Link { index: 1 }, &view)
            .is_none()
    );
    state.clear_selection(identity, 41);
    assert!(state.copy_text(identity, &selected, &view).is_none());
    view.frame.revision += 1;
    assert!(
        state
            .copy_text(identity, &CopyTarget::Link { index: 0 }, &view)
            .is_none()
    );
    view.frame.revision -= 1;
    state.replace_rules(filters::Rules::default());
    assert!(
        state
            .copy_text(identity, &CopyTarget::Link { index: 0 }, &view)
            .is_none()
    );
}

#[test]
fn terminal_mode_revokes_old_acks_and_inline_but_keeps_station_links() {
    use pttstella::{navigation::Intent, settings::ArticleMode};
    let mut state = SurfaceState::new(filters::Rules::default());
    let view = view();
    let old = displayed(&mut state, &view);
    assert!(state.inline_page(old, view.stamp(), &view).is_some());
    assert!(state.reading_context(&view).is_some());
    state.display_changed();
    let pending = Identity::of(
        state
            .queue(&view, false, &DisplaySettings::default())
            .unwrap(),
    );
    assert!(state.set_article_mode(ArticleMode::Terminal));
    state.acknowledge(pending).unwrap();
    assert!(state.current(old, &view).is_none());
    assert!(!state.navigation_shown(view.stamp()));
    assert!(state.inline_page(old, view.stamp(), &view).is_none());
    assert!(state.reading_move(&view, Intent::NextPage).is_none());
    let terminal = displayed(&mut state, &view);
    let paint = state.current(terminal, &view).unwrap();
    assert_eq!(paint.frame, view.frame);
    assert!(paint.article.is_none() && paint.source_frame.is_none());
    assert!(!paint.links.is_empty());
    assert!(state.navigation_shown(view.stamp()));
    assert!(state.inline_identity(&view).is_none());
    assert!(state.set_article_mode(ArticleMode::Web));
    let web = displayed(&mut state, &view);
    assert!(state.inline_page(web, view.stamp(), &view).is_some());
    assert!(state.reading_context(&view).is_some());
    assert!(state.current(old, &view).is_none());
}

#[test]
fn leaving_web_reading_restores_station_grid_for_every_list_and_menu() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/navigation.json")).unwrap();
    for name in ["list", "boards", "main_menu", "mail", "archive"] {
        let mut state = SurfaceState::new(filters::Rules::default());
        let mut view = view();
        let reading = displayed(&mut state, &view);
        assert!(state.current(reading, &view).unwrap().article.is_some());
        let mut screen = Screen::new(80, 24).unwrap();
        screen.feed(&pttstella::encoding::encode(fixtures[name].as_str().unwrap()).unwrap());
        let revision = view.frame.revision;
        view.frame = screen.frame();
        view.frame.revision += revision + 1;
        state.update(&view);
        let id = displayed(&mut state, &view);
        let shown = state.current(id, &view).unwrap();
        assert!(
            shown.article.is_none() && shown.source_frame.is_none(),
            "{name}"
        );
        assert_eq!(shown.frame, view.frame);
    }
}
