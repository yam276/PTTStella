//! Independent synthetic post editor. Faults process only a documented command
//! prefix and forbid retries; no branch accepts a publish/save-menu choice.
use super::navigation_page;
use crate::editor_source::fixture::Native;
use futures_util::{SinkExt, StreamExt};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    time::{Duration, sleep},
};
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftFailure {
    /// Local-editor storage checks may read the template but never replace it.
    LocalOnly,
    ChangedTemplate,
    AfterTitle,
    AfterDelete,
    AfterWrite,
    Disconnect,
    CloseCapture,
    CloseTransfer,
}
fn initial() -> Native {
    Native::new(
        &["看板預設格式", "\x1b[32m請依格式填寫\x1b[m", ""],
        "[測試] 範本標題",
        true,
    )
}
fn expected(native: &Native, case: DraftFailure, injected: bool) -> Result<(), String> {
    let (title, lines) = match case {
        DraftFailure::ChangedTemplate => (
            "[測試] 範本標題",
            vec!["看板預設格式（遠端修改）", "\x1b[32m請依格式填寫\x1b[m", ""],
        ),
        DraftFailure::AfterTitle => (
            "本機草稿測試",
            vec!["看板預設格式", "\x1b[32m請依格式填寫\x1b[m", ""],
        ),
        DraftFailure::AfterDelete => ("本機草稿測試", vec!["\x1b[32m請依格式填寫\x1b[m", ""]),
        DraftFailure::AfterWrite | DraftFailure::Disconnect => {
            ("本機草稿測試", vec!["第一行中文", ""])
        }
        DraftFailure::LocalOnly | DraftFailure::CloseCapture | DraftFailure::CloseTransfer => (
            "[測試] 範本標題",
            vec!["看板預設格式", "\x1b[32m請依格式填寫\x1b[m", ""],
        ),
    };
    let lines = lines
        .iter()
        .map(|s| crate::encoding::encode(s).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let entered = match case {
        DraftFailure::LocalOnly | DraftFailure::CloseCapture => native.menu_count >= 1,
        DraftFailure::CloseTransfer => native.menu_count >= 2,
        _ => injected,
    };
    if !entered || native.title != title || native.editor.lines != lines {
        return Err(format!(
            "draft failure {case:?} did not preserve the independently expected title/body or enter its intended boundary"
        ));
    }
    Ok(())
}
pub(super) async fn run(
    socket: &mut WebSocketStream<TcpStream>,
    slow: bool,
    case: Option<DraftFailure>,
) -> Result<(), String> {
    sleep(Duration::from_millis(120)).await;
    socket
        .send(Message::Binary(navigation_page("main_menu")?.into()))
        .await
        .map_err(|e| e.to_string())?;
    sleep(Duration::from_millis(120)).await;
    socket
        .send(Message::Binary(navigation_page("list")?.into()))
        .await
        .map_err(|e| e.to_string())?;
    match socket.next().await {
        Some(Ok(Message::Close(_))) if case.is_none() => {
            return socket.flush().await.map_err(|e| e.to_string());
        }
        Some(Ok(Message::Binary(bytes))) if bytes.as_ref() == [16] => {}
        _ => return Err("draft probe expected explicit post start or Close".into()),
    }
    let mut native = initial();
    let mut injected = false;
    let mut interrupted = false;
    let mut packets = 0usize;
    let mut received = 0usize;
    socket
        .send(Message::Binary(native.wire().into()))
        .await
        .map_err(|e| e.to_string())?;
    while let Some(message) = socket.next().await {
        match message.map_err(|e| e.to_string())? {
            Message::Close(_) => {
                socket.flush().await.map_err(|e| e.to_string())?;
                return case.map_or(Ok(()), |case| expected(&native, case, injected));
            }
            Message::Binary(bytes) => {
                packets = packets.saturating_add(1);
                received = received.saturating_add(bytes.len());
                if packets > 16_384 || received > 16 * crate::drafts::MAX_BODY {
                    return Err("draft probe aggregate input bound exceeded".into());
                }
                if bytes.len() > crate::drafts::MAX_BODY * 2 + 8 {
                    return Err("draft probe input bound exceeded".into());
                }
                if interrupted {
                    return Err(
                        "draft probe forbids commands after the injected partial operation".into(),
                    );
                }
                let title_write = bytes.first() == Some(&25) && bytes.contains(&13);
                let deletes = bytes.iter().all(|b| matches!(b, 25 | 12)) && bytes.contains(&25);
                if matches!(
                    case,
                    Some(
                        DraftFailure::LocalOnly
                            | DraftFailure::CloseCapture
                            | DraftFailure::CloseTransfer
                            | DraftFailure::ChangedTemplate
                    )
                ) && (title_write || deletes)
                {
                    return Err(
                        "read/close/changed-template probe forbids replacement writes".into(),
                    );
                }
                if case == Some(DraftFailure::ChangedTemplate)
                    && bytes.as_ref() == [24]
                    && native.menu_count == 1
                {
                    native.editor.lines[0] = crate::encoding::encode("看板預設格式（遠端修改）")
                        .map_err(|e| e.to_string())?;
                    injected = true;
                }
                if case == Some(DraftFailure::AfterTitle) && title_write {
                    let end = bytes
                        .iter()
                        .position(|b| *b == 13)
                        .ok_or("missing explicit title terminator")?
                        + 1;
                    native.accept(&bytes[..end]);
                    injected = true;
                    interrupted = true;
                } else if case == Some(DraftFailure::AfterDelete) && deletes {
                    native.accept(&bytes[..1]);
                    injected = true;
                    interrupted = true;
                } else if matches!(
                    case,
                    Some(DraftFailure::AfterWrite | DraftFailure::Disconnect)
                ) && bytes
                    .starts_with(&crate::encoding::encode("第一行中文").map_err(|e| e.to_string())?)
                {
                    let end = bytes
                        .iter()
                        .position(|b| *b == 13)
                        .ok_or("body requires first-line CR")?
                        + 1;
                    native.accept(&bytes[..end]);
                    injected = true;
                    interrupted = true;
                } else {
                    // Preserve the existing normal fixture's intermediate redraws.
                    let split = if deletes {
                        Some(1)
                    } else if title_write {
                        bytes.iter().position(|b| *b == 13).map(|i| i + 1)
                    } else {
                        None
                    };
                    if let Some(n) = split.filter(|n| *n < bytes.len()) {
                        native.accept(&bytes[..n]);
                        socket
                            .send(Message::Binary(native.wire().into()))
                            .await
                            .map_err(|e| e.to_string())?;
                        sleep(Duration::from_millis(80)).await;
                        native.accept(&bytes[n..]);
                    } else {
                        native.accept(&bytes);
                    }
                }
                if slow
                    || matches!(
                        case,
                        Some(DraftFailure::CloseCapture | DraftFailure::CloseTransfer)
                    )
                {
                    sleep(Duration::from_millis(350)).await;
                }
                socket
                    .send(Message::Binary(native.wire().into()))
                    .await
                    .map_err(|e| e.to_string())?;
                if case == Some(DraftFailure::Disconnect) && injected {
                    expected(&native, DraftFailure::Disconnect, true)?;
                    // Deliberately end TCP without a WebSocket Close. This is
                    // an explicit transport-failure case, not an acknowledged close.
                    return socket.get_mut().shutdown().await.map_err(|e| e.to_string());
                }
            }
            _ => return Err("draft probe rejects non-editor messages".into()),
        }
    }
    Err("draft peer did not receive Close".into())
}
