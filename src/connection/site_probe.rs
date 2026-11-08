//! Independent synthetic station peers. Only explicit probes construct these
//! loopback listeners; no production endpoint, account or stream is recorded.
use crate::{encoding::TextEncoding, sites::Target};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use std::{
    net::TcpListener as Listener,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};
use tokio::{io::AsyncReadExt, net::TcpListener, sync::Notify};
use tokio_tungstenite::tungstenite::Message;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    Connected,
    Disconnected,
    CancelDial,
    Native,
}
#[derive(Clone, Debug, Serialize)]
pub struct Audit {
    pub stage: String,
    pub accepted: Vec<String>,
    pub closed: Vec<String>,
    pub inputs_hex: Vec<String>,
    pub cancelled_dial_eof: bool,
    pub events: Vec<String>,
    pub passed: bool,
}
struct Shared {
    audit: Mutex<Audit>,
    stop: Notify,
    cancelled: AtomicBool,
    path: PathBuf,
}
impl Shared {
    fn record(&self, event: &str, edit: impl FnOnce(&mut Audit)) -> Result<(), String> {
        let mut audit = self
            .audit
            .lock()
            .map_err(|_| "site peer audit lock failed")?;
        if audit.events.len() >= 32 {
            return Err("site peer event budget exceeded".into());
        }
        audit.stage = event.into();
        audit.events.push(event.into());
        edit(&mut audit);
        std::fs::write(
            &self.path,
            serde_json::to_vec_pretty(&*audit).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }
}
pub struct Peers {
    pub first: Target,
    pub gbk: Target,
    pub uao: Target,
    pub hanging: Target,
    pub case: Case,
    shared: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<Result<Audit, String>>>>,
}
impl Peers {
    pub fn start(case: Case, directory: &Path) -> Result<Self, String> {
        let listeners = (0..4)
            .map(|_| {
                let l = Listener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
                l.set_nonblocking(true).map_err(|e| e.to_string())?;
                Ok(l)
            })
            .collect::<Result<Vec<_>, String>>()?;
        let targets = listeners
            .iter()
            .zip([
                TextEncoding::Uao,
                TextEncoding::Gbk,
                TextEncoding::Uao,
                TextEncoding::Uao,
            ])
            .map(|(l, e)| {
                Target::new(
                    format!("ws://{}/bbs", l.local_addr().map_err(|e| e.to_string())?),
                    e,
                )
            })
            .collect::<Result<Vec<_>, String>>()?;
        let shared = Arc::new(Shared {
            audit: Mutex::new(Audit {
                stage: "listening".into(),
                accepted: Vec::new(),
                closed: Vec::new(),
                inputs_hex: vec![String::new(); 3],
                cancelled_dial_eof: false,
                events: Vec::new(),
                passed: false,
            }),
            stop: Notify::new(),
            cancelled: AtomicBool::new(false),
            path: directory.join("site-peer-live.json"),
        });
        shared.record("listening", |_| {})?;
        std::fs::write(directory.join("site-peer-targets.json"),serde_json::to_vec_pretty(&serde_json::json!({"first":targets[0],"gbk":targets[1],"uao":targets[2],"hanging":targets[3],"case":format!("{case:?}"),"expected_native_input_hex":["","6bd6d0cec4","a4a4a4e5"]})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        let state = shared.clone();
        let worker=std::thread::Builder::new().name("pttstella-site-peers".into()).spawn(move||{
            let runtime=tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e|e.to_string())?;
            runtime.block_on(async {
                let listeners=listeners.into_iter().map(TcpListener::from_std).collect::<std::io::Result<Vec<_>>>().map_err(|e|e.to_string())?;
                if state.cancelled.load(Ordering::Acquire){return Err("site peers cancelled before completion".into());}
                tokio::select! {biased;
                    result=tokio::time::timeout(Duration::from_secs(300),serve(&listeners,&state,case))=>result.map_err(|_|"site peers timed out".to_string())??,
                    _=state.stop.notified()=>return Err("site peers cancelled before completion".into()),
                }
                state.record("finished",|a|a.passed=true)?;
                state.audit.lock().map_err(|_|"site peer audit lock failed".to_string()).map(|a|a.clone())
            })
        }).map_err(|e|e.to_string())?;
        Ok(Self {
            first: targets[0].clone(),
            gbk: targets[1].clone(),
            uao: targets[2].clone(),
            hanging: targets[3].clone(),
            case,
            shared,
            worker: Mutex::new(Some(worker)),
        })
    }
    pub fn audit(&self) -> Result<Audit, String> {
        self.shared
            .audit
            .lock()
            .map(|a| a.clone())
            .map_err(|_| "site peer audit lock failed".into())
    }
    pub fn finish(&self, cancel: bool) -> Result<Audit, String> {
        if cancel {
            self.shared.cancelled.store(true, Ordering::Release);
            self.shared.stop.notify_one();
        }
        let worker = self
            .worker
            .lock()
            .map_err(|_| "site peer worker lock failed")?
            .take()
            .ok_or("site peers already joined")?;
        worker
            .join()
            .map_err(|_| "site peer worker panicked".to_string())?
    }
}
impl Drop for Peers {
    fn drop(&mut self) {
        if let Ok(worker) = self.worker.get_mut()
            && let Some(worker) = worker.take()
        {
            self.shared.cancelled.store(true, Ordering::Release);
            self.shared.stop.notify_one();
            let _ = worker.join();
        }
    }
}
async fn early(listeners: &[&TcpListener]) -> Result<(), String> {
    if listeners.is_empty() {
        return std::future::pending().await;
    }
    let futures = listeners
        .iter()
        .map(|l| Box::pin(l.accept()))
        .collect::<Vec<_>>();
    let (result, _, _) = futures_util::future::select_all(futures).await;
    result.map_err(|e| e.to_string())?;
    Err("new station dialled before old Close acknowledgement / dial cancellation".into())
}
async fn guarded(
    listener: &TcpListener,
    later: &[&TcpListener],
    state: &Shared,
    index: usize,
    native: bool,
) -> Result<(), String> {
    tokio::select! {biased;result=station(listener,state,index,native)=>result,result=early(later)=>result}
}
async fn serve(listeners: &[TcpListener], state: &Shared, case: Case) -> Result<(), String> {
    guarded(
        &listeners[0],
        &[&listeners[1], &listeners[2], &listeners[3]],
        state,
        0,
        false,
    )
    .await?;
    if case == Case::CancelDial {
        let later = [&listeners[1], &listeners[2]];
        tokio::select! {biased;result=hanging(&listeners[3],state)=>result?,result=early(&later)=>result?}
    }
    guarded(
        &listeners[1],
        &[&listeners[2]],
        state,
        1,
        case == Case::Native,
    )
    .await?;
    guarded(&listeners[2], &[], state, 2, case == Case::Native).await
}
fn wire(index: usize, ack: &str) -> Vec<u8> {
    let mut bytes = format!(
        "\x1b[2J\x1b[H{}\r\n\x1b[31m",
        ["FIRST UAO", "SECOND GBK", "THIRD UAO"][index]
    )
    .into_bytes();
    if index == 1 {
        bytes.extend_from_slice(b"\xd6\x1b[32m\xd0\xce\xc4");
    } else {
        bytes.extend_from_slice(b"\xa4\x1b[32m\xa4\xa4\xe5");
    }
    bytes.extend_from_slice(format!("\x1b[0m\x1b[24;1H{ack}").as_bytes());
    bytes
}
async fn station(
    listener: &TcpListener,
    state: &Shared,
    index: usize,
    native: bool,
) -> Result<(), String> {
    let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
    let name = ["first", "gbk", "uao"][index];
    state.record(&format!("{name}_accepted"), |a| {
        a.accepted.push(name.into())
    })?;
    let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .read_buffer_size(4096)
        .write_buffer_size(0)
        .max_write_buffer_size(8192)
        .max_message_size(Some(4096))
        .max_frame_size(Some(4096));
    let mut socket = tokio_tungstenite::accept_async_with_config(stream, Some(config))
        .await
        .map_err(|e| e.to_string())?;
    socket
        .send(Message::Binary(wire(index, "READY").into()))
        .await
        .map_err(|e| e.to_string())?;
    state.record(&format!("{name}_ready"), |_| {})?;
    let expected: &[u8] = if !native {
        b""
    } else if index == 1 {
        b"k\xd6\xd0\xce\xc4"
    } else {
        b"\xa4\xa4\xa4\xe5"
    };
    let mut received = Vec::new();
    loop {
        match socket.next().await {
            Some(Ok(Message::Binary(bytes))) => {
                if bytes.is_empty() || received.len() + bytes.len() > expected.len() {
                    return Err(format!("{name}: unexpected/replayed station input"));
                }
                received.extend_from_slice(&bytes);
                if !expected.starts_with(&received) {
                    return Err(format!(
                        "{name}: encoded bytes differ ({} received)",
                        received.len()
                    ));
                }
                state.record(&format!("{name}_input"), |a| {
                    a.inputs_hex[index] = received.iter().map(|b| format!("{b:02x}")).collect()
                })?;
                let ack = if received == expected {
                    if index == 1 {
                        "GBK INPUT EXACT"
                    } else {
                        "UAO INPUT EXACT"
                    }
                } else {
                    "ASCII KEY OK"
                };
                socket
                    .send(Message::Binary(wire(index, ack).into()))
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Some(Ok(Message::Close(_))) => {
                if received != expected {
                    return Err(format!("{name}: Close before exact required input"));
                }
                // Tungstenite queued the reply. Hold it to detect an early dial.
                tokio::time::sleep(Duration::from_millis(150)).await;
                socket.flush().await.map_err(|e| e.to_string())?;
                state.record(&format!("{name}_closed"), |a| a.closed.push(name.into()))?;
                return Ok(());
            }
            Some(Ok(Message::Ping(payload))) => socket
                .send(Message::Pong(payload))
                .await
                .map_err(|e| e.to_string())?,
            _ => return Err(format!("{name}: expected binary input or explicit Close")),
        }
    }
}
async fn hanging(listener: &TcpListener, state: &Shared) -> Result<(), String> {
    let (mut stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
    state.record("hanging_accepted", |a| a.accepted.push("hanging".into()))?;
    let mut request = Vec::new();
    let mut buffer = [0; 512];
    loop {
        let count = stream.read(&mut buffer).await.map_err(|e| e.to_string())?;
        if count == 0 {
            if !request.starts_with(b"GET /bbs HTTP/1.1\r\n")
                || !request.windows(4).any(|b| b == b"\r\n\r\n")
            {
                return Err("cancelled dial did not send a complete upgrade request".into());
            }
            state.record("hanging_cancelled_eof", |a| a.cancelled_dial_eof = true)?;
            return Ok(());
        }
        if request.len() + count > 4096 {
            return Err("hanging upgrade exceeded header budget".into());
        }
        request.extend_from_slice(&buffer[..count]);
        if request.windows(4).any(|b| b == b"\r\n\r\n") {
            state.record("hanging_request_held", |_| {})?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn first_closed(peers: &Peers) {
        let (mut socket, _) = tokio_tungstenite::connect_async(peers.first.endpoint.as_str())
            .await
            .unwrap();
        assert!(matches!(socket.next().await, Some(Ok(Message::Binary(_)))));
        socket.close(None).await.unwrap();
        assert!(matches!(socket.next().await, Some(Ok(Message::Close(_)))));
    }
    #[tokio::test]
    async fn verifier_rejects_big5_bytes_on_the_gbk_station_and_keeps_unexpected_data_out_of_audit()
    {
        let directory = tempfile::tempdir().unwrap();
        let peers = Peers::start(Case::Native, directory.path()).unwrap();
        first_closed(&peers).await;
        let (mut socket, _) = tokio_tungstenite::connect_async(peers.gbk.endpoint.as_str())
            .await
            .unwrap();
        assert!(matches!(socket.next().await, Some(Ok(Message::Binary(_)))));
        socket
            .send(Message::Binary(b"k".to_vec().into()))
            .await
            .unwrap();
        assert!(matches!(socket.next().await, Some(Ok(Message::Binary(_)))));
        socket
            .send(Message::Binary(b"\xa4\xa4\xa4\xe5".to_vec().into()))
            .await
            .unwrap();
        assert!(
            peers
                .finish(false)
                .unwrap_err()
                .contains("encoded bytes differ")
        );
        let audit = peers.audit().unwrap();
        assert!(!audit.passed);
        assert_eq!(audit.inputs_hex, ["", "6b", ""]);
    }
    #[tokio::test]
    async fn verifier_rejects_a_new_tcp_dial_before_old_close() {
        let directory = tempfile::tempdir().unwrap();
        let peers = Peers::start(Case::Connected, directory.path()).unwrap();
        let (mut socket, _) = tokio_tungstenite::connect_async(peers.first.endpoint.as_str())
            .await
            .unwrap();
        assert!(matches!(socket.next().await, Some(Ok(Message::Binary(_)))));
        let url = url::Url::parse(peers.gbk.endpoint.as_str()).unwrap();
        let _early = tokio::net::TcpStream::connect(("127.0.0.1", url.port().unwrap()))
            .await
            .unwrap();
        assert!(
            peers
                .finish(false)
                .unwrap_err()
                .contains("before old Close")
        );
        assert!(peers.audit().unwrap().closed.is_empty());
    }
    #[test]
    fn cancelled_owner_joins_the_pending_accept_thread_without_claiming_pass() {
        let directory = tempfile::tempdir().unwrap();
        let peers = Peers::start(Case::Connected, directory.path()).unwrap();
        assert!(peers.finish(true).unwrap_err().contains("cancelled"));
        assert!(peers.worker.lock().unwrap().is_none());
        assert!(!peers.audit().unwrap().passed);
    }
}
