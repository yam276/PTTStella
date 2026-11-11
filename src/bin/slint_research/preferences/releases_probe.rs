//! Explicit loopback fixture; the actual App exit must cancel the third body.
use super::*;
use crate::verification::{capture, require, wait};
use slint::Model;
use std::path::Path;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

pub(super) struct Fixture {
    task: Option<tokio::task::JoinHandle<Result<Vec<String>, String>>>,
    original_settings: Vec<u8>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

async fn request(socket: &mut tokio::net::TcpStream) -> Result<String, String> {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        require(
            bytes.len() < 16384,
            "update fixture request exceeded header budget",
        )?;
        bytes.push(socket.read_u8().await.map_err(|e| e.to_string())?);
    }
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    let lower = text.to_ascii_lowercase();
    require(
        lower.starts_with("get /latest http/1.1\r\n")
            && !lower.contains("authorization:")
            && !lower.contains("cookie:")
            && !lower.contains("referer:"),
        "update request sent unexpected path/credentials",
    )?;
    Ok(text)
}
fn response(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

async fn stage(receiver: &mut tokio::sync::mpsc::Receiver<usize>) -> Result<usize, String> {
    tokio::time::timeout(Duration::from_secs(3), receiver.recv())
        .await
        .map_err(|_| "update fixture did not receive a request within its deadline")?
        .ok_or_else(|| "update fixture stopped before the next request".into())
}

impl Preferences {
    pub(crate) async fn verify_releases(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        dir: &Path,
    ) -> Result<(), String> {
        require(
            self.release_worker.borrow().is_none(),
            "general synthetic mode started public update work",
        )?;
        let original_settings = self
            .model
            .borrow()
            .store()?
            .read()?
            .ok_or("settings seed missing")?;
        ui.invoke_settings_show();
        let window = self
            .window
            .borrow()
            .as_ref()
            .and_then(|w| w.as_weak().upgrade())
            .ok_or("settings window missing")?;
        window.global::<crate::SettingsState>().set_page(7);
        require(
            !window
                .global::<crate::SettingsState>()
                .get_release_enabled()
                && !window
                    .global::<crate::SettingsState>()
                    .get_release_checking(),
            "general probe enabled external release check",
        )?;
        window
            .global::<crate::SettingsState>()
            .invoke_release_check();
        require(
            self.release_worker.borrow().is_none(),
            "synthetic click started public check",
        )?;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| e.to_string())?;
        let endpoint = format!(
            "http://{}/latest",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        let (ready, mut stages) = tokio::sync::mpsc::channel(1);
        let (release, held) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            let (mut socket, _) = listener.accept().await.map_err(|e| e.to_string())?;
            requests.push(request(&mut socket).await?);
            ready.send(1).await.map_err(|e| e.to_string())?;
            held.await.map_err(|e| e.to_string())?;
            socket.write_all(response("200 OK", r#"{"tag_name":"v999.0.0","draft":false,"prerelease":false,"html_url":"https://evil.invalid/","body":"not displayed"}"#).as_bytes()).await.map_err(|e| e.to_string())?;
            drop(socket);
            let (mut socket, _) = listener.accept().await.map_err(|e| e.to_string())?;
            requests.push(request(&mut socket).await?);
            socket
                .write_all(response("429 Too Many Requests", "wait").as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            drop(socket);
            ready.send(2).await.map_err(|e| e.to_string())?;
            let (mut socket, _) = listener.accept().await.map_err(|e| e.to_string())?;
            requests.push(request(&mut socket).await?);
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\n\r\n{")
                .await
                .map_err(|e| e.to_string())?;
            ready.send(3).await.map_err(|e| e.to_string())?;
            let mut byte = [0];
            tokio::select! {
                extra = listener.accept() => { return Err(format!("unexpected duplicate update connection: {}", extra.is_ok())); }
                closed = tokio::time::timeout(Duration::from_secs(3), socket.read(&mut byte)) => {
                    match closed.map_err(|_| "app exit did not cancel release HTTP")? {
                        Ok(0) => {},
                        Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {},
                        other => return Err(format!("expected update cancellation EOF/reset: {other:?}")),
                    }
                }
            }
            Ok(requests)
        });
        self.release_probe.replace(Some(Fixture {
            task: Some(task),
            original_settings,
        }));
        self.start_releases(pttstella::updates::Checker::loopback(&endpoint)?, true)?;
        require(
            stage(&mut stages).await? == 1,
            "startup release request missing",
        )?;
        wait(
            || {
                window
                    .global::<crate::SettingsState>()
                    .get_release_checking()
            },
            "checking state missing",
        )
        .await?;
        for _ in 0..20 {
            window
                .global::<crate::SettingsState>()
                .invoke_release_check();
        }
        release.send(()).map_err(|_| "release fixture stopped")?;
        wait(
            || {
                window
                    .global::<crate::SettingsState>()
                    .get_release_available()
                    && !window
                        .global::<crate::SettingsState>()
                        .get_release_checking()
            },
            "stable offer missing",
        )
        .await?;
        require(
            window.global::<crate::SettingsState>().get_release_status()
                == msg!("update-available", version = "999.0.0"),
            "version label differs from shared checker",
        )?;
        require(
            window
                .global::<crate::SettingsState>()
                .get_release_labels()
                .row_data(0)
                .as_deref()
                == Some(msg!("update-check").as_str()),
            "localized release check label missing",
        )?;
        window
            .global::<crate::SettingsState>()
            .invoke_release_open();
        require(
            ui.get_input_notice() == "https://github.com/yam276/PTTStella/releases/tag/v999.0.0",
            "remote JSON URL reached browser adapter",
        )?;
        capture(&window, dir, "release-available.png")?;
        ui.set_input_notice("guard-marker".into());
        window.global::<crate::SettingsState>().set_page(0);
        window
            .global::<crate::SettingsState>()
            .invoke_release_check();
        window
            .global::<crate::SettingsState>()
            .invoke_release_open();
        window
            .global::<crate::SettingsState>()
            .invoke_repository_open();
        require(
            ui.get_input_notice() == "guard-marker",
            "off-page release action escaped guard",
        )?;
        window.global::<crate::SettingsState>().set_page(7);
        require(
            window
                .global::<crate::SettingsState>()
                .get_release_enabled(),
            "release controls did not enable after startup completion",
        )?;
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::WindowActiveChanged(true));
        window
            .global::<crate::SettingsState>()
            .invoke_focus_field(27);
        wait(
            || window.global::<crate::SettingsState>().get_focused_target() == 27,
            "release check button could not take toolkit focus",
        )
        .await?;
        for event in [
            slint::platform::WindowEvent::KeyPressed { text: " ".into() },
            slint::platform::WindowEvent::KeyReleased { text: " ".into() },
        ] {
            window.window().dispatch_event(event);
        }
        require(
            stage(&mut stages).await? == 2,
            "manual retry did not stay on loopback",
        )?;
        wait(
            || {
                !window
                    .global::<crate::SettingsState>()
                    .get_release_checking()
            },
            "failed check did not settle",
        )
        .await?;
        require(
            window
                .global::<crate::SettingsState>()
                .get_release_available()
                && !self.release_status.borrow().up_to_date
                && window.global::<crate::SettingsState>().get_release_status()
                    == msg!("update-available", version = "999.0.0"),
            "silent failure lost valid offer or claimed success",
        )?;
        window
            .global::<crate::SettingsState>()
            .invoke_repository_open();
        require(
            ui.get_input_notice() == env!("CARGO_PKG_REPOSITORY"),
            "repository adapter target differs",
        )?;
        window
            .global::<crate::SettingsState>()
            .invoke_release_check();
        require(stage(&mut stages).await? == 3, "third body fixture missing")?;
        wait(
            || {
                window
                    .global::<crate::SettingsState>()
                    .get_release_checking()
            },
            "stalled check did not remain pending",
        )
        .await?;
        self.hide();
        ui.set_input_notice("closed-guard".into());
        window
            .global::<crate::SettingsState>()
            .invoke_release_check();
        window
            .global::<crate::SettingsState>()
            .invoke_release_open();
        window
            .global::<crate::SettingsState>()
            .invoke_repository_open();
        require(
            ui.get_input_notice() == "closed-guard" && self.release_status.borrow().checking,
            "closed settings admitted actions or silently cancelled application-scoped check",
        )?;
        ui.invoke_settings_show();
        require(
            window
                .global::<crate::SettingsState>()
                .get_release_available()
                && window
                    .global::<crate::SettingsState>()
                    .get_release_checking(),
            "reopened settings lost pending/available status",
        )?;
        capture(&window, dir, "release-pending-close.png")?;
        self.hide();
        std::fs::write(dir.join("release-actions.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "status":"passed_pending_exit_audit", "synthetic_default_disabled":true,
            "duplicate_callbacks":20, "available":"999.0.0", "remote_url_ignored":true,
            "failed_retry_retained_offer":true, "off_page_and_closed_actions_rejected":true,
                    "manual_retry_toolkit_space_key":true,
            "pending_http_at_return":true, "scope":"toolkit callbacks and loopback only; browser and OS pointer/keyboard untested"
        })).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }

    pub(crate) fn audit_release_probe(&self, dir: &Path) -> Result<(), String> {
        require(
            self.stopped.get()
                && self.release_listener_joined.get()
                && self.release_worker.borrow().is_none()
                && self.release_task.borrow().is_none()
                && self.window.borrow().is_none(),
            "app exit retained release task/window",
        )?;
        let mut fixture = self
            .release_probe
            .borrow_mut()
            .take()
            .ok_or("release fixture missing")?;
        let mut task = fixture
            .task
            .take()
            .ok_or("release fixture already consumed")?;
        let runtime = tokio::runtime::Handle::try_current().map_err(|e| e.to_string())?;
        let requests = runtime.block_on(async {
            match tokio::time::timeout(Duration::from_secs(3), &mut task).await {
                Ok(joined) => joined.map_err(|e| e.to_string())?,
                Err(_) => {
                    task.abort();
                    let _ = task.await;
                    Err("update fixture close audit timed out; task aborted and joined".into())
                }
            }
        })?;
        require(
            requests.len() == 3,
            "wrong independent update request count",
        )?;
        require(
            self.model.borrow().store()?.read()?.as_deref()
                == Some(fixture.original_settings.as_slice()),
            "release controls changed saved preferences",
        )?;
        std::fs::write(dir.join("release-closed.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "status":"passed", "requests":requests, "request_count":3,
            "worker_joined":true, "stalled_http_closed":true, "status_task_retired":true,
            "settings_unchanged":true, "scope":"actual normal app-quit callback from toolkit probe; native OS Quit/IME/browser remain separate"
        })).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }
}
