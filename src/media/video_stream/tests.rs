use super::*;
use crate::media::probe_server::Server;
use std::sync::atomic::Ordering;
fn file() -> (tempfile::NamedTempFile, Vec<u8>) {
    let file = tempfile::Builder::new().suffix(".mp4").tempfile().unwrap();
    let bytes = (0..256 * 1024).map(|i| (i % 251) as u8).collect::<Vec<_>>();
    std::fs::write(file.path(), &bytes).unwrap();
    (file, bytes)
}
async fn start(
    url: &str,
) -> (
    String,
    tokio::sync::oneshot::Sender<()>,
    tokio::task::JoinHandle<Report>,
) {
    let stream = Stream::bind(url).await.unwrap();
    let endpoint = stream.url().to_owned();
    let (stop, closed) = tokio::sync::oneshot::channel();
    let join = tokio::spawn(stream.serve(async {
        let _ = closed.await;
    }));
    (endpoint, stop, join)
}
#[test]
fn request_and_response_ranges_have_independent_exact_bounds() {
    let path = "/token.mp4";
    assert_eq!(
        request(
            "GET /token.mp4 HTTP/1.1\r\nRange: bytes=20-44\r\n\r\n",
            path
        )
        .unwrap()
        .range,
        Some(Range::From(20, Some(44)))
    );
    assert_eq!(Range::parse("bytes=-25"), Some(Range::Suffix(25)));
    for bad in [
        "bytes=5-4",
        "bytes=-0",
        "bytes=1-2,4-5",
        "bytes=18446744073709551616-",
        "bytes=+1-2",
    ] {
        assert_eq!(Range::parse(bad), None);
    }
    assert!(content_range("bytes 20-44/100", Some(25)).is_some());
    for bad in [
        "bytes 20-44/40",
        "bytes 44-20/100",
        "bytes 0-18446744073709551615/18446744073709551615",
    ] {
        assert!(content_range(bad, Some(25)).is_none());
    }
    assert!(content_range("bytes 20-44/100", Some(26)).is_none());
    assert!(matches!(
        request("GET /wrong HTTP/1.1\r\n\r\n", path),
        Err(Error::NotFound)
    ));
    assert!(matches!(
        request("POST /token.mp4 HTTP/1.1\r\n\r\n", path),
        Err(Error::Method)
    ));
    assert!(matches!(
        request(
            "GET /token.mp4 HTTP/1.1\r\nRange: bytes=0-1\r\nRange: bytes=2-3\r\n\r\n",
            path
        ),
        Err(Error::Range)
    ));
}
#[tokio::test]
async fn no_preload_then_exact_range_head_and_full_body_without_cache() {
    let (file, bytes) = file();
    let mut source = Server::start(file.path(), false).unwrap();
    let (endpoint, stop, join) = start(&source.url()).await;
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(source.requests.load(Ordering::Acquire), 0);
    let client = super::super::http::client().unwrap();
    let range = client
        .get(&endpoint)
        .header("Range", "bytes=32768-32800")
        .send()
        .await
        .unwrap();
    assert_eq!(range.status(), 206);
    assert_eq!(
        range.headers()["content-range"],
        format!("bytes 32768-32800/{}", bytes.len())
    );
    assert_eq!(range.bytes().await.unwrap().as_ref(), &bytes[32768..32801]);
    let head = client.head(&endpoint).send().await.unwrap();
    assert_eq!(head.headers()["content-length"], bytes.len().to_string());
    assert!(head.bytes().await.unwrap().is_empty());
    let ranged_head = client
        .head(&endpoint)
        .header("Range", "bytes=8-16")
        .send()
        .await
        .unwrap();
    assert_eq!(ranged_head.status(), 206);
    assert_eq!(ranged_head.headers()["content-length"], "9");
    assert_eq!(
        ranged_head.headers()["content-range"],
        format!("bytes 8-16/{}", bytes.len())
    );
    assert!(ranged_head.bytes().await.unwrap().is_empty());
    let all = client.get(&endpoint).send().await.unwrap();
    assert_eq!(all.bytes().await.unwrap().as_ref(), bytes);
    stop.send(()).unwrap();
    let report = join.await.unwrap();
    assert!(report.stopped);
    assert_eq!(
        (report.upstream_requests, report.failed, report.bytes),
        (4, 0, bytes.len() as u64 + 33)
    );
    assert!(report.max_transfer_slice <= COPY_BYTES);
    let source = source.finish().unwrap();
    assert!(source.joined);
    assert_eq!(source.requests, 4);
}
#[tokio::test]
async fn token_method_and_range_failures_never_contact_upstream() {
    let (file, _) = file();
    let mut source = Server::start(file.path(), false).unwrap();
    let (endpoint, stop, join) = start(&source.url()).await;
    let client = super::super::http::client().unwrap();
    let mut wrong = Url::parse(&endpoint).unwrap();
    wrong.set_path("/wrong.mp4");
    assert_eq!(client.get(wrong).send().await.unwrap().status(), 404);
    assert_eq!(client.post(&endpoint).send().await.unwrap().status(), 405);
    assert_eq!(
        client
            .get(&endpoint)
            .header("Range", "bytes=0-1,2-3")
            .send()
            .await
            .unwrap()
            .status(),
        416
    );
    stop.send(()).unwrap();
    let report = join.await.unwrap();
    assert_eq!((report.rejected, report.upstream_requests), (3, 0));
    assert!(source.finish().unwrap().joined);
}
#[tokio::test]
async fn stopping_stalled_body_closes_decoder_socket_and_future() {
    let (file, _) = file();
    let mut source = Server::start(file.path(), true).unwrap();
    let (endpoint, stop, join) = start(&source.url()).await;
    let client = super::super::http::client().unwrap();
    let response = client.get(&endpoint).send().await.unwrap();
    assert_eq!(source.requests.load(Ordering::Acquire), 1);
    stop.send(()).unwrap();
    let report = tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .unwrap()
        .unwrap();
    assert!(report.stopped);
    assert_eq!((report.cancelled_streams, report.bytes), (1, 0));
    assert!(response.bytes().await.is_err());
    let source = source.finish().unwrap();
    assert!(source.joined);
    assert_eq!(source.bytes, 0);
}
#[tokio::test]
async fn header_slow_clients_are_bounded_and_cancelled_without_upstream() {
    let (file, _) = file();
    let mut source = Server::start(file.path(), false).unwrap();
    let stream = Stream::bind(&source.url()).await.unwrap();
    let metrics = stream.metrics.clone();
    let endpoint = Url::parse(stream.url()).unwrap();
    let (stop, closed) = tokio::sync::oneshot::channel();
    let join = tokio::spawn(stream.serve(async {
        let _ = closed.await;
    }));
    let mut clients = Vec::new();
    for _ in 0..CLIENTS + 1 {
        clients.push(
            TcpStream::connect(("127.0.0.1", endpoint.port().unwrap()))
                .await
                .unwrap(),
        );
    }
    tokio::time::timeout(Duration::from_secs(2), async {
        while metrics.accepted.load(Ordering::Acquire) < CLIENTS as u64 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    stop.send(()).unwrap();
    let report = join.await.unwrap();
    assert_eq!(report.accepted, CLIENTS as u64);
    assert_eq!(report.max_active, CLIENTS);
    assert_eq!(report.cancelled_streams, CLIENTS);
    assert_eq!(report.upstream_requests, 0);
    drop(clients);
    assert!(source.finish().unwrap().joined);
}
#[tokio::test]
async fn invalid_targets_are_rejected_before_a_listener_or_request() {
    for value in [
        "file:///tmp/movie.mp4",
        "https://user:pass@example.invalid/a.mp4",
        "http://example.invalid/a.png",
        "https://youtube.com/watch?v=fixture",
    ] {
        assert!(Stream::bind(value).await.is_err());
    }
}

async fn mock(reply: &'static [u8]) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let join = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut head = Vec::new();
        let mut part = [0u8; 512];
        loop {
            let n = stream.read(&mut part).await.unwrap();
            assert!(n > 0);
            head.extend_from_slice(&part[..n]);
            if head.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            assert!(head.len() < 4096);
        }
        let _ = stream.write_all(reply).await;
        String::from_utf8(head).unwrap()
    });
    (
        format!("http://{address}/video.mp4?fixture=1#fragment"),
        join,
    )
}
#[tokio::test]
async fn remote_metadata_and_credential_redirects_fail_before_any_body_forwarding() {
    for reply in [
        &b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 4\r\n\r\nhtml"[..],
        &b"HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Encoding: gzip\r\nContent-Length: 4\r\n\r\nhtml"[..],
        &b"HTTP/1.1 206 Partial Content\r\nContent-Type: video/mp4\r\nContent-Length: 24\r\nContent-Range: bytes 20-44/100\r\n\r\n"[..],
        &b"HTTP/1.1 206 Partial Content\r\nContent-Type: video/mp4\r\nContent-Length: 25\r\nContent-Range: bytes 0-24/100\r\n\r\n"[..],
        &b"HTTP/1.1 302 Found\r\nLocation: http://user:pass@127.0.0.1:9/video.mp4\r\nContent-Length: 0\r\n\r\n"[..],
    ] {
        let(source,origin)=mock(reply).await;let(endpoint,stop,join)=start(&source).await;let client=super::super::http::client().unwrap();let response=client.get(endpoint).header("Range","bytes=20-44").send().await.unwrap();assert_eq!(response.status(),502);assert!(response.bytes().await.unwrap().is_empty());stop.send(()).unwrap();let report=join.await.unwrap();assert_eq!((report.failed,report.upstream_requests,report.bytes),(1,1,0));origin.await.unwrap();
    }
}
#[tokio::test]
async fn forwarding_preserves_query_and_range_without_forwarding_browser_credentials() {
    let(source,origin)=mock(b"HTTP/1.1 206 Partial Content\r\nContent-Type: VIDEO/MP4\r\nContent-Length: 3\r\nContent-Range: bytes 0-2/64\r\n\r\nABC").await;
    let (endpoint, stop, join) = start(&source).await;
    let client = super::super::http::client().unwrap();
    let response = client
        .get(endpoint)
        .header("Range", "bytes=0-2")
        .header("Authorization", "Bearer synthetic")
        .header("Cookie", "fixture=private")
        .header("Referer", "http://example.invalid/private")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 206);
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"ABC");
    stop.send(()).unwrap();
    assert_eq!(join.await.unwrap().bytes, 3);
    let head = origin.await.unwrap();
    assert!(head.starts_with("GET /video.mp4?fixture=1 HTTP/1.1\r\n"));
    let lower = head.to_ascii_lowercase();
    for header in ["authorization:", "cookie:", "referer:"] {
        assert!(!lower.contains(header));
    }
    assert!(lower.contains("accept-encoding: identity"));
    assert!(lower.contains("user-agent: pttstella/"));
}

#[tokio::test]
async fn range_body_cannot_exceed_the_verified_representation_length() {
    let(source,origin)=mock(b"HTTP/1.1 206 Partial Content\r\nContent-Type: video/mp4\r\nTransfer-Encoding: chunked\r\nContent-Range: bytes 0-2/64\r\n\r\n4\r\nABCD\r\n0\r\n\r\n").await;
    let (endpoint, stop, join) = start(&source).await;
    let client = super::super::http::client().unwrap();
    let response = client
        .get(endpoint)
        .header("Range", "bytes=0-2")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 206);
    assert_eq!(response.headers()["content-length"], "3");
    assert!(response.bytes().await.is_err());
    stop.send(()).unwrap();
    let report = join.await.unwrap();
    assert_eq!((report.failed, report.bytes), (1, 0));
    origin.await.unwrap();
}
