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
}

async fn fixture() -> (
    std::net::SocketAddr,
    String,
    oneshot::Receiver<u8>,
    tokio::task::JoinHandle<Result<(), russh::Error>>,
) {
    let key = PrivateKey::random(&mut russh::keys::key::safe_rng(), Algorithm::Ed25519).unwrap();
    let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (done, result) = oneshot::channel();
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await?;
        let config = server::Config {
            keys: vec![key],
            ..server::Config::default()
        };
        server::run_stream(
            Arc::new(config),
            stream,
            Peer {
                done: Some(done),
                ..Peer::default()
            },
        )
        .await?
        .await
    });
    (address, fingerprint, result, task)
}

#[tokio::test]
async fn pinned_ssh_negotiates_bbs_terminal_and_transports_unmodified_bytes() {
    timeout(Duration::from_secs(5), async {
        let (address, key, stage, peer) = fixture().await;
        let ssh = open(address, &key, ALGORITHMS).await.unwrap();
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
        assert_eq!(stage.await.unwrap(), 5);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn changed_ssh_host_key_is_rejected_before_authentication() {
    timeout(Duration::from_secs(5), async {
        let (address, _, stage, peer) = fixture().await;
        assert_eq!(
            open(address, PTT_FINGERPRINT, ALGORITHMS)
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
        let dial = tokio::spawn(open(address, PTT_FINGERPRINT, ALGORITHMS));
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
