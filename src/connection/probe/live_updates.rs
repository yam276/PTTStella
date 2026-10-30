//! Self-authored loopback article with two independently scripted append cycles.
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};
pub(in crate::connection) fn page(
    first: usize,
    last: usize,
    total: usize,
    origin: Option<&str>,
) -> Result<Vec<u8>, String> {
    let mut text = String::from("\x1b[0m\x1b[2J\x1b[H");
    for (row, line) in (first..=last).enumerate() {
        let body = match line {
            1 => "作者 Stella (local fixture)                                   看板 Stella"
                .to_owned(),
            2 => "標題 [測試] Live replies - local only".to_owned(),
            3 => "時間 Thu Oct 29 12:00:00 2026".to_owned(),
            6 => origin.map_or_else(
                || "local image row".to_owned(),
                |o| format!("{o}/colours.png"),
            ),
            40.. => format!(
                "\x1b[1;37m推 \x1b[1;33mAlice\x1b[0;33m: REPLY {line:04}\x1b[0m\x1b[{};68H10/29 12:34",
                row + 1
            ),
            _ => format!("ROW {line:04} - stable article content"),
        };
        text.push_str(&format!("\x1b[{};1H{body}", row + 1));
    }
    let percent = last * 100 / total;
    text.push_str(&format!("\x1b[24;1H\x1b[30;47m\x1b[2K瀏覽 第 1/3 頁 ({percent}%) 目前顯示: 第 {first}~{last} 行 (y)回應(X)推文(h)說明\x1b[0m\x1b[24;80H"));
    crate::encoding::encode(&text).map_err(|e| e.to_string())
}
fn information() -> Result<Vec<u8>, String> {
    crate::encoding::encode(&format!("\x1b[0m\x1b[2J\x1b[19;1H┌{}┐\x1b[20;1H│ 文章代碼(AID): \x1b[1m#00000101\x1b[0m (Stella) [ptt.cc] local\x1b[20;77H│\x1b[21;1H│ 文章網址: synthetic\x1b[21;77H│\x1b[22;1H│ 這一篇文章值 0 Ptt幣\x1b[22;77H│\x1b[23;1H└──────────────────────── [再按 Q 切換價格列表]  ─┘\x1b[24;1H\x1b[37;44m\x1b[2K 請按任意鍵繼續 \x1b[0m\x1b[24;80H","─".repeat(37))).map_err(|e|e.to_string())
}
pub(super) async fn run<S>(
    socket: &mut WebSocketStream<S>,
    origin: Option<&str>,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let started = std::time::Instant::now();
    let mut stages = vec![(b"\x1b[6~".to_vec(), page(23, 45, 45, origin)?)];
    for (old_first, old_last, total) in [(23, 45, 48), (26, 48, 51)] {
        stages.extend([
            (b"Q".to_vec(), information()?),
            (b" ".to_vec(), super::article_page("articles")?),
            (b"#".to_vec(), super::article_page("aid_prompt")?),
            (b"00000101\r".to_vec(), super::article_page("articles")?),
            (b"Q".to_vec(), super::article_page("information")?),
            (b" ".to_vec(), super::article_page("articles")?),
            (b"\x1b[C".to_vec(), page(1, 23, total, origin)?),
            (
                format!(":{old_first}\r").into_bytes(),
                page(old_first, old_last, total, origin)?,
            ),
            (b"\x1b[6~".to_vec(), page(total - 22, total, total, origin)?),
        ]);
    }
    for (index, (expected, response)) in stages.into_iter().enumerate() {
        let received = socket.next().await;
        if !matches!(&received,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected) {
            eprintln!("live updates mismatch at {index}: {received:?}");
            return Err(format!(
                "live updates step {index} expected {} bytes, received {received:?}",
                expected.len()
            ));
        }
        eprintln!(
            "live updates peer step {index} verified at {}ms",
            started.elapsed().as_millis()
        );
        let split = response.len() / 2;
        socket
            .send(Message::Binary(response[..split].to_vec().into()))
            .await
            .map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        socket
            .send(Message::Binary(response[split..].to_vec().into()))
            .await
            .map_err(|e| e.to_string())?;
    }
    // A human may leave the opt-in timer running while examining the frame.
    // Unchanged checks have the same exact verified path and no PageDown.
    for cycle in 0..64 {
        let received = socket.next().await;
        if matches!(&received, Some(Ok(Message::Close(_)))) {
            return socket.flush().await.map_err(|e| e.to_string());
        }
        if !matches!(&received,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==b"Q") {
            return Err(format!(
                "unchanged refresh expected Q or Close; received {received:?}"
            ));
        }
        socket
            .send(Message::Binary(information()?.into()))
            .await
            .map_err(|e| e.to_string())?;
        for (expected, response) in [
            (b" ".as_slice(), super::article_page("articles")?),
            (b"#", super::article_page("aid_prompt")?),
            (b"00000101\r", super::article_page("articles")?),
            (b"Q", super::article_page("information")?),
            (b" ", super::article_page("articles")?),
            (b"\x1b[C", page(1, 23, 51, origin)?),
            (b":29\r", page(29, 51, 51, origin)?),
        ] {
            let received = socket.next().await;
            if !matches!(&received,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected) {
                return Err(format!("unchanged refresh {cycle} rejected {received:?}"));
            }
            socket
                .send(Message::Binary(response.into()))
                .await
                .map_err(|e| e.to_string())?;
        }
        eprintln!("live updates unchanged cycle {cycle} verified");
    }
    Err("live updates exceeded 64 unchanged checks".into())
}

/// Cancellation is a local command. A delayed station query may still arrive,
/// but the peer must receive no dismissal/re-entry bytes after the owned Q.
pub(super) async fn cancel<S>(
    socket: &mut WebSocketStream<S>,
    origin: Option<&str>,
) -> Result<(), String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    for (expected, response) in [
        (b"\x1b[6~".as_slice(), page(23, 45, 45, origin)?),
        (b"Q", information()?),
    ] {
        let received = socket.next().await;
        if !matches!(&received,Some(Ok(Message::Binary(bytes))) if bytes.as_ref()==expected) {
            return Err(format!(
                "cancel probe expected {} bytes, received {received:?}",
                expected.len()
            ));
        }
        eprintln!("live cancellation peer accepted {} bytes", expected.len());
        if expected == b"Q" {
            tokio::time::sleep(std::time::Duration::from_secs(6)).await;
        }
        socket
            .send(Message::Binary(response.into()))
            .await
            .map_err(|e| e.to_string())?;
    }
    match socket.next().await {
        Some(Ok(Message::Close(_))) => socket.flush().await.map_err(|e| e.to_string()),
        other => Err(format!("cancelled query must not continue: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixture_uses_real_push_styles_and_constant_identity_with_independent_counts() {
        let mut screen = crate::screen::Screen::new(80, 24).unwrap();
        for (first, last, total, count) in [
            (1, 23, 45, 0),
            (23, 45, 45, 6),
            (26, 48, 48, 9),
            (29, 51, 51, 12),
        ] {
            screen.feed(&page(first, last, total, Some("http://127.0.0.1:9876")).unwrap());
            assert_eq!(crate::filters::pushes(&screen.frame()).len(), count);
            assert!(
                matches!(crate::navigation::analyze(&screen.frame()),crate::navigation::Context::ArticleReading{first_line,last_line,..} if first_line==first&&last_line==last)
            );
        }
    }
}
