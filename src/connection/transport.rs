//! WSS lifecycle and bounded writes. No screen or browsing policy.
use super::{CloseOutcome, Dial, WebSocket};
use futures_util::{SinkExt, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio::time::timeout;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_tungstenite::{
    Connector, connect_async_tls_with_config,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
    },
};

pub(super) enum Socket {
    WebSocket(Box<WebSocket>),
    Telnet(TcpStream),
}
impl Socket {
    pub(super) async fn next(
        &mut self,
    ) -> Option<Result<Message, tokio_tungstenite::tungstenite::Error>> {
        match self {
            Self::WebSocket(socket) => socket.next().await,
            Self::Telnet(socket) => {
                let mut bytes = vec![0; 8192];
                match socket.read(&mut bytes).await {
                    Ok(0) => None,
                    Ok(length) => {
                        bytes.truncate(length);
                        Some(Ok(Message::Binary(bytes.into())))
                    }
                    Err(error) => Some(Err(error.into())),
                }
            }
        }
    }
    pub(super) async fn send(
        &mut self,
        message: Message,
    ) -> Result<(), tokio_tungstenite::tungstenite::Error> {
        match self {
            Self::WebSocket(socket) => socket.send(message).await,
            Self::Telnet(socket) => match message {
                Message::Binary(bytes) => socket.write_all(&bytes).await.map_err(Into::into),
                _ => Ok(()),
            },
        }
    }
    pub(super) async fn flush(&mut self) -> Result<(), tokio_tungstenite::tungstenite::Error> {
        match self {
            Self::WebSocket(socket) => socket.flush().await,
            Self::Telnet(socket) => socket.flush().await.map_err(Into::into),
        }
    }
}
pub(super) async fn dial(endpoint: String) -> Result<Socket, String> {
    let endpoint = crate::sites::Endpoint::try_from(endpoint)?;
    let url = url::Url::parse(endpoint.as_str()).map_err(|e| e.to_string())?;
    if url.scheme() == "telnet" {
        let host = url
            .host_str()
            .ok_or_else(|| crate::msg!("site-address-invalid"))?
            .trim_matches(['[', ']']);
        return timeout(
            Duration::from_secs(15),
            TcpStream::connect((host, url.port().unwrap_or(23))),
        )
        .await
        .map_err(|_| crate::msg!("connection-timeout"))?
        .map(Socket::Telnet)
        .map_err(|e| crate::msg!("connection-failed", e = e.to_string()));
    }
    let origin = endpoint.origin()?;
    let endpoint = endpoint.as_str();
    let mut request = endpoint.into_client_request().map_err(|e| e.to_string())?;
    let connector = if request.uri().scheme_str() == Some("wss") {
        Some(tls_connector()?)
    } else {
        None
    };
    request.headers_mut().insert(
        "Origin",
        origin.parse().map_err(|_| "invalid fixed origin")?,
    );
    let config = WebSocketConfig::default()
        .max_message_size(Some(64 * 1024))
        .max_frame_size(Some(64 * 1024));
    timeout(
        Duration::from_secs(15),
        connect_async_tls_with_config(request, Some(config), false, connector),
    )
    .await
    .map_err(|_| crate::msg!("connection-timeout").to_owned())?
    .map(|(socket, _)| Socket::WebSocket(Box::new(socket)))
    .map_err(|e| crate::msg!("connection-failed", e = e.to_string()))
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
    if let Some(connection) = socket.take() {
        let mut connection = match connection {
            Socket::Telnet(mut stream) => {
                return match timeout(Duration::from_secs(2), stream.shutdown()).await {
                    Ok(Ok(())) => CloseOutcome::TransportClosed,
                    _ => CloseOutcome::Unconfirmed,
                };
            }
            Socket::WebSocket(connection) => connection,
        };
        let close = async {
            connection
                .as_mut()
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
        .map_err(|_| crate::msg!("input-send-timeout").to_owned())?
        .map_err(|e| crate::msg!("input-send-failed", e = e.to_string()))
}
