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
