//! Synthetic round trip through the production connection owner.
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::TcpListener,
    time::{Duration, timeout},
};
use tokio_tungstenite::tungstenite::{
    Message,
    protocol::{CloseFrame, frame::coding::CloseCode},
};

pub(super) async fn serve(
    listener: TcpListener,
    bytes: Vec<u8>,
    reload_settings: bool,
) -> Result<(), String> {
    timeout(Duration::from_secs(60), async {
        let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let mut socket = tokio_tungstenite::accept_async(stream)
            .await
            .map_err(|e| e.to_string())?;
        socket
            .send(Message::Binary(bytes.into()))
            .await
            .map_err(|e| e.to_string())?;
        if reload_settings {
            return match socket.next().await {
                Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
                _ => Err("settings reload probe received unexpected input".into()),
            };
        }
        let expected: &[&[u8]] = &[
            b"\x03",
            b"a",
            b"b",
            b"c",
            b"\xa4\xa4",
            b"\xa4\xa4",
            b"\xa4\xa4",
            b"A\rB",
            b"\x1b[A",
            b"\r",
            b"n",
            b"\r",
        ];
        for (index, expected) in expected.iter().enumerate() {
            match socket.next().await {
                Some(Ok(Message::Binary(bytes))) if bytes.as_ref() == *expected => {}
                _ => {
                    let reason = format!("unexpected loopback input at step {index}");
                    let _ = socket
                        .close(Some(CloseFrame {
                            code: CloseCode::Protocol,
                            reason: reason.clone().into(),
                        }))
                        .await;
                    return Err(reason);
                }
            }
            if index == 9 {
                socket
                    .send(Message::Binary(
                        b"\x1b[2J\x1b[22;1HConfirm synthetic action [y/N] n"
                            .to_vec()
                            .into(),
                    ))
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        navigation(&mut socket).await?;
        socket
            .send(Message::Binary(
                b"\x1b[2J\x1b[1;1HINPUT ROUND TRIP COMPLETE".to_vec().into(),
            ))
            .await
            .map_err(|e| e.to_string())?;
        socket
            .close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: "synthetic confirmation complete".into(),
            }))
            .await
            .map_err(|e| e.to_string())?;
        match socket.next().await {
            Some(Ok(Message::Close(_))) => Ok(()),
            _ => Err("loopback probe did not close cleanly".into()),
        }
    })
    .await
    .map_err(|_| "loopback probe timed out".to_owned())?
}

fn navigation_page(name: &str) -> Result<Vec<u8>, String> {
    let pages: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/navigation.json"))
            .map_err(|e| e.to_string())?;
    crate::encoding::encode(
        pages[name]
            .as_str()
            .ok_or("missing synthetic navigation page")?,
    )
    .map_err(|e| e.to_string())
}
async fn navigation<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    socket
        .send(Message::Binary(navigation_page("list")?.into()))
        .await
        .map_err(|e| e.to_string())?;
    let mut shifted = navigation_page("list")?;
    shifted.extend_from_slice(b"\x1b[5;1H \x1b[4;1H>\x1b[4;1H");
    let mut next_item = navigation_page("list")?;
    next_item.extend_from_slice(b"\x1b[5;1H \x1b[6;1H>\x1b[6;1H");
    let stages: Vec<(&[u8], Vec<u8>)> = vec![
        (b"\x1b[C", shifted),
        (b"\x1b[B\x1b[B\r", navigation_page("reading_first")?),
        (b"\x1b[6~", navigation_page("reading_middle")?),
        (b"\x1b[5~", navigation_page("reading_first")?),
        (b"\x1b[6~", navigation_page("reading_middle")?),
        (b"\x1b[6~", navigation_page("reading_last")?),
        (b"\x1b[D", navigation_page("list")?),
        (b"\x1b[B", next_item),
        (b"\x1b[A", navigation_page("list")?),
        (
            b"\x1b[C",
            b"\x1b[2J\x1b[22;1HConfirm synthetic action [y/N]".to_vec(),
        ),
    ];
    for (index, (expected, page)) in stages.into_iter().enumerate() {
        if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected)
        {
            return Err(format!("unexpected navigation probe input at step {index}"));
        }
        socket
            .send(Message::Binary(page.into()))
            .await
            .map_err(|e| e.to_string())?;
    }
    if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"\x1b[C")
    {
        return Err("unknown screen received unintended navigation input".into());
    }
    Ok(())
}
