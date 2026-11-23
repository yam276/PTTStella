use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const NEW: &str = r#"{"tag_name":"v0.3.0","draft":false,"prerelease":false,"html_url":"https://evil.invalid/","body":"ignored"}"#;
fn client() -> Client {
    Client::builder()
        .use_preconfigured_tls(
            rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(rustls::RootCertStore::empty())
            .with_no_client_auth(),
        )
        .no_proxy()
        .redirect(Policy::none())
        .user_agent("PTTStella/test")
        .build()
        .unwrap()
}
fn reply(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
async fn peer(response: String) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/latest", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let byte = socket.read_u8().await.unwrap();
            request.push(byte);
            assert!(request.len() < 16384);
        }
        let _ = socket.write_all(response.as_bytes()).await;
        String::from_utf8(request).unwrap()
    });
    (url, task)
}
fn pending(request: Request) -> Checker {
    Checker {
        request: Some(request),
        status: Status {
            checking: true,
            ..Status::default()
        },
        source: Source::Published,
    }
}

#[test]
fn only_newer_stable_semver_releases_are_offered() {
    for (tag, current, new) in [
        ("v0.3.0", "0.2.0-dev.1", true),
        ("v0.2.0", "0.2.0-dev.1", true),
        ("v0.1.0", "0.2.0-dev.1", false),
        ("v0.2.0", "0.2.0", false),
        ("v0.10.0", "0.9.9", true),
        ("1.0.0", "0.99.9", true),
        ("v0.2.0+remote", "0.2.0+local", false),
    ] {
        let body = format!(r#"{{"tag_name":"{tag}","draft":false,"prerelease":false}}"#);
        assert_eq!(
            compare(body.as_bytes(), current).unwrap().is_some(),
            new,
            "{tag} vs {current}"
        );
    }
    let release = compare(NEW.as_bytes(), "0.2.0").unwrap().unwrap();
    assert_eq!(release.version, "0.3.0");
    assert_eq!(
        release.url,
        "https://github.com/yam276/PTTStella/releases/tag/v0.3.0"
    );
    for body in [
        r#"{"tag_name":"v9.0.0","draft":true,"prerelease":false}"#,
        r#"{"tag_name":"v9.0.0","draft":false,"prerelease":true}"#,
        r#"{"tag_name":"v9.0.0-rc.1","draft":false,"prerelease":false}"#,
        r#"{"tag_name":"https://evil.invalid","draft":false,"prerelease":false}"#,
        r#"{"tag_name":"v01.2.3","draft":false,"prerelease":false}"#,
        "{}",
        "not JSON",
    ] {
        assert_eq!(compare(body.as_bytes(), "0.2.0"), Err(Error::Invalid));
    }
}

#[tokio::test]
async fn successful_check_uses_public_headers_and_never_sends_credentials() {
    let (url, task) = peer(reply("200 OK", NEW)).await;
    let release = fetch(client(), &url, "0.2.0", Duration::from_secs(2))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(release.version, "0.3.0");
    let headers = task.await.unwrap().to_ascii_lowercase();
    assert!(headers.starts_with("get /latest http/1.1\r\n"));
    assert!(headers.contains("accept: application/vnd.github+json\r\n"));
    assert!(headers.contains("x-github-api-version: 2022-11-28\r\n"));
    assert!(headers.contains("user-agent: pttstella/test\r\n"));
    assert!(
        !headers.contains("authorization:")
            && !headers.contains("cookie:")
            && !headers.contains("referer:")
    );
}

#[tokio::test]
async fn failures_are_silent_not_current_and_no_redirect_is_followed() {
    for response in [
        reply("404 Not Found", "private"),
        reply("403 Forbidden", "rate limited"),
        reply("429 Too Many Requests", "wait"),
        reply("200 OK", "bad JSON"),
        "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/forbidden\r\nContent-Length: 0\r\n\r\n"
            .into(),
    ] {
        let (url, task) = peer(response).await;
        let mut owner = pending(Box::pin(async move {
            fetch(client(), &url, "0.2.0", Duration::from_secs(2)).await
        }));
        let status = owner.next().await;
        assert_eq!(status, Status::default());
        task.await.unwrap();
    }
    let mut owner = pending(Box::pin(async { Err(Error::Network) }));
    owner.status.available = Some(Release {
        version: "0.3.0".into(),
        url: format!("{RELEASES}v0.3.0"),
    });
    let status = owner.next().await;
    assert_eq!(status.available.unwrap().version, "0.3.0");
    assert!(!status.up_to_date && !status.checking);
}

#[tokio::test]
async fn declared_and_streaming_oversize_responses_are_bounded() {
    for response in [
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            MAX_BYTES + 1
        ),
        format!(
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
            MAX_BYTES + 1,
            "x".repeat(MAX_BYTES + 1)
        ),
    ] {
        let (url, task) = peer(response).await;
        assert_eq!(
            fetch(client(), &url, "0.2.0", Duration::from_secs(2)).await,
            Err(Error::TooLarge)
        );
        task.await.unwrap();
    }
}

#[tokio::test]
async fn header_and_body_stalls_timeout_and_drop_closes_the_request() {
    for body in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/latest", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(socket.read_u8().await.unwrap());
            }
            if body {
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\n\r\n{")
                    .await
                    .unwrap();
            }
            let mut byte = [0];
            assert_eq!(
                tokio::time::timeout(Duration::from_secs(2), socket.read(&mut byte))
                    .await
                    .unwrap()
                    .unwrap(),
                0
            );
        });
        assert_eq!(
            fetch(client(), &url, "0.2.0", Duration::from_millis(80)).await,
            Err(Error::Timeout)
        );
        server.await.unwrap();
    }
}

#[tokio::test]
async fn single_request_owner_deduplicates_cancels_and_can_retry_silent_failure() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Cancelled(Arc<AtomicBool>);
    impl Drop for Cancelled {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let guard = Cancelled(dropped.clone());
    let mut owner = pending(Box::pin(async move {
        let _guard = guard;
        std::future::pending().await
    }));
    owner.start(); // Does not replace or start a second request.
    assert!(!dropped.load(Ordering::SeqCst));
    assert!(
        tokio::time::timeout(Duration::from_millis(10), owner.next())
            .await
            .is_err()
    );
    drop(owner);
    assert!(dropped.load(Ordering::SeqCst));
    let mut owner = pending(Box::pin(async { Err(Error::Timeout) }));
    assert_eq!(owner.next().await, Status::default());
    assert!(owner.request.is_none());
    // A later manual request is accepted; it is dropped without polling/network.
    owner.start();
    assert!(owner.status().checking);
    drop(owner);
    let mut owner = pending(Box::pin(async { Ok(None) }));
    assert_eq!(
        owner.next().await,
        Status {
            up_to_date: true,
            ..Status::default()
        }
    );
}

#[tokio::test]
async fn closing_owner_cancels_an_active_http_body_without_waiting_for_the_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/latest", listener.local_addr().unwrap());
    let (started, ready) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            request.push(socket.read_u8().await.unwrap());
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\n\r\n{")
            .await
            .unwrap();
        started.send(()).unwrap();
        let mut byte = [0];
        // Dropping an unread HTTP response may close with FIN or RST.
        // Both prove cancellation; data, other errors and a timeout still fail.
        let closed = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut byte))
            .await
            .unwrap();
        match closed {
            Ok(0) => {}
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
            other => panic!("expected cancelled connection, got {other:?}"),
        }
    });
    let mut owner = pending(Box::pin(async move {
        fetch(client(), &url, "0.2.0", Duration::from_secs(8)).await
    }));
    tokio::select! {
        _=owner.next()=>panic!("stalled body completed"),
        result=tokio::time::timeout(Duration::from_secs(2),ready)=>result.unwrap().unwrap(),
    }
    drop(owner);
    server.await.unwrap();
}

#[test]
fn synthetic_update_sources_cannot_contact_a_remote_or_ambient_proxy_target() {
    for url in [
        "https://github.com/yam276/PTTStella/releases/latest",
        "http://localhost:1234/latest",
        "http://127.0.0.1/latest",
        "http://127.0.0.2:1234/latest",
        "http://user:secret@127.0.0.1:1234/latest",
        "http://127.0.0.1:1234/latest?redirect=remote",
        "file:///tmp/release.json",
    ] {
        assert!(Checker::loopback(url).is_err(), "{url}");
    }
    assert!(Checker::loopback("http://127.0.0.1:1234/latest").is_ok());
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(socket.read_u8().await.unwrap());
        assert!(bytes.len() < 16384);
    }
    let request = String::from_utf8(bytes).unwrap();
    let headers = request.to_ascii_lowercase();
    assert!(
        !headers.contains("authorization:")
            && !headers.contains("cookie:")
            && !headers.contains("referer:")
    );
    request
}

async fn within<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(3), future)
        .await
        .expect("bounded update fixture did not make progress")
}

#[tokio::test]
async fn native_worker_deduplicates_retries_locally_and_retains_a_valid_offer_on_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/latest", listener.local_addr().unwrap());
    let (ready, started) = tokio::sync::oneshot::channel();
    let (release, held) = tokio::sync::oneshot::channel();
    let (done, mut responses) = tokio::sync::mpsc::channel(1);
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_request(&mut socket).await;
        ready.send(()).unwrap();
        held.await.unwrap();
        socket
            .write_all(reply("429 Too Many Requests", "wait").as_bytes())
            .await
            .unwrap();
        drop(socket);
        done.send(1).await.unwrap();
        for (index, body) in [
            (
                2,
                r#"{"tag_name":"v999.0.0","draft":false,"prerelease":false,"html_url":"https://evil.invalid/"}"#,
            ),
            (3, "invalid JSON"),
        ] {
            let (mut socket, _) = listener.accept().await.unwrap();
            read_request(&mut socket).await;
            socket
                .write_all(reply("200 OK", body).as_bytes())
                .await
                .unwrap();
            drop(socket);
            done.send(index).await.unwrap();
        }
    });
    let mut worker = Worker::start(Checker::loopback(&endpoint).unwrap(), true).unwrap();
    let mut statuses = worker.updates();
    within(started).await.unwrap();
    for _ in 0..100 {
        assert!(!worker.request());
    }
    release.send(()).unwrap();
    assert_eq!(within(responses.recv()).await, Some(1));
    within(statuses.wait_for(|s| !s.checking)).await.unwrap();
    assert_eq!(*statuses.borrow(), Status::default());
    for index in [2, 3] {
        assert!(worker.request());
        assert_eq!(within(responses.recv()).await, Some(index));
        within(statuses.wait_for(|s| !s.checking && s.available.is_some()))
            .await
            .unwrap();
        assert_eq!(
            statuses.borrow().available.as_ref().unwrap().url,
            "https://github.com/yam276/PTTStella/releases/tag/v999.0.0"
        );
        assert!(!statuses.borrow().up_to_date);
    }
    worker.shutdown().await.unwrap();
    within(server).await.unwrap();
}

#[tokio::test]
async fn native_worker_shutdown_joins_and_closes_a_stalled_http_response() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/latest", listener.local_addr().unwrap());
    let (ready, started) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        read_request(&mut socket).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\n\r\n{")
            .await
            .unwrap();
        ready.send(()).unwrap();
        let mut byte = [0];
        match tokio::time::timeout(Duration::from_secs(2), socket.read(&mut byte))
            .await
            .unwrap()
        {
            Ok(0) => {}
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
            other => panic!("expected cancellation EOF/reset, got {other:?}"),
        }
    });
    let mut worker = Worker::start(Checker::loopback(&endpoint).unwrap(), true).unwrap();
    within(started).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), worker.shutdown())
        .await
        .unwrap()
        .unwrap();
    within(server).await.unwrap();
    worker.shutdown().await.unwrap(); // Idempotent, no detached handle.
}

#[tokio::test]
async fn cancellation_discards_a_check_queued_before_the_worker_gets_polled() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/latest", listener.local_addr().unwrap());
    let mut worker = Worker::start(Checker::loopback(&endpoint).unwrap(), false).unwrap();
    assert!(worker.request());
    worker.cancel();
    assert!(!worker.request());
    worker.shutdown().await.unwrap();
    assert!(
        tokio::time::timeout(Duration::from_millis(30), listener.accept())
            .await
            .is_err()
    );
}
