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

fn expected_merged_fixture(view: &View) -> pttstella::screen::Frame {
    let mut expected = view.frame.clone();
    // Independent fixture expectation: only row 7's five-letter ALICE repeats.
    for glyph in expected
        .glyphs
        .iter_mut()
        .filter(|g| g.row == 6 && (3..8).contains(&g.col))
    {
        glyph.text = "█".into();
        glyph.left = pttstella::screen::Style {
            fg: 0,
            ..Default::default()
        };
        glyph.right = glyph.left;
    }
    expected
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
    let expected = expected_merged_fixture(&view);
    assert_eq!(packet.source_frame.as_ref(), Some(&expected));
    assert_eq!(packet.frame.glyphs, expected.glyphs); // Retained body keeps its known ID grouping.
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
    assert_eq!(paint.frame, expected_merged_fixture(&view));
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

#[test]
fn account_word_actions_revalidate_snapshot_and_preserve_existing_notes() {
    let rules = filters::Rules::default()
        .edit_policy("Alice", "keep note", filters::Policy::Allow, false)
        .unwrap();
    let mut state = SurfaceState::new(rules);
    let mut view = view();
    let identity = displayed(&mut state, &view);
    for policy in [
        filters::Policy::Mask,
        filters::Policy::MaskRow,
        filters::Policy::Allow,
    ] {
        let rules = state
            .account_rule(identity, view.stamp(), (5, 4), policy, false, &view)
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                rules.entries[0].id.as_str(),
                rules.entries[0].note.as_str(),
                rules.entries[0].policy
            ),
            ("Alice", "keep note", policy)
        );
    }
    assert!(
        state
            .account_rule(
                identity,
                view.stamp(),
                (6, 4),
                filters::Policy::Mask,
                false,
                &view
            )
            .is_none()
    ); // merged ID
    assert!(
        state
            .account_rule(
                identity,
                view.stamp(),
                (500, 4),
                filters::Policy::Mask,
                false,
                &view
            )
            .is_none()
    );
    view.input_epoch += 1;
    assert!(
        state
            .account_rule(
                identity,
                view.stamp(),
                (5, 4),
                filters::Policy::Mask,
                false,
                &view
            )
            .is_none()
    );
}

#[test]
fn reply_focus_toggles_without_persisting_or_losing_article_history_and_revokes_outputs() {
    for mode in [ArticleMode::Web, ArticleMode::Terminal] {
        let mut state = SurfaceState::new(filters::Rules::default());
        state.set_article_mode(mode);
        let mut view = view();
        let original = displayed(&mut state, &view);
        let baseline = state.current(original, &view).unwrap().frame.clone();
        let article = state
            .current(original, &view)
            .unwrap()
            .article
            .as_ref()
            .map(|a| a.id);
        let lease = state.lease_output(original, None, view.input_epoch);
        assert!(state.focus_reply(original, view.stamp(), Some(6), &view)); // merged ID still has Rust identity
        assert!(state.current(original, &view).is_none());
        assert!(lease.check(&view).is_err());
        let focused = displayed(&mut state, &view);
        let paint = state.current(focused, &view).unwrap();
        assert_eq!(paint.article.as_ref().map(|a| a.id), article);
        assert_eq!(
            pttstella::text::rows(&paint.frame).unwrap()[7].text,
            pttstella::text::rows(&baseline).unwrap()[7].text
        );
        assert!(paint.account_words.iter().any(|w| w.row == 7));
        assert!(paint.links.iter().any(|l| l.url.contains("visible.png")));
        assert!(
            paint
                .frame
                .glyphs
                .iter()
                .filter(|g| g.row == 5)
                .all(|g| g.left.bg == 4)
        );
        if let Some(source) = &paint.source_frame {
            assert!(
                source
                    .glyphs
                    .iter()
                    .filter(|g| g.row == 5)
                    .all(|g| g.left.bg == 4)
            );
            assert_eq!(
                pttstella::text::rows(source).unwrap()[7].text,
                pttstella::text::rows(&baseline).unwrap()[7].text
            );
        }
        assert!(state.rules().entries.is_empty());
        assert!(!state.focus_reply(original, view.stamp(), Some(5), &view));
        assert!(state.focus_reply(focused, view.stamp(), Some(5), &view));
        let restored = displayed(&mut state, &view);
        assert_eq!(state.current(restored, &view).unwrap().frame, baseline);
        assert!(state.focus_reply(restored, view.stamp(), Some(8), &view));
        let focused = displayed(&mut state, &view);
        assert!(state.focus_reply(focused, view.stamp(), None, &view));
        let restored = displayed(&mut state, &view);
        assert!(state.focus_reply(restored, view.stamp(), Some(5), &view));
        view.generation += 1;
        state.update(&view);
        let fresh = displayed(&mut state, &view);
        assert!(state.current(fresh, &view).unwrap().reply_focus.is_none());
    }
}

#[test]
fn focused_id_survives_contiguous_pages_but_not_a_new_article_segment() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut view = view();
    let original = displayed(&mut state, &view);
    assert!(state.focus_reply(original, view.stamp(), Some(5), &view));
    let focused = displayed(&mut state, &view);
    let article = state
        .current(focused, &view)
        .unwrap()
        .article
        .as_ref()
        .unwrap()
        .id;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/filters.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    state.article_input(&view.context(), b"\x1b[6~");
    screen.feed(
        &pttstella::encoding::encode(
            &fixture["reading"]
                .as_str()
                .unwrap()
                .replace("24~46", "47~69"),
        )
        .unwrap(),
    );
    view.frame = screen.frame();
    view.frame.revision += 100;
    state.update(&view);
    let next = displayed(&mut state, &view);
    let paint = state.current(next, &view).unwrap();
    assert_eq!(paint.article.as_ref().unwrap().id, article);
    assert_eq!(paint.reply_focus.as_deref(), Some("Alice"));
    assert_eq!(paint.frame.rows, 47);
    let rows = pttstella::text::rows(&paint.frame).unwrap();
    assert_ne!(rows[7].text, "█".repeat(80));
    assert_eq!(rows[7].text, rows[30].text);
    for row in [5, 28] {
        assert!(
            paint
                .frame
                .glyphs
                .iter()
                .filter(|g| g.row == row)
                .all(|g| g.left.bg == 4)
        );
    }
    state.clear_article();
    state.display_changed();
    let fresh = displayed(&mut state, &view);
    assert!(state.current(fresh, &view).unwrap().reply_focus.is_none());
}

#[test]
fn raw_reading_switch_revokes_cached_selection_links_and_delayed_ack() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let view = view();
    let old = displayed(&mut state, &view);
    state.select(old, 77, &Selection::All, &view).unwrap();
    hover(&mut state, old, &view);
    state.display_changed();
    let pending = Identity::of(
        state
            .queue(&view, false, &DisplaySettings::default())
            .unwrap(),
    );
    assert!(state.set_raw_reading(true));
    assert!(state.current(old, &view).is_none());
    assert!(!state.navigation_shown(view.stamp()));
    assert!(!state.clear_hover());
    state.acknowledge(pending).unwrap();
    assert!(state.current(pending, &view).is_none());
    assert!(state.export_rows(pending, Some(77), &view).is_err());
    let raw = displayed(&mut state, &view);
    assert!(state.current(raw, &view).unwrap().article.is_none());
    assert!(state.set_raw_reading(false));
    assert!(state.current(raw, &view).is_none());
    let fresh = displayed(&mut state, &view);
    assert!(state.current(fresh, &view).unwrap().article.is_some());
}

#[test]
fn clearing_reply_focus_cannot_reuse_the_old_presentation() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let view = view();
    let old = displayed(&mut state, &view);
    assert!(state.focus_reply(old, view.stamp(), Some(5), &view));
    let focused = displayed(&mut state, &view);
    state.select(focused, 78, &Selection::All, &view).unwrap();
    let lease = state.lease_output(focused, Some(78), view.input_epoch);
    state.clear_article();
    state.display_changed();
    assert!(lease.check(&view).is_err());
    assert!(state.current(focused, &view).is_none());
    let restored = displayed(&mut state, &view);
    assert_ne!(focused.presentation, restored.presentation);
    assert!(state.current(focused, &view).is_none());
    assert!(
        state
            .current(restored, &view)
            .unwrap()
            .reply_focus
            .is_none()
    );
}

#[test]
fn reading_modes_lengths_and_account_policies_keep_text_links_and_exports_consistent() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/filters.json")).unwrap();
    for mode in [ArticleMode::Web, ArticleMode::Terminal] {
        for last in [16, 23] {
            for policy in [
                None,
                Some(filters::Policy::Mask),
                Some(filters::Policy::MaskRow),
                Some(filters::Policy::Allow),
            ] {
                let mut rules = filters::Rules::default();
                if let Some(policy) = policy {
                    rules = rules.edit_policy("Alice", "", policy, false).unwrap();
                }
                let mut state = SurfaceState::new(rules);
                state.set_article_mode(mode);
                let mut view = view();
                let mut screen = Screen::new(80, 24).unwrap();
                screen.feed(
                    &pttstella::encoding::encode(
                        &fixture["reading"]
                            .as_str()
                            .unwrap()
                            .replace("24~46", &format!("1~{last}")),
                    )
                    .unwrap(),
                );
                view.frame = screen.frame();
                let identity = displayed(&mut state, &view);
                let paint = state.current(identity, &view).unwrap();
                let rows = pttstella::text::rows(&paint.frame).unwrap();
                assert_eq!(rows.len(), 24);
                assert_eq!(paint.frame.cols, 80);
                assert_eq!(paint.article.is_some(), mode == ArticleMode::Web);
                let blocked = matches!(
                    policy,
                    Some(filters::Policy::Mask | filters::Policy::MaskRow)
                );
                let expected = if blocked {
                    vec!["https://example.com/visible.png"]
                } else {
                    vec![
                        "https://example.com/hidden.png",
                        "https://example.com/visible.png",
                    ]
                };
                assert_eq!(
                    paint
                        .links
                        .iter()
                        .map(|l| l.url.as_str())
                        .collect::<Vec<_>>(),
                    expected
                );
                assert_eq!(paint.image_links.len(), expected.len());
                let visible = paint
                    .links
                    .iter()
                    .position(|l| l.url.ends_with("/visible.png"))
                    .unwrap();
                assert_eq!(
                    state
                        .copy_text(
                            identity,
                            &super::super::protocol::CopyTarget::Link { index: visible },
                            &view
                        )
                        .unwrap(),
                    "https://example.com/visible.png"
                );
                assert!(state.select(identity, 79, &Selection::All, &view).is_some());
                assert_eq!(state.export_rows(identity, Some(79), &view).unwrap(), 0..24);
                assert!(state.reveal(identity, true, &view));
                assert!(state.current(identity, &view).is_none());
                let revealed = displayed(&mut state, &view);
                let paint = state.current(revealed, &view).unwrap();
                assert_eq!(paint.links.len(), 2);
                assert!(pttstella::text::rows(&paint.frame).is_some());
            }
        }
    }
}

#[test]
fn full_article_loading_does_not_require_ack_preserves_cancelled_data_and_never_runs_in_terminal_mode()
 {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut view = view();
    let mut source = Screen::new(80, 24).unwrap();
    source.feed(
        &super::super::probe::surface_sample(
            Some("inline_pages"),
            true,
            Some("http://127.0.0.1:1234"),
        )
        .unwrap(),
    );
    view.frame = source.frame();
    let packet = state
        .queue(&view, false, &DisplaySettings::default())
        .unwrap()
        .clone();
    let id = packet.article.as_ref().unwrap().id;
    assert_eq!(
        state.next_full_page(&view),
        Some(pttstella::navigation::Intent::NextPage)
    );
    // One network request remains outstanding even if its old UI frame is ACKed.
    assert!(
        state
            .acknowledge(Identity {
                presentation: packet.presentation,
                generation: packet.generation,
                revision: packet.frame.revision
            })
            .is_ok()
    );
    assert!(state.next_full_page(&view).is_none());
    assert!(!state.full_load_action(view.generation, id + 1, false, &view));
    assert!(state.full_load_action(view.generation, id, false, &view));
    assert!(state.full_load_deadline().is_none());
    let current = displayed(&mut state, &view);
    assert_eq!(
        state
            .current(current, &view)
            .unwrap()
            .article
            .as_ref()
            .unwrap()
            .load,
        Some(super::super::article_cache::LoadState::Cancelled)
    );
    assert!(state.next_full_page(&view).is_none());
    assert!(state.set_article_mode(ArticleMode::Terminal));
    displayed(&mut state, &view);
    assert!(state.displayed.as_ref().unwrap().article.is_none());
    assert!(!state.full_load_active());
    assert!(state.next_full_page(&view).is_none());
    assert!(!state.full_load_action(view.generation, id, true, &view));
}

#[test]
fn clamped_short_article_retains_its_tail_and_opens_the_acknowledged_historical_link() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut view = view();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(
        &super::super::probe::short_overlap_page(false, Some("http://127.0.0.1:1234")).unwrap(),
    );
    view.frame = screen.frame();
    let old = displayed(&mut state, &view);
    assert!(state.next_full_page(&view).is_some());
    state.article_input(&view.context(), b"\x1b[6~");
    screen.feed(
        &super::super::probe::short_overlap_page(true, Some("http://127.0.0.1:1234")).unwrap(),
    );
    view.frame = screen.frame();
    state.update(&view);
    assert!(state.current(old, &view).is_none());
    let fresh = displayed(&mut state, &view);
    let paint = state.current(fresh, &view).unwrap();
    let article = paint.article.as_ref().unwrap();
    assert_eq!(
        (article.first_line, article.last_line, article.end),
        (1, 32, true)
    );
    let text = pttstella::text::rows(&paint.frame).unwrap();
    assert!(text[23].text.starts_with("Synthetic signature:"));
    assert!(text[31].text.starts_with("Last synthetic reply"));
    let index = paint
        .links
        .iter()
        .position(|link| link.url == "https://example.com/pttstella")
        .unwrap();
    assert_eq!(paint.links[index].row, 1);
    // The OpenLink consumer resolves from this ACK, not from station-page indices.
    assert_eq!(
        state.current(fresh, &view).unwrap().links[index].url,
        "https://example.com/pttstella"
    );
    assert!(
        !pttstella::links::detect(&view.frame)
            .iter()
            .any(|link| link.url == "https://example.com/pttstella")
    );
}

#[test]
fn preload_ingests_all_pages_while_the_first_paint_ack_is_withheld() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut view = view();
    let mut source = Screen::new(80, 24).unwrap();
    source.feed(&super::super::probe::long_load_page(1, Some("http://127.0.0.1:1234")).unwrap());
    view.frame = source.frame();
    let old = state
        .queue(&view, false, &DisplaySettings::default())
        .unwrap()
        .clone();
    let id = old.article.as_ref().unwrap().id;
    let permit = state.authorize_reading(view.stamp(), id).unwrap();
    for page in 2..=38 {
        assert_eq!(state.reading_scope(), Some(permit));
        assert_eq!(
            state.next_full_page(&view),
            Some(pttstella::navigation::Intent::NextPage)
        );
        assert!(state.next_full_page(&view).is_none());
        state.article_input(&view.context(), b"\x1b[6~");
        source.feed(
            &super::super::probe::long_load_page(page, Some("http://127.0.0.1:1234")).unwrap(),
        );
        view.frame = source.frame();
        state.update(&view);
        assert!(state.has_in_flight());
        assert!(
            state
                .queue(&view, false, &DisplaySettings::default())
                .is_none()
        );
    }
    assert_eq!(
        state.reading_progress(),
        Some((
            id,
            1,
            837,
            Some(super::super::article_cache::LoadState::Complete)
        ))
    );
    assert!(state.next_full_page(&view).is_none());
    let old_identity = Identity {
        presentation: old.presentation,
        generation: old.generation,
        revision: old.frame.revision,
    };
    state.acknowledge(old_identity).unwrap();
    assert!(state.current(old_identity, &view).is_none());
    let final_packet = state
        .queue(&view, false, &DisplaySettings::default())
        .unwrap();
    let rows = pttstella::text::rows(&final_packet.frame).unwrap();
    assert!(rows[0].text.starts_with("Full article synthetic line 0001"));
    assert!(
        rows[836]
            .text
            .starts_with("Full article synthetic line 0837")
    );
    assert_eq!(final_packet.article.as_ref().unwrap().last_line, 837);
}

#[test]
fn preload_cancel_late_page_and_article_switch_revoke_old_scope_without_waiting_for_ack() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut view = view();
    let mut source = Screen::new(80, 24).unwrap();
    source.feed(&super::super::probe::long_load_page(1, Some("http://127.0.0.1:1234")).unwrap());
    view.frame = source.frame();
    let old = state
        .queue(&view, false, &DisplaySettings::default())
        .unwrap()
        .clone();
    let id = old.article.as_ref().unwrap().id;
    let stamp = view.stamp();
    assert!(state.next_full_page(&view).is_some());
    state.article_input(&view.context(), b"\x1b[6~");
    assert!(state.full_load_action(view.generation, id, false, &view));
    source.feed(&super::super::probe::long_load_page(2, Some("http://127.0.0.1:1234")).unwrap());
    view.frame = source.frame();
    state.update(&view);
    assert_eq!(
        state.reading_progress(),
        Some((
            id,
            1,
            45,
            Some(super::super::article_cache::LoadState::Cancelled)
        ))
    );
    assert!(state.next_full_page(&view).is_none());
    state.article_input(&view.context(), b"q");
    assert!(state.reading_scope().is_none());
    assert!(state.authorize_reading(stamp, id).is_none());
    view.generation += 1;
    let mut fresh = Screen::new(80, 24).unwrap();
    fresh.feed(&super::super::probe::long_load_page(1, Some("http://127.0.0.1:1234")).unwrap());
    view.frame = fresh.frame();
    state.update(&view);
    let new_id = state.reading_progress().unwrap().0;
    assert_ne!(new_id, id);
    assert_eq!(state.reading_progress().unwrap().2, 23);
    assert!(state.authorize_reading(stamp, id).is_none());
    assert!(!state.full_load_action(view.generation, id, true, &view));
    state
        .acknowledge(Identity {
            presentation: old.presentation,
            generation: old.generation,
            revision: old.frame.revision,
        })
        .unwrap();
    let new_packet = state
        .queue(&view, false, &DisplaySettings::default())
        .unwrap()
        .clone();
    assert_eq!(new_packet.article.as_ref().unwrap().last_line, 23);
    assert!(state.authorize_reading(view.stamp(), new_id).is_some());
    state.set_article_mode(ArticleMode::Terminal);
    assert!(state.reading_scope().is_none());
    assert!(state.next_full_page(&view).is_none());
}

#[test]
fn author_actions_use_acknowledged_list_fields_and_reject_stale_or_hidden_authors() {
    let mut view = view();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/navigation.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(&pttstella::encoding::encode(fixture["list"].as_str().unwrap()).unwrap());
    view.frame = screen.frame();
    let mut state = SurfaceState::new(filters::Rules::default());
    let identity = displayed(&mut state, &view);
    for col in [17, 21, 29] {
        for policy in [
            filters::Policy::Mask,
            filters::Policy::MaskRow,
            filters::Policy::Allow,
        ] {
            let rules = state
                .account_rule(identity, view.stamp(), (3, col), policy, false, &view)
                .unwrap()
                .unwrap();
            assert_eq!(
                (rules.entries[0].id.as_str(), rules.entries[0].policy),
                ("alice", policy)
            );
        }
    }
    for (row, col) in [(2, 17), (3, 16), (3, 30), (23, 17)] {
        assert!(
            state
                .account_rule(
                    identity,
                    view.stamp(),
                    (row, col),
                    filters::Policy::Mask,
                    false,
                    &view
                )
                .is_none()
        );
    }
    assert!(!state.focus_reply(identity, view.stamp(), Some(3), &view));
    view.input_epoch += 1;
    assert!(
        state
            .account_rule(
                identity,
                view.stamp(),
                (3, 17),
                filters::Policy::Mask,
                false,
                &view
            )
            .is_none()
    );
    let rules = filters::Rules::default()
        .edit_policy("alice", "", filters::Policy::MaskRow, false)
        .unwrap();
    let mut state = SurfaceState::new(rules);
    let identity = displayed(&mut state, &view);
    assert!(
        state
            .account_rule(
                identity,
                view.stamp(),
                (3, 17),
                filters::Policy::Allow,
                false,
                &view
            )
            .is_none()
    );
}

#[test]
fn context_removal_requires_matching_rule_current_visible_word_and_successful_save() {
    for policy in [filters::Policy::Mask, filters::Policy::Allow] {
        let rules = filters::Rules::default()
            .edit_policy("Alice", "keep", policy, false)
            .unwrap();
        let mut state = SurfaceState::new(rules.clone());
        let view = view();
        let original = displayed(&mut state, &view);
        assert_eq!(
            state
                .current(original, &view)
                .unwrap()
                .account_words
                .iter()
                .find(|w| w.row == 5 && w.id == "Alice")
                .unwrap()
                .policy,
            Some(policy)
        );
        let mismatch = if policy == filters::Policy::Allow {
            filters::Policy::Mask
        } else {
            filters::Policy::Allow
        };
        assert!(
            state
                .account_rule(original, view.stamp(), (5, 3), mismatch, true, &view)
                .is_none()
        );
        let removed = state
            .account_rule(original, view.stamp(), (5, 3), policy, true, &view)
            .unwrap()
            .unwrap();
        assert!(removed.entries.is_empty());
        assert_eq!(state.rules(), &rules); // draft/failed persistence cannot change effective rules
        state.replace_rules(removed);
        assert!(
            state
                .account_rule(original, view.stamp(), (5, 3), policy, true, &view)
                .is_none()
        );
        let fresh = displayed(&mut state, &view);
        assert!(
            state
                .account_rule(fresh, view.stamp(), (5, 3), policy, true, &view)
                .is_none()
        );
    }
}

#[test]
fn adjacent_article_requires_enabled_current_local_cache_and_a_real_retained_edge() {
    let mut state = SurfaceState::new(filters::Rules::default());
    let mut view = view();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/filters.json")).unwrap();
    let mut screen = Screen::new(80, 24).unwrap();
    screen.feed(
        &pttstella::encoding::encode(
            &fixture["reading"]
                .as_str()
                .unwrap()
                .replace("24~46", "1~23"),
        )
        .unwrap(),
    );
    view.frame = screen.frame();
    displayed(&mut state, &view);
    assert!(!state.adjacent_article_allowed(view.stamp(), false, &view));
    state.set_article_boundary_navigation(true);
    displayed(&mut state, &view);
    assert!(!state.adjacent_article_allowed(view.stamp(), false, &view)); // active preload
    state.stop_full_load();
    displayed(&mut state, &view);
    assert!(state.adjacent_article_allowed(view.stamp(), false, &view));
    assert!(!state.adjacent_article_allowed(view.stamp(), true, &view));
    let old = view.stamp();
    view.input_epoch += 1;
    assert!(!state.adjacent_article_allowed(old, false, &view));
    view.input_pending = true;
    assert!(!state.adjacent_article_allowed(view.stamp(), false, &view));
    view.input_pending = false;
    state.set_article_mode(ArticleMode::Terminal);
    displayed(&mut state, &view);
    assert!(!state.adjacent_article_allowed(view.stamp(), false, &view));
}
