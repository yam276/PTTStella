//! WSS lifecycle and bounded writes. No screen or browsing policy.
use super::{CloseOutcome, Dial, Socket};
use futures_util::{SinkExt, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;
use tokio_tungstenite::{
    Connector, connect_async_tls_with_config,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
    },
};

pub(super) async fn dial(endpoint: String) -> Result<Socket, String> {
    let mut request = endpoint.into_client_request().map_err(|e| e.to_string())?;
    let connector = if request.uri().scheme_str() == Some("wss") {
        Some(tls_connector()?)
    } else {
        None
    };
    request.headers_mut().insert(
        "Origin",
        "https://term.ptt.cc"
            .parse()
            .map_err(|_| "invalid fixed origin")?,
    );
    let config = WebSocketConfig::default()
        .max_message_size(Some(64 * 1024))
        .max_frame_size(Some(64 * 1024));
    timeout(
        Duration::from_secs(15),
        connect_async_tls_with_config(request, Some(config), false, connector),
    )
    .await
    .map_err(|_| "連線逾時".to_owned())?
    .map(|(socket, _)| socket)
    .map_err(|e| format!("無法連線: {e}"))
}

pub(super) fn tls_connector() -> Result<Connector, String> {
    Ok(Connector::Rustls(Arc::new(crate::tls::client_config()?)))
}

pub(super) async fn stop(socket: &mut Option<Socket>, dialing: &mut Option<Dial>) -> CloseOutcome {
    if let Some(task) = dialing.take() {
        task.abort();
        if let Ok(Ok(connected)) = task.await {
            // Dial and connected socket never coexist in this owner.
            *socket = Some(connected);
        }
    }
    if let Some(mut connection) = socket.take() {
        let close = async {
            connection
                .close(Some(CloseFrame {
                    code: CloseCode::Normal,
                    reason: "PTTStella disconnect".into(),
                }))
                .await
                .map_err(|_| CloseOutcome::Unconfirmed)?;
            let mut acknowledged = false;
            // close() only queues/flushes Close. Keep reading until the server
            // answers and closes its transport; never send BBS keys while closing.
            loop {
                match connection.next().await {
                    Some(Ok(Message::Close(_))) => acknowledged = true,
                    Some(Ok(_)) => {}
                    None => {
                        return Ok(if acknowledged {
                            CloseOutcome::Acknowledged
                        } else {
                            CloseOutcome::PeerClosedWithoutReply
                        });
                    }
                    Some(Err(error)) => {
                        use tokio_tungstenite::tungstenite::{Error, error::ProtocolError};
                        let ended = matches!(
                            &error,
                            Error::Protocol(ProtocolError::ResetWithoutClosingHandshake)
                        ) || matches!(&error,Error::Io(e) if matches!(e.kind(),std::io::ErrorKind::UnexpectedEof|std::io::ErrorKind::ConnectionReset|std::io::ErrorKind::ConnectionAborted));
                        return if ended {
                            Ok(if acknowledged {
                                CloseOutcome::Acknowledged
                            } else {
                                CloseOutcome::PeerClosedWithoutReply
                            })
                        } else {
                            Err(CloseOutcome::Unconfirmed)
                        };
                    }
                }
            }
        };
        return match timeout(Duration::from_secs(2), close).await {
            Ok(Ok(outcome)) => outcome,
            _ => CloseOutcome::Unconfirmed,
        };
    }
    CloseOutcome::NoConnection
}

pub(super) async fn send(socket: &mut Socket, message: Message) -> Result<(), String> {
    timeout(Duration::from_secs(3), socket.send(message))
        .await
        .map_err(|_| "送出逾時；站端結果未知，未自動重送".to_owned())?
        .map_err(|e| format!("送出失敗，未自動重送: {e}"))
}
