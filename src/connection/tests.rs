//! Real production transport against loopback only; never contacts PTT.
use super::*;
use tokio::{io::AsyncReadExt, net::TcpListener};

#[test]
fn explicit_tls_provider_builds_with_desktop_feature_unification() {
    assert!(matches!(tls_connector().unwrap(), Connector::Rustls(_)));
}

#[tokio::test]
async fn window_probe_checks_vim_translation_on_realistic_rows_and_owned_shutdown() {
    use crate::vim::{Effect, Key, Keyboard};
    let connection = Connection::start_window_probe().unwrap();
    connection.connect().unwrap();
    let mut view = wait(&connection, |v| v.received > 0).await;
    let mut keyboard = Keyboard::default();
    keyboard.toggle();
    for value in ["j", "k", "g", "g", "G"] {
        let context = navigation::analyze(&view.frame);
        assert!(matches!(context, navigation::Context::ArticleList { .. }));
        let key = Key {
            key: value.into(),
            code: format!("Key{value}"),
            ctrl: false,
            alt: false,
            shift: false,
            repeat: false,
        };
        let effect = keyboard.key(&key, &context);
        keyboard.release(&key.code);
        assert!(!keyboard.accept_text(false));
        if let Effect::Browse(intent) = effect {
            let revision = view.frame.revision;
            connection.browse_key(view.stamp(), intent).await.unwrap();
            view = wait(&connection, |v| {
                v.frame.revision > revision && !v.input_pending
            })
            .await;
        } else {
            assert_eq!(effect, Effect::None);
        }
        assert_eq!(keyboard.status(&context).mode, "normal");
    }
    // The peer independently expects Down, Up, Home, End, then Close; neither
    // literal g/j/k/G nor an extra command can produce a successful join.
    connection.shutdown().await.unwrap();
}

#[tokio::test]
async fn keyboard_noop_streaming_and_unanswered_moves_have_one_owner() {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("reading_first").into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| v.received > 0).await;
    connection
        .browse_key(view.stamp(), Intent::First)
        .await
        .unwrap();
    assert!(timeout(Duration::from_millis(30), ws.next()).await.is_err());
    let lines = Intent::Lines {
        down: true,
        count: 1,
    };
    connection.browse_key(view.stamp(), lines).await.unwrap();
    connection.browse_key(view.stamp(), lines).await.unwrap();
    for _ in 0..2 {
        assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"j");
    }
    let pending = wait(&connection, |v| v.input_pending && v.input_epoch == 2).await;
    assert_eq!(
        connection
            .browse_key(pending.stamp(), Intent::Back)
            .await
            .unwrap_err(),
        navigation::Error::AwaitingScreen.to_string()
    );
    let released = wait(&connection, |v| !v.input_pending && v.input_epoch == 2).await;
    assert!(released.message.contains("結果未確認"));
    assert!(timeout(Duration::from_millis(30), ws.next()).await.is_err());
    let closing = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
    });
    connection.shutdown().await.unwrap();
    closing.await.unwrap();
}

async fn local() -> (TcpListener, Connection) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("ws://{}/bbs", listener.local_addr().unwrap());
    let connection = Connection::start_at(Screen::new(80, 24).unwrap(), endpoint).unwrap();
    (listener, connection)
}

async fn wait(connection: &Connection, predicate: impl Fn(&View) -> bool) -> View {
    let mut updates = connection.updates.clone();
    timeout(Duration::from_secs(4), async {
        loop {
            let view = updates.borrow_and_update().clone();
            if predicate(&view) {
                return view;
            }
            updates.changed().await.unwrap();
        }
    })
    .await
    .expect("expected connection state within deadline")
}

fn navigation_page(name: &str) -> Vec<u8> {
    let pages: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/navigation.json")).unwrap();
    crate::encoding::encode(pages[name].as_str().unwrap()).unwrap()
}

async fn idle_local() -> (TcpListener, Connection) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("ws://{}/bbs", listener.local_addr().unwrap());
    let connection = Connection::start_with_timing(
        Screen::new(80, 24).unwrap(),
        endpoint,
        liveness::Timing {
            idle: Duration::from_millis(120),
            poll: Duration::from_millis(10),
            resume_gap: Duration::from_secs(2),
            response_timeout: Duration::from_millis(500),
        },
    )
    .unwrap();
    (listener, connection)
}

#[tokio::test]
async fn idle_refresh_requires_ui_and_screen_admission_and_waits_for_redraw() {
    let (listener, connection) = idle_local().await;
    assert!(connection.updates.borrow().keep_alive);
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    let first = wait(&connection, |v| v.received > 0).await;
    assert!(
        timeout(Duration::from_millis(160), ws.next())
            .await
            .is_err()
    );
    connection.keep_alive_gate(
        Stamp {
            generation: first.generation + 1,
            ..first.stamp()
        },
        true,
    );
    assert!(
        timeout(Duration::from_millis(160), ws.next())
            .await
            .is_err()
    );
    connection.keep_alive_gate(first.stamp(), true);
    wait(&connection, |v| v.keep_alive_ready).await;
    assert_eq!(
        timeout(Duration::from_secs(3), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .into_data()
            .as_ref(),
        b"\x0c\x0c"
    );
    let pending = wait(&connection, |v| v.keep_alive_sent == 1 && v.input_pending).await;
    assert_eq!(pending.frame.cursor, first.frame.cursor);
    assert_eq!(
        connection
            .navigate(first.stamp(), Intent::Open { row: 5 })
            .await
            .unwrap_err(),
        navigation::Error::Stale.to_string()
    );
    connection.keep_alive_gate(pending.stamp(), true);
    assert!(
        timeout(Duration::from_millis(200), ws.next())
            .await
            .is_err(),
        "no reply must not cause another refresh"
    );
    connection.keep_alive(false).unwrap();
    wait(&connection, |v| !v.keep_alive).await;
    ws.send(Message::Binary(navigation_page("reading_middle").into()))
        .await
        .unwrap();
    let reading = wait(&connection, |v| !v.input_pending && v.frame.cursor.1 == 23).await;
    connection.keep_alive_gate(reading.stamp(), true);
    assert!(
        timeout(Duration::from_millis(160), ws.next())
            .await
            .is_err(),
        "explicit off must suppress writes"
    );
    connection.keep_alive(true).unwrap();
    let enabled = wait(&connection, |v| v.keep_alive).await;
    connection.keep_alive_gate(enabled.stamp(), true);
    assert_eq!(
        timeout(Duration::from_secs(3), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .into_data()
            .as_ref(),
        b"\x0c\x0c"
    );
    let closing = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
    });
    connection.shutdown().await.unwrap();
    closing.await.unwrap();
    assert!(!connection.updates.borrow().keep_alive_ready);
}

#[tokio::test]
async fn idle_refresh_pauses_on_local_activity_unknown_frames_and_reconnect() {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("reading_first").into()))
        .await
        .unwrap();
    let first = wait(&connection, |v| v.received > 0).await;
    connection.keep_alive_gate(first.stamp(), true);
    wait(&connection, |v| v.keep_alive_ready).await;
    connection.input(first.generation, b"x".to_vec()).unwrap();
    assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"x");
    ws.send(Message::Binary(navigation_page("reading_middle").into()))
        .await
        .unwrap();
    let updated = wait(&connection, |v| !v.input_pending && v.input_epoch == 1).await;
    connection.keep_alive_gate(first.stamp(), true);
    assert!(
        timeout(Duration::from_millis(160), ws.next())
            .await
            .is_err(),
        "stale UI cannot permit refresh"
    );
    connection.keep_alive_gate(updated.stamp(), true);
    wait(&connection, |v| v.keep_alive_ready).await;
    connection.keep_alive_gate(updated.stamp(), false);
    wait(&connection, |v| !v.keep_alive_ready).await;
    assert!(
        timeout(Duration::from_millis(160), ws.next())
            .await
            .is_err(),
        "composition/modal/selection gate must pause"
    );
    ws.send(Message::Binary(
        b"\x1b[2J\x1b[22;1HPassword: ".to_vec().into(),
    ))
    .await
    .unwrap();
    let unknown = wait(&connection, |v| v.frame.revision > updated.frame.revision).await;
    connection.keep_alive_gate(unknown.stamp(), true);
    assert!(
        timeout(Duration::from_millis(160), ws.next())
            .await
            .is_err(),
        "an unknown prompt must reject even a ready UI"
    );
    let closing = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
    });
    connection.disconnect().unwrap();
    closing.await.unwrap();
    wait(&connection, |v| v.phase == Phase::Disconnected).await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    wait(&connection, |v| {
        v.received > 0 && v.generation > first.generation
    })
    .await;
    connection.keep_alive_gate(first.stamp(), true);
    assert!(
        timeout(Duration::from_millis(160), ws.next())
            .await
            .is_err(),
        "reconnect must not reuse a permit"
    );
    let closing = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
    });
    connection.shutdown().await.unwrap();
    closing.await.unwrap();
}

#[tokio::test]
async fn an_unanswered_idle_refresh_pauses_itself_and_releases_mouse_navigation() {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("reading_first").into()))
        .await
        .unwrap();
    let first = wait(&connection, |v| v.received > 0).await;
    connection.keep_alive_gate(first.stamp(), true);
    assert_eq!(
        timeout(Duration::from_secs(3), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .into_data()
            .as_ref(),
        b"\x0c\x0c"
    );
    let paused = wait(&connection, |v| {
        !v.keep_alive && v.keep_alive_failure.is_some() && !v.input_pending
    })
    .await;
    assert_eq!(paused.phase, Phase::Connected);
    assert!(
        timeout(Duration::from_millis(160), ws.next())
            .await
            .is_err()
    );
    connection.keep_alive(true).unwrap();
    let resumed = wait(&connection, |v| {
        v.keep_alive && v.keep_alive_failure.is_none()
    })
    .await;
    assert_ne!(resumed.message, paused.message);
    connection
        .navigate(resumed.stamp(), Intent::NextPage)
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\x1b[6~"
    );
    let closing = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
    });
    connection.shutdown().await.unwrap();
    closing.await.unwrap();
}

#[tokio::test]
async fn idle_response_deadline_cannot_clear_a_later_manual_input() {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    let first = wait(&connection, |v| v.received > 0).await;
    connection.keep_alive_gate(first.stamp(), true);
    assert_eq!(
        timeout(Duration::from_secs(3), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .into_data()
            .as_ref(),
        b"\x0c\x0c"
    );
    tokio::time::sleep(Duration::from_millis(350)).await;
    connection.input(first.generation, b"x".to_vec()).unwrap();
    assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"x");
    wait(&connection, |v| v.input_epoch == 2 && v.input_pending).await;
    assert!(
        timeout(Duration::from_millis(200), ws.next())
            .await
            .is_err()
    );
    let pending = connection.updates.borrow().clone();
    assert!(pending.keep_alive && pending.input_pending && pending.keep_alive_failure.is_none());
    let released = wait(&connection, |v| v.input_epoch == 2 && !v.input_pending).await;
    assert!(released.keep_alive && released.keep_alive_failure.is_none());
    assert!(released.message.contains("結果未確認"));
    let closing = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
    });
    connection.shutdown().await.unwrap();
    closing.await.unwrap();
}

#[tokio::test]
async fn pending_input_deadline_never_overwrites_a_later_peer_close_reason() {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| v.received > 0).await;
    connection.input(view.generation, b"x".to_vec()).unwrap();
    assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"x");
    wait(&connection, |v| v.input_pending).await;
    ws.close(Some(tokio_tungstenite::tungstenite::protocol::CloseFrame {
        code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Policy,
        reason: "synthetic pending close".into(),
    }))
    .await
    .unwrap();
    let closed = wait(&connection, |v| v.phase == Phase::Disconnected).await;
    assert!(closed.message.contains("synthetic pending close"));
    tokio::time::sleep(Duration::from_millis(650)).await;
    assert_eq!(connection.updates.borrow().message, closed.message);
    connection.shutdown().await.unwrap();
}

#[tokio::test]
async fn navigation_admission_checks_real_frame_input_order_and_one_pending_operation() {
    let (listener, connection) = local().await;
    let (updated_tx, updated_rx) = oneshot::channel();
    let (opened_tx, opened_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        ws.send(Message::Binary(navigation_page("list").into()))
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            b"\x1b[A"
        );
        updated_rx.await.unwrap();
        let mut updated = navigation_page("list");
        updated.extend_from_slice(b"\x1b[5;1H \x1b[4;1H>\x1b[4;1H");
        ws.send(Message::Binary(updated.into())).await.unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            b"\x1b[B\x1b[B\r"
        );
        opened_rx.await.unwrap();
        ws.send(Message::Binary(navigation_page("reading_middle").into()))
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            b"\x1b[6~"
        );
        ws.send(Message::Binary(navigation_page("reading_last").into()))
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            b"\x1b[D"
        );
        ws.send(Message::Binary(navigation_page("list").into()))
            .await
            .unwrap();
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
    });
    connection.connect().unwrap();
    let old = wait(&connection, |v| v.received > 0).await;
    connection
        .input(old.generation, b"\x1b[A".to_vec())
        .unwrap();
    assert_eq!(
        connection
            .navigate(old.stamp(), Intent::Open { row: 5 })
            .await
            .unwrap_err(),
        navigation::Error::Stale.to_string()
    );
    let pending = wait(&connection, |v| v.input_epoch == 1 && v.input_pending).await;
    assert_eq!(
        connection
            .navigate(pending.stamp(), Intent::Open { row: 5 })
            .await
            .unwrap_err(),
        navigation::Error::AwaitingScreen.to_string()
    );
    updated_tx.send(()).unwrap();
    let updated = wait(&connection, |v| v.frame.cursor.1 == 3 && !v.input_pending).await;
    connection
        .navigate(updated.stamp(), Intent::Open { row: 5 })
        .await
        .unwrap();
    let pending = wait(&connection, |v| v.input_epoch == 2 && v.input_pending).await;
    assert_eq!(
        connection
            .navigate(pending.stamp(), Intent::Open { row: 5 })
            .await
            .unwrap_err(),
        navigation::Error::AwaitingScreen.to_string()
    );
    opened_tx.send(()).unwrap();
    let reading = wait(&connection, |v| {
        matches!(
            navigation::analyze(&v.frame),
            navigation::Context::ArticleReading { percent: 66, .. }
        )
    })
    .await;
    connection
        .navigate(reading.stamp(), Intent::NextPage)
        .await
        .unwrap();
    let last = wait(&connection, |v| {
        matches!(
            navigation::analyze(&v.frame),
            navigation::Context::ArticleReading { percent: 100, .. }
        )
    })
    .await;
    assert_eq!(
        connection
            .navigate(last.stamp(), Intent::NextPage)
            .await
            .unwrap_err(),
        navigation::Error::Unavailable.to_string()
    );
    connection
        .navigate(last.stamp(), Intent::Back)
        .await
        .unwrap();
    let returned = wait(&connection, |v| {
        matches!(
            navigation::analyze(&v.frame),
            navigation::Context::ArticleList { .. }
        )
    })
    .await;
    assert_eq!(
        connection
            .navigate(last.stamp(), Intent::Back)
            .await
            .unwrap_err(),
        navigation::Error::Stale.to_string()
    );
    // An already cancelled queued request must not send bytes when it reaches the owner.
    let (reply, answer) = oneshot::channel();
    drop(answer);
    connection
        .enqueue(Command::Navigate {
            keyboard: false,
            stamp: returned.stamp(),
            intent: Intent::Open { row: 4 },
            reply,
        })
        .unwrap();
    connection.shutdown().await.unwrap();
    timeout(Duration::from_secs(4), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn unknown_partial_or_wrong_session_navigation_sends_no_input() {
    let (listener, connection) = local().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let mut bytes = navigation_page("list");
        bytes.extend_from_slice(b"\x1b[");
        ws.send(Message::Binary(bytes.into())).await.unwrap();
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
    });
    connection.connect().unwrap();
    let view = wait(&connection, |v| v.received > 0).await;
    assert_eq!(
        connection
            .navigate(view.stamp(), Intent::Open { row: 5 })
            .await
            .unwrap_err(),
        navigation::Error::Unavailable.to_string()
    );
    assert_eq!(
        connection
            .navigate(
                Stamp {
                    generation: view.generation + 1,
                    ..view.stamp()
                },
                Intent::Open { row: 5 }
            )
            .await
            .unwrap_err(),
        navigation::Error::Disconnected.to_string()
    );
    connection.shutdown().await.unwrap();
    timeout(Duration::from_secs(4), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn fragmented_binary_reaches_screen_and_input_is_explicit_ordered_and_escaped() {
    let (listener, connection) = local().await;
    let (quiet_tx, quiet_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        for bytes in [b"\xff".as_slice(), b"\xfd\x1f\x1b[31m\xa4", b"\x1b[32m\xa4"] {
            ws.send(Message::Binary(bytes.to_vec().into()))
                .await
                .unwrap();
        }
        let reply = timeout(Duration::from_secs(4), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(
            reply.into_data().as_ref(),
            [255, 251, 31, 255, 250, 31, 0, 80, 0, 24, 255, 240]
        );
        assert!(
            timeout(Duration::from_millis(150), ws.next())
                .await
                .is_err(),
            "must not auto-login"
        );
        quiet_tx.send(()).unwrap();
        for expected in [b"first\r".as_slice(), &[255, 255, b'X']] {
            let message = timeout(Duration::from_secs(4), ws.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(message.into_data().as_ref(), expected);
        }
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
    });
    connection.connect().unwrap();
    let view = wait(&connection, |v| v.received >= 15).await;
    let glyph = &view.frame.glyphs[0];
    assert_eq!((&*glyph.text, glyph.left.fg, glyph.right.fg), ("中", 1, 2));
    quiet_rx.await.unwrap();
    connection
        .input(view.generation, b"first\r".to_vec())
        .unwrap();
    connection.input(view.generation, vec![255, b'X']).unwrap();
    connection.shutdown().await.unwrap();
    timeout(Duration::from_secs(4), server)
        .await
        .unwrap()
        .unwrap();
    let mut updates = connection.updates.clone();
    updates.borrow_and_update();
    assert!(updates.changed().await.is_err());
    connection.shutdown().await.unwrap();
}

#[tokio::test]
async fn reconnect_resets_screen_and_never_replays_old_input() {
    let (listener, connection) = local().await;
    let server = tokio::spawn(async move {
        for label in [b"OLD\x1b]unfinished-control-string".as_slice(), b"NEW"] {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            ws.send(Message::Binary(label.to_vec().into()))
                .await
                .unwrap();
            if label == b"NEW" {
                assert_eq!(
                    ws.next().await.unwrap().unwrap().into_data().as_ref(),
                    b"fresh"
                );
            }
            assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
        }
    });
    connection.connect().unwrap();
    let old = wait(&connection, |v| v.received > 0).await;
    connection.disconnect().unwrap();
    wait(&connection, |v| v.phase == Phase::Disconnected).await;
    connection.connect().unwrap();
    let new = wait(&connection, |v| {
        v.phase == Phase::Connected && v.received == 3 && v.generation > old.generation
    })
    .await;
    assert_eq!(
        new.frame
            .glyphs
            .iter()
            .take(3)
            .map(|g| g.text.as_str())
            .collect::<String>(),
        "NEW"
    );
    connection
        .input(old.generation, b"stale-secret".to_vec())
        .unwrap();
    connection.input(new.generation, b"fresh".to_vec()).unwrap();
    connection.shutdown().await.unwrap();
    timeout(Duration::from_secs(4), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn disconnect_cancels_and_joins_pending_handshake() {
    let (listener, connection) = local().await;
    let (accepted_tx, accepted_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        accepted_tx.send(()).unwrap();
        let mut bytes = Vec::new();
        timeout(Duration::from_secs(4), stream.read_to_end(&mut bytes))
            .await
            .unwrap()
            .unwrap();
    });
    connection.connect().unwrap();
    accepted_rx.await.unwrap();
    connection.disconnect().unwrap();
    wait(&connection, |v| {
        v.phase == Phase::Disconnected && v.generation > 0
    })
    .await;
    connection.shutdown().await.unwrap();
    timeout(Duration::from_secs(4), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn peer_close_is_reported_and_worker_can_stop() {
    use tokio_tungstenite::tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};
    for close in [
        None,
        Some(CloseFrame {
            code: CloseCode::Policy,
            reason: "synthetic policy".into(),
        }),
    ] {
        let expected = match &close {
            Some(_) => "站端已關閉連線（1008）：synthetic policy",
            None => "站端已關閉連線（未提供原因）",
        };
        let (listener, connection) = local().await;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            ws.close(close).await.unwrap();
        });
        connection.connect().unwrap();
        let closed = wait(&connection, |v| {
            v.phase == Phase::Disconnected && v.message.contains("站端")
        })
        .await;
        assert_eq!(closed.message, expected);
        connection
            .input(closed.generation, b"n\r".to_vec())
            .unwrap();
        // Shutdown is queued after input, so a joined worker proves the input was handled.
        connection.shutdown().await.unwrap();
        assert_eq!(connection.updates.borrow().message, expected);
        server.await.unwrap();
    }
}

#[tokio::test]
async fn unexpected_text_and_oversized_binary_fail_without_drawing_payload() {
    for payload in [
        Message::Text("not raw bytes".into()),
        Message::Binary(vec![b'X'; 65537].into()),
    ] {
        let (listener, connection) = local().await;
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            ws.send(payload).await.unwrap();
            let _ = ws.next().await;
        });
        connection.connect().unwrap();
        let view = wait(&connection, |v| v.phase == Phase::Error).await;
        assert_eq!(view.received, 0);
        assert!(view.frame.glyphs.iter().all(|g| g.text == " "));
        connection.shutdown().await.unwrap();
        timeout(Duration::from_secs(4), server)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn invalid_and_offline_input_is_rejected_without_connecting() {
    let (_listener, connection) = local().await;
    assert!(connection.input(0, Vec::new()).is_err());
    assert!(connection.input(0, vec![0; 4097]).is_err());
    connection.input(0, b"offline".to_vec()).unwrap();
    let view = wait(&connection, |v| v.message.contains("未送出")).await;
    assert_eq!(view.phase, Phase::Disconnected);
    assert_eq!(view.received, 0);
    connection.shutdown().await.unwrap();
}
