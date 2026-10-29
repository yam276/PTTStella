use super::*;
use crate::{
    editor_color::{Color, fixture::Editor},
    selection::{Position, Selection},
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

async fn local() -> (TcpListener, Connection) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let connection = Connection::start_at(
        Screen::new(80, 24).unwrap(),
        format!("ws://{}/bbs", listener.local_addr().unwrap()),
    )
    .unwrap();
    connection.keep_alive(false).unwrap();
    connection.connect().unwrap();
    (listener, connection)
}
async fn ready(connection: &Connection) -> View {
    let mut updates = connection.updates.clone();
    timeout(Duration::from_secs(5), async {
        loop {
            let v = updates.borrow_and_update().clone();
            if v.phase == Phase::Connected
                && !v.output_pending
                && crate::editor_color::status(&v.frame).is_some()
            {
                return v;
            }
            updates.changed().await.unwrap();
        }
    })
    .await
    .unwrap()
}
fn range() -> Selection {
    Selection::Range {
        anchor: Position { row: 0, col: 1 },
        focus: Position { row: 2, col: 5 },
    }
}
const RED: Color = Color {
    foreground: 9,
    background: 0,
};
#[tokio::test]
async fn editor_colour_transport_preserves_multiline_chinese_and_existing_sgr() {
    let (listener, connection) = local().await;
    let peer = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
        let mut editor = Editor::new(
            &["P\x1b[32m中\x1b[0m文\x1b[34mQ", "", "第三行"],
            true,
            false,
            true,
        );
        ws.send(Message::Binary(editor.wire().into()))
            .await
            .unwrap();
        for _ in 0..1024 {
            match timeout(Duration::from_secs(10), ws.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap()
            {
                Message::Binary(bytes) => {
                    editor.accept(&bytes);
                    let wire = editor.wire();
                    let split = wire.len() / 2;
                    ws.send(Message::Binary(wire[..split].to_vec().into()))
                        .await
                        .unwrap();
                    tokio::time::sleep(Duration::from_millis(2)).await;
                    ws.send(Message::Binary(wire[split..].to_vec().into()))
                        .await
                        .unwrap();
                }
                Message::Close(_) => {
                    ws.flush().await.unwrap();
                    let expected = [
                        "P\x1b[32m\x1b[0;1;31;40m中\x1b[0m\x1b[0;1;31;40m文\x1b[34m\x1b[0;1;31;40mQ\x1b[m",
                        "",
                        "\x1b[0;1;31;40m第三行\x1b[m",
                    ];
                    assert_eq!(
                        editor.lines,
                        expected
                            .iter()
                            .map(|s| crate::encoding::encode(s).unwrap())
                            .collect::<Vec<_>>()
                    );
                    assert!(editor.preview);
                    assert!(!editor.insert);
                    return editor.commands.len();
                }
                _ => panic!("unexpected message"),
            }
        }
        panic!("input bound exceeded")
    });
    let view = ready(&connection).await;
    connection
        .request_editor_color(view.stamp(), range(), RED)
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(
        connection.shutdown().await.unwrap(),
        CloseOutcome::Acknowledged
    );
    let count = peer.await.unwrap();
    assert!(count > 5 && count < 200);
}
#[tokio::test]
async fn manual_key_cancels_colour_wait_without_replaying_a_step() {
    let (listener, connection) = local().await;
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    let editor = Editor::new(&["test"], true, true, true);
    ws.send(Message::Binary(editor.wire().into()))
        .await
        .unwrap();
    let view = ready(&connection).await;
    let reply = connection
        .request_editor_color(view.stamp(), Selection::Line { row: 0 }, RED)
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\x1bu"
    );
    connection.input(view.generation, vec![1]).unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\x01"
    );
    assert!(reply.wait().await.is_err());
    assert!(
        timeout(Duration::from_millis(100), ws.next())
            .await
            .is_err()
    );
    let peer = tokio::spawn(async move {
        assert!(matches!(ws.next().await, Some(Ok(Message::Close(_)))));
        ws.flush().await.unwrap();
    });
    connection.shutdown().await.unwrap();
    peer.await.unwrap();
}
#[tokio::test]
async fn expired_or_dropped_selection_never_writes_and_disconnect_revokes_the_job() {
    let (listener, connection) = local().await;
    let (stream, _) = listener.accept().await.unwrap();
    let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
    let editor = Editor::new(&["test"], true, true, true);
    ws.send(Message::Binary(editor.wire().into()))
        .await
        .unwrap();
    let view = ready(&connection).await;
    let mut stale = view.stamp();
    stale.revision += 1;
    assert!(
        connection
            .request_editor_color(stale, Selection::Line { row: 0 }, RED)
            .unwrap()
            .wait()
            .await
            .is_err()
    );
    drop(
        connection
            .request_editor_color(view.stamp(), Selection::Line { row: 0 }, RED)
            .unwrap(),
    );
    assert!(
        timeout(Duration::from_millis(100), ws.next())
            .await
            .is_err()
    );
    let reply = connection
        .request_editor_color(view.stamp(), Selection::Line { row: 0 }, RED)
        .unwrap();
    assert_eq!(
        ws.next().await.unwrap().unwrap().into_data().as_ref(),
        b"\x1bu"
    );
    ws.close(None).await.unwrap();
    assert!(reply.wait().await.is_err());
    connection.connect().unwrap();
    let (stream, _) = listener.accept().await.unwrap();
    let mut second = tokio_tungstenite::accept_async(stream).await.unwrap();
    second
        .send(Message::Binary(editor.wire().into()))
        .await
        .unwrap();
    let mut updates = connection.updates.clone();
    let new = timeout(Duration::from_secs(5), async {
        loop {
            let v = updates.borrow_and_update().clone();
            if v.generation > view.generation
                && v.phase == Phase::Connected
                && !v.output_pending
                && crate::editor_color::status(&v.frame).is_some()
            {
                break v;
            }
            updates.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(new.generation > view.generation);
    assert!(
        timeout(Duration::from_millis(100), second.next())
            .await
            .is_err()
    );
    let peer = tokio::spawn(async move {
        assert!(matches!(second.next().await, Some(Ok(Message::Close(_)))));
        second.flush().await.unwrap();
    });
    connection.shutdown().await.unwrap();
    peer.await.unwrap();
}
