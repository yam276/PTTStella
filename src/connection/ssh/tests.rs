use super::*;
use russh::{ChannelId, Pty, keys::PrivateKey, server};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::oneshot,
};

const ALGORITHMS: &[Algorithm] = &[Algorithm::Ed25519];
const DATA: &[u8] = &[0xa4, 0xa4, 0xff, b'\r'];

#[derive(Default)]
struct Peer {
    stage: u8,
    done: Option<oneshot::Sender<u8>>,
}
impl Drop for Peer {
    fn drop(&mut self) {
        if let Some(done) = self.done.take() {
            let _ = done.send(self.stage);
        }
    }
}

impl server::Handler for Peer {
    type Error = russh::Error;

    async fn auth_none(&mut self, user: &str) -> Result<server::Auth, Self::Error> {
        assert_eq!(self.stage, 0);
        assert_eq!(user, "bbs");
        self.stage = 1;
        Ok(server::Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        _: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(self.stage, 1);
        self.stage = 2;
        reply.accept().await;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        term: &str,
        cols: u32,
        rows: u32,
        width: u32,
        height: u32,
        modes: &[(Pty, u32)],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(self.stage, 2);
        assert_eq!((term, cols, rows, width, height), ("xterm", 80, 24, 0, 0));
        assert!(modes.is_empty());
        self.stage = 3;
        session.channel_success(channel)
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(self.stage, 3);
        self.stage = 4;
        session.channel_success(channel)?;
        session.data(channel, DATA.to_vec())
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        bytes: &[u8],
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(self.stage, 4);
        assert_eq!(bytes, DATA);
        self.stage = 5;
        session.data(channel, b"received".to_vec())
    }

    async fn channel_close(
        &mut self,
        _: ChannelId,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        assert_eq!(self.stage, 5);
        self.stage = 6;
        Ok(())
    }
}

/// OpenSSH grows the session window as the shell starts, before it replies to
/// the shell request. RFC 4254 also lets a server advise flow control first.
struct Notices {
    grown: bool,
    answered: Option<oneshot::Sender<bool>>,
}
impl server::Handler for Notices {
    type Error = russh::Error;

    async fn auth_none(&mut self, _: &str) -> Result<server::Auth, Self::Error> {
        Ok(server::Auth::Accept)
    }

    async fn channel_open_session(
        &mut self,
        _: Channel<server::Msg>,
        reply: server::ChannelOpenHandle,
        _: &mut server::Session,
    ) -> Result<(), Self::Error> {
        reply.accept().await;
        Ok(())
    }

    fn adjust_window(&mut self, _: ChannelId, current: u32) -> u32 {
        self.grown = true;
        current
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut server::Session,
    ) -> Result<(), Self::Error> {
        if let Some(answered) = self.answered.take() {
            let _ = answered.send(self.grown);
        }
        session.xon_xoff_request(channel, false)?;
        session.channel_success(channel)
    }
}

async fn serve<H: server::Handler<Error = russh::Error> + Send + 'static>(
    handler: H,
    config: server::Config,
) -> (
    std::net::SocketAddr,
    String,
    tokio::task::JoinHandle<Result<(), russh::Error>>,
) {
    let key = PrivateKey::random(&mut russh::keys::key::safe_rng(), Algorithm::Ed25519).unwrap();
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let config = server::Config {
            keys: [vec![key], config.keys.clone()].concat(),
            ..config
        };
        server::run_stream(Arc::new(config), stream, handler)
            .await?
            .await
    });
    (address, fingerprint, task)
}

async fn fixture() -> (
    std::net::SocketAddr,
    String,
    oneshot::Receiver<u8>,
    tokio::task::JoinHandle<Result<(), russh::Error>>,
) {
    let (done, result) = oneshot::channel();
    let peer = Peer {
        done: Some(done),
        ..Peer::default()
    };
    let (address, fingerprint, task) = serve(peer, server::Config::default()).await;
    (address, fingerprint, result, task)
}

#[tokio::test]
async fn pinned_ssh_negotiates_bbs_terminal_and_transports_unmodified_bytes() {
    timeout(Duration::from_secs(5), async {
        let (address, key, stage, peer) = fixture().await;
        let ssh = open(address, &["SHA256:unrelated", &key], ALGORITHMS)
            .await
            .unwrap();
        let mut socket = super::super::Socket::Ssh(Box::new(ssh));
        assert!(!socket.uses_telnet_framing());
        assert_eq!(socket.next().await.unwrap().unwrap().into_data(), DATA);
        socket
            .send(Message::Binary(DATA.to_vec().into()))
            .await
            .unwrap();
        assert_eq!(
            socket.next().await.unwrap().unwrap().into_data(),
            b"received".as_slice()
        );
        assert_eq!(
            super::super::transport::stop(&mut Some(socket), &mut None).await,
            CloseOutcome::TransportClosed
        );
        peer.await.unwrap().unwrap();
        assert_eq!(stage.await.unwrap(), 6);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn changed_ssh_host_key_is_rejected_before_authentication() {
    timeout(Duration::from_secs(5), async {
        let (address, _, stage, peer) = fixture().await;
        assert_eq!(
            open(address, PTT_FINGERPRINTS, ALGORITHMS)
                .await
                .err()
                .unwrap(),
            crate::msg!("site-ssh-host-key")
        );
        let _ = peer.await.unwrap();
        assert_eq!(stage.await.unwrap(), 0);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn aborting_ssh_key_exchange_closes_the_underlying_tcp_connection() {
    timeout(Duration::from_secs(5), async {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let dial = tokio::spawn(open(address, PTT_FINGERPRINTS, ALGORITHMS));
        let mut peer = BufReader::new(listener.accept().await.unwrap().0);
        let mut greeting = String::new();
        peer.read_line(&mut greeting).await.unwrap();
        assert!(greeting.starts_with("SSH-2.0-"));
        peer.write_all(b"SSH-2.0-PTTStellaTest\r\n").await.unwrap();
        // Receive the start of KEXINIT after the library has started its I/O task.
        peer.read_u8().await.unwrap();
        dial.abort();
        assert!(dial.await.err().unwrap().is_cancelled());
        let mut remainder = Vec::new();
        peer.read_to_end(&mut remainder).await.unwrap();
        assert!(remainder.len() < 8192);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn ssh_request_reply_follows_window_and_flow_control_notices() {
    timeout(Duration::from_secs(5), async {
        let (answered, grown) = oneshot::channel();
        let notices = Notices {
            grown: false,
            answered: Some(answered),
        };
        let config = server::Config {
            window_size: 32,
            ..server::Config::default()
        };
        let (address, key, peer) = serve(notices, config).await;
        let config = client::Config {
            preferred: Preferred {
                key: Cow::Borrowed(ALGORITHMS),
                ..Preferred::default()
            },
            ..client::Config::default()
        };
        let tcp = TcpStream::connect(address).await.unwrap();
        let mut session = client::connect_stream(Arc::new(config), tcp, HostKey(vec![key]))
            .await
            .unwrap();
        assert!(session.authenticate_none("bbs").await.unwrap().success());
        let mut channel = session.channel_open_session().await.unwrap();
        // russh grows a server window only after receiving data.
        channel.data_bytes(vec![0; 32]).await.unwrap();
        channel.request_shell(true).await.unwrap();
        request_accepted(&mut channel).await.unwrap();
        assert!(grown.await.unwrap());
        drop((channel, session));
        let _ = peer.await;
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn ptt_key_algorithms_negotiate_the_current_ptt_host_key_offer() {
    timeout(Duration::from_secs(5), async {
        // PTT now offers ssh-ed25519, ecdsa-sha2-nistp256 and SHA-1 ssh-rsa.
        let ecdsa = PrivateKey::random(
            &mut russh::keys::key::safe_rng(),
            Algorithm::Ecdsa {
                curve: russh::keys::EcdsaCurve::NistP256,
            },
        )
        .unwrap();
        let config = server::Config {
            keys: vec![ecdsa],
            ..server::Config::default()
        };
        let (address, _, peer) = serve(Peer::default(), config).await;
        // Negotiation reaches the pin check, which rejects the synthetic key.
        assert_eq!(
            open(address, PTT_FINGERPRINTS, PTT_KEY_ALGORITHMS)
                .await
                .err()
                .unwrap(),
            crate::msg!("site-ssh-host-key")
        );
        let _ = peer.await;
    })
    .await
    .unwrap();
}
