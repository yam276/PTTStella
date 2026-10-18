//! Real Session/transport, independent exact mouse bytes, loopback only.
use super::*;
use crate::mouse::{
    Action, Button, Modifiers, MotionReport, Report, Tracking, WheelDirection, WheelReport,
    input::{Event, Lease, Route, Sample, SampleReport},
};
type Peer = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;
fn event(view: &View, id: u64, action: Action) -> Event {
    Event {
        lease: Lease {
            stamp: view.stamp(),
            mode_epoch: view.mouse.modes.epoch(),
            policy_epoch: view.mouse.policy_epoch,
            transaction: id,
        },
        report: Report {
            col: 4,
            row: 3,
            button: Button::Left,
            action,
            modifiers: Modifiers::default(),
        },
    }
}
fn release(event: Event) -> Event {
    Event {
        report: Report {
            action: Action::Release,
            ..event.report
        },
        ..event
    }
}
fn motion(view: &View, id: u64, col: usize) -> Sample {
    Sample {
        lease: event(view, id, Action::Press).lease,
        sequence: id,
        report: SampleReport::Motion(MotionReport {
            col,
            row: 3,
            button: None,
            modifiers: Modifiers::default(),
        }),
    }
}

// These inputs validate the synthetic peer itself. They are not trusted GUI
// acceptance; the native runner still has to exercise the actual pointer path.
#[tokio::test]
async fn native_sample_peers_require_their_exact_final_gesture_and_acknowledge_close() {
    for case in [
        MouseSampleProbe::Drag,
        MouseSampleProbe::Motion,
        MouseSampleProbe::Wheel,
    ] {
        let connection = Connection::start_mouse_samples_probe(case).unwrap();
        connection.connect().unwrap();
        let start = wait(&connection, |view| {
            view.received > 0 && !view.output_pending
        })
        .await;
        let sequence: &[&[u8]] = match case {
            MouseSampleProbe::Drag => &[b"\x1b[<0;5;5M", b"\x1b[<32;11;5M", b"\x1b[<0;11;5m"],
            MouseSampleProbe::RoundTrip => &[
                b"\x1b[<0;5;5M",
                b"\x1b[<32;11;5M",
                b"\x1b[<32;5;5M",
                b"\x1b[<0;5;5m",
            ],
            MouseSampleProbe::Motion => &[b"\x1b[<0;5;5M", b"\x1b[<0;5;5m", b"\x1b[<35;11;5M"],
            MouseSampleProbe::Wheel => &[b"\x1b[<65;5;5M", b"\x1b[<64;5;5M"],
        };
        for bytes in sequence {
            connection.input(start.generation, bytes.to_vec()).unwrap();
        }
        wait(&connection, |view| {
            view.mouse.modes.tracking() == Tracking::Highlight && !view.output_pending
        })
        .await;
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
    }
}

#[tokio::test]
async fn native_drag_peer_cannot_pass_after_its_bound_without_the_required_release() {
    let connection = Connection::start_mouse_samples_probe(MouseSampleProbe::Drag).unwrap();
    connection.connect().unwrap();
    let start = wait(&connection, |view| {
        view.received > 0 && !view.output_pending
    })
    .await;
    connection
        .input(start.generation, b"\x1b[<0;5;5M".to_vec())
        .unwrap();
    for _ in 0..16 {
        connection
            .input(start.generation, b"\x1b[<32;11;5M".to_vec())
            .unwrap();
    }
    wait(&connection, |view| view.phase == Phase::Error).await;
    let error = connection.shutdown().await.unwrap_err();
    assert!(error.contains("never completed"), "{error}");
}

#[tokio::test]
async fn native_round_trip_peer_requires_return_motion_before_its_release() {
    let connection = Connection::start_mouse_samples_probe(MouseSampleProbe::RoundTrip).unwrap();
    connection.connect().unwrap();
    let start = wait(&connection, |view| {
        view.received > 0 && !view.output_pending
    })
    .await;
    for bytes in [
        b"\x1b[<0;10;9M".as_slice(),
        b"\x1b[<32;50;9M",
        b"\x1b[<0;10;9m",
    ] {
        connection.input(start.generation, bytes.to_vec()).unwrap();
    }
    let failed = wait(&connection, |view| view.phase == Phase::Error).await;
    let status = failed
        .frame
        .glyphs
        .iter()
        .filter(|g| g.row == 1)
        .map(|g| g.text.as_str())
        .collect::<String>();
    assert!(status.starts_with("FAILED:"), "{status}");
    let error = connection.shutdown().await.unwrap_err();
    assert!(error.contains("requires held motion"), "{error}");
}

#[tokio::test]
async fn native_round_trip_regions_require_redraw_return_and_close_without_extra_input() {
    let connection = Connection::start_mouse_samples_probe(MouseSampleProbe::RoundTrip).unwrap();
    connection.connect().unwrap();
    let start = wait(&connection, |view| {
        view.received > 0 && !view.output_pending
    })
    .await;
    for (col, row, bg) in [(8, 7, 3), (23, 11, 3), (48, 7, 6), (63, 11, 6)] {
        let glyph = start
            .frame
            .glyphs
            .iter()
            .find(|g| g.col == col && g.row == row)
            .unwrap();
        assert_eq!(glyph.left.bg, bg);
    }
    for bytes in [b"\x1b[<0;8;9M".as_slice(), b"\x1b[<32;10;9M"] {
        connection.input(start.generation, bytes.to_vec()).unwrap();
    }
    wait(&connection, |view| {
        !view.output_pending
            && view
                .frame
                .glyphs
                .iter()
                .filter(|g| g.row == 1)
                .map(|g| g.text.as_str())
                .collect::<String>()
                .starts_with("READY:")
    })
    .await;
    connection
        .input(start.generation, b"\x1b[<32;64;12M".to_vec())
        .unwrap();
    let redraw = wait(&connection, |view| {
        !view.output_pending
            && view
                .frame
                .glyphs
                .iter()
                .filter(|g| g.row == 1)
                .map(|g| g.text.as_str())
                .collect::<String>()
                .starts_with("HOLD:")
    })
    .await;
    let status = redraw
        .frame
        .glyphs
        .iter()
        .filter(|g| g.row == 1)
        .map(|g| g.text.as_str())
        .collect::<String>();
    assert!(status.starts_with("HOLD: return to A"), "{status}");
    for bytes in [b"\x1b[<32;24;12M".as_slice(), b"\x1b[<0;10;9m"] {
        connection.input(start.generation, bytes.to_vec()).unwrap();
    }
    wait(&connection, |view| {
        view.mouse.modes.tracking() == Tracking::Highlight && !view.output_pending
    })
    .await;
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
}
#[tokio::test]
async fn mouse_motion_preserves_a_following_click_and_owned_drag_across_redraw() {
    let (_listener, connection, mut peer, _) = ready().await;
    peer.send(Message::Binary(b"\x1b[?1003h".to_vec().into()))
        .await
        .unwrap();
    let current = wait(&connection, |view| {
        view.mouse.modes.tracking() == Tracking::Motion
            && view.mouse.route == Route::Station
            && !view.output_pending
    })
    .await;
    connection.mouse_sample(motion(&current, 1, 4)).unwrap();
    expect(&mut peer, b"\x1b[<35;5;4M").await;
    assert_eq!(connection.updates.borrow().input_epoch, current.input_epoch);
    assert!(!connection.updates.borrow().input_pending);
    assert!(connection.updates.borrow().mouse.active.is_none());
    connection.mouse_sample(motion(&current, 2, 4)).unwrap();
    let press = event(&current, 3, Action::Press);
    connection.mouse_report(press).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4M").await; // duplicate motion sent no bytes
    let held = wait(&connection, |view| view.mouse.active == Some(press.lease)).await;
    let drag = Sample {
        lease: press.lease,
        sequence: 3,
        report: SampleReport::Motion(MotionReport {
            col: 6,
            row: 3,
            button: Some(Button::Left),
            modifiers: Modifiers::default(),
        }),
    };
    connection.mouse_sample(drag).unwrap();
    expect(&mut peer, b"\x1b[<32;7;4M").await;
    peer.send(Message::Binary(navigation_page("reading_middle").into()))
        .await
        .unwrap();
    let redraw = wait(&connection, |view| {
        view.frame.revision > held.frame.revision && !view.output_pending
    })
    .await;
    assert_eq!(redraw.mouse.active, Some(press.lease));
    connection.mouse_report(release(press)).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4m").await;
    let released = wait(&connection, |view| {
        view.mouse.active.is_none() && view.input_epoch == current.input_epoch + 2
    })
    .await;
    let tick = Sample {
        report: SampleReport::Wheel(WheelReport {
            col: 4,
            row: 3,
            direction: WheelDirection::Up,
            modifiers: Modifiers::default(),
        }),
        ..motion(&released, 4, 4)
    };
    connection.mouse_sample(tick).unwrap();
    expect(&mut peer, b"\x1b[<64;5;4M").await;
    let wheeled = wait(&connection, |view| {
        view.input_epoch == released.input_epoch + 1 && view.input_pending
    })
    .await;
    assert!(wheeled.mouse.active.is_none());
    connection.mouse_sample(motion(&wheeled, 5, 8)).unwrap();
    expect(&mut peer, b"\x1b[<35;9;4M").await;
    quiet(&mut peer).await; // no wheel release or local navigation
    finish_board_peer(&connection, peer).await;
}
#[tokio::test]
async fn mouse_samples_are_revoked_before_queued_work_and_remain_blocked_during_sync() {
    let (_listener, connection, mut peer, _) = ready().await;
    peer.send(Message::Binary(b"\x1b[?1003h".to_vec().into()))
        .await
        .unwrap();
    let current = wait(&connection, |view| {
        view.mouse.modes.tracking() == Tracking::Motion && !view.output_pending
    })
    .await;
    connection.mouse_sample(motion(&current, 1, 4)).unwrap();
    connection.revoke_mouse(current.generation);
    quiet(&mut peer).await;
    let revoked = connection.updates.borrow().clone();
    assert!(revoked.mouse.policy_epoch > current.mouse.policy_epoch);
    connection.input(revoked.generation, b"x".to_vec()).unwrap();
    connection.mouse_sample(motion(&revoked, 2, 5)).unwrap();
    expect(&mut peer, b"x").await;
    quiet(&mut peer).await;
    peer.send(Message::Binary(
        [navigation_page("list"), b"\x1b[?1002h".to_vec()]
            .concat()
            .into(),
    ))
    .await
    .unwrap();
    let drag_only = wait(&connection, |view| {
        view.mouse.modes.tracking() == Tracking::Drag && !view.input_pending && !view.output_pending
    })
    .await;
    connection.mouse_sample(motion(&drag_only, 3, 6)).unwrap();
    quiet(&mut peer).await; // 1002 cannot report an unheld pointer
    peer.send(Message::Binary(b"\x1b[?2026h".to_vec().into()))
        .await
        .unwrap();
    let pending = wait(&connection, |view| view.output_pending).await;
    connection
        .mouse_sample(Sample {
            report: SampleReport::Wheel(WheelReport {
                col: 4,
                row: 3,
                direction: WheelDirection::Down,
                modifiers: Modifiers::default(),
            }),
            ..motion(&pending, 4, 4)
        })
        .unwrap();
    quiet(&mut peer).await;
    peer.send(Message::Ping(b"sample-sync".to_vec().into()))
        .await
        .unwrap();
    assert_eq!(
        peer.next().await.unwrap().unwrap(),
        Message::Pong(b"sample-sync".to_vec().into())
    );
    finish_board_peer(&connection, peer).await;
}
async fn expect(peer: &mut Peer, bytes: &[u8]) {
    assert!(
        matches!(timeout(Duration::from_secs(2),peer.next()).await.unwrap_or_else(|_|panic!("timed out waiting for {bytes:02x?}")),Some(Ok(Message::Binary(actual))) if actual.as_ref()==bytes),
        "expected {bytes:?}"
    );
}
async fn quiet(peer: &mut Peer) {
    assert!(
        timeout(Duration::from_millis(60), peer.next())
            .await
            .is_err()
    );
}
async fn ready() -> (TcpListener, Connection, Peer, View) {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut peer = tokio_tungstenite::accept_async(stream).await.unwrap();
    peer.send(Message::Binary(
        [navigation_page("list"), b"\x1b[?1000;1006h".to_vec()]
            .concat()
            .into(),
    ))
    .await
    .unwrap();
    let view = wait(&connection, |v| {
        v.mouse.route == Route::Station && !v.output_pending
    })
    .await;
    (listener, connection, peer, view)
}
#[tokio::test]
async fn station_mouse_serializes_owned_edges_across_redraw_and_rejects_duplicates() {
    let (_listener, connection, mut peer, initial) = ready().await;
    let press = event(&initial, 1, Action::Press);
    connection.mouse_report(press).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4M").await;
    let held = wait(&connection, |v| {
        v.mouse.active == Some(press.lease) && v.input_epoch == 1
    })
    .await;
    assert!(!held.keep_alive_ready);
    assert!(held.input_pending);
    connection.mouse_report(press).unwrap();
    quiet(&mut peer).await;
    peer.send(Message::Binary(navigation_page("reading_middle").into()))
        .await
        .unwrap();
    let redrawn = wait(&connection, |v| {
        v.frame.revision > initial.frame.revision && !v.output_pending
    })
    .await;
    assert_eq!(redrawn.mouse.active, Some(press.lease));
    connection.mouse_report(release(press)).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4m").await;
    wait(&connection, |v| {
        v.mouse.active.is_none() && v.input_epoch == 2 && !v.input_pending
    })
    .await;
    connection.mouse_report(release(press)).unwrap();
    connection.mouse_report(press).unwrap();
    quiet(&mut peer).await;
    finish_board_peer(&connection, peer).await;
}
#[tokio::test]
async fn station_mouse_controls_preempt_queued_press_and_keep_coalesced_off_on_observable() {
    let (_listener, connection, mut peer, initial) = ready().await;
    let cancelled = event(&initial, 1, Action::Press);
    // No await before both calls: the control must win before the queued press.
    connection.mouse_report(cancelled).unwrap();
    connection.cancel_mouse(cancelled.lease);
    quiet(&mut peer).await;
    let next = event(&connection.updates.borrow(), 2, Action::Press);
    connection.mouse_report(next).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4M").await;
    wait(&connection, |v| v.mouse.active == Some(next.lease)).await;
    connection.cancel_mouse(cancelled.lease);
    connection.mouse_report(release(next)).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4m").await;
    let current = wait(&connection, |v| {
        v.mouse.active.is_none() && v.input_epoch == 2
    })
    .await;
    let old = event(&current, 3, Action::Press);
    connection.mouse_report(old).unwrap();
    connection.mouse_enabled(current.generation, false);
    connection.mouse_enabled(current.generation, true);
    quiet(&mut peer).await;
    let updated = wait(&connection, |v| {
        v.mouse.enabled && v.mouse.policy_epoch > current.mouse.policy_epoch
    })
    .await;
    let fresh = event(&updated, 4, Action::Press);
    connection.mouse_report(fresh).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4M").await;
    connection.revoke_mouse(updated.generation);
    connection.mouse_report(release(fresh)).unwrap();
    quiet(&mut peer).await;
    assert!(connection.updates.borrow().mouse.active.is_none());
    finish_board_peer(&connection, peer).await;
}
#[tokio::test]
async fn station_mouse_mode_changes_inside_sync_revoke_before_retained_frame_releases() {
    let (_listener, connection, mut peer, initial) = ready().await;
    let press = event(&initial, 1, Action::Press);
    connection.mouse_report(press).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4M").await;
    let held = wait(&connection, |v| v.mouse.active.is_some()).await;
    peer.send(Message::Binary(
        b"\x1b[?2026h\x1b[?1000l\x1b[?1000h".to_vec().into(),
    ))
    .await
    .unwrap();
    let changed = wait(&connection, |v| {
        v.output_pending && v.mouse.modes.epoch() > initial.mouse.modes.epoch()
    })
    .await;
    assert_eq!(changed.frame, held.frame);
    assert!(changed.mouse.active.is_none());
    assert_eq!(changed.mouse.route, Route::Blocked);
    connection.mouse_report(release(press)).unwrap();
    connection
        .mouse_report(event(&changed, 2, Action::Press))
        .unwrap();
    quiet(&mut peer).await;
    peer.send(Message::Ping(b"mouse-sync".to_vec().into()))
        .await
        .unwrap();
    assert_eq!(
        peer.next().await.unwrap().unwrap(),
        Message::Pong(b"mouse-sync".to_vec().into())
    );
    peer.send(Message::Binary(b"\x1b[?2026l".to_vec().into()))
        .await
        .unwrap();
    let resumed = wait(&connection, |v| {
        v.mouse.route == Route::Station && !v.output_pending
    })
    .await;
    let next = event(&resumed, 3, Action::Press);
    connection.mouse_report(next).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4M").await;
    connection.mouse_report(release(next)).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4m").await;
    finish_board_peer(&connection, peer).await;
}
#[tokio::test]
async fn station_mouse_format_bounds_and_manual_intervention_never_send_late_edges() {
    let (_listener, connection, mut peer, initial) = ready().await;
    let mut invalid = event(&initial, 1, Action::Press);
    invalid.report.col = 80;
    connection.mouse_report(invalid).unwrap();
    quiet(&mut peer).await;
    let press = event(&connection.updates.borrow(), 2, Action::Press);
    connection.mouse_report(press).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4M").await;
    connection.input(initial.generation, b"x".to_vec()).unwrap();
    connection.mouse_report(release(press)).unwrap();
    expect(&mut peer, b"x").await;
    quiet(&mut peer).await;
    peer.send(Message::Binary(b"\x1b[?1001h".to_vec().into()))
        .await
        .unwrap();
    let unsupported = wait(&connection, |v| {
        v.mouse.modes.tracking() == Tracking::Highlight && !v.output_pending
    })
    .await;
    assert_eq!(unsupported.mouse.route, Route::Blocked);
    connection
        .mouse_report(event(&unsupported, 3, Action::Press))
        .unwrap();
    quiet(&mut peer).await;
    assert!(connection.updates.borrow().mouse.active.is_none());
    connection.mouse_enabled(unsupported.generation, false);
    let local = wait(&connection, |v| !v.mouse.enabled).await;
    assert_eq!(local.mouse.route, Route::Local);
    finish_board_peer(&connection, peer).await;
}
#[tokio::test]
async fn station_mouse_reconnect_resets_modes_and_old_controls_cannot_change_new_session() {
    let (listener, connection, mut peer, initial) = ready().await;
    let old = event(&initial, 1, Action::Press);
    connection.mouse_report(old).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4M").await;
    let closing = tokio::spawn(async move {
        assert!(matches!(peer.next().await, Some(Ok(Message::Close(_)))));
        peer.flush().await.unwrap();
    });
    connection.disconnect().unwrap();
    wait(&connection, |v| v.phase == Phase::Disconnected).await;
    closing.await.unwrap();
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut peer = tokio_tungstenite::accept_async(stream).await.unwrap();
    peer.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    let fresh = wait(&connection, |v| {
        v.generation > initial.generation
            && v.phase == Phase::Connected
            && v.received > 0
            && !v.output_pending
    })
    .await;
    assert_eq!(fresh.mouse.modes, crate::mouse::Modes::default());
    assert!(fresh.mouse.active.is_none());
    connection.mouse_enabled(initial.generation, false);
    connection.revoke_mouse(initial.generation);
    connection.cancel_mouse(old.lease);
    connection.mouse_report(release(old)).unwrap();
    connection.mouse_sample(motion(&initial, 2, 4)).unwrap();
    quiet(&mut peer).await;
    assert!(connection.updates.borrow().mouse.enabled);
    peer.send(Message::Binary(b"\x1b[?1000;1006h".to_vec().into()))
        .await
        .unwrap();
    let ready = wait(&connection, |v| {
        v.mouse.route == Route::Station && !v.output_pending
    })
    .await;
    let press = event(&ready, 1, Action::Press);
    connection.mouse_report(press).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4M").await;
    let held = wait(&connection, |v| v.mouse.active == Some(press.lease)).await;
    connection.mouse_enabled(initial.generation, false);
    connection.revoke_mouse(initial.generation);
    connection.cancel_mouse(old.lease);
    connection.mouse_report(release(old)).unwrap();
    connection.mouse_sample(motion(&initial, 2, 4)).unwrap();
    quiet(&mut peer).await;
    assert_eq!(connection.updates.borrow().mouse.active, Some(press.lease));
    assert_eq!(connection.updates.borrow().message, held.message);
    finish_board_peer(&connection, peer).await;
    assert!(connection.updates.borrow().mouse.active.is_none());
    assert!(!connection.updates.borrow().mouse.ready);
    assert_eq!(
        connection.updates.borrow().mouse.modes,
        crate::mouse::Modes::default()
    );
    assert_eq!(connection.updates.borrow().mouse.route, Route::Local);
}
#[tokio::test]
async fn station_mouse_queue_reserves_manual_capacity_and_controls_bypass_full_input_queue() {
    let (_listener, connection, mut peer, initial) = ready().await;
    for _ in 0..56 {
        connection.input(initial.generation, b"x".to_vec()).unwrap();
    }
    assert_eq!(connection.commands.capacity(), 8);
    assert!(connection.mouse_sample(motion(&initial, 1, 4)).is_err());
    assert!(
        connection
            .mouse_report(event(&initial, 1, Action::Press))
            .is_err()
    );
    for _ in 0..8 {
        connection.input(initial.generation, b"x".to_vec()).unwrap();
    }
    assert_eq!(connection.commands.capacity(), 0);
    assert!(
        connection
            .mouse_report(event(&initial, 1, Action::Release))
            .is_err()
    );
    connection.mouse_enabled(initial.generation, false);
    connection.revoke_mouse(initial.generation);
    for _ in 0..64 {
        expect(&mut peer, b"x").await;
    }
    let view = wait(&connection, |v| v.input_epoch == 64 && !v.mouse.enabled).await;
    assert_eq!(view.mouse.route, Route::Local);
    quiet(&mut peer).await;
    finish_board_peer(&connection, peer).await;
}
#[tokio::test]
async fn station_mouse_hold_pauses_keepalive_even_after_no_response_wait_expires() {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut peer = tokio_tungstenite::accept_async(stream).await.unwrap();
    peer.send(Message::Binary(
        [navigation_page("list"), b"\x1b[?1000;1006h".to_vec()]
            .concat()
            .into(),
    ))
    .await
    .unwrap();
    let first = wait(&connection, |v| {
        v.mouse.route == Route::Station && !v.output_pending
    })
    .await;
    let press = event(&first, 1, Action::Press);
    connection.mouse_report(press).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4M").await;
    let held = wait(&connection, |v| {
        v.mouse.active == Some(press.lease) && v.input_epoch == 1
    })
    .await;
    connection.keep_alive_gate(held.stamp(), true);
    assert!(
        timeout(Duration::from_millis(700), peer.next())
            .await
            .is_err()
    );
    assert!(!connection.updates.borrow().keep_alive_ready);
    assert!(connection.updates.borrow().mouse.active.is_some());
    assert_ne!(
        connection.updates.borrow().message,
        crate::msg!("input-unanswered")
    );
    connection.mouse_report(release(press)).unwrap();
    expect(&mut peer, b"\x1b[<0;5;4m").await;
    finish_board_peer(&connection, peer).await;
}

#[tokio::test]
async fn station_mouse_owns_pointer_navigation_but_explicit_buttons_and_keys_remain_available() {
    let (_listener, connection, mut peer, _initial) = ready().await;
    for mode in [
        b"\x1b[?1000h".as_slice(),
        b"\x1b[?1003h".as_slice(),
        b"\x1b[?1001h".as_slice(),
    ] {
        let previous = connection.updates.borrow().received;
        peer.send(Message::Binary(mode.to_vec().into()))
            .await
            .unwrap();
        let view = wait(&connection, |v| {
            v.received >= previous + mode.len() && !v.output_pending
        })
        .await;
        let epoch = view.input_epoch;
        let policy = view.mouse.policy_epoch;
        let error = connection
            .navigate(view.stamp(), Intent::NextItem)
            .await
            .unwrap_err();
        assert_eq!(error, crate::msg!("station-mouse-local-blocked"));
        quiet(&mut peer).await;
        assert_eq!(connection.updates.borrow().input_epoch, epoch);
        assert_eq!(connection.updates.borrow().mouse.policy_epoch, policy);
    }
    // A toolbar action is an explicit command, even with an unsupported station mode.
    let current = connection.updates.borrow().clone();
    connection
        .request_navigate(
            current.stamp(),
            Intent::NextItem,
            navigation::Source::Button,
        )
        .unwrap()
        .wait()
        .await
        .unwrap();
    expect(&mut peer, b"\x1b[B").await;
    peer.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    let moved = wait(&connection, |v| {
        v.frame.revision > current.frame.revision && !v.output_pending && !v.input_pending
    })
    .await;
    connection
        .browse_key(moved.stamp(), Intent::NextItem)
        .await
        .unwrap();
    expect(&mut peer, b"\x1b[B").await;
    peer.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    let keyed = wait(&connection, |v| {
        v.frame.revision > moved.frame.revision && !v.output_pending && !v.input_pending
    })
    .await;
    connection.mouse_enabled(keyed.generation, false);
    let disabled = wait(&connection, |v| !v.mouse.enabled).await;
    connection
        .navigate(disabled.stamp(), Intent::NextItem)
        .await
        .unwrap();
    expect(&mut peer, b"\x1b[B").await;
    finish_board_peer(&connection, peer).await;
}
