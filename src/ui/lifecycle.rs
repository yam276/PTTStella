//! All surface exits revoke input, stop the renderer and join owned work here.
use dioxus::prelude::*;
use pttstella::{
    connection::{CloseOutcome, Connection},
    login::vault::Job,
    media::preview::Preview,
};
use std::{future::Future, time::Duration};

use super::{commands::Pending, files::Files, outbound::Packet};

#[derive(Default)]
pub(super) struct Resources {
    pub(super) eval: Option<document::Eval>,
    pub(super) media: Preview,
    pub(super) inline: super::inline_images::InlineImages,
    pub(super) login: Option<Job>,
    pub(super) commands: Pending,
    pub(super) files: Files,
    pub(super) settings: super::settings_window::SettingsWindow,
    pub(super) tools: Option<super::protocol::ToolsObservation>,
}

#[derive(serde::Deserialize)]
struct RendererFinished {
    clean: bool,
    #[serde(default)]
    tools: Option<super::protocol::ToolsObservation>,
}
impl RendererFinished {
    fn acknowledge(mut self, stopped: Option<bool>) -> Self {
        self.clean = stopped.unwrap_or(self.clean) && self.clean;
        self
    }
}

/// Wait without dropping overdue cleanup. OS credential prompts cannot be
/// canceled; the async owner stays alive and reports that cleanup is pending.
async fn retained_wait<T>(
    work: impl Future<Output = T>,
    overdue: impl FnOnce(),
    deadline: Duration,
) -> T {
    tokio::pin!(work);
    tokio::select! {
        result = &mut work => result,
        _ = tokio::time::sleep(deadline) => {overdue(); work.await}
    }
}

async fn renderer(mut eval: document::Eval) -> Result<RendererFinished, String> {
    let _ = eval.send(Packet::Stop);
    // A second, static control query does not depend on the surface data channel.
    let interrupt =
        document::eval("window.dispatchEvent(new Event('pttstella:stop')); return null;");
    let (interrupted, finished) = tokio::join!(interrupt.join::<serde_json::Value>(), async {
        let joined = eval.join::<RendererFinished>();
        tokio::pin!(joined);
        let mut receiving = true;
        let mut acknowledged = None;
        loop {
            tokio::select! {
                result = &mut joined => {
                    let result = result.map_err(|e|pttstella::msg!("cleanup-eval-unconfirmed", e = e.to_string()))?;
                    // The returned value is a second acknowledgement from the
                    // same finally block if joining races its Stopped message.
                    return Ok::<RendererFinished, String>(result.acknowledge(acknowledged));
                },
                event = eval.recv::<serde_json::Value>(), if receiving => match event {
                    Ok(value) if value["kind"] == "stopped" => acknowledged = value["clean"].as_bool(),
                    Ok(_) => {}, // Drain queued paint/input/error notifications.
                    Err(_) => receiving = false,
                },
            }
        }
    });
    match finished {
        Ok(finished) => Ok(finished),
        Err(error) => Err(pttstella::msg!(
            "cleanup-stop-error",
            error = error,
            value1 = if interrupted.is_ok() {
                pttstella::msg!("sent")
            } else {
                pttstella::msg!("unconfirmed")
            }
        )),
    }
}

impl Resources {
    pub(super) async fn finish(
        &mut self,
        connection: &Connection,
        mut message: Signal<String>,
    ) -> Result<CloseOutcome, String> {
        self.commands.cancel();
        self.files.revoke();
        self.media.close();
        self.inline.close();
        connection.revoke_mouse(connection.updates.borrow().generation);
        connection.keep_alive_gate(connection.updates.borrow().stamp(), false);
        // Cancellation is best effort; shutdown below remains the owned, joined
        // operation even if the bounded command queue is already closed/full.
        let _ = connection.cancel_login();
        let operation = match connection.updates.borrow().destination {
            pttstella::navigation_job::Progress::Active { operation, .. } => Some(operation),
            _ => None,
        };
        if let Some(operation) = operation {
            let _ = connection.cancel_navigation(operation);
        }
        let render = async {
            match self.eval.take() {
                Some(eval) => renderer(eval).await,
                None => Ok(RendererFinished {
                    clean: true,
                    tools: None,
                }), // Preflight failed before JS was created.
            }
        };
        let credentials = async {
            if let Some(mut job) = self.login.take() {
                let _ = job.wait().await; // Late credentials are zeroized on drop.
            }
        };
        let (renderer, disconnect, (), files) = retained_wait(
            async {
                tokio::join!(
                    render,
                    connection.shutdown(),
                    credentials,
                    self.files.finish()
                )
            },
            || {
                message.set(pttstella::msg!("cleanup-wait"));
                dioxus::desktop::window().set_visible(true);
            },
            Duration::from_secs(5),
        )
        .await;
        let settings = retained_wait(
            self.settings.finish(),
            || message.set(pttstella::msg!("cleanup-wait")),
            Duration::from_secs(5),
        )
        .await;
        let files = match (files, settings) {
            (Ok(()), result) => result,
            (Err(error), Ok(())) => Err(error),
            (Err(error), Err(settings)) => Err(format!("{error}；{settings}")),
        };
        let renderer = renderer.and_then(|finished| {
            self.tools = finished.tools;
            if finished.clean {
                Ok(())
            } else {
                Err(pttstella::msg!("cleanup-listeners-stopped"))
            }
        });
        let renderer = match (renderer, files) {
            (Ok(()), result) => result,
            (Err(error), Ok(())) => Err(error),
            (Err(error), Err(files)) => Err(format!("{error}；{files}")),
        };
        match (renderer, disconnect) {
            (Ok(()), result) => result,
            (Err(error), Ok(_)) => Err(error),
            (Err(error), Err(disconnect)) => Err(format!("{error}；{disconnect}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    use tokio::sync::oneshot;
    #[test]
    fn joined_renderer_keeps_probe_observations_when_stopped_notification_races() {
        let tools = serde_json::json!({"presses":3,"moves":5,"drag_releases":1,"coast_frames":8,"max_coast_distance":92.0,"last_release":null});
        for (stopped, joined, clean) in [
            (None, true, true),
            (Some(true), true, true),
            (Some(false), true, false),
            (Some(true), false, false),
        ] {
            let result: RendererFinished =
                serde_json::from_value(serde_json::json!({"clean":joined,"tools":tools})).unwrap();
            let result = result.acknowledge(stopped);
            assert_eq!(result.clean, clean);
            assert_eq!(serde_json::to_value(result.tools).unwrap(), tools);
        }
    }

    #[tokio::test]
    async fn overdue_cleanup_keeps_ownership_until_acknowledged() {
        let (ack, receiver) = oneshot::channel();
        let notified = Rc::new(Cell::new(false));
        let flag = notified.clone();
        let work = retained_wait(receiver, || flag.set(true), Duration::ZERO);
        tokio::pin!(work);
        tokio::select! {
            _ = &mut work => panic!("cleanup was abandoned"),
            _ = tokio::time::sleep(Duration::from_millis(10)) => {}
        }
        assert!(notified.get());
        assert!(!ack.is_closed());
        ack.send("joined").unwrap();
        assert_eq!(work.await.unwrap(), "joined");
    }
}
