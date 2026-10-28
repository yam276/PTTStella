//! Production Session and loopback peer; every expected write is independent.
use super::*;
use crate::article_target::ArticleTarget;
use crate::navigation_job::Destination;
type LocalSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;
fn page(name: &str) -> Vec<u8> {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/article-open.json")).unwrap();
    crate::encoding::encode(fixture["screens"][name].as_str().unwrap()).unwrap()
}
fn target() -> ArticleTarget {
    ArticleTarget::new("Stella", "00000101").unwrap()
}
async fn expect(ws: &mut LocalSocket, expected: &[u8]) {
    let actual = timeout(Duration::from_secs(2), ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        matches!(actual,Message::Binary(bytes) if bytes.as_ref()==expected),
        "expected {expected:?}"
    );
}
async fn send(ws: &mut LocalSocket, name: &str) {
    ws.send(Message::Binary(page(name).into())).await.unwrap();
}
async fn connected(name: &str) -> (tokio::net::TcpListener, Connection, LocalSocket, View) {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    send(&mut ws, name).await;
    let view = wait(&connection, |view| {
        view.received > 0 && !view.output_pending
    })
    .await;
    (listener, connection, ws, view)
}
async fn start_information(connection: &Connection, ws: &mut LocalSocket, view: &View) {
    connection
        .request_open_article(view.stamp(), target())
        .unwrap()
        .wait()
        .await
        .unwrap();
    expect(ws, b"#").await;
    send(ws, "aid_prompt").await;
    expect(ws, b"00000101\r").await;
    send(ws, "articles").await;
    expect(ws, b"Q").await;
}
#[tokio::test]
async fn article_destination_uses_the_one_writer_through_canonical_board_and_verified_identity() {
    let (_listener, connection, mut ws, initial) = connected("main").await;
    connection
        .request_open_article(
            initial.stamp(),
            ArticleTarget::new("stella", "00000101").unwrap(),
        )
        .unwrap()
        .wait()
        .await
        .unwrap();
    expect(&mut ws, b"\r").await;
    let active = connection.updates.borrow().clone();
    assert!(active.input_pending && !active.keep_alive_ready);
    assert!(
        connection
            .request_switch_board(active.stamp(), BoardName::parse("Other").unwrap())
            .unwrap()
            .wait()
            .await
            .is_err()
    );
    for (scene, bytes) in [
        ("boards", b"s".as_slice()),
        ("board_prompt", b"stella\r"),
        ("selected", b"\x1b[C"),
        ("articles", b"#"),
        ("aid_prompt", b"00000101\r"),
        ("articles", b"Q"),
        ("information", b" "),
        ("articles", b"\x1b[C"),
    ] {
        send(&mut ws, scene).await;
        expect(&mut ws, bytes).await;
        let view = connection.updates.borrow().clone();
        assert!(!view.keep_alive_ready);
        assert_eq!(view.keep_alive_sent, 0);
    }
    send(&mut ws, "reading").await;
    let reached = wait(&connection, |view| {
        matches!(
            &view.destination,
            NavigationProgress::Reached {
                target: Destination::Article(_),
                ..
            }
        )
    })
    .await;
    assert_eq!(
        reached.destination,
        NavigationProgress::Reached {
            serial: 1,
            target: Destination::Article(target())
        }
    );
    assert!(!reached.input_pending);
    let visit = reached.board_visit.unwrap();
    assert_eq!(visit.board.as_str(), "Stella");
    assert_eq!(visit.generation, initial.generation);
    finish_board_peer(&connection, ws).await;
}
#[tokio::test]
async fn synchronized_information_cannot_dismiss_a_pause_while_ping_and_shutdown_remain_live() {
    let (_listener, connection, mut ws, view) = connected("articles").await;
    start_information(&connection, &mut ws, &view).await;
    let bytes = page("information");
    let middle = bytes.len() / 2;
    ws.send(Message::Binary(
        [b"\x1b[?2026h".as_slice(), &bytes[..middle]]
            .concat()
            .into(),
    ))
    .await
    .unwrap();
    wait(&connection, |view| view.output_pending).await;
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    ws.send(Message::Ping(vec![4, 2].into())).await.unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap(),
        Message::Pong(vec![4, 2].into())
    );
    ws.send(Message::Binary(
        [&bytes[middle..], b"\x1b[?2026l"].concat().into(),
    ))
    .await
    .unwrap();
    expect(&mut ws, b" ").await;
    // Shutdown at the restoration wait must never send Right.
    finish_board_peer(&connection, ws).await;
}
#[tokio::test]
async fn missing_mismatched_or_manual_screens_stop_without_any_followup_or_retry() {
    for (scene, expected_board) in [
        ("missing", "Stella"),
        ("deleted", "Stella"),
        ("permission", "Stella"),
        ("wrong_aid", "Stella"),
        ("wrong_board", "Stella"),
        ("other_articles", "Other"),
        ("entry", "Stella"),
        ("help", "Stella"),
    ] {
        let (_listener, connection, mut ws, view) = connected("articles").await;
        start_information(&connection, &mut ws, &view).await;
        send(&mut ws, scene).await;
        wait(&connection, |view| {
            matches!(view.destination, NavigationProgress::Idle)
        })
        .await;
        let visit = connection.updates.borrow().board_visit.clone().unwrap();
        assert_eq!(visit.board.as_str(), expected_board);
        assert!(
            timeout(Duration::from_millis(30), ws.next()).await.is_err(),
            "{scene}"
        );
        send(&mut ws, "information").await;
        assert!(
            timeout(Duration::from_millis(30), ws.next()).await.is_err(),
            "late {scene}"
        );
        finish_board_peer(&connection, ws).await;
    }
}
#[tokio::test]
async fn cancellation_and_raw_intervention_retire_every_remaining_article_step() {
    for (stop, late) in [
        (0, "aid_prompt"),
        (1, "articles"),
        (2, "information"),
        (3, "articles"),
        (4, "reading"),
    ] {
        for raw in [false, true] {
            let (_listener, connection, mut ws, view) = connected("articles").await;
            connection
                .request_open_article(view.stamp(), target())
                .unwrap()
                .wait()
                .await
                .unwrap();
            expect(&mut ws, b"#").await;
            for (scene, bytes) in [
                ("aid_prompt", b"00000101\r".as_slice()),
                ("articles", b"Q"),
                ("information", b" "),
                ("articles", b"\x1b[C"),
            ]
            .into_iter()
            .take(stop)
            {
                send(&mut ws, scene).await;
                expect(&mut ws, bytes).await;
            }
            // Stale session input cannot revoke a current job.
            connection
                .input(view.generation + 1, b"x".to_vec())
                .unwrap();
            let current = wait(&connection, |current| {
                current.message == crate::msg!("input-stale")
            })
            .await;
            assert!(matches!(
                current.destination,
                NavigationProgress::Active { .. }
            ));
            if raw {
                connection.input(view.generation, b"x".to_vec()).unwrap();
                expect(&mut ws, b"x").await;
            } else {
                connection.cancel_navigation(view.stamp()).unwrap();
            }
            wait(&connection, |current| {
                matches!(current.destination, NavigationProgress::Idle)
            })
            .await;
            send(&mut ws, late).await;
            assert!(
                timeout(Duration::from_millis(30), ws.next()).await.is_err(),
                "stage {stop}, raw {raw}"
            );
            finish_board_peer(&connection, ws).await;
        }
    }
}
#[tokio::test]
async fn timeout_and_reconnect_do_not_replay_article_input_or_relabel_an_old_visit() {
    let (listener, connection, mut ws, view) = connected("articles").await;
    connection
        .request_open_article(view.stamp(), target())
        .unwrap()
        .wait()
        .await
        .unwrap();
    expect(&mut ws, b"#").await;
    let mut updates = connection.updates.clone();
    timeout(Duration::from_secs(7), async {
        loop {
            if updates.borrow_and_update().message == crate::msg!("article-open-timeout") {
                break;
            }
            updates.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    send(&mut ws, "aid_prompt").await;
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    let old = connection.updates.borrow().board_visit.clone().unwrap();
    let peer = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
        ws.flush().await.unwrap();
    });
    connection.disconnect().unwrap();
    let disconnected = wait(&connection, |view| view.phase == Phase::Disconnected).await;
    peer.await.unwrap();
    assert_eq!(disconnected.board_visit.unwrap().generation, old.generation);
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    let fresh = wait(&connection, |view| view.phase == Phase::Connected).await;
    assert!(fresh.generation > old.generation);
    assert!(fresh.board_visit.is_none());
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    send(&mut ws, "articles").await;
    let arrived = wait(&connection, |view| view.board_visit.is_some()).await;
    assert_eq!(
        arrived.board_visit.as_ref().unwrap().generation,
        fresh.generation
    );
    assert!(arrived.board_visit.unwrap().serial > old.serial);
    finish_board_peer(&connection, ws).await;
}
#[tokio::test]
async fn confirmed_board_fact_survives_a_pending_article_job_and_later_unknown_pixels() {
    let (_listener, connection, mut ws, initial) = connected("boards").await;
    assert!(initial.board_visit.is_none());
    connection
        .request_open_article(initial.stamp(), target())
        .unwrap()
        .wait()
        .await
        .unwrap();
    expect(&mut ws, b"s").await;
    send(&mut ws, "board_prompt").await;
    expect(&mut ws, b"Stella\r").await;
    send(&mut ws, "articles").await;
    expect(&mut ws, b"#").await;
    let arrived = wait(&connection, |view| view.board_visit.is_some()).await;
    assert!(arrived.input_pending);
    let fact = arrived.board_visit.clone().unwrap();
    assert_eq!(fact.board.as_str(), "Stella");
    send(&mut ws, "entry").await;
    let stopped = wait(&connection, |view| {
        matches!(view.destination, NavigationProgress::Idle)
    })
    .await;
    assert_eq!(stopped.board_visit, Some(fact));
    finish_board_peer(&connection, ws).await;
}
