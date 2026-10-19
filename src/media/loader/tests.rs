use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::timeout,
};

const PNG: &[u8] = include_bytes!("../../../tests/fixtures/media/colours.png");
const GIF: &[u8] = include_bytes!("../../../tests/fixtures/media/motion.gif");

#[test]
fn inspect_and_cache_have_independent_resource_limits() {
    let image = inspect(PNG.to_vec()).unwrap();
    assert_eq!(
        (image.width, image.height, image.mime),
        (64, 32, "image/png")
    );
    assert!(inspect(GIF.to_vec()).unwrap().animated_format);
    assert_eq!(frame_count(GIF, "image/gif"), Ok(2));
    assert_eq!(
        frame_count(&GIF[..GIF.len() - 10], "image/gif"),
        Err(Error::Format)
    );
    let first = GIF
        .windows(4)
        .position(|bytes| bytes == b"\x21\xf9\x04\x04")
        .unwrap();
    let mut many = GIF[..first].to_vec();
    for _ in 0..129 {
        many.extend_from_slice(&GIF[first..GIF.len() - 1]);
    }
    many.push(0x3b);
    assert_eq!(inspect(many).unwrap_err(), Error::Dimensions);
    assert_eq!(
        inspect(b"<html>not an image</html>".to_vec()).unwrap_err(),
        Error::Format
    );
    let mut oversized = PNG.to_vec();
    oversized[16..20].copy_from_slice(&9000u32.to_be_bytes());
    assert_eq!(inspect(oversized).unwrap_err(), Error::Dimensions);
    let mut loader = Loader::new();
    let mut data = PNG.to_vec();
    data.resize(MAX_BYTES, 0);
    let image = Arc::new(inspect(data).unwrap());
    for index in 0..10 {
        loader.retain(&Loaded {
            key: index.to_string(),
            image: image.clone(),
            cached: false,
            expires: Instant::now() + CACHE_TTL,
        });
    }
    assert_eq!(loader.cache.len(), 3);
    assert_eq!(loader.bytes, CACHE_BYTES);
    assert_eq!(loader.cache.front().unwrap().key, "7");
    loader.prune(Instant::now() + CACHE_TTL);
    assert!(loader.cache.is_empty());
    assert_eq!(loader.bytes, 0);
    let mut headers = header::HeaderMap::new();
    headers.insert(header::CACHE_CONTROL, "public, max-age=12".parse().unwrap());
    assert_eq!(cache_ttl(&headers), Duration::from_secs(12));
    headers.insert(header::AGE, "10".parse().unwrap());
    assert_eq!(cache_ttl(&headers), Duration::from_secs(2));
    headers.append(header::CACHE_CONTROL, "no-store".parse().unwrap());
    assert_eq!(cache_ttl(&headers), Duration::ZERO);
}

#[tokio::test]
async fn total_deadline_covers_a_stalled_response_body() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let url = format!("http://{}/stalled.png", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        request_path(&mut socket).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10000\r\n\r\n")
            .await
            .unwrap();
        assert_peer_closed(&mut socket).await;
    });
    let mut loader = Loader::new();
    assert_eq!(loader.deadline, Duration::from_secs(20));
    loader.deadline = Duration::from_millis(60);
    assert_eq!(loader.load(&url).await.err(), Some(Error::Timeout));
    server.await.unwrap();
}

async fn request_headers(socket: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    while bytes.len() < 4096 && !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(socket.read_u8().await.unwrap());
    }
    String::from_utf8(bytes).unwrap()
}

async fn request_path(socket: &mut tokio::net::TcpStream) -> String {
    request_headers(socket)
        .await
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_owned()
}

async fn assert_peer_closed(socket: &mut tokio::net::TcpStream) {
    let mut byte = [0];
    let closed = timeout(Duration::from_secs(2), socket.read(&mut byte))
        .await
        .unwrap();
    // Cancellation/timeout can produce FIN or RST (observed on macOS).
    // Data, timeout and other errors still fail; next-load success is separate.
    assert!(
        matches!(closed, Ok(0))
            || matches!(closed, Err(ref error) if error.kind() == std::io::ErrorKind::ConnectionReset)
    );
}

#[tokio::test]
async fn production_http_loader_cache_redirect_format_and_stream_capacity() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let count = Arc::new(AtomicUsize::new(0));
    let requests = count.clone();
    let server = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let path = request_path(&mut socket).await;
            requests.fetch_add(1, Ordering::SeqCst);
            let (header, body) = match path.as_str() {
                "/image.png" => (format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nCache-Control: max-age=60\r\nConnection: close\r\n\r\n", PNG.len()), PNG.to_vec()),
                "/no.png" => (format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", PNG.len()), PNG.to_vec()),
                "/bad.png" => ("HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n".to_owned(), b"html".to_vec()),
                "/huge.png" => (format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", MAX_BYTES+1), Vec::new()),
                "/redirect.png" => ("HTTP/1.1 302 Found\r\nLocation: /image.png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(), Vec::new()),
                "/loop.png" => ("HTTP/1.1 302 Found\r\nLocation: /loop.png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(), Vec::new()),
                "/chunk.png" => {
                    let mut body = format!("{:x}\r\n", MAX_BYTES+1).into_bytes();
                    body.resize(body.len()+MAX_BYTES+1, b'x');body.extend_from_slice(b"\r\n0\r\n\r\n");
                    ("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_owned(), body)
                },
                _ => ("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(), Vec::new()),
            };
            if socket.write_all(header.as_bytes()).await.is_ok() {
                let _ = socket.write_all(&body).await;
            }
        }
    });
    let mut loader = Loader::new();
    let loaded = loader
        .load(&format!("{origin}/image.png#first"))
        .await
        .unwrap();
    assert_eq!(loaded.image.bytes, PNG);
    loader.retain(&loaded);
    assert!(
        loader
            .load(&format!("{origin}/image.png#second"))
            .await
            .unwrap()
            .cached
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    for _ in 0..2 {
        let loaded = loader.load(&format!("{origin}/no.png")).await.unwrap();
        loader.retain(&loaded);
    }
    assert_eq!(count.load(Ordering::SeqCst), 3);
    for (path, expected) in [
        ("bad.png", Error::Format),
        ("huge.png", Error::TooLarge),
        ("chunk.png", Error::TooLarge),
        ("missing.png", Error::Http(404)),
    ] {
        assert_eq!(
            loader.load(&format!("{origin}/{path}")).await.err(),
            Some(expected)
        );
    }
    assert_eq!(
        loader
            .load(&format!("{origin}/redirect.png"))
            .await
            .unwrap()
            .image
            .bytes,
        PNG
    );
    assert!(loader.load(&format!("{origin}/loop.png")).await.is_err());
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

#[tokio::test]
async fn dropping_pending_load_closes_its_request_and_next_load_can_complete() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (started, start) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        assert_eq!(request_path(&mut socket).await, "/slow.png");
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10000\r\n\r\n")
            .await
            .unwrap();
        started.send(()).unwrap();
        assert_peer_closed(&mut socket).await;
        let (mut socket, _) = listener.accept().await.unwrap();
        request_path(&mut socket).await;
        socket
            .write_all(
                format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", PNG.len()).as_bytes(),
            )
            .await
            .unwrap();
        socket.write_all(PNG).await.unwrap();
    });
    let mut loader = Loader::new();
    let mut pending = loader.load(&format!("{origin}/slow.png"));
    tokio::select! { _ = start => {}, result = &mut pending => panic!("unexpected early result: {:?}",result.err()) }
    drop(pending);
    assert_eq!(
        timeout(
            Duration::from_secs(3),
            loader.load(&format!("{origin}/image.png"))
        )
        .await
        .unwrap()
        .unwrap()
        .image
        .bytes,
        PNG
    );
    server.await.unwrap();
}

#[tokio::test]
async fn image_requests_identify_the_app_through_redirects_without_browsing_context() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut observed = Vec::new();
        for step in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = request_headers(&mut socket).await;
            let has_agent = request
                .lines()
                .any(|line| line.to_ascii_lowercase().starts_with("user-agent:"));
            observed.push(request);
            // Some image hosts close the connection instead of returning HTTP 4xx.
            if !has_agent {
                return observed;
            }
            if step == 0 {
                socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: /image.png\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
            } else {
                socket
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            PNG.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .unwrap();
                socket.write_all(PNG).await.unwrap();
            }
        }
        observed
    });
    let mut loader = Loader::new();
    let loaded = timeout(
        Duration::from_secs(3),
        loader.load(&format!("{origin}/redirect.png")),
    )
    .await
    .unwrap();
    let requests = timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
    for request in &requests {
        let headers: Vec<_> = request
            .lines()
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_ascii_lowercase(), value.trim()))
            .collect();
        assert_eq!(
            headers
                .iter()
                .find(|(name, _)| name == "user-agent")
                .map(|(_, value)| *value),
            Some(concat!("PTTStella/", env!("CARGO_PKG_VERSION")))
        );
        assert!(
            !headers
                .iter()
                .any(|(name, _)| matches!(name.as_str(), "referer" | "cookie" | "authorization"))
        );
    }
    assert_eq!(requests.len(), 2);
    assert_eq!(loaded.unwrap().image.bytes, PNG);
}
