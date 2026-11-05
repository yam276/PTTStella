//! PTT's authenticated SSH transport. The SSH service account is public and
//! separate from the BBS credentials sent by the connection owner.
use russh::{
    Channel, ChannelMsg, Disconnect, Preferred, client,
    keys::{Algorithm, HashAlg, PublicKeyOrCertificate},
};
use std::{borrow::Cow, net::Shutdown, sync::Arc, time::Duration};
use tokio::{
    net::{TcpStream, ToSocketAddrs},
    time::timeout,
};
use tokio_tungstenite::tungstenite::{Error, Message};

use super::CloseOutcome;

// Published by PTT in its 2024 RSA key rotation announcement.
const PTT_FINGERPRINT: &str = "SHA256:9/gkjxWjuteMj//K0sRo1gj7h5qTp0HPIH3n7+e3Jqc";

struct HostKey(String);
impl client::Handler for HostKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(matches!(key, PublicKeyOrCertificate::PublicKey { key, .. }
            if key.fingerprint(HashAlg::Sha256).to_string() == self.0))
    }
}

// russh starts its I/O task during key exchange. Keep a shutdown handle so
// aborting a dial or dropping a broken session also interrupts that task's I/O.
struct TransportGuard(Option<std::net::TcpStream>);
impl Drop for TransportGuard {
    fn drop(&mut self) {
        if let Some(stream) = &self.0 {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

pub(super) struct Ssh {
    channel: Channel<client::Msg>,
    session: client::Handle<HostKey>,
    _transport: TransportGuard,
}

pub(super) async fn connect() -> Result<Ssh, String> {
    const KEY_ALGORITHMS: &[Algorithm] = &[
        Algorithm::Rsa {
            hash: Some(HashAlg::Sha512),
        },
        Algorithm::Rsa {
            hash: Some(HashAlg::Sha256),
        },
    ];
    open(("ptt.cc", 22), PTT_FINGERPRINT, KEY_ALGORITHMS).await
}

async fn open(
    address: impl ToSocketAddrs,
    fingerprint: &str,
    key_algorithms: &'static [Algorithm],
) -> Result<Ssh, String> {
    timeout(Duration::from_secs(15), async {
        let tcp = TcpStream::connect(address).await?.into_std()?;
        tcp.set_nodelay(true)?;
        let transport = TransportGuard(Some(tcp.try_clone()?));
        let config = client::Config {
            preferred: Preferred {
                key: Cow::Borrowed(key_algorithms),
                ..Preferred::default()
            },
            window_size: 256 * 1024,
            channel_buffer_size: 16,
            ..client::Config::default()
        };
        let mut session = client::connect_stream(
            Arc::new(config),
            TcpStream::from_std(tcp)?,
            HostKey(fingerprint.into()),
        )
        .await?;
        if !session.authenticate_none("bbs").await?.success() {
            return Err(russh::Error::NotAuthenticated);
        }
        let mut channel = session.channel_open_session().await?;
        channel
            .request_pty(true, "xterm", 80, 24, 0, 0, &[])
            .await?;
        request_accepted(&mut channel).await?;
        channel.request_shell(true).await?;
        request_accepted(&mut channel).await?;
        Ok(Ssh {
            channel,
            session,
            _transport: transport,
        })
    })
    .await
    .map_err(|_| crate::msg!("connection-timeout"))?
    .map_err(|error| match error {
        russh::Error::UnknownKey => crate::msg!("site-ssh-host-key"),
        _ => crate::msg!("connection-failed", e = error.to_string()),
    })
}

async fn request_accepted(channel: &mut Channel<client::Msg>) -> Result<(), russh::Error> {
    if matches!(channel.wait().await, Some(ChannelMsg::Success)) {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "SSH terminal request refused",
        )
        .into())
    }
}

impl Ssh {
    pub(super) async fn next(&mut self) -> Option<Result<Message, Error>> {
        loop {
            match self.channel.wait().await? {
                ChannelMsg::Data { data } => return Some(Ok(Message::Binary(data))),
                ChannelMsg::Eof | ChannelMsg::Close => return None,
                _ => {}
            }
        }
    }

    pub(super) async fn send(&self, message: Message) -> Result<(), Error> {
        if let Message::Binary(bytes) = message {
            self.channel
                .data_bytes(bytes)
                .await
                .map_err(std::io::Error::other)?;
        }
        Ok(())
    }

    pub(super) async fn close(self) -> CloseOutcome {
        let Self {
            mut channel,
            session,
            _transport: mut transport,
        } = self;
        let closed = timeout(Duration::from_secs(2), async {
            channel.close().await?;
            // Drain the reciprocal CHANNEL_CLOSE before ending the TCP stream.
            // Leave time to terminate the SSH session if the peer does not reply.
            let _ = timeout(Duration::from_secs(1), async {
                while let Some(message) = channel.wait().await {
                    if matches!(message, ChannelMsg::Close) {
                        break;
                    }
                }
            })
            .await;
            session
                .disconnect(Disconnect::ByApplication, "PTTStella disconnect", "")
                .await?;
            drop(channel);
            // russh returns Disconnect when the client ends its own I/O loop.
            // The joined loop confirms local shutdown, not a peer acknowledgement.
            match session.await {
                Ok(()) | Err(russh::Error::Disconnect) => Ok(()),
                Err(error) => Err(error),
            }
        })
        .await;
        if matches!(closed, Ok(Ok(()))) {
            // The I/O task has already shut down. Release the cloned socket
            // normally, avoiding an abortive shutdown on Windows.
            transport.0.take();
            CloseOutcome::TransportClosed
        } else {
            CloseOutcome::Unconfirmed
        }
    }
}

#[cfg(test)]
mod tests;
