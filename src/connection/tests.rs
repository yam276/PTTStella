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
