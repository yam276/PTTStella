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

pub(super) async fn serve(listener: TcpListener, bytes: Vec<u8>) -> Result<(), String> {
    timeout(Duration::from_secs(35), async {
        let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let mut socket = tokio_tungstenite::accept_async(stream)
            .await
            .map_err(|e| e.to_string())?;
        socket
            .send(Message::Binary(bytes.into()))
            .await
            .map_err(|e| e.to_string())?;
        let expected: &[&[u8]] = &[b"a", b"b", b"c", b"\xa4\xa4", b"\x1b[A", b"\r", b"n", b"\r"];
        for (index, expected) in expected.iter().enumerate() {
            match socket.next().await {
                Some(Ok(Message::Binary(bytes))) if bytes.as_ref() == *expected => {}
                _ => return Err(format!("unexpected loopback input at step {index}")),
            }
            if index == 5 {
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
