//! Real production transport against loopback only; never contacts PTT.
use super::transport::tls_connector;
use super::*;
use crate::navigation;
use futures_util::{SinkExt, StreamExt};
use std::time::Instant;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_tungstenite::{
    Connector,
    tungstenite::{Message, protocol::frame::coding::CloseCode},
};

#[test]
fn explicit_tls_provider_builds_with_desktop_feature_unification() {
    assert!(matches!(tls_connector().unwrap(), Connector::Rustls(_)));
}

#[tokio::test]
async fn reply_footer_reads_past_halfway_to_end_and_rejects_a_later_prompt() {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    for (index, percent) in [9, 18, 26, 35, 44, 52, 64, 74, 86, 100]
        .into_iter()
        .enumerate()
    {
        let first = 1 + index * 22;
        let bytes = crate::encoding::encode(&format!("\x1b[2J\x1b[HOwn synthetic article page {}\x1b[24;1H\x1b[30;47m\x1b[2K瀏覽 第 {}/10 頁 ({percent}%) 目前顯示: 第 {first}~{} 行 (y)回應(X)推文(h)說明\x1b[0m\x1b[24;80H",index+1,index+1,first+22)).unwrap();
        // Exercise the real receiver with a response split across messages.
        let split = bytes.len() / 2;
        ws.send(Message::Binary(bytes[..split].to_vec().into()))
            .await
            .unwrap();
        ws.send(Message::Binary(bytes[split..].to_vec().into()))
            .await
            .unwrap();
        let view = wait(&connection, |v| {
            !v.input_pending
                && !v.output_pending
                && v.context()
                    == navigation::Context::ArticleReading {
                        first_line: first,
                        last_line: first + 22,
                        percent,
                    }
        })
        .await;
        let result = connection
            .navigate(view.stamp(), navigation::Intent::NextPage)
            .await;
        if percent < 100 {
            result.unwrap();
            assert_eq!(
                ws.next().await.unwrap().unwrap().into_data().as_ref(),
                b"\x1b[6~"
            );
        } else {
            assert!(result.is_err());
        }
    }
    ws.send(Message::Binary(
        crate::encoding::encode("\x1b[24;1H\x1b[30;47m\x1b[2K是否繼續？[Y/n]\x1b[0m\x1b[24;80H")
            .unwrap()
            .into(),
    ))
    .await
    .unwrap();
    let prompt = wait(&connection, |v| {
        !v.output_pending && matches!(v.context(), navigation::Context::Unknown { .. })
    })
    .await;
    assert!(
        connection
            .navigate(prompt.stamp(), navigation::Intent::NextPage)
            .await
            .is_err()
    );
    assert!(timeout(Duration::from_millis(80), ws.next()).await.is_err());
    finish_board_peer(&connection, ws).await;
}

#[tokio::test]
async fn pointer_menu_favorites_pages_and_row_open_share_the_production_sender() {
    use crate::navigation::Intent;
    let connection = Connection::start_pointer_rows_probe().unwrap();
    connection.connect().unwrap();
    let mut view = wait(&connection, |v| v.received > 0 && !v.input_pending).await;
    assert_eq!(view.context().kind(), "main_menu");
    connection
        .navigate(view.stamp(), Intent::Open { row: 14 })
        .await
        .unwrap();
    let revision = view.frame.revision;
    view = wait(&connection, |v| {
        v.frame.revision > revision && !v.input_pending
    })
    .await;
    assert_eq!(view.context().kind(), "unknown");
    connection
        .input(view.generation, b"\x1b[D".to_vec())
        .unwrap();
    view = wait(&connection, |v| {
        v.context().kind() == "main_menu" && !v.input_pending
    })
    .await;
    for (intent, expected) in [
        (Intent::Open { row: 13 }, "board_list"),
        (Intent::Open { row: 3 }, "article_list"),
        (Intent::NextPage, "article_list"),
        (Intent::PreviousPage, "article_list"),
        (Intent::Open { row: 5 }, "article_reading"),
        (Intent::Back, "article_list"),
    ] {
        let revision = view.frame.revision;
        connection.navigate(view.stamp(), intent).await.unwrap();
        view = wait(&connection, |v| {
            v.frame.revision > revision && !v.input_pending
        })
        .await;
        assert_eq!(view.context().kind(), expected);
    }
    connection.shutdown().await.unwrap();
}

#[tokio::test]
async fn nested_menu_pointer_open_and_back_use_the_same_revalidated_sender() {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    let mut bytes = navigation_page("main_menu");
    bytes.extend_from_slice(&crate::encoding::encode("\x1b[1;1H\x1b[2K\x1b[37;44m個人設定區\x1b[24;1H\x1b[2K\x1b[30;47m個人設定 (h)說明 (←)回上層\x1b[0m\x1b[14;22H").unwrap());
    ws.send(Message::Binary(bytes.clone().into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| {
        v.context().kind() == "system_menu" && !v.input_pending
    })
    .await;
    let open = connection
        .request_navigate(
            view.stamp(),
            navigation::Intent::Open { row: 14 },
            navigation::Source::Pointer,
        )
        .unwrap();
    open.wait().await.unwrap();
    assert!(
        matches!(ws.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"\x1b[B\r")
    );
    ws.send(Message::Binary(bytes.into())).await.unwrap();
    let view = wait(&connection, |v| {
        v.frame.revision > view.frame.revision && !v.input_pending
    })
    .await;
    connection
        .navigate(view.stamp(), navigation::Intent::Back)
        .await
        .unwrap();
    assert!(
        matches!(ws.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"\x1b[D")
    );
    ws.send(Message::Binary(
        b"\x1b[2J\x1b[HConfirm? [Y/n]".to_vec().into(),
    ))
    .await
    .unwrap();
    let view = wait(&connection, |v| {
        v.context().kind() == "unknown" && !v.input_pending
    })
    .await;
    assert!(
        connection
            .navigate(view.stamp(), navigation::Intent::Open { row: 14 })
            .await
            .is_err()
    );
    let peer = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
        ws.flush().await.unwrap();
    });
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
    peer.await.unwrap();
}

#[tokio::test]
async fn synchronized_output_retains_complete_frames_but_accepts_raw_input_and_ping() {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    let before = wait(&connection, |v| v.received > 0 && !v.output_pending).await;
    connection.keep_alive_gate(before.stamp(), true);
    ws.send(Message::Binary(
        b"\x1b[?2026h\x1b[2J\x1b[Hpartial".to_vec().into(),
    ))
    .await
    .unwrap();
    let held = wait(&connection, |v| {
        v.output_pending && v.received > before.received
    })
    .await;
    assert_eq!(held.frame, before.frame);
    assert!(!held.keep_alive_ready);
    assert_eq!(held.context().kind(), "unknown");
    assert!(
        connection
            .browse_key(before.stamp(), navigation::Intent::Forward)
            .await
            .is_err()
    );
    assert!(
        connection
            .navigate(before.stamp(), navigation::Intent::Forward)
            .await
            .is_err()
    );
    // The idle interval elapses while output is held, without sending a refresh.
    assert!(
        timeout(Duration::from_millis(160), ws.next())
            .await
            .is_err()
    );
    ws.send(Message::Ping(b"sync-alive".to_vec().into()))
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap(),
        Message::Pong(b"sync-alive".to_vec().into())
    );
    connection.input(held.generation, vec![13]).unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\r"
    );
    assert_eq!(connection.updates.borrow().frame, before.frame);
    let final_page = [navigation_page("reading_middle"), b"\x1b[?2026l".to_vec()].concat();
    ws.send(Message::Binary(final_page.into())).await.unwrap();
    let after = wait(&connection, |v| {
        !v.output_pending && v.frame.revision > before.frame.revision
    })
    .await;
    assert_eq!(after.context().kind(), "article_reading");
    let mut expected = Screen::new(80, 24).unwrap();
    expected.feed(&navigation_page("reading_middle"));
    assert_eq!(after.frame.glyphs, expected.frame().glyphs);
    let peer = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
        ws.flush().await.unwrap();
    });
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
    peer.await.unwrap();
}

#[tokio::test]
async fn missing_sync_end_recovers_and_close_during_a_new_batch_still_joins() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(b"\x1b[?2026hRECOVER".to_vec().into()))
        .await
        .unwrap();
    let held = wait(&connection, |v| v.output_pending && v.received > 0).await;
    assert!(held.frame.glyphs.iter().all(|g| g.text == " "));
    let recovered = wait(&connection, |v| !v.output_pending && v.frame.revision > 0).await;
    assert_eq!(recovered.frame.cursor, (7, 0));
    assert_eq!(recovered.frame.glyphs[0].text, "R");
    assert!(recovered.frame.control_complete);
    assert_eq!(recovered.context().kind(), "unknown");
    ws.send(Message::Binary(b"\x1b[?2026h\rNEW".to_vec().into()))
        .await
        .unwrap();
    let held = wait(&connection, |v| {
        v.output_pending && v.received > recovered.received
    })
    .await;
    assert_eq!(held.frame, recovered.frame);
    let peer = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
        ws.flush().await.unwrap();
    });
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
    peer.await.unwrap();
    assert!(connection.worker.lock().unwrap().is_none());
}

#[tokio::test]
async fn failed_shutdown_still_joins_the_worker_and_keeps_its_failure_on_repeat() {
    let (_listener, connection) = local().await;
    connection.worker.lock().unwrap().as_ref().unwrap().abort();
    tokio::task::yield_now().await;
    let first = connection.shutdown().await.unwrap_err();
    assert!(connection.worker.lock().unwrap().is_none());
    assert!(connection.probe_peer.lock().unwrap().is_none());
    assert_eq!(connection.shutdown().await.unwrap_err(), first);
}

#[tokio::test]
async fn accepted_browse_is_enqueued_before_native_input_without_polling_its_reply() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| v.received > 0).await;
    let reply = connection
        .request_browse_key(
            view.stamp(),
            navigation::Intent::Lines {
                down: true,
                count: 1,
            },
        )
        .unwrap();
    connection.input(view.generation, vec![13]).unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\x1b[B"
    );
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\r"
    );
    reply.wait().await.unwrap();
    let peer = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
        ws.flush().await.unwrap();
    });
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
    peer.await.unwrap();
}

#[tokio::test]
async fn dropping_enqueued_ui_waits_revokes_login_and_browse_before_the_writer_runs() {
    for login in [true, false] {
        let (listener, connection) = local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        ws.send(Message::Binary(if login {
            probe::login_page().unwrap().into()
        } else {
            navigation_page("list").into()
        }))
        .await
        .unwrap();
        let view = wait(&connection, |v| v.received > 0).await;
        // Poll the real public API once: it enqueues the command then waits for
        // its oneshot. This current-thread runtime does not poll the writer
        // between this enqueue and cancellation, making the race deterministic.
        if login {
            let mut wait = Box::pin(connection.login(view.stamp(), credentials()));
            assert!(futures_util::poll!(&mut wait).is_pending());
            drop(wait);
        } else {
            let mut wait = Box::pin(connection.browse_key(view.stamp(), navigation::Intent::Back));
            assert!(futures_util::poll!(&mut wait).is_pending());
            drop(wait);
        }
        let peer = tokio::spawn(async move {
            assert!(
                matches!(ws.next().await, Some(Ok(Message::Close(_)))),
                "canceled operation sent station bytes"
            );
            ws.flush().await.unwrap();
        });
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
        peer.await.unwrap();
    }
}

#[tokio::test]
async fn shutdown_sends_normal_close_and_waits_for_delayed_peer_ack_and_fin() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    wait(&connection, |v| v.phase == Phase::Connected).await;
    let peer = tokio::spawn(async move {
        let Message::Close(Some(frame)) = ws.next().await.unwrap().unwrap() else {
            panic!("expected explicit close")
        };
        assert_eq!(frame.code, CloseCode::Normal);
        assert_eq!(frame.reason, "PTTStella disconnect");
        tokio::time::sleep(Duration::from_millis(120)).await;
        ws.flush().await.unwrap();
        ws.get_mut().shutdown().await.unwrap();
        let mut byte = [0u8; 1];
        assert_eq!(
            timeout(Duration::from_secs(2), ws.get_mut().read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    });
    let start = Instant::now();
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
    assert!(start.elapsed() >= Duration::from_millis(100));
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
    peer.await.unwrap();
}

#[tokio::test]
async fn shutdown_without_peer_ack_is_bounded_and_does_not_claim_confirmation() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    wait(&connection, |v| v.phase == Phase::Connected).await;
    let peer = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(Some(_))))));
        // Deliberately never flush the automatic close reply.
        let mut byte = [0u8; 1];
        assert_eq!(
            timeout(Duration::from_secs(4), ws.get_mut().read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    });
    let start = Instant::now();
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Unconfirmed
    );
    assert!(start.elapsed() < Duration::from_secs(4));
    peer.await.unwrap();
}

#[tokio::test]
async fn peer_transport_end_without_close_reply_is_not_a_timeout_or_ack() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    wait(&connection, |v| v.phase == Phase::Connected).await;
    let peer = tokio::spawn(async move {
        assert!(
            matches!(ws.next().await,Some(Ok(Message::Close(Some(frame)))) if frame.code==CloseCode::Normal)
        );
        // Peer ends the transport without flushing its queued Close response.
        drop(ws);
    });
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::PeerClosedWithoutReply
    );
    peer.await.unwrap();
}

fn credentials() -> Credentials {
    Credentials::new(
        "StellaTest".into(),
        zeroize::Zeroizing::new(b"synthetic+42".to_vec()),
    )
    .unwrap()
}

#[tokio::test]
async fn verification_is_manual_and_checks_wrong_code_then_recovery_without_resending_credentials()
{
    use crate::login::Guidance;
    let connection = Connection::start_verification_probe().unwrap();
    connection.connect().unwrap();
    let view = wait(&connection, |v| v.received > 0 && !v.output_pending).await;
    connection.login(view.stamp(), credentials()).await.unwrap();
    let view = wait(&connection, |v| {
        v.login_guidance == Guidance::CodeRequired && !v.output_pending
    })
    .await;
    assert!(view.login_pending);
    assert!(!view.keep_alive_ready);
    // A stale raw input cannot advance guidance or reach the peer.
    connection
        .input(view.generation + 1, b"000000\r".to_vec())
        .unwrap();
    wait(&connection, |v| v.message == crate::msg!("input-stale")).await;
    assert_eq!(
        connection.updates.borrow().login_guidance,
        Guidance::CodeRequired
    );
    connection
        .input(view.generation, b"123456\r".to_vec())
        .unwrap();
    let view = wait(&connection, |v| {
        v.login_guidance == Guidance::CodeRejected && !v.output_pending
    })
    .await;
    assert!(view.login_pending);
    connection
        .input(view.generation, b"ABCDEFGH\r".to_vec())
        .unwrap();
    let view = wait(&connection, |v| {
        v.login_guidance == Guidance::Browsing && !v.output_pending
    })
    .await;
    assert_eq!(view.context().kind(), "main_menu");
    assert!(!view.login_pending);
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
}

#[tokio::test]
async fn cancelling_verification_retires_guidance_and_reconnect_never_resumes_it() {
    use crate::login::Guidance;
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(probe::login_page().unwrap().into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| v.received > 0 && !v.output_pending).await;
    connection.login(view.stamp(), credentials()).await.unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"StellaTest\r"
    );
    ws.send(Message::Binary(probe::password_page().unwrap().into()))
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"synthetic+42\r"
    );
    ws.send(Message::Binary(
        probe::verification_page(false).unwrap().into(),
    ))
    .await
    .unwrap();
    wait(&connection, |v| v.login_guidance == Guidance::CodeRequired).await;
    connection.cancel_login().unwrap();
    wait(&connection, |v| {
        !v.login_pending && v.login_guidance == Guidance::Idle
    })
    .await;
    ws.send(Message::Binary(
        probe::verification_page(true).unwrap().into(),
    ))
    .await
    .unwrap();
    let view = wait(&connection, |v| {
        crate::login::verification(&v.frame) == Some(Guidance::CodeRejected) && !v.output_pending
    })
    .await;
    assert_eq!(view.login_guidance, Guidance::Idle);
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    ws.close(None).await.unwrap();
    wait(&connection, |v| v.phase == Phase::Disconnected).await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(
        probe::verification_page(false).unwrap().into(),
    ))
    .await
    .unwrap();
    let view = wait(&connection, |v| {
        v.generation > view.generation && v.received > 0 && !v.output_pending
    })
    .await;
    assert_eq!(view.login_guidance, Guidance::Idle);
    assert!(!view.login_pending);
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    let close = tokio::spawn(async move {
        let _ = ws.next().await;
        let _ = ws.flush().await;
    });
    connection.shutdown().await.unwrap();
    close.await.unwrap();
}

#[tokio::test]
async fn login_deadline_drops_password_and_late_prompt_sends_nothing() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(probe::login_page().unwrap().into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| v.received > 0).await;
    connection.login(view.stamp(), credentials()).await.unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"StellaTest\r"
    );
    let mut updates = connection.updates.clone();
    timeout(Duration::from_secs(18), async {
        loop {
            if updates.borrow_and_update().message.contains("登入等待逾時") {
                break;
            }
            updates.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    ws.send(Message::Binary(probe::password_page().unwrap().into()))
        .await
        .unwrap();
    assert!(timeout(Duration::from_millis(80), ws.next()).await.is_err());
    let close = tokio::spawn(async move {
        let _ = ws.next().await;
        let _ = ws.flush().await;
    });
    connection.shutdown().await.unwrap();
    close.await.unwrap();
}

#[tokio::test]
async fn cancelled_login_does_not_survive_reconnect_or_resume_at_a_password_prompt() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(probe::login_page().unwrap().into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| v.received > 0).await;
    connection.login(view.stamp(), credentials()).await.unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"StellaTest\r"
    );
    connection.cancel_login().unwrap();
    wait(&connection, |v| v.message.contains("已取消登入")).await;
    ws.send(Message::Binary(probe::password_page().unwrap().into()))
        .await
        .unwrap();
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    ws.close(None).await.unwrap();
    wait(&connection, |v| v.phase == Phase::Disconnected).await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(probe::password_page().unwrap().into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| {
        v.generation > view.generation && v.received > 0
    })
    .await;
    assert!(!view.login_pending);
    assert!(connection.login(view.stamp(), credentials()).await.is_err());
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    let close = tokio::spawn(async move {
        let _ = ws.next().await;
        let _ = ws.flush().await;
    });
    connection.shutdown().await.unwrap();
    close.await.unwrap();
}
#[tokio::test]
async fn explicit_login_waits_for_fragmented_prompt_and_never_repeats_password() {
    let connection = Connection::start_login_probe().unwrap();
    connection.connect().unwrap();
    let view = wait(&connection, |v| v.received > 0).await;
    connection.login(view.stamp(), credentials()).await.unwrap();
    wait(&connection, |v| {
        navigation::analyze(&v.frame).kind() == "main_menu"
    })
    .await;
    let stamp = connection.updates.borrow().stamp();
    assert!(connection.login(stamp, credentials()).await.is_err());
    connection.shutdown().await.unwrap();
}
#[tokio::test]
async fn manual_input_cancels_waiting_login_and_stale_request_sends_nothing() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(probe::login_page().unwrap().into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| v.received > 0).await;
    let stale = Stamp {
        revision: view.frame.revision + 1,
        ..view.stamp()
    };
    assert!(connection.login(stale, credentials()).await.is_err());
    assert!(timeout(Duration::from_millis(30), ws.next()).await.is_err());
    connection.login(view.stamp(), credentials()).await.unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"StellaTest\r"
    );
    connection.input(view.generation, b"x".to_vec()).unwrap();
    assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"x");
    ws.send(Message::Binary(probe::password_page().unwrap().into()))
        .await
        .unwrap();
    assert!(timeout(Duration::from_millis(80), ws.next()).await.is_err());
    let close = tokio::spawn(async move {
        let _ = ws.next().await;
        let _ = ws.flush().await;
    });
    connection.shutdown().await.unwrap();
    close.await.unwrap();
}

#[tokio::test]
async fn window_probe_checks_vim_translation_on_realistic_rows_and_owned_shutdown() {
    use crate::vim::{Effect, Key, Keyboard};
    let connection = Connection::start_window_probe().unwrap();
    connection.connect().unwrap();
    let mut view = wait(&connection, |v| v.received > 0).await;
    let mut keyboard = Keyboard::default();
    keyboard.toggle();
    let mut stages: Vec<(&str, &[&str])> = vec![
        ("main_menu", &["j", "k", "g", "g", "G", "l"]),
        ("board_list", &["j", "k", "g", "g", "G", "l"]),
        ("board_list", &["j", "k", "g", "g", "G", "l"]),
        ("article_list", &["j", "k", "g", "g", "G", "h"]),
    ];
    for _ in 0..5 {
        stages.extend([
            ("board_list", ["l"].as_slice()),
            ("article_list", ["h"].as_slice()),
        ]);
    }
    stages.extend([
        ("board_list", ["g", "g", "4", "j", "l"].as_slice()),
        ("article_list", ["h"].as_slice()),
    ]);
    stages.extend([
        ("continue_prompt", ["l"].as_slice()),
        ("continue_prompt", ["Enter"].as_slice()),
        ("continue_prompt", [" "].as_slice()),
        ("article_list", ["j", "k", "h"].as_slice()),
        ("command_help", ["j", "k", "g", "g", "G", "h"].as_slice()),
        ("empty_list", ["h"].as_slice()),
    ]);
    let mut operations = 0;
    for (profile, values) in stages {
        for &value in values {
            let context = navigation::analyze(&view.frame);
            assert_eq!(context.kind(), profile);
            keyboard.context(&context);
            if profile == "command_help" && operations == 40 {
                for intent in [
                    navigation::Intent::Forward,
                    navigation::Intent::Open { row: 3 },
                ] {
                    assert_eq!(
                        connection
                            .browse_key(view.stamp(), intent)
                            .await
                            .unwrap_err(),
                        navigation::Error::RequiresNativeInput.to_string()
                    );
                }
            }
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
                operations += 1;
                if !matches!(operations, 31 | 32) {
                    view = wait(&connection, |v| {
                        v.frame.revision > revision && !v.input_pending
                    })
                    .await;
                }
            } else {
                assert_eq!(effect, Effect::None);
            }
            assert_eq!(keyboard.status(&context).mode, "normal");
        }
    }
    assert_eq!(operations, 46);
    // Peer expects 46 translated operations including help and empty-list browsing, then
    // Close. Literal Vim letters/control codes or extra input fail the join.
    connection.shutdown().await.unwrap();
}

#[tokio::test]
async fn continue_prompt_rejects_duplicate_stale_and_wrong_context_requests() {
    for source in [navigation::Source::Keyboard, navigation::Source::Pointer] {
        let (listener, connection) = idle_local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        ws.send(Message::Binary(navigation_page("board_entry").into()))
            .await
            .unwrap();
        let view = wait(&connection, |v| v.received > 0).await;
        let stamp = view.stamp();
        connection
            .request_navigate(stamp, Intent::Continue, source)
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b" ");
        let pending = wait(&connection, |v| v.input_pending).await;
        assert_eq!(
            connection
                .request_navigate(stamp, Intent::Continue, source)
                .unwrap()
                .wait()
                .await
                .unwrap_err(),
            navigation::Error::Stale.to_string()
        );
        assert_eq!(
            connection
                .request_navigate(pending.stamp(), Intent::Continue, source)
                .unwrap()
                .wait()
                .await
                .unwrap_err(),
            navigation::Error::AwaitingScreen.to_string()
        );
        assert!(timeout(Duration::from_millis(30), ws.next()).await.is_err());
        ws.send(Message::Binary(navigation_page("list").into()))
            .await
            .unwrap();
        let list = wait(&connection, |v| {
            v.frame.revision > view.frame.revision && !v.input_pending
        })
        .await;
        assert_eq!(
            connection
                .request_navigate(stamp, Intent::Continue, source)
                .unwrap()
                .wait()
                .await
                .unwrap_err(),
            navigation::Error::Stale.to_string()
        );
        assert_eq!(
            connection
                .request_navigate(list.stamp(), Intent::Continue, source)
                .unwrap()
                .wait()
                .await
                .unwrap_err(),
            navigation::Error::Unavailable.to_string()
        );
        assert!(timeout(Duration::from_millis(30), ws.next()).await.is_err());
        connection
            .request_navigate(list.stamp(), Intent::Forward, source)
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            b"\r"
        );
        let peer = tokio::spawn(async move {
            assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
            ws.flush().await.unwrap();
        });
        connection.shutdown().await.unwrap();
        peer.await.unwrap();
    }
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

#[tokio::test]
async fn list_pointer_edges_send_exact_home_end_once_per_fresh_frame() {
    for (page, lottery) in [
        ("main_menu", false),
        ("boards_caption", false),
        ("list_edge_cases", false),
        ("list", true),
    ] {
        let (listener, connection) = idle_local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let mut frame = navigation_page(page);
        if lottery {
            frame.extend(crate::encoding::encode("\x1b[4;18H[彩券]       \x1b[5;1H").unwrap());
        }
        ws.send(Message::Binary(frame.clone().into()))
            .await
            .unwrap();
        let mut view = wait(&connection, |v| v.received > 0 && !v.output_pending).await;
        for (intent, expected) in [
            (Intent::First, b"\x1b[H".as_slice()),
            (Intent::Last, b"\x1b[F"),
        ] {
            connection.navigate(view.stamp(), intent).await.unwrap();
            assert_eq!(
                ws.next().await.unwrap().unwrap().into_data().as_ref(),
                expected
            );
            assert!(connection.navigate(view.stamp(), intent).await.is_err());
            assert!(timeout(Duration::from_millis(30), ws.next()).await.is_err());
            let received = view.received;
            ws.send(Message::Binary(frame.clone().into()))
                .await
                .unwrap();
            view = wait(&connection, |v| {
                v.received > received && !v.input_pending && !v.output_pending
            })
            .await;
        }
        let peer = tokio::spawn(async move {
            assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
            ws.flush().await.unwrap();
        });
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
        peer.await.unwrap();
    }
}

#[tokio::test]
async fn list_keyboard_moves_continue_without_redraw_but_transitions_and_mouse_wait() {
    for page in ["main_menu", "boards_caption", "list_edge_cases"] {
        let (listener, connection) = local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        ws.send(Message::Binary(navigation_page(page).into()))
            .await
            .unwrap();
        let view = wait(&connection, |v| v.received > 0).await;
        // PTT need not redraw when Home/End/PgDn is already at a boundary.
        // Consume every request but deliberately publish no replacement frame.
        for (intent, expected) in [
            (Intent::First, b"\x1b[H".as_slice()),
            (Intent::Last, b"\x1b[F"),
            (Intent::NextPage, b"\x1b[6~"),
            (
                Intent::Lines {
                    down: true,
                    count: 2,
                },
                b"\x1b[B\x1b[B",
            ),
        ] {
            connection.browse_key(view.stamp(), intent).await.unwrap();
            assert_eq!(
                ws.next().await.unwrap().unwrap().into_data().as_ref(),
                expected
            );
            assert!(
                connection
                    .navigate(view.stamp(), Intent::Forward)
                    .await
                    .is_err()
            );
        }
        connection
            .browse_key(view.stamp(), Intent::Forward)
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            b"\r"
        );
        assert!(
            connection
                .browse_key(view.stamp(), Intent::Back)
                .await
                .is_err()
        );
        assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
        let peer = tokio::spawn(async move {
            assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
            ws.flush().await.unwrap();
        });
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
        peer.await.unwrap();
    }
}

async fn local() -> (TcpListener, Connection) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("ws://{}/bbs", listener.local_addr().unwrap());
    let connection = Connection::start_at(Screen::new(80, 24).unwrap(), endpoint).unwrap();
    (listener, connection)
}

fn board_page(name: &str) -> Vec<u8> {
    probe::board_page(name).unwrap()
}

async fn finish_board_peer(
    connection: &Connection,
    mut ws: tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>,
) {
    let peer = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
        ws.flush().await.unwrap();
    });
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
    peer.await.unwrap();
}

#[tokio::test]
async fn board_switch_serializes_confirmed_steps_and_never_uses_a_synchronized_partial_prompt() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    let mut main = board_page("main");
    main.extend(crate::encoding::encode("\x1b[14;21H  \x1b[15;21H●\x1b[15;22H").unwrap());
    ws.send(Message::Binary(main.into())).await.unwrap();
    let initial = wait(&connection, |v| v.received > 0 && !v.output_pending).await;
    connection
        .request_switch_board(initial.stamp(), BoardName::parse("stella").unwrap())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\x1b[A\r"
    );
    let active = connection.updates.borrow().clone();
    assert_eq!(active.frame.revision, initial.frame.revision);
    assert_eq!(active.input_epoch, initial.input_epoch + 1);
    assert!(active.input_pending && !active.keep_alive_ready);
    assert!(matches!(
        active.destination,
        NavigationProgress::Active { .. }
    ));
    assert!(
        connection
            .request_switch_board(active.stamp(), BoardName::parse("Beta").unwrap())
            .unwrap()
            .wait()
            .await
            .is_err()
    );
    assert!(matches!(
        connection.updates.borrow().destination,
        NavigationProgress::Active { .. }
    ));
    // The menu may paint the selected Favorite before its queued Enter is
    // processed. Never send a second confirmation into the following screen.
    ws.send(Message::Binary(board_page("main").into()))
        .await
        .unwrap();
    wait(&connection, |v| {
        v.frame.revision > initial.frame.revision && !v.output_pending
    })
    .await;
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    ws.send(Message::Binary(board_page("boards").into()))
        .await
        .unwrap();
    assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"s");
    let prompt = board_page("prompt");
    let middle = prompt.len() / 2;
    ws.send(Message::Binary(
        [b"\x1b[?2026h".as_slice(), &prompt[..middle]]
            .concat()
            .into(),
    ))
    .await
    .unwrap();
    wait(&connection, |v| v.output_pending).await;
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    ws.send(Message::Ping(vec![7, 8].into())).await.unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap(),
        Message::Pong(vec![7, 8].into())
    );
    ws.send(Message::Binary(
        [&prompt[middle..], b"\x1b[?2026l"].concat().into(),
    ))
    .await
    .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"stella\r"
    );
    ws.send(Message::Binary(board_page("selected").into()))
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\x1b[C"
    );
    ws.send(Message::Binary(board_page("articles").into()))
        .await
        .unwrap();
    let reached=wait(&connection,|v|matches!(&v.destination,NavigationProgress::Reached{target,..} if target.board().as_str()=="Stella")).await;
    assert!(!reached.input_pending);
    connection
        .request_switch_board(reached.stamp(), BoardName::parse("STELLA").unwrap())
        .unwrap()
        .wait()
        .await
        .unwrap();
    wait(&connection, |v| {
        matches!(v.destination, NavigationProgress::Reached { serial: 2, .. })
    })
    .await;
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    finish_board_peer(&connection, ws).await;
}

#[tokio::test]
async fn board_switch_from_list_with_pinned_display_author_uses_only_confirmed_steps() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    let mut initial = board_page("other_articles");
    initial.extend(
        crate::encoding::encode(
            "\x1b[6;1H\x1b[0m\x1b[2K    ★    810/16 [測試作者]   □ 自製置底公告\x1b[5;1H",
        )
        .unwrap(),
    );
    ws.send(Message::Binary(initial.into())).await.unwrap();
    let view = wait(&connection, |v| v.received > 0 && !v.output_pending).await;
    assert_eq!(view.context().kind(), "article_list");
    connection
        .request_switch_board(view.stamp(), BoardName::parse("Stella").unwrap())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\x1b[D"
    );
    // No board name is sent before a verified board list and search prompt.
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    for (scene, expected) in [("empty_boards", b"s".as_slice()), ("prompt", b"Stella\r")] {
        ws.send(Message::Binary(board_page(scene).into()))
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            expected
        );
    }
    ws.send(Message::Binary(board_page("empty_articles").into()))
        .await
        .unwrap();
    wait(&connection, |v| matches!(&v.destination, NavigationProgress::Reached { target, .. } if target.board().as_str() == "Stella")).await;
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    finish_board_peer(&connection, ws).await;
}

#[tokio::test]
async fn board_switch_from_reading_pauses_keepalive_and_waits_for_each_back_before_direct_empty_arrival()
 {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(board_page("reading").into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| {
        v.context().kind() == "article_reading" && !v.output_pending
    })
    .await;
    connection
        .request_switch_board(view.stamp(), BoardName::parse("Stella").unwrap())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\x1b[D"
    );
    let active = connection.updates.borrow().clone();
    connection.keep_alive_gate(active.stamp(), true);
    assert!(
        timeout(Duration::from_millis(180), ws.next())
            .await
            .is_err()
    );
    assert!(connection.updates.borrow().keep_alive);
    assert_eq!(connection.updates.borrow().keep_alive_sent, 0);
    for (scene, expected) in [
        ("other_articles", b"\x1b[D".as_slice()),
        ("empty_boards", b"s"),
        ("prompt", b"Stella\r"),
    ] {
        ws.send(Message::Binary(board_page(scene).into()))
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            expected
        );
    }
    ws.send(Message::Binary(board_page("empty_articles").into()))
        .await
        .unwrap();
    wait(&connection,|v|matches!(&v.destination,NavigationProgress::Reached{target,..} if target.board().as_str()=="Stella")).await;
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    finish_board_peer(&connection, ws).await;
}

#[tokio::test]
async fn destination_cancel_is_owned_by_its_request_and_raw_input_retires_later_steps_across_reconnect()
 {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(board_page("boards").into()))
        .await
        .unwrap();
    let initial = wait(&connection, |v| v.received > 0 && !v.output_pending).await;
    connection
        .request_switch_board(initial.stamp(), BoardName::parse("Stella").unwrap())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"s");
    connection
        .input(initial.generation + 1, b"stale".to_vec())
        .unwrap();
    connection
        .cancel_navigation(Stamp {
            generation: initial.generation + 1,
            ..initial.stamp()
        })
        .unwrap();
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    assert!(matches!(
        connection.updates.borrow().destination,
        NavigationProgress::Active { .. }
    ));
    connection.cancel_navigation(initial.stamp()).unwrap();
    wait(&connection, |v| {
        matches!(v.destination, NavigationProgress::Idle)
    })
    .await;
    ws.send(Message::Binary(board_page("prompt").into()))
        .await
        .unwrap();
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    ws.send(Message::Binary(board_page("boards").into()))
        .await
        .unwrap();
    let next = wait(&connection, |v| {
        v.context().kind() == "board_list"
            && v.frame.revision > initial.frame.revision
            && !v.output_pending
    })
    .await;
    connection
        .request_switch_board(next.stamp(), BoardName::parse("Stella").unwrap())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"s");
    connection.cancel_navigation(initial.stamp()).unwrap(); // Cannot cancel this newer operation.
    ws.send(Message::Binary(board_page("prompt").into()))
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"Stella\r"
    );
    connection.input(next.generation, b"x".to_vec()).unwrap();
    assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"x");
    ws.send(Message::Binary(board_page("selected").into()))
        .await
        .unwrap();
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    assert!(matches!(
        connection.updates.borrow().destination,
        NavigationProgress::Idle
    ));
    ws.close(None).await.unwrap();
    wait(&connection, |v| v.phase == Phase::Disconnected).await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(board_page("prompt").into()))
        .await
        .unwrap();
    wait(&connection, |v| {
        v.generation > next.generation && v.received > 0 && !v.output_pending
    })
    .await;
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    assert!(matches!(
        connection.updates.borrow().destination,
        NavigationProgress::Idle
    ));
    finish_board_peer(&connection, ws).await;
}

#[tokio::test]
async fn board_switch_stops_at_entry_without_ever_pressing_continue() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(board_page("boards").into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| v.received > 0 && !v.output_pending).await;
    connection
        .request_switch_board(view.stamp(), BoardName::parse("Stella").unwrap())
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"s");
    ws.send(Message::Binary(board_page("prompt").into()))
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"Stella\r"
    );
    ws.send(Message::Binary(board_page("entry").into()))
        .await
        .unwrap();
    wait(&connection, |v| {
        v.message == crate::msg!("board-switch-manual")
    })
    .await;
    assert!(matches!(
        connection.updates.borrow().destination,
        NavigationProgress::Idle
    ));
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    finish_board_peer(&connection, ws).await;
}

#[tokio::test]
async fn board_switch_timeout_and_shutdown_do_not_resume_queued_steps() {
    for expire in [false, true] {
        let (listener, connection) = local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        ws.send(Message::Binary(board_page("boards").into()))
            .await
            .unwrap();
        let view = wait(&connection, |v| v.received > 0 && !v.output_pending).await;
        connection
            .request_switch_board(view.stamp(), BoardName::parse("Stella").unwrap())
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"s");
        if expire {
            let mut updates = connection.updates.clone();
            timeout(Duration::from_secs(7), async {
                loop {
                    if updates.borrow_and_update().message == crate::msg!("board-switch-timeout") {
                        break;
                    }
                    updates.changed().await.unwrap();
                }
            })
            .await
            .unwrap();
            ws.send(Message::Binary(board_page("prompt").into()))
                .await
                .unwrap();
            assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
        }
        finish_board_peer(&connection, ws).await;
    }
}

#[tokio::test]
async fn board_switch_rejects_stale_unknown_and_held_source_frames_before_any_write() {
    let (listener, connection) = local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(board_page("boards").into()))
        .await
        .unwrap();
    let view = wait(&connection, |v| v.received > 0 && !v.output_pending).await;
    assert!(
        connection
            .request_switch_board(
                Stamp {
                    input_epoch: view.input_epoch + 1,
                    ..view.stamp()
                },
                BoardName::parse("Stella").unwrap()
            )
            .unwrap()
            .wait()
            .await
            .is_err()
    );
    ws.send(Message::Binary(b"\x1b[?2026h\x1b[Hpartial".to_vec().into()))
        .await
        .unwrap();
    wait(&connection, |v| v.output_pending).await;
    assert!(
        connection
            .request_switch_board(view.stamp(), BoardName::parse("Stella").unwrap())
            .unwrap()
            .wait()
            .await
            .is_err()
    );
    ws.send(Message::Binary(
        [board_page("prompt"), b"\x1b[?2026l".to_vec()]
            .concat()
            .into(),
    ))
    .await
    .unwrap();
    let unknown = wait(&connection, |v| {
        v.context().kind() == "unknown" && !v.output_pending
    })
    .await;
    assert!(
        connection
            .request_switch_board(unknown.stamp(), BoardName::parse("Stella").unwrap())
            .unwrap()
            .wait()
            .await
            .is_err()
    );
    assert_eq!(connection.updates.borrow().input_epoch, 0);
    assert!(timeout(Duration::from_millis(40), ws.next()).await.is_err());
    finish_board_peer(&connection, ws).await;
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
    let mut revision = unknown.frame.revision;
    for name in ["main_menu", "boards", "favorites", "classes"] {
        ws.send(Message::Binary(navigation_page(name).into()))
            .await
            .unwrap();
        let view = wait(&connection, |v| v.frame.revision > revision).await;
        revision = view.frame.revision;
        assert_ne!(navigation::analyze(&view.frame).kind(), "unknown");
        connection.keep_alive_gate(view.stamp(), true);
        assert!(
            timeout(Duration::from_millis(160), ws.next())
                .await
                .is_err(),
            "keyboard-only profile must not admit idle refresh: {name}"
        );
        assert!(!connection.updates.borrow().keep_alive_ready);
    }
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
            source: navigation::Source::Pointer,
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
            assert!(matches!(
                timeout(Duration::from_secs(2), ws.next()).await.unwrap(),
                Some(Ok(Message::Close(_)))
            ));
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
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
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
#[path = "article_tests.rs"]
mod article_tests;
#[path = "mouse_tests.rs"]
mod mouse_tests;

#[tokio::test]
async fn telnet_gbk_switch_closes_websocket_before_new_input_and_disables_ptt_commands() {
    use crate::{encoding::TextEncoding, sites::Target};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    let original = wait(&connection, |view| {
        view.phase == Phase::Connected
            && matches!(view.context(), navigation::Context::ArticleList { .. })
    })
    .await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = Target::new(
        format!("telnet://{}", listener.local_addr().unwrap()),
        TextEncoding::Gbk,
    )
    .unwrap();
    connection.connect_to(target.clone()).unwrap();
    // The new dial waits for the owned Close handshake. No station logout keys.
    assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
    ws.flush().await.unwrap();
    let (mut tcp, _) = timeout(Duration::from_secs(4), listener.accept())
        .await
        .unwrap()
        .unwrap();
    tcp.write_all(b"\xff\xfd\x00\x1b[31m\xd6\x1b[32m\xd0\xce\xc4")
        .await
        .unwrap();
    let mut negotiation = [0; 3];
    tcp.read_exact(&mut negotiation).await.unwrap();
    assert_eq!(negotiation, [255, 251, 0]);
    let view = wait(&connection, |view| {
        view.phase == Phase::Connected && view.frame.glyphs.first().is_some_and(|g| g.text == "中")
    })
    .await;
    assert_eq!(view.site, target);
    assert!(view.generation > original.generation);
    assert!(!view.keep_alive_ready);
    assert!(matches!(
        view.context(),
        navigation::Context::Unknown {
            reason: navigation::UnknownReason::DifferentSite
        }
    ));
    assert_eq!(view.frame.glyphs[1].text, "文");
    assert!(
        connection
            .request_switch_board(
                view.stamp(),
                crate::board_navigation::BoardName::parse("Stella").unwrap()
            )
            .unwrap()
            .wait()
            .await
            .is_err()
    );
    assert!(
        connection
            .request_login(
                view.stamp(),
                crate::login::Credentials::new(
                    "Synthetic".into(),
                    zeroize::Zeroizing::new(b"unused".to_vec())
                )
                .unwrap()
            )
            .unwrap()
            .wait()
            .await
            .is_err()
    );
    connection
        .input(original.generation, b"STALE".to_vec())
        .unwrap();
    connection
        .input(
            view.generation,
            crate::input::encode_with("中文", TextEncoding::Gbk).unwrap(),
        )
        .unwrap();
    let mut input = [0; 4];
    tcp.read_exact(&mut input).await.unwrap();
    assert_eq!(input, [0xd6, 0xd0, 0xce, 0xc4]);
    assert!(
        timeout(Duration::from_millis(50), tcp.read_u8())
            .await
            .is_err()
    );
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::TransportClosed
    );
    assert_eq!(tcp.read(&mut [0; 1]).await.unwrap(), 0);
}

#[tokio::test]
async fn station_banner_probe_uses_the_selected_decoder_and_closes_without_input() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("telnet://{}", listener.local_addr().unwrap());
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream.write_all(&[255, 253, 0]).await.unwrap();
        let mut negotiation = [0; 3];
        stream.read_exact(&mut negotiation).await.unwrap();
        assert_eq!(negotiation, [255, 251, 0]);
        assert!(
            timeout(Duration::from_millis(100), stream.read_u8())
                .await
                .is_err(),
            "negotiation alone must not complete the banner probe"
        );
        stream.write_all(b"\xd6\xd0\xce\xc4").await.unwrap();
        assert_eq!(stream.read(&mut [0; 1]).await.unwrap(), 0);
    });
    let report = super::probe_site(
        crate::sites::Target::new(endpoint, crate::encoding::TextEncoding::Gbk).unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(report["status"], "passed");
    assert_eq!(report["encoding"], "gbk");
    assert_eq!(report["unmapped_glyphs"], 0);
    assert_eq!(report["received_bytes"], 7);
    assert_eq!(report["credential_input_sent"], false);
    assert_eq!(report["disconnect"], "transport_closed");
    peer.await.unwrap();
}

#[tokio::test]
async fn mail_and_archive_pointer_navigation_revalidates_exact_bytes_and_rejects_stale_frames() {
    for name in ["mail", "archive"] {
        let (listener, connection) = idle_local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let bytes = navigation_page(name);
        ws.send(Message::Binary(bytes.clone().into()))
            .await
            .unwrap();
        let view = wait(&connection, |v| {
            v.context().kind() == "document_list" && !v.input_pending
        })
        .await;
        connection
            .navigate(view.stamp(), navigation::Intent::Open { row: 3 })
            .await
            .unwrap();
        assert!(matches!(ws.next().await,Some(Ok(Message::Binary(b))) if b.as_ref()==b"\x1b[A\r"));
        ws.send(Message::Binary(bytes.into())).await.unwrap();
        let next = wait(&connection, |v| {
            v.frame.revision > view.frame.revision && !v.input_pending
        })
        .await;
        assert!(
            connection
                .navigate(view.stamp(), navigation::Intent::Back)
                .await
                .is_err()
        );
        connection
            .navigate(next.stamp(), navigation::Intent::NextPage)
            .await
            .unwrap();
        assert!(matches!(ws.next().await,Some(Ok(Message::Binary(b))) if b.as_ref()==b"\x1b[6~"));
        ws.send(Message::Binary(
            b"\x1b[2J\x1b[HConfirm? [Y/n]".to_vec().into(),
        ))
        .await
        .unwrap();
        let view = wait(&connection, |v| {
            v.context().kind() == "unknown" && !v.input_pending
        })
        .await;
        assert!(
            connection
                .navigate(view.stamp(), navigation::Intent::Open { row: 3 })
                .await
                .is_err()
        );
        let peer = tokio::spawn(async move {
            assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
            ws.flush().await.unwrap();
        });
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
        peer.await.unwrap();
    }
}

#[tokio::test]
async fn modern_board_and_indented_class_pointer_use_exact_revalidated_row_commands() {
    for (name, kind, row) in [
        ("favorites_modern", "board_list", 3),
        ("boards_modern", "board_list", 3),
        ("classes_indented", "board_list", 7),
        ("favorites_root_actions", "board_list", 3),
        ("classes_root_actions", "board_list", 7),
    ] {
        let (listener, connection) = idle_local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        ws.send(Message::Binary(navigation_page(name).into()))
            .await
            .unwrap();
        let view = wait(&connection, |v| {
            v.context().kind() == kind && !v.input_pending
        })
        .await;
        connection
            .navigate(view.stamp(), navigation::Intent::Open { row })
            .await
            .unwrap();
        assert!(matches!(ws.next().await,Some(Ok(Message::Binary(b))) if b.as_ref()==b"\x1b[A\r"));
        ws.send(Message::Binary(
            b"\x1b[2J\x1b[HConfirm? [Y/n]".to_vec().into(),
        ))
        .await
        .unwrap();
        let view = wait(&connection, |v| {
            v.context().kind() == "unknown" && !v.input_pending
        })
        .await;
        assert!(
            connection
                .navigate(view.stamp(), navigation::Intent::Open { row })
                .await
                .is_err()
        );
        let peer = tokio::spawn(async move {
            assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
            ws.flush().await.unwrap();
        });
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
        peer.await.unwrap();
    }
}

#[tokio::test]
async fn unanswered_pointer_list_boundary_refreshes_once_before_allowing_more_navigation() {
    use navigation::{Intent, Source};
    for (name, action, movement, opposite, reverse, source) in [
        (
            "list",
            Intent::NextPage,
            b"\x1b[6~".as_slice(),
            Intent::PreviousPage,
            b"\x1b[5~".as_slice(),
            Source::Pointer,
        ),
        (
            "list_dense",
            Intent::Last,
            b"\x1b[F",
            Intent::First,
            b"\x1b[H",
            Source::Button,
        ),
        (
            "favorites",
            Intent::PreviousPage,
            b"\x1b[5~",
            Intent::NextPage,
            b"\x1b[6~",
            Source::Pointer,
        ),
    ] {
        let (listener, connection) = idle_local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let page = navigation_page(name);
        ws.send(Message::Binary(page.clone().into())).await.unwrap();
        let original = wait(&connection, |v| v.received > 0 && !v.input_pending).await;
        connection
            .request_navigate(original.stamp(), action, source)
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            movement
        );
        let pending = wait(&connection, |v| v.input_epoch == 1 && v.input_pending).await;
        assert_eq!(
            connection
                .navigate(pending.stamp(), Intent::Open { row: 4 })
                .await
                .unwrap_err(),
            navigation::Error::AwaitingScreen.to_string()
        );
        assert!(
            timeout(Duration::from_millis(100), ws.next())
                .await
                .is_err(),
            "no immediate duplicate or refresh"
        );
        assert_eq!(
            timeout(Duration::from_secs(1), ws.next())
                .await
                .expect("boundary must request one redraw rather than wait five seconds")
                .unwrap()
                .unwrap()
                .into_data()
                .as_ref(),
            b"\x0c\x0c"
        );
        let refreshing = wait(&connection, |v| v.input_epoch == 2 && v.input_pending).await;
        assert_eq!(refreshing.frame.revision, original.frame.revision);
        assert!(
            connection
                .navigate(refreshing.stamp(), Intent::Back)
                .await
                .is_err(),
            "redraw must be received before another action"
        );
        ws.send(Message::Binary(page.clone().into())).await.unwrap();
        let ready = wait(&connection, |v| {
            v.frame.revision > original.frame.revision && !v.input_pending
        })
        .await;
        connection.navigate(ready.stamp(), opposite).await.unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            reverse
        );
        ws.send(Message::Binary(page.clone().into())).await.unwrap();
        let ready = wait(&connection, |v| {
            v.frame.revision > ready.frame.revision && !v.input_pending
        })
        .await;
        let row = match ready.context() {
            navigation::Context::ArticleList { cursor_row, .. }
            | navigation::Context::BoardList { cursor_row, .. } => cursor_row,
            context => panic!("expected list, got {context:?}"),
        };
        connection
            .navigate(ready.stamp(), Intent::Open { row })
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            b"\r"
        );
        ws.send(Message::Binary(navigation_page("reading_first").into()))
            .await
            .unwrap();
        let reading = wait(&connection, |v| {
            v.context().kind() == "article_reading" && !v.input_pending
        })
        .await;
        connection
            .navigate(reading.stamp(), Intent::Back)
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            b"\x1b[D"
        );
        ws.send(Message::Binary(page.into())).await.unwrap();
        wait(&connection, |v| {
            v.context().kind() == original.context().kind() && !v.input_pending
        })
        .await;
        assert!(
            timeout(Duration::from_millis(420), ws.next())
                .await
                .is_err(),
            "answered movement and later transition cancel the one-shot redraw"
        );
        finish_board_peer(&connection, ws).await;
    }
}

#[tokio::test]
async fn list_boundary_refresh_is_cancelled_by_answer_input_transition_and_disconnect() {
    use navigation::{Intent, Source};
    for mode in [
        "answer",
        "input",
        "prompt",
        "synchronized",
        "disconnect",
        "keyboard",
        "reading",
    ] {
        let (listener, connection) = idle_local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let initial = if mode == "reading" {
            "reading_first"
        } else {
            "list"
        };
        ws.send(Message::Binary(navigation_page(initial).into()))
            .await
            .unwrap();
        let original = wait(&connection, |v| v.received > 0 && !v.input_pending).await;
        connection
            .request_navigate(
                original.stamp(),
                Intent::NextPage,
                if mode == "keyboard" {
                    Source::Keyboard
                } else {
                    Source::Pointer
                },
            )
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            b"\x1b[6~"
        );
        match mode {
            "answer" => {
                ws.send(Message::Binary(navigation_page("list_dense").into()))
                    .await
                    .unwrap();
            }
            "input" => {
                connection
                    .input(original.generation, b"\x1b[A".to_vec())
                    .unwrap();
                assert_eq!(
                    ws.next().await.unwrap().unwrap().into_data().as_ref(),
                    b"\x1b[A"
                );
            }
            "prompt" => {
                ws.send(Message::Binary(
                    b"\x1b[2J\x1b[HOwn synthetic input prompt".to_vec().into(),
                ))
                .await
                .unwrap();
            }
            "synchronized" => {
                ws.send(Message::Binary(b"\x1b[?2026h\x1b[H".to_vec().into()))
                    .await
                    .unwrap();
            }
            "disconnect" => {
                connection.disconnect().unwrap();
                assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
                ws.flush().await.unwrap();
                connection.shutdown().await.unwrap();
                continue;
            }
            _ => {}
        }
        assert!(
            timeout(Duration::from_millis(650), ws.next())
                .await
                .is_err(),
            "no late refresh after {mode}"
        );
        finish_board_peer(&connection, ws).await;
    }
}

#[tokio::test]
async fn unanswered_list_refresh_never_repeats_or_replays_the_movement() {
    let (listener, connection) = idle_local().await;
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    ws.send(Message::Binary(navigation_page("list").into()))
        .await
        .unwrap();
    let initial = wait(&connection, |v| v.received > 0 && !v.input_pending).await;
    connection
        .navigate(initial.stamp(), navigation::Intent::NextPage)
        .await
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\x1b[6~"
    );
    assert_eq!(
        timeout(Duration::from_secs(1), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .into_data()
            .as_ref(),
        b"\x0c\x0c"
    );
    let pending = wait(&connection, |v| v.input_epoch == 2 && v.input_pending).await;
    assert!(
        connection
            .navigate(pending.stamp(), navigation::Intent::Back)
            .await
            .is_err()
    );
    assert!(
        timeout(Duration::from_millis(650), ws.next())
            .await
            .is_err()
    );
    let released = wait(&connection, |v| !v.input_pending).await;
    assert_eq!(released.frame.revision, initial.frame.revision);
    assert_eq!(released.input_epoch, 2);
    assert_eq!(released.message, crate::msg!("input-unanswered"));
    finish_board_peer(&connection, ws).await;
}

#[tokio::test]
async fn adjacent_article_positioning_is_not_published_as_a_new_article() {
    use navigation::{Context, Intent, Source};
    for down in [false, true] {
        let (listener, connection) = idle_local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let page = |first, percent| {
            crate::encoding::encode(&format!("\x1b[2J\x1b[HOriginal synthetic article\x1b[24;1H\x1b[30;47m\x1b[2K瀏覽 第 2/4 頁 ({percent}%) 目前顯示: 第 {first}~{} 行 (y)回應(X)推文(h)說明",first+22)).unwrap()
        };
        ws.send(Message::Binary(page(23, 50).into())).await.unwrap();
        let initial = wait(&connection, |v| v.received > 0 && !v.input_pending).await;
        connection
            .request_navigate(
                initial.stamp(),
                Intent::AdjacentArticle { down },
                Source::Keyboard,
            )
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            if down { b"\x1b[F" } else { b"\x1b[H" }
        );
        // No arrow before a complete, confirmed boundary response.
        assert!(
            tokio::time::timeout(Duration::from_millis(80), ws.next())
                .await
                .is_err()
        );
        let target = if down { 45 } else { 1 };
        ws.send(Message::Binary(
            page(target, if down { 100 } else { 25 }).into(),
        ))
        .await
        .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            if down { b"\x1b[B" } else { b"\x1b[A" }
        );
        let between = wait(&connection, |v| v.input_epoch == 2).await;
        assert!(between.input_pending);
        ws.send(Message::Binary(page(1, 25).into())).await.unwrap();
        let arrived = wait(&connection, |v| {
            !v.input_pending && v.frame.revision > between.frame.revision
        })
        .await;
        assert!(matches!(
            arrived.context(),
            Context::ArticleReading { first_line: 1, .. }
        ));
        let close = tokio::spawn(async move {
            let message = ws.next().await.unwrap().unwrap();
            assert!(matches!(message, Message::Close(_)));
            ws.flush().await.unwrap();
        });
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
        close.await.unwrap();
    }
}

#[tokio::test]
async fn adjacent_positioning_never_sends_a_late_arrow_after_interruption_or_timeout() {
    use navigation::{Intent, Source};
    for reason in ["raw", "timeout", "synchronized", "disconnect"] {
        let (listener, connection) = idle_local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let page = |first| {
            crate::encoding::encode(&format!("\x1b[2J\x1b[HOriginal synthetic article\x1b[24;1H\x1b[30;47m\x1b[2K瀏覽 第 2/4 頁 (50%) 目前顯示: 第 {first}~{} 行 (y)回應(X)推文(h)說明\x1b[0m\x1b[24;80H",first+22)).unwrap()
        };
        ws.send(Message::Binary(page(23).into())).await.unwrap();
        let initial = wait(&connection, |v| v.received > 0 && !v.input_pending).await;
        connection
            .request_navigate(
                initial.stamp(),
                Intent::AdjacentArticle { down: false },
                Source::Keyboard,
            )
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(
            ws.next().await.unwrap().unwrap().into_data().as_ref(),
            b"\x1b[H"
        );
        if reason == "raw" {
            connection
                .input(initial.generation, b"\x1b[D".to_vec())
                .unwrap();
            assert_eq!(
                ws.next().await.unwrap().unwrap().into_data().as_ref(),
                b"\x1b[D"
            );
            ws.send(Message::Binary(page(1).into())).await.unwrap();
        } else if reason == "synchronized" {
            ws.send(Message::Binary(
                [b"\x1b[?2026h".as_slice(), &page(1)].concat().into(),
            ))
            .await
            .unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(80), ws.next())
                    .await
                    .is_err()
            );
            ws.send(Message::Binary(b"\x1b[?2026l".to_vec().into()))
                .await
                .unwrap();
            assert_eq!(
                ws.next().await.unwrap().unwrap().into_data().as_ref(),
                b"\x1b[A"
            );
            ws.send(Message::Binary(page(1).into())).await.unwrap();
        } else if reason == "timeout" {
            // A complete but wrong range must not advance the staged arrow.
            ws.send(Message::Binary(page(23).into())).await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(600), ws.next())
                    .await
                    .is_err()
            );
            ws.send(Message::Binary(page(1).into())).await.unwrap();
        } else {
            ws.send(Message::Close(None)).await.unwrap();
            assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
            wait(&connection, |v| v.phase == Phase::Disconnected).await;
            assert_eq!(
                connection.shutdown().await.unwrap(),
                CloseOutcome::Acknowledged
            );
            continue;
        }
        wait(&connection, |v| !v.input_pending).await;
        assert!(
            tokio::time::timeout(Duration::from_millis(80), ws.next())
                .await
                .is_err()
        );
        let close = tokio::spawn(async move {
            assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
            ws.flush().await.unwrap();
        });
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
        close.await.unwrap();
    }
}

#[tokio::test]
async fn article_refresh_cancellation_and_manual_input_revoke_late_query_steps() {
    for manual in [false, true] {
        let (listener, connection) = local().await;
        connection.connect().unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/article-open.json")).unwrap();
        let wire = crate::encoding::encode(
            &fixture["screens"]["reading"]
                .as_str()
                .unwrap()
                .replace("1/3 頁 (33%)", "1/1 頁 (100%)"),
        )
        .unwrap();
        ws.send(Message::Binary(wire.into())).await.unwrap();
        let view = wait(&connection, |v| {
            matches!(
                v.context(),
                crate::navigation::Context::ArticleReading { percent: 100, .. }
            ) && !v.output_pending
        })
        .await;
        let rows = crate::text::rows(&view.frame).unwrap();
        let head = std::array::from_fn(|i| rows[i].text.trim_end().to_owned());
        connection
            .request_go_to(
                view.stamp(),
                crate::navigation_job::Destination::Refresh {
                    board: BoardName::parse("Stella").unwrap(),
                    head,
                },
            )
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"Q");
        if manual {
            connection.input(view.generation, b"x".to_vec()).unwrap();
            assert_eq!(ws.next().await.unwrap().unwrap().into_data().as_ref(), b"x");
        } else {
            connection.cancel_navigation(view.stamp()).unwrap();
        }
        wait(&connection, |v| {
            matches!(v.destination, NavigationProgress::Idle)
        })
        .await;
        let query = format!(
            "\x1b[2J\x1b[19;1H┌{}┐\x1b[20;1H│ 文章代碼(AID): \x1b[1m#00000101\x1b[0m (Stella) [ptt.cc] local\x1b[20;77H│\x1b[21;1H│ 文章網址: local\x1b[21;77H│\x1b[22;1H│ 這一篇文章值 0 Ptt幣\x1b[22;77H│\x1b[23;1H└──────────────────────── [再按 Q 切換價格列表]  ─┘\x1b[24;1H\x1b[37;44m 請按任意鍵繼續 \x1b[0m\x1b[24;80H",
            "─".repeat(37)
        );
        ws.send(Message::Binary(
            crate::encoding::encode(&query).unwrap().into(),
        ))
        .await
        .unwrap();
        assert!(
            timeout(Duration::from_millis(120), ws.next())
                .await
                .is_err()
        );
        finish_board_peer(&connection, ws).await;
    }
}
