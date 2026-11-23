//! One scoped loopback stream, polled by its owning video worker.
//! Binding makes no remote request. Bytes are forwarded only after the native
//! decoder's explicit GET/HEAD, with backpressure and no whole-video cache.
use futures_util::{StreamExt, future::BoxFuture, stream::FuturesUnordered};
use reqwest::{Client, header};
use serde::Serialize;
use std::{
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use url::Url;
const CLIENTS: usize = 4;
const HEAD_BYTES: usize = 4096;
const COPY_BYTES: usize = 64 * 1024;
const IDLE: Duration = Duration::from_secs(5);
#[derive(Default)]
struct Metrics {
    accepted: AtomicU64,
    upstream: AtomicU64,
    bytes: AtomicU64,
    rejected: AtomicU64,
    failed: AtomicU64,
    max_slice: AtomicUsize,
    max_active: AtomicUsize,
}
fn add(value: &AtomicU64, n: u64) {
    let _ = value.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
        Some(v.saturating_add(n))
    });
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    pub accepted: u64,
    pub upstream_requests: u64,
    pub bytes: u64,
    pub rejected: u64,
    pub failed: u64,
    pub max_transfer_slice: usize,
    pub max_active: usize,
    pub cancelled_streams: usize,
    pub stopped: bool,
}
pub struct Stream {
    listener: TcpListener,
    endpoint: String,
    path: String,
    source: Url,
    client: Client,
    metrics: Arc<Metrics>,
}
impl Stream {
    pub async fn bind(value: &str) -> Result<Self, String> {
        let target = super::classify(value)?
            .filter(|t| t.kind == super::Kind::Video)
            .ok_or("native stream requires a direct video URL")?;
        let mut source = super::validate(&target.url)?;
        source.set_fragment(None);
        let mut random = [0u8; 24];
        getrandom::fill(&mut random).map_err(|_| "native stream token unavailable")?;
        let token = random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let extension = source
            .path()
            .rsplit('.')
            .next()
            .ok_or("video extension missing")?
            .to_ascii_lowercase();
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| "native stream listener unavailable")?;
        let address = listener
            .local_addr()
            .map_err(|_| "native stream address unavailable")?;
        let path = format!("/{token}.{extension}");
        let endpoint = format!("http://{address}{path}");
        Ok(Self {
            listener,
            endpoint,
            path,
            source,
            client: super::http::client()?,
            metrics: Arc::new(Metrics::default()),
        })
    }
    pub fn url(&self) -> &str {
        &self.endpoint
    }
    /// No listener task is spawned. The caller polls this future on the owned
    /// runtime alongside decoder controls, then resolves stop and awaits it.
    /// Dropping in-flight futures closes their sockets/HTTP bodies directly.
    pub async fn serve(self, stop: impl Future<Output = ()>) -> Report {
        tokio::pin!(stop);
        let mut clients = FuturesUnordered::<BoxFuture<'static, ()>>::new();
        loop {
            tokio::select! {biased;
                _=&mut stop=>break,
                _=clients.next(),if !clients.is_empty()=>{},
                accepted=self.listener.accept(),if clients.len()<CLIENTS=>{
                    let Ok((socket,peer))=accepted else{add(&self.metrics.failed,1);break;};
                    if !peer.ip().is_loopback(){drop(socket);continue;}
                    add(&self.metrics.accepted,1);
                    let client=self.client.clone();let source=self.source.clone();let path=self.path.clone();let metrics=self.metrics.clone();
                    clients.push(Box::pin(connection(socket,client,source,path,metrics)));
                    self.metrics.max_active.fetch_max(clients.len(),Ordering::Relaxed);
                }
            }
        }
        let cancelled_streams = clients.len();
        drop(clients);
        drop(self.listener);
        drop(self.client);
        Report {
            accepted: self.metrics.accepted.load(Ordering::Relaxed),
            upstream_requests: self.metrics.upstream.load(Ordering::Relaxed),
            bytes: self.metrics.bytes.load(Ordering::Relaxed),
            rejected: self.metrics.rejected.load(Ordering::Relaxed),
            failed: self.metrics.failed.load(Ordering::Relaxed),
            max_transfer_slice: self.metrics.max_slice.load(Ordering::Relaxed),
            max_active: self.metrics.max_active.load(Ordering::Relaxed),
            cancelled_streams,
            stopped: true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Range {
    From(u64, Option<u64>),
    Suffix(u64),
}
impl Range {
    fn parse(value: &str) -> Option<Self> {
        let (a, b) = value.strip_prefix("bytes=")?.split_once('-')?;
        fn number(v: &str) -> Option<u64> {
            (!v.is_empty() && v.len() <= 20 && v.bytes().all(|b| b.is_ascii_digit()))
                .then(|| v.parse().ok())
                .flatten()
        }
        if a.is_empty() {
            return number(b).filter(|n| *n > 0).map(Self::Suffix);
        }
        let start = number(a)?;
        let end = if b.is_empty() { None } else { Some(number(b)?) };
        (end.is_none_or(|end| start <= end)).then_some(Self::From(start, end))
    }
    fn header(&self) -> String {
        match self {
            Self::From(a, Some(b)) => format!("bytes={a}-{b}"),
            Self::From(a, None) => format!("bytes={a}-"),
            Self::Suffix(n) => format!("bytes=-{n}"),
        }
    }
}
struct Request {
    head: bool,
    range: Option<Range>,
}
#[derive(Debug)]
enum Error {
    BadRequest,
    NotFound,
    Method,
    Range,
    Network,
    Timeout,
    Upstream(u16),
}
impl Error {
    fn status(&self) -> u16 {
        match self {
            Self::BadRequest => 400,
            Self::NotFound => 404,
            Self::Method => 405,
            Self::Range => 416,
            Self::Timeout => 504,
            Self::Upstream(s) if (400..600).contains(s) => *s,
            _ => 502,
        }
    }
}
fn request(head: &str, path: &str) -> Result<Request, Error> {
    if !head.is_ascii() {
        return Err(Error::BadRequest);
    }
    let mut lines = head.split("\r\n");
    let mut first = lines.next().ok_or(Error::BadRequest)?.split(' ');
    let method = first.next().ok_or(Error::BadRequest)?;
    let target = first.next().ok_or(Error::BadRequest)?;
    let version = first.next().ok_or(Error::BadRequest)?;
    if first.next().is_some() || !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return Err(Error::BadRequest);
    }
    if !matches!(method, "GET" | "HEAD") {
        return Err(Error::Method);
    }
    if target != path {
        return Err(Error::NotFound);
    }
    let mut range = None;
    for line in lines.take_while(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').ok_or(Error::BadRequest)?;
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err(Error::BadRequest);
        }
        let value = value.trim();
        if name.eq_ignore_ascii_case("range") {
            if range.is_some() {
                return Err(Error::Range);
            }
            range = Some(Range::parse(value).ok_or(Error::Range)?);
        } else if name.eq_ignore_ascii_case("transfer-encoding")
            || name.eq_ignore_ascii_case("content-length") && value != "0"
        {
            return Err(Error::BadRequest);
        }
    }
    Ok(Request {
        head: method == "HEAD",
        range,
    })
}
async fn read_request(socket: &mut TcpStream, path: &str) -> Result<Request, Error> {
    let mut head = Vec::with_capacity(512);
    let mut part = [0u8; 512];
    loop {
        let left = (HEAD_BYTES - head.len()).min(part.len());
        if left == 0 {
            return Err(Error::BadRequest);
        }
        let n = tokio::time::timeout(IDLE, socket.read(&mut part[..left]))
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|_| Error::BadRequest)?;
        if n == 0 {
            return Err(Error::BadRequest);
        }
        head.extend_from_slice(&part[..n]);
        if let Some(end) = head.windows(4).position(|w| w == b"\r\n\r\n") {
            return request(
                std::str::from_utf8(&head[..end + 4]).map_err(|_| Error::BadRequest)?,
                path,
            );
        }
    }
}
#[derive(Debug)]
struct ContentRange {
    first: u64,
    last: u64,
    total: u64,
}
impl ContentRange {
    fn length(&self) -> Option<u64> {
        self.last.checked_sub(self.first)?.checked_add(1)
    }
    fn requested(&self, request: &Range) -> bool {
        match request {
            Range::From(first, last) => {
                self.first == *first && last.is_none_or(|last| self.last <= last)
            }
            Range::Suffix(n) => {
                self.first == self.total.saturating_sub(*n) && self.last == self.total - 1
            }
        }
    }
}
fn content_range(value: &str, length: Option<u64>) -> Option<ContentRange> {
    let (span, total) = value.strip_prefix("bytes ")?.split_once('/')?;
    let (first, last) = span.split_once('-')?;
    let number = |v: &str| {
        (!v.is_empty() && v.len() <= 20 && v.bytes().all(|b| b.is_ascii_digit()))
            .then(|| v.parse::<u64>().ok())
            .flatten()
    };
    let value = ContentRange {
        first: number(first)?,
        last: number(last)?,
        total: number(total)?,
    };
    (value.first <= value.last
        && value.last < value.total
        && length.is_none_or(|n| value.length() == Some(n)))
    .then_some(value)
}
async fn forward(
    socket: &mut TcpStream,
    client: Client,
    source: Url,
    path: &str,
    metrics: &Metrics,
    started: &mut bool,
) -> Result<(), Error> {
    let request = read_request(socket, path).await?;
    let mut outgoing = if request.head {
        client.head(source)
    } else {
        client.get(source)
    }
    .header(header::ACCEPT_ENCODING, "identity");
    if let Some(range) = &request.range {
        outgoing = outgoing.header(header::RANGE, range.header());
    }
    add(&metrics.upstream, 1);
    let mut response = tokio::time::timeout(Duration::from_secs(20), outgoing.send())
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(|_| Error::Network)?;
    let status = response.status().as_u16();
    if !matches!(status, 200 | 206) {
        return Err(Error::Upstream(status));
    }
    let headers = response.headers();
    if headers
        .get(header::CONTENT_ENCODING)
        .is_some_and(|v| v.as_bytes() != b"identity")
    {
        return Err(Error::Network);
    }
    let mime = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .split(';')
        .next()
        .unwrap_or_default()
        .trim();
    if mime.len() > 128 {
        return Err(Error::Network);
    }
    let mime = mime.to_ascii_lowercase();
    if !matches!(
        mime.as_str(),
        "video/mp4" | "video/webm" | "video/ogg" | "application/ogg" | "application/octet-stream"
    ) {
        return Err(Error::Network);
    }
    // HEAD's decoded body length is zero. Preserve the origin's representation
    // length from the actual header, not Response::content_length/body hints.
    let length = headers
        .get(header::CONTENT_LENGTH)
        .map(|v| {
            v.to_str()
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or(Error::Network)
        })
        .transpose()?;
    let span = headers
        .get(header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok());
    let range = if status == 206 {
        let range = span
            .filter(|v| v.len() <= 128)
            .and_then(|v| content_range(v, length))
            .ok_or(Error::Network)?;
        if !request.range.as_ref().is_some_and(|r| range.requested(r)) {
            return Err(Error::Network);
        }
        Some(range)
    } else {
        None
    };
    let length = range.as_ref().and_then(ContentRange::length).or(length);
    let mut head = format!(
        "HTTP/1.1 {status} Video\r\nContent-Type: {mime}\r\nCache-Control: no-store\r\nConnection: close\r\n"
    );
    if let Some(n) = length {
        head.push_str(&format!("Content-Length: {n}\r\n"));
    }
    if status == 206
        && let Some(span) = span
    {
        head.push_str(&format!("Content-Range: {span}\r\n"));
    }
    if headers
        .get(header::ACCEPT_RANGES)
        .is_some_and(|v| v.as_bytes() == b"bytes")
    {
        head.push_str("Accept-Ranges: bytes\r\n");
    }
    head.push_str("\r\n");
    tokio::time::timeout(IDLE, socket.write_all(head.as_bytes()))
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(|_| Error::Network)?;
    *started = true;
    if request.head {
        return Ok(());
    }
    let mut remaining = length;
    while let Some(chunk) = tokio::time::timeout(IDLE, response.chunk())
        .await
        .map_err(|_| Error::Timeout)?
        .map_err(|_| Error::Network)?
    {
        if let Some(n) = remaining {
            remaining = Some(n.checked_sub(chunk.len() as u64).ok_or(Error::Network)?);
        }
        for part in chunk.chunks(COPY_BYTES) {
            metrics.max_slice.fetch_max(part.len(), Ordering::Relaxed);
            tokio::time::timeout(IDLE, socket.write_all(part))
                .await
                .map_err(|_| Error::Timeout)?
                .map_err(|_| Error::Network)?;
            add(&metrics.bytes, part.len() as u64);
        }
    }
    if remaining.is_some_and(|n| n != 0) {
        return Err(Error::Network);
    }
    Ok(())
}
async fn connection(
    mut socket: TcpStream,
    client: Client,
    source: Url,
    path: String,
    metrics: Arc<Metrics>,
) {
    let mut started = false;
    if let Err(error) = forward(&mut socket, client, source, &path, &metrics, &mut started).await {
        if matches!(
            error,
            Error::BadRequest | Error::NotFound | Error::Method | Error::Range
        ) {
            add(&metrics.rejected, 1);
        } else {
            add(&metrics.failed, 1);
        }
        if !started {
            let reply = format!(
                "HTTP/1.1 {} Rejected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                error.status()
            );
            let _ = tokio::time::timeout(IDLE, socket.write_all(reply.as_bytes())).await;
        }
    }
}
#[cfg(test)]
#[path = "video_stream/tests.rs"]
mod tests;
