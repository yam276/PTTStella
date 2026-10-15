//! Real production transport against loopback only; never contacts PTT.
use super::*;
use tokio::{io::AsyncReadExt, net::TcpListener};

#[test]
fn explicit_tls_provider_builds_with_desktop_feature_unification() {
    assert!(matches!(tls_connector().unwrap(), Connector::Rustls(_)));
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
