//! Synthetic round trip through the production connection owner.
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::TcpListener,
    time::{Duration, timeout},
};
use tokio_tungstenite::tungstenite::{
    Message,
    protocol::{CloseFrame, frame::coding::CloseCode},
};

pub(super) enum Scenario {
    Desktop { reload_settings: bool },
    Window,
    Login,
    Filters,
    Surface,
}

pub(super) fn filters_page(next: bool) -> Result<Vec<u8>, String> {
    let pages: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/filters.json"))
            .map_err(|e| e.to_string())?;
    crate::encoding::encode(
        pages[if next { "reading_next" } else { "reading" }]
            .as_str()
            .ok_or("missing filters fixture")?,
    )
    .map_err(|e| e.to_string())
}

async fn filters<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    for (expected, next) in [
        (b"\x1b[6~".as_slice(), true),
        (b"\x1b[5~".as_slice(), false),
    ] {
        if !matches!(socket.next().await,Some(Ok(Message::Binary(b))) if b.as_ref()==expected) {
            return Err("filters probe unexpected station input".into());
        }
        socket
            .send(Message::Binary(filters_page(next)?.into()))
            .await
            .map_err(|e| e.to_string())?;
    }
    match socket.next().await {
        Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
        _ => Err("filters probe expected Close without extra input".into()),
    }
}

pub(super) fn login_page() -> Result<Vec<u8>, String> {
    crate::encoding::encode("\x1b[2J\x1b[1;1HPTTStella 合成登入頁（本機測試）\x1b[21;1H請輸入代號，或以 guest 參觀，或以 new 註冊: ").map_err(|e|e.to_string())
}

pub(super) fn password_page() -> Result<Vec<u8>, String> {
    crate::encoding::encode("\x1b[21;1H請輸入代號，或以 guest 參觀，或以 new 註冊: StellaTest\x1b[22;1H請輸入您的密碼: ").map_err(|e|e.to_string())
}

async fn login<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"StellaTest\r")
    {
        return Err("synthetic login account mismatch".into());
    }
    let bytes = password_page()?;
    for chunk in bytes.chunks(3) {
        socket
            .send(Message::Binary(chunk.to_vec().into()))
            .await
            .map_err(|e| e.to_string())?;
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"synthetic+42\r")
    {
        return Err("synthetic login password mismatch".into());
    }
    socket
        .send(Message::Binary(navigation_page("main_menu")?.into()))
        .await
        .map_err(|e| e.to_string())?;
    match socket.next().await {
        Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
        _ => Err("synthetic login received extra input instead of Close".into()),
    }
}

pub(super) fn window_page(stage: usize) -> Result<Vec<u8>, String> {
    let name = match stage {
        0..=4 => "main_menu",
        5..=9 => "boards_caption",
        10..=14 => "favorites",
        15..=19 => "list_edge_cases",
        34..=36 => "board_entry",
        37..=39 => "list_edge_cases",
        _ if stage.is_multiple_of(2) => "boards_caption",
        _ => "list_edge_cases",
    };
    let mut bytes = navigation_page(name)?;
    if stage == 30 {
        bytes.extend_from_slice(
            &crate::encoding::encode("\x1b[7;1H  \x1b[4;1H●").map_err(|e| e.to_string())?,
        );
    }
    if (37..=39).contains(&stage) {
        bytes.extend_from_slice(
            format!("\x1b[18;1H 3\x1b[{};1H>", if stage == 38 { 5 } else { 4 }).as_bytes(),
        );
    }
    bytes.extend_from_slice(format!("\x1b[1;73Htest{stage:02}").as_bytes());
    bytes.extend_from_slice(if stage == 38 {
        b"\x1b[5;1H"
    } else if matches!(stage, 37 | 39) {
        b"\x1b[4;1H"
    } else if name == "board_entry" {
        b"\x1b[24;80H"
    } else if name == "main_menu" {
        b"\x1b[14;22H"
    } else if name == "boards_caption" && stage == 30 {
        b"\x1b[4;2H"
    } else if name == "boards_caption" {
        b"\x1b[7;2H"
    } else if name == "list_edge_cases" {
        b"\x1b[18;2H"
    } else {
        b"\x1b[5;2H"
    });
    Ok(bytes)
}

async fn window<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    for step in 0usize..40 {
        let expected: &[u8] = match step {
            37 => b"\x1b[B",
            38 => b"\x1b[A",
            39 => b"\x1b[D",
            34..=36 => b" ",
            30 => b"\x1b[H",
            31 => b"\x1b[B\x1b[B\x1b[B\x1b[B",
            32 => b"\r",
            33 => b"\x1b[D",
            20.. if step.is_multiple_of(2) => b"\r",
            20.. => b"\x1b[D",
            _ => match step % 5 {
                0 => b"\x1b[B",
                1 => b"\x1b[A",
                2 => b"\x1b[H",
                3 => b"\x1b[F",
                _ if step == 19 => b"\x1b[D",
                _ => b"\r",
            },
        };
        if !matches!(socket.next().await, Some(Ok(Message::Binary(bytes))) if bytes.as_ref() == expected)
        {
            return Err(format!("native-window Vim bytes differ at step {step}"));
        }
        // Home at the first item and consecutive movement may produce no redraw.
        if matches!(step, 30 | 31) {
            continue;
        }
        socket
            .send(Message::Binary(window_page(step + 1)?.into()))
            .await
            .map_err(|e| e.to_string())?;
    }
    match socket.next().await {
        Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
        Some(Ok(Message::Binary(bytes))) => Err(format!(
            "native-window extra input: {:02x?}",
            &bytes[..bytes.len().min(16)]
        )),
        other => Err(format!("native-window expected Close, received {other:?}")),
    }
}

pub(super) async fn serve(
    listener: TcpListener,
    bytes: Vec<u8>,
    scenario: Scenario,
) -> Result<(), String> {
    let deadline = if matches!(scenario, Scenario::Filters) {
        900
    } else if matches!(
        scenario,
        Scenario::Window | Scenario::Login | Scenario::Surface
    ) {
        300
    } else {
        90
    };
    timeout(Duration::from_secs(deadline), async {
        let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
        let mut socket = tokio_tungstenite::accept_async(stream)
            .await
            .map_err(|e| e.to_string())?;
        socket
            .send(Message::Binary(bytes.into()))
            .await
            .map_err(|e| e.to_string())?;
        if matches!(scenario, Scenario::Window) {
            return window(&mut socket).await;
        }
        if matches!(scenario, Scenario::Login) {
            return login(&mut socket).await;
        }
        if matches!(scenario, Scenario::Filters) {
            return filters(&mut socket).await;
        }
        if matches!(scenario, Scenario::Surface) {
            return match socket.next().await {
                Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
                _ => Err("surface probe received unexpected station input".into()),
            };
        }
        if matches!(
            scenario,
            Scenario::Desktop {
                reload_settings: true
            }
        ) {
            return match socket.next().await {
                Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
                _ => Err("settings reload probe received unexpected input".into()),
            };
        }
        let expected: &[&[u8]] = &[
            b"\x03",
            b"a",
            b"b",
            b"c",
            b"\xa4\xa4",
            b"\r", // A separate ordinary Enter immediately after the IME commit.
            b"\xa4\xa4",
            b"\xa4\xa4",
            b"A\rB",
            b"\x1b[A",
            b"\r",
            b"n",
            b"\r",
        ];
        for (index, expected) in expected.iter().enumerate() {
            match socket.next().await {
                Some(Ok(Message::Binary(bytes))) if bytes.as_ref() == *expected => {}
                _ => {
                    let reason = format!("unexpected loopback input at step {index}");
                    let _ = socket
                        .close(Some(CloseFrame {
                            code: CloseCode::Protocol,
                            reason: reason.clone().into(),
                        }))
                        .await;
                    return Err(reason);
                }
            }
            if index == 10 {
                socket
                    .send(Message::Binary(
                        b"\x1b[2J\x1b[22;1HConfirm synthetic action [y/N] n"
                            .to_vec()
                            .into(),
                    ))
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        navigation(&mut socket).await?;
        vim_navigation(&mut socket).await?;
        socket
            .send(Message::Binary(
                b"\x1b[2J\x1b[1;1HINPUT ROUND TRIP COMPLETE".to_vec().into(),
            ))
            .await
            .map_err(|e| e.to_string())?;
        socket
            .close(Some(CloseFrame {
                code: CloseCode::Policy,
                reason: "synthetic confirmation complete".into(),
            }))
            .await
            .map_err(|e| e.to_string())?;
        match socket.next().await {
            Some(Ok(Message::Close(_))) => Ok(()),
            _ => Err("loopback probe did not close cleanly".into()),
        }
    })
    .await
    .map_err(|_| "loopback probe timed out".to_owned())?
}

async fn vim_navigation<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    socket
        .send(Message::Binary(navigation_page("reading_middle")?.into()))
        .await
        .map_err(|e| e.to_string())?;
    let stages: &[(&[u8], &str)] = &[
        (b"h", "reading_middle"),
        (b"\x04", "reading_middle"),
        (b"jjj", "reading_middle"),
        (b"kkkkkkkkkkk", "reading_middle"),
        (b"\x1b[H", "reading_first"),
        (b"\x1b[F", "reading_last"),
        (b"\x1b[D", "list"),
        (b"\r", "reading_middle"),
        (b"\xa4\xa4", "reading_middle"),
        (b"\x1b[5~", "reading_first"),
    ];
    for (index, (expected, page)) in stages.iter().enumerate() {
        if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==*expected)
        {
            return Err(format!("unexpected Vim input at step {index}"));
        }
        // Give every stage a distinct revision, even for deliberately stationary replies.
        let mut bytes = navigation_page(page)?;
        bytes.extend_from_slice(format!("\x1b[1;75H{index:02}").as_bytes());
        bytes.extend_from_slice(if *page == "list" {
            b"\x1b[5;1H"
        } else {
            b"\x1b[24;80H"
        });
        socket
            .send(Message::Binary(bytes.into()))
            .await
            .map_err(|e| e.to_string())?;
    }
    if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"\x1b[C")
    {
        return Err("Vim completion did not restore native input".into());
    }
    Ok(())
}

fn navigation_page(name: &str) -> Result<Vec<u8>, String> {
    let pages: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/navigation.json"))
            .map_err(|e| e.to_string())?;
    crate::encoding::encode(
        pages[name]
            .as_str()
            .ok_or("missing synthetic navigation page")?,
    )
    .map_err(|e| e.to_string())
}
async fn navigation<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    socket
        .send(Message::Binary(navigation_page("list")?.into()))
        .await
        .map_err(|e| e.to_string())?;
    let mut shifted = navigation_page("list")?;
    shifted.extend_from_slice(b"\x1b[5;1H \x1b[4;1H>\x1b[4;1H");
    let mut next_item = navigation_page("list")?;
    next_item.extend_from_slice(b"\x1b[5;1H \x1b[6;1H>\x1b[6;1H");
    let stages: Vec<(&[u8], Vec<u8>)> = vec![
        (b"\x1b[C", shifted),
        (b"\x1b[B\x1b[B\r", navigation_page("reading_first")?),
        (b"\x1b[6~", navigation_page("reading_middle")?),
        (b"\x1b[5~", navigation_page("reading_first")?),
        (b"\x1b[6~", navigation_page("reading_middle")?),
        (b"\x1b[6~", navigation_page("reading_last")?),
        (b"\x1b[D", navigation_page("list")?),
        (b"\x1b[B", next_item),
        (b"\x1b[A", navigation_page("list")?),
        (
            b"\x1b[C",
            b"\x1b[2J\x1b[22;1HConfirm synthetic action [y/N]".to_vec(),
        ),
    ];
    for (index, (expected, page)) in stages.into_iter().enumerate() {
        if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected)
        {
            return Err(format!("unexpected navigation probe input at step {index}"));
        }
        socket
            .send(Message::Binary(page.into()))
            .await
            .map_err(|e| e.to_string())?;
    }
    if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"\x1b[C")
    {
        return Err("unknown screen received unintended navigation input".into());
    }
    Ok(())
}
