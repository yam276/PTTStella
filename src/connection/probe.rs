//! Synthetic round trip through the production connection owner.
pub(super) mod round_trip;
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
    Login2fa,
    Boards,
    Articles,
    Bookmarks,
    Mouse { boundary: bool },
    MouseSamples(MouseSampleProbe),
    Filters,
    Surface,
    ControlInput,
    EditorClipboard,
    EditorColor,
    ToastReconnect,
    PointerRows,
    ListBoundary,
    ArticleSwitch,
    InlinePages { pages: Vec<Vec<u8>>, delay_ms: u64 },
}

#[derive(Clone, Copy)]
pub enum MouseSampleProbe {
    Drag,
    RoundTrip,
    Motion,
    Wheel,
}

fn input_summary(bytes: &[u8]) -> String {
    if bytes == b"\x1b" {
        "Escape key".to_owned()
    } else {
        format!("{} bytes", bytes.len())
    }
}

async fn mouse_samples<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    case: MouseSampleProbe,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    match case {
        MouseSampleProbe::RoundTrip => {
            let mut gesture = round_trip::Gesture::default();
            let mut completed = false;
            for _ in 0..258 {
                let Some(Ok(Message::Binary(bytes))) = socket.next().await else {
                    return Err("round trip expected bounded mouse input before Close".into());
                };
                match gesture.accept(&bytes)? {
                    round_trip::Step::Continue => {}
                    round_trip::Step::Ready => socket
                        .send(Message::Binary(round_trip::READY.to_vec().into()))
                        .await
                        .map_err(|e| e.to_string())?,
                    round_trip::Step::Redraw => socket
                        .send(Message::Binary(round_trip::REDRAW.to_vec().into()))
                        .await
                        .map_err(|e| e.to_string())?,
                    round_trip::Step::Complete => {
                        completed = true;
                        break;
                    }
                }
            }
            if !completed {
                return Err("round trip exceeded its bounded gesture without release".into());
            }
            socket
                .send(Message::Binary(article_page("articles")?.into()))
                .await
                .map_err(|e| e.to_string())?;
        }
        MouseSampleProbe::Drag | MouseSampleProbe::Motion => {
            if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"\x1b[<0;5;5M")
            {
                return Err("sample probe expected the anchor left press".into());
            }
            if matches!(case, MouseSampleProbe::Motion) {
                if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"\x1b[<0;5;5m")
                {
                    return Err("motion probe expected the anchor release".into());
                }
                socket
                    .send(Message::Binary(b"\x1b[?1003h".to_vec().into()))
                    .await
                    .map_err(|e| e.to_string())?;
            }
            // Independent allowed coordinates for one horizontal native gesture.
            // The final cell is mandatory; intermediate cadence is OS dependent.
            let allowed: &[&[u8]] = if matches!(case, MouseSampleProbe::Drag) {
                &[
                    b"\x1b[<32;5;5M",
                    b"\x1b[<32;6;5M",
                    b"\x1b[<32;7;5M",
                    b"\x1b[<32;8;5M",
                    b"\x1b[<32;9;5M",
                    b"\x1b[<32;10;5M",
                    b"\x1b[<32;11;5M",
                ]
            } else {
                &[
                    b"\x1b[<35;5;5M",
                    b"\x1b[<35;6;5M",
                    b"\x1b[<35;7;5M",
                    b"\x1b[<35;8;5M",
                    b"\x1b[<35;9;5M",
                    b"\x1b[<35;10;5M",
                    b"\x1b[<35;11;5M",
                ]
            };
            let mut final_cell = false;
            let mut completed = false;
            for _ in 0..16 {
                let Some(Ok(Message::Binary(bytes))) = socket.next().await else {
                    return Err("sample probe expected bounded native cell motion".into());
                };
                if allowed.contains(&bytes.as_ref()) {
                    final_cell |= bytes.as_ref() == allowed[6];
                    if final_cell && matches!(case, MouseSampleProbe::Motion) {
                        completed = true;
                        break;
                    }
                } else if final_cell
                    && matches!(case, MouseSampleProbe::Drag)
                    && bytes.as_ref() == b"\x1b[<0;11;5m"
                {
                    completed = true;
                    break;
                } else {
                    return Err(format!(
                        "sample probe rejected unexpected input ({})",
                        input_summary(&bytes)
                    ));
                }
            }
            if !completed {
                return Err(
                    "sample probe never completed its final native cell and required edge".into(),
                );
            }
        }
        MouseSampleProbe::Wheel => {
            if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"\x1b[<65;5;5M")
            {
                return Err("wheel probe expected a downward native tick at its anchor".into());
            }
            socket
                .send(Message::Binary(
                    b"\x1b[?1002h\x1b[2;1H\x1b[2KWHEEL: now scroll UP"
                        .to_vec()
                        .into(),
                ))
                .await
                .map_err(|e| e.to_string())?;
            let mut up = false;
            for _ in 0..8 {
                match socket.next().await {
                    Some(Ok(Message::Binary(bytes))) if bytes.as_ref() == b"\x1b[<64;5;5M" => {
                        up = true;
                        break;
                    }
                    Some(Ok(Message::Binary(bytes))) if bytes.as_ref() == b"\x1b[<65;5;5M" => {}
                    _ => return Err("wheel probe expected bounded vertical ticks only".into()),
                }
            }
            if !up {
                return Err("wheel probe never received its upward tick".into());
            }
            socket
                .send(Message::Binary(article_page("articles")?.into()))
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    socket
        .send(Message::Binary(b"\x1b[?1001h".to_vec().into()))
        .await
        .map_err(|e| e.to_string())?;
    // A single native wheel gesture can already have a few ticks in flight.
    // Permit only bounded exact upward repeats; never a synthetic release.
    let mut repeats = 0;
    loop {
        match socket.next().await {
            Some(Ok(Message::Close(_))) => return socket.flush().await.map_err(|e| e.to_string()),
            Some(Ok(Message::Binary(bytes)))
                if matches!(case, MouseSampleProbe::Wheel)
                    && repeats < 7
                    && bytes.as_ref() == b"\x1b[<64;5;5M" =>
            {
                repeats += 1
            }
            _ => {
                return Err(
                    "sample probe expected Close without local keys or extra mouse edges".into(),
                );
            }
        }
    }
}

// Handwritten grid coordinates and exact SGR edges. The native runner also
// performs local-only selection and blocked clicks between these inputs.
async fn mouse<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    boundary: bool,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let steps: &[(&[u8], &[u8])] = if boundary {
        &[(b"\x1b[<0;5;5M", b"")]
    } else {
        &[
            (b"\x1b[<0;5;5M".as_slice(), b"".as_slice()),
            (b"\x1b[<0;5;5m", b"\x1b[?1001h"),
            (b"\r", b"\x1b[?1000h"),
            (b"\x1b[<2;5;5M", b""),
            (b"\x1b[<2;5;5m", b""),
            (
                b"\x1b[<1;5;5M",
                b"\x1b[?2026h\x1b[?1000l\x1b[?1000h\x1b[?2026l",
            ),
        ]
    };
    for &(expected, reply) in steps {
        if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected)
        {
            return Err(format!("native mouse input differs from {expected:02x?}"));
        }
        if !reply.is_empty() {
            socket
                .send(Message::Binary(reply.to_vec().into()))
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    match socket.next().await {
        Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
        _ => Err(
            "native mouse expected Close without a cancelled release or duplicate local input"
                .into(),
        ),
    }
}

pub(super) fn switch_page(article: char, second: bool) -> Result<Vec<u8>, String> {
    let first = if second { 23 } else { 1 };
    let last = first + 22;
    let mut page = String::from("\x1b[?2026h\x1b[2J\x1b[H");
    for (row, line) in (first..=last).enumerate() {
        page.push_str(&format!(
            "\x1b[{};1HARTICLE {article} line {line:03}",
            row + 1
        ));
    }
    page.push_str(&format!("\x1b[24;1H\x1b[30;47m\x1b[2K瀏覽 第 {}/2 頁 ({}%) 目前顯示: 第 {first}~{last} 行 (y)回應(X)推文(h)說明\x1b[0m\x1b[24;80H", if second {2} else {1}, if second {100} else {50}));
    page.push_str("\x1b[?2026l");
    crate::encoding::encode(&page).map_err(|e| e.to_string())
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

pub(super) fn board_page(name: &str) -> Result<Vec<u8>, String> {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/board-switch.json"))
            .map_err(|error| error.to_string())?;
    crate::encoding::encode(
        data["screens"][name]
            .as_str()
            .ok_or("missing board-switch scene")?,
    )
    .map_err(|error| error.to_string())
}

pub(super) fn article_page(name: &str) -> Result<Vec<u8>, String> {
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/fixtures/article-open.json"))
            .map_err(|e| e.to_string())?;
    let mut page = crate::encoding::encode(
        data["screens"][name]
            .as_str()
            .ok_or("missing article-open scene")?,
    )
    .map_err(|e| e.to_string())?;
    // Destination probes publish a complete one-page synthetic article.
    // Long-article loading has its own peer with exact paging expectations.
    if name.starts_with("reading_") {
        page.extend(crate::encoding::encode("\x1b[24;1H\x1b[30;47m\x1b[2K瀏覽 第 1/1 頁 (100%) 目前顯示: 第 1~23 行 (h)說明 (←)離開\x1b[0m\x1b[24;80H").map_err(|e|e.to_string())?);
    }
    Ok(page)
}
const ARTICLE_FIRST_OPEN: &[(&[u8], &str)] = &[
    (b"\r", "boards"),
    (b"s", "board_prompt"),
    (b"Stella\r", "selected"),
    (b"\x1b[C", "articles"),
    (b"#", "aid_prompt"),
    (b"00000101\r", "articles"),
    (b"Q", "information"),
    (b" ", "articles"),
    (b"\x1b[C", "reading"),
];
/// True means the peer already completed Close while the delayed reply waited.
async fn article_steps<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    steps: &[(&[u8], &str)],
) -> Result<bool, String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    for &(expected, scene) in steps {
        if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected)
        {
            return Err(format!("article probe input differs before {scene}"));
        }
        if scene == "information_four" {
            tokio::select! {
                _=tokio::time::sleep(Duration::from_secs(3))=>{},
                incoming=socket.next()=>return match incoming {
                    Some(Ok(Message::Close(_)))=>socket.flush().await.map(|()|true).map_err(|e|e.to_string()),
                    _=>Err("article probe expected Close while query was delayed".into()),
                },
            }
        }
        socket
            .send(Message::Binary(article_page(scene)?.into()))
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(false)
}
async fn articles<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    bookmarks: bool,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    if article_steps(socket, ARTICLE_FIRST_OPEN).await? {
        return Ok(());
    }
    if !bookmarks
        && article_steps(
            socket,
            &[
                (b"\x1b[D", "articles"),
                (b"#", "aid_prompt"),
                (b"00000102\r", "articles"),
                (b"Q", "information_two"),
                (b" ", "articles"),
                (b"\x1b[C", "reading_two"),
                (b"\x1b[D", "articles"),
                (b"#", "aid_prompt"),
                (b"00000103\r", "missing"),
                (b" ", "articles"), // Only the user's explicit pause recovery is accepted.
                (b"#", "aid_prompt"),
                (b"00000104\r", "articles"),
                (b"Q", "information_four"),
            ],
        )
        .await?
    {
        return Ok(());
    }
    match socket.next().await {
        Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
        _ => Err("article probe expected Close without a late query continuation".into()),
    }
}

async fn boards<S>(socket: &mut tokio_tungstenite::WebSocketStream<S>) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    for (step, (expected, scene)) in [
        (b"\r".as_slice(), "boards"),
        (b"s", "prompt"),
        (b"Stella\r", "selected"),
        (b"\x1b[C", "articles"),
        (b"\x1b[D", "boards"),
        (b"s", "prompt"),
        (b"Other\r", "other_articles"),
        (b"\x1b[D", "boards"),
    ]
    .into_iter()
    .enumerate()
    {
        if !matches!(socket.next().await,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected)
        {
            return Err(format!("board switch probe input differs at step {step}"));
        }
        if step == 7 {
            tokio::select! {
                _=tokio::time::sleep(Duration::from_secs(3))=>{},
                incoming=socket.next()=>return match incoming {
                    Some(Ok(Message::Close(_)))=>socket.flush().await.map_err(|e|e.to_string()),
                    _=>Err("board switch probe expected cancellation or Close while waiting".into()),
                },
            }
        }
        socket
            .send(Message::Binary(board_page(scene)?.into()))
            .await
            .map_err(|error| error.to_string())?;
    }
    match socket.next().await {
        Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|error| error.to_string()),
        _ => Err("board switch probe expected Close without extra steps".into()),
    }
}

pub(super) fn password_page() -> Result<Vec<u8>, String> {
    crate::encoding::encode("\x1b[21;1H請輸入代號，或以 guest 參觀，或以 new 註冊: StellaTest\x1b[22;1H請輸入您的密碼: ").map_err(|e|e.to_string())
}

pub(super) fn verification_page(rejected: bool) -> Result<Vec<u8>, String> {
    crate::encoding::encode(&format!("\x1b[2J\x1b[HPTTStella synthetic verification{}\x1b[22;1H請輸入兩階段(2FA)驗證碼(6位限時數字 或8位救援碼): ", if rejected { "\x1b[21;1H驗證碼錯誤！請確認後重新輸入。" } else { "" })).map_err(|e|e.to_string())
}

async fn login<S>(
    socket: &mut tokio_tungstenite::WebSocketStream<S>,
    verification: bool,
) -> Result<(), String>
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
    if verification {
        for (rejected, expected) in [(false, b"123456\r".as_slice()), (true, b"ABCDEFGH\r")] {
            let page = verification_page(rejected)?;
            for chunk in page.chunks(5) {
                socket
                    .send(Message::Binary(chunk.to_vec().into()))
                    .await
                    .map_err(|e| e.to_string())?;
            }
            let mut received = Vec::new();
            while received.len() < expected.len() {
                match socket.next().await {
                    Some(Ok(Message::Binary(bytes)))
                        if received.len() + bytes.len() <= expected.len() =>
                    {
                        received.extend_from_slice(&bytes)
                    }
                    _ => return Err("synthetic verification input mismatch".into()),
                }
            }
            if received != expected {
                return Err("synthetic verification code mismatch".into());
            }
        }
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
    if (40..=45).contains(&stage) {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/navigation-actions.json"))
                .map_err(|e| e.to_string())?;
        let name = if stage == 45 { "empty_boards" } else { "help" };
        let mut bytes = crate::encoding::encode(
            data["screens"][name]
                .as_str()
                .ok_or("missing action fixture")?,
        )
        .map_err(|e| e.to_string())?;
        bytes.extend_from_slice(format!("\x1b[1;73Htest{stage:02}").as_bytes());
        let row = match stage {
            41 => 4,
            44 => 5,
            _ => 3,
        };
        if stage == 45 {
            bytes.extend_from_slice(b"\x1b[24;80H");
        } else {
            bytes.extend_from_slice(format!("\x1b[3;1H \x1b[{row};1H>\x1b[{row};1H").as_bytes());
        }
        return Ok(bytes);
    }
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
    for step in 0usize..46 {
        let expected: &[u8] = match step {
            40 => b"\x1b[B",
            41 => b"\x1b[A",
            42 => b"\x1b[H",
            43 => b"\x1b[F",
            44 | 45 => b"\x1b[D",
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
        let page = window_page(step + 1)?;
        let middle = page.len() / 2;
        // Exercise the actual WebView/session with split synchronized output,
        // including ignored ESC intermediates and an oversized CSI tail.
        let begin = [
            b"\x1b[?2026h\x1b(0\x1b[".as_slice(),
            &[b'1'; 70],
            b"m",
            &page[..middle],
        ]
        .concat();
        socket
            .send(Message::Binary(begin.into()))
            .await
            .map_err(|e| e.to_string())?;
        tokio::time::sleep(Duration::from_millis(120)).await;
        socket
            .send(Message::Binary(
                [&page[middle..], b"\x1b[?2026l"].concat().into(),
            ))
            .await
            .map_err(|e| e.to_string())?;
    }
    match socket.next().await {
        Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
        Some(Ok(Message::Binary(bytes))) => Err(format!(
            "native-window extra input ({})",
            input_summary(&bytes)
        )),
        other => Err(format!("native-window expected Close, received {other:?}")),
    }
}

pub(super) async fn serve(
    listener: TcpListener,
    bytes: Vec<u8>,
    scenario: Scenario,
) -> Result<(), String> {
    if matches!(scenario, Scenario::ToastReconnect) {
        return timeout(Duration::from_secs(900), async {
            for stage in 1..=2 {
                let (stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
                let mut socket = tokio_tungstenite::accept_async(stream)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut wire = bytes.clone();
                wire.extend(
                    format!("\x1b[3;1HSESSION {stage}/2 - no login or replay allowed").into_bytes(),
                );
                socket
                    .send(Message::Binary(wire.into()))
                    .await
                    .map_err(|e| e.to_string())?;
                if stage == 1 {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    socket
                        .send(Message::Close(Some(CloseFrame {
                            code: CloseCode::Normal,
                            reason: "Synthetic reconnect test".into(),
                        })))
                        .await
                        .map_err(|e| e.to_string())?;
                }
                match socket.next().await {
                    Some(Ok(Message::Close(_))) => {
                        socket.flush().await.map_err(|e| e.to_string())?
                    }
                    _ => {
                        return Err(
                            "reconnect probe forbids login, replay and all station input".into(),
                        );
                    }
                }
            }
            Ok(())
        })
        .await
        .map_err(|_| "reconnect probe deadline expired".to_owned())?;
    }
    let deadline = if matches!(
        scenario,
        Scenario::Filters
            | Scenario::EditorColor
            | Scenario::ArticleSwitch
            | Scenario::MouseSamples(MouseSampleProbe::RoundTrip)
    ) {
        900
    } else if matches!(
        scenario,
        Scenario::Window
            | Scenario::Login
            | Scenario::Login2fa
            | Scenario::Surface
            | Scenario::ControlInput
            | Scenario::EditorClipboard
            | Scenario::PointerRows
            | Scenario::ListBoundary
            | Scenario::InlinePages { .. }
            | Scenario::Boards
            | Scenario::Articles
            | Scenario::Bookmarks
            | Scenario::Mouse { .. }
            | Scenario::MouseSamples(_)
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
        if let Scenario::InlinePages {pages,delay_ms} = scenario {
            let started=std::time::Instant::now();
            for (index, second) in pages.into_iter().enumerate() {
            let received=socket.next().await;
            if !matches!(&received,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"\x1b[6~") {
                return Err(format!("cached reading probe expected exactly one PageDown; received {received:?}"));
            }
            eprintln!("synthetic article page {} requested at {}ms",index+2,started.elapsed().as_millis());
            // Each response is deliberately fragmented; local history must not refetch.
            // Historical browsing must not send PageUp or refetch old images.
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            let split=second.len()/2;
            socket.send(Message::Binary(second[..split].to_vec().into())).await.map_err(|e|e.to_string())?;
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            socket.send(Message::Binary(second[split..].to_vec().into())).await.map_err(|e|e.to_string())?;
            }
            return match socket.next().await {
                Some(Ok(Message::Close(_)))=>socket.flush().await.map_err(|e|e.to_string()),
                _=>Err("cached reading must browse history locally and then Close without further station bytes".into()),
            };
        }
        if let Scenario::Mouse { boundary } = scenario {
            return mouse(&mut socket, boundary).await;
        }
        if let Scenario::MouseSamples(case) = scenario {
            let result=mouse_samples(&mut socket, case).await;
            if result.is_err() && matches!(case,MouseSampleProbe::RoundTrip) {
                // Failure stays visible immediately; this run still returns its error.
                let _=socket.send(Message::Binary(b"\x1b[2;1H\x1b[2K\x1b[1;31mFAILED: gesture rejected; close and restart this test\x1b[0m\x1b[24;80H".to_vec().into())).await;
            }
            return result;
        }
        if matches!(scenario, Scenario::Boards) {
            return boards(&mut socket).await;
        }
        if matches!(scenario, Scenario::Articles | Scenario::Bookmarks) {
            return articles(&mut socket, matches!(scenario, Scenario::Bookmarks)).await;
        }
        if matches!(scenario, Scenario::Login | Scenario::Login2fa) {
            return login(&mut socket, matches!(scenario, Scenario::Login2fa)).await;
        }
        if matches!(scenario, Scenario::Filters) {
            return filters(&mut socket).await;
        }
        if matches!(scenario, Scenario::PointerRows) {
            let stages = [
                (b"\x1b[B\r".as_slice(), b"\x1b[0m\x1b[2J\x1b[HPTTStella synthetic settings; Left returns".to_vec()),
                (b"\x1b[D", pointer_menu()?),
                (b"\r", navigation_page("favorites")?),
                (b"\x1b[A\r", navigation_page("list")?),
                (b"\x1b[6~", navigation_page("list_dense")?),
                (b"\x1b[5~", navigation_page("list")?),
                (b"\x1b[B\r", {
                    let mut page = navigation_page("reading_first")?;
                    page.extend(crate::encoding::encode("\x1b[24;1H\x1b[30;47m\x1b[2K瀏覽 第 1/1 頁 (100%) 目前顯示: 第 1~23 行 (h)說明 (←)離開\x1b[0m\x1b[24;80H").map_err(|e|e.to_string())?);
                    page
                }),
                (b"\x1b[D", navigation_page("list")?),
            ];
            for (expected, page) in stages {
                if !matches!(socket.next().await, Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected) {
                    return Err("row/edge probe received unexpected station input".into());
                }
                socket.send(Message::Binary(page.into())).await.map_err(|e|e.to_string())?;
            }
            return match socket.next().await {
                Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e|e.to_string()),
                _ => Err("row/edge probe expected Close without extra input".into()),
            };
        }
        if matches!(scenario, Scenario::ArticleSwitch) {
            for (expected, article, second) in [
                (b"\x1b[6~".as_slice(), 'A', true),
                (b"\x1b[H", 'A', false),
                (b"\x1b[A", 'B', false),
                (b"\x1b[6~", 'B', true),
                (b"\x1b[B", 'C', false),
                (b"\x1b[6~", 'C', true),
            ] {
                if !matches!(socket.next().await, Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected) {
                    return Err("article switch probe received unexpected input".into());
                }
                let page = switch_page(article, second)?;
                let split=page.len()/2;
                socket.send(Message::Binary(page[..split].to_vec().into())).await.map_err(|e|e.to_string())?;
                tokio::time::sleep(Duration::from_millis(80)).await;
                socket.send(Message::Binary(page[split..].to_vec().into())).await.map_err(|e|e.to_string())?;
            }
            return match socket.next().await {
                Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e|e.to_string()),
                _ => Err("article switch probe expected Close without extra input".into()),
            };
        }
        if matches!(scenario, Scenario::ListBoundary) {
            for (expected, response) in [
                (b"\x1b[6~".as_slice(), None),
                (b"\x0c\x0c", Some(navigation_page("list")?)),
                (b"\x1b[5~", Some(navigation_page("list_dense")?)),
                (b"\r", Some(navigation_page("reading_first")?)),
                (b"\x1b[D", Some(navigation_page("list")?)),
            ] {
                if !matches!(socket.next().await, Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected) {
                    return Err("list boundary probe received unexpected input".into());
                }
                if let Some(page) = response {
                    socket.send(Message::Binary(page.into())).await.map_err(|e|e.to_string())?;
                }
            }
            return match socket.next().await {
                Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e|e.to_string()),
                _ => Err("list boundary probe expected Close without extra input".into()),
            };
        }
        if matches!(scenario, Scenario::EditorColor) {
            let mut editor=crate::editor_color::fixture::Editor::new(&["P\x1b[32m中\x1b[0m文\x1b[34mQ","","第三行"],true,false,true);
            for _ in 0..4096 {
                match socket.next().await {
                    Some(Ok(Message::Binary(bytes)))=>{
                        if bytes.len()>4096{return Err("editor colour probe input too large".into());}
                        editor.accept(&bytes);
                        socket.send(Message::Binary(editor.wire().into())).await.map_err(|e|e.to_string())?;
                    },
                    Some(Ok(Message::Close(_)))=>{
                        socket.flush().await.map_err(|e|e.to_string())?;
                        let expected=["P\x1b[32m\x1b[0;1;31;40m中\x1b[0m\x1b[0;1;31;40m文\x1b[34m\x1b[0;1;31;40mQ\x1b[m","","\x1b[0;1;31;40m第三行\x1b[m"];
                        let expected=expected.iter().map(|s|crate::encoding::encode(s).map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?;
                        return if editor.lines==expected && editor.preview && !editor.insert {Ok(())}else{Err("editor colour probe document/mode differs from independent expected".into())};
                    },
                    _=>return Err("editor colour probe expected bounded input or Close".into()),
                }
            }
            return Err("editor colour probe exceeded input bound".into());
        }
        if matches!(scenario, Scenario::EditorClipboard) {
            for (index, expected) in [b"\x16".as_slice(), b"\x16", b"ANSI clipboard check:\x15[31m\xa4\xa4\x15[0m\rEND", b"\x16"].into_iter().enumerate() {
                if !matches!(socket.next().await, Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected) {
                    return Err(format!("editor clipboard probe rejected step {}", index + 1));
                }
                let preview = if index == 0 || index == 3 {
                    "\x1b[5;1H\x1b[2K\x1b[1;31mANSI PREVIEW ON\x1b[0m"
                } else {
                    "\x1b[5;1H\x1b[2K*[1;31mANSI PREVIEW OFF*[m"
                };
                socket.send(Message::Binary(format!("\x1b[1;1H\x1b[2KEDITOR CLIPBOARD {}/4 VERIFIED{preview}\x1b[3;1H",index+1).into_bytes().into()))
                    .await.map_err(|e|e.to_string())?;
            }
            return match socket.next().await {
                Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e|e.to_string()),
                _ => Err("editor clipboard probe expected Close without extra input".into()),
            };
        }
        if matches!(scenario, Scenario::ControlInput) {
            for (index, expected) in [24_u8, 26, 1].into_iter().enumerate() {
                if !matches!(socket.next().await, Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==[expected]) {
                    return Err(format!("control input probe rejected step {}", index + 1));
                }
                socket.send(Message::Binary(format!("\x1b[1;1H\x1b[2KCONTROL INPUT {}/3 VERIFIED",index+1).into_bytes().into()))
                    .await.map_err(|e| e.to_string())?;
            }
            return match socket.next().await {
                Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
                _ => Err("control input probe expected Close without extra input".into()),
            };
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

pub(super) fn navigation_page(name: &str) -> Result<Vec<u8>, String> {
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
pub(super) fn pointer_menu() -> Result<Vec<u8>, String> {
    let mut page = navigation_page("main_menu")?;
    page.extend(crate::encoding::encode("\x1b[15;1H\x1b[0m\x1b[2K                      (U)ser         【 個人設定區 】\x1b[14;22H").map_err(|e|e.to_string())?);
    Ok(page)
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
        (b"\x1b[6~", next_item),
        (b"\x1b[5~", navigation_page("list")?),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_probe_diagnostics_do_not_echo_unexpected_text_or_binary_input() {
        assert_eq!(input_summary(b"synthetic-secret"), "16 bytes");
        assert_eq!(input_summary(&[0xff; 32768]), "32768 bytes");
        assert_eq!(input_summary(b"\x1b"), "Escape key");
    }
    #[tokio::test]
    async fn article_probe_close_during_delayed_query_does_not_wait_for_another_frame() {
        use tokio_tungstenite::{WebSocketStream, tungstenite::protocol::Role};
        let (server, client) = tokio::io::duplex(64 * 1024);
        let mut server = WebSocketStream::from_raw_socket(server, Role::Server, None).await;
        let mut client = WebSocketStream::from_raw_socket(client, Role::Client, None).await;
        let owner = tokio::spawn(async move { articles(&mut server, false).await });
        // This case tests the close lifetime, independently of reducer identity
        // tests. The peer still validates each input before replying.
        for bytes in [
            b"\r".as_slice(),
            b"s",
            b"Stella\r",
            b"\x1b[C",
            b"#",
            b"00000101\r",
            b"Q",
            b" ",
            b"\x1b[C",
            b"\x1b[D",
            b"#",
            b"00000102\r",
            b"Q",
            b" ",
            b"\x1b[C",
            b"\x1b[D",
            b"#",
            b"00000103\r",
            b" ",
            b"#",
            b"00000104\r",
        ] {
            client
                .send(Message::Binary(bytes.to_vec().into()))
                .await
                .unwrap();
            assert!(matches!(
                timeout(Duration::from_secs(1), client.next())
                    .await
                    .unwrap(),
                Some(Ok(Message::Binary(_)))
            ));
        }
        client
            .send(Message::Binary(b"Q".to_vec().into()))
            .await
            .unwrap();
        client.send(Message::Close(None)).await.unwrap();
        assert!(matches!(
            timeout(Duration::from_secs(1), client.next())
                .await
                .unwrap(),
            Some(Ok(Message::Close(_)))
        ));
        timeout(Duration::from_secs(1), owner)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
}
