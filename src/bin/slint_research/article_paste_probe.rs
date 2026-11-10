//! Owned synthetic clipboard results through the production reader and UI dialog.
use super::Paste;
use crate::{
    TerminalWindow,
    article::session::ArticleSession,
    output::Output,
    verification::{self, Verification, require, wait},
};
use pttstella::{connection::Connection, navigation::Context};
use std::{
    error::Error,
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn idle(paste: &Paste) -> bool {
    !paste.reading.get()
        && paste.worker.borrow().is_none()
        && paste.read_job.borrow().is_none()
        && paste.send_job.borrow().is_none()
}
async fn focus(ui: &TerminalWindow, output: &Output) -> Result<(), String> {
    wait(
        || {
            verification::foreground(ui, true);
            ui.invoke_focus_terminal();
            (if ui.get_article_active() {
                ui.get_article_focused()
            } else {
                ui.get_terminal_input_focused()
            }) && output.current(ui).is_some()
        },
        "paste source did not acquire a confirmed native focus",
    )
    .await
}
fn read(paste: &Rc<Paste>, ui: &TerminalWindow, text: &str) {
    let text = text.to_owned();
    paste.request_with(ui, move || Ok(text));
}
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<ArticleSession>,
    connection: Rc<Connection>,
    output: Rc<Output>,
    paste: Rc<Paste>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(
        ui,
        directory,
        "article-paste",
        move |ui, directory| async move {
            verification::capture(&ui, &directory, "startup.png")?;
            tokio::time::sleep(Duration::from_millis(300)).await;
            wait(|| ui.get_article_active(), "complete paste article missing").await?;
            focus(&ui, &output).await?;
            let original = connection.updates.borrow().stamp();
            read(&paste, &ui, "cancel\r中");
            wait(
                || ui.get_paste_open(),
                "article multi-line preview did not open",
            )
            .await?;
            require(
                ui.get_article_active() && connection.updates.borrow().stamp() == original,
                "preview switched to raw or sent bytes before confirmation",
            )?;
            verification::capture(&ui, &directory, "preview.png")?;
            // A geometry-only repaint must not revoke admitted preview text;
            // confirming before the geometry ACK must still send no bytes.
            ui.set_frame_paint_pending(true);
            ui.invoke_paste_updated();
            require(
                ui.get_paste_open(),
                "geometry-only repaint revoked the current paste preview",
            )?;
            ui.invoke_paste_confirm();
            require(
                !ui.get_paste_open()
                    && connection.updates.borrow().stamp() == original
                    && ui.get_input_notice().as_str() == pttstella::msg!("paste-stale").as_str(),
                "unacknowledged geometry admitted paste confirmation",
            )?;
            ui.set_frame_paint_pending(false);
            focus(&ui, &output).await?;
            read(&paste, &ui, "cancel\r中");
            wait(|| ui.get_paste_open(), "cancel preview did not reopen").await?;
            ui.invoke_paste_cancel();
            focus(&ui, &output).await?;
            require(
                !ui.get_paste_open() && ui.get_article_active(),
                "cancelled preview lost the article",
            )?;

            // A late read belongs to the old policy even if network identity stays.
            let (release, blocked) = std::sync::mpsc::sync_channel::<String>(1);
            paste.request_with(&ui, move || blocked.recv().map_err(|e| e.to_string()));
            require(
                paste.reading.get(),
                "controlled clipboard read was not owned",
            )?;
            owner.replace_rules(pttstella::filters::Rules::default());
            require(
                paste.read_source.get().is_none(),
                "policy change kept a clipboard read admitted",
            )?;
            ui.set_input_notice("newer owned notice".into());
            release.send("late\r中".into()).map_err(|e| e.to_string())?;
            wait(|| idle(&paste), "late clipboard worker did not join").await?;
            require(
                !ui.get_paste_open() && ui.get_input_notice() == "newer owned notice",
                "late clipboard result replaced current state",
            )?;
            focus(&ui, &output).await?;
            read(&paste, &ui, "obsolete\r中");
            wait(|| ui.get_paste_open(), "policy preview missing").await?;
            owner.replace_rules(pttstella::filters::Rules::default());
            require(
                !ui.get_paste_open(),
                "source-identical policy edit kept a preview",
            )?;
            ui.invoke_paste_confirm();
            focus(&ui, &output).await?;
            let attempts = Arc::new(AtomicUsize::new(0));
            let attempted = attempts.clone();
            ui.set_native_composing(true);
            paste.request_with(&ui, move || {
                attempted.fetch_add(1, Ordering::SeqCst);
                Ok("IME should not read".into())
            });
            require(
                attempts.load(Ordering::SeqCst) == 0 && !paste.reading.get(),
                "composition started a clipboard read",
            )?;
            ui.set_native_composing(false);
            focus(&ui, &output).await?;
            paste.request_with(&ui, || Err("owned clipboard read failure".into()));
            wait(|| idle(&paste), "failed clipboard worker did not join").await?;
            require(
                ui.get_article_active() && ui.get_input_notice() == "owned clipboard read failure",
                "clipboard failure handed off the article",
            )?;
            read(&paste, &ui, "🙂");
            wait(|| idle(&paste), "invalid encoding worker did not join").await?;
            require(
                ui.get_article_active()
                    && !ui.get_input_notice().is_empty()
                    && connection.updates.borrow().stamp() == original,
                "invalid or cancelled clipboard input emitted bytes",
            )?;

            // A pasted navigation-looking character is raw input, never local j.
            read(&paste, &ui, "j");
            wait(
                || {
                    let view = connection.updates.borrow();
                    view.input_epoch > original.input_epoch
                        && view.frame.revision > original.revision
                        && !view.input_pending
                        && !view.output_pending
                        && !ui.get_article_active()
                        && ui.get_article_station()
                        && idle(&paste)
                },
                "single-line paste did not make exactly one raw handoff",
            )
            .await?;
            ui.invoke_article_resume_reading();
            wait(|| ui.get_article_active(), "paste article did not resume").await?;
            focus(&ui, &output).await?;
            let before = connection.updates.borrow().stamp();
            read(&paste, &ui, "A\r\n中");
            wait(
                || ui.get_paste_open(),
                "fresh confirmed paste preview missing",
            )
            .await?;
            require(
                ui.get_article_active() && connection.updates.borrow().stamp() == before,
                "multi-line paste wrote before confirmation",
            )?;
            ui.invoke_paste_confirm();
            wait(
                || {
                    let view = connection.updates.borrow();
                    matches!(view.context(), Context::MainMenu { .. })
                        && view.input_epoch > before.input_epoch
                        && !view.input_pending
                        && !view.output_pending
                        && idle(&paste)
                },
                "confirmed UAO paste did not reach the exact peer",
            )
            .await?;
            focus(&ui, &output).await?;
            require(
                !ui.get_article_active() && !ui.get_article_station() && owner.requests.get() == 0,
                "leaving reading retained raw hold or fetched a page",
            )?;
            verification::capture(&ui, &directory, "confirmed-station.png")?;
            // Snapshot/layout callbacks can retire the source admitted above.
            // This close test needs a newly admissible read, not a rejected
            // request that never owned a worker.
            focus(&ui, &output).await?;
            wait(
                || paste.current(&ui).is_some() && ui.get_context_focus_restorable(),
                "close clipboard source did not become ready",
            )
            .await?;

            // Dropping the owned provider unblocks the pending native read; finish
            // aborts its UI completion and joins the actual worker before close.
            let (release, blocked) = std::sync::mpsc::sync_channel::<String>(1);
            paste.request_with(&ui, move || blocked.recv().map_err(|e| e.to_string()));
            require(
                paste.worker.borrow().is_some(),
                "close fixture lacked its read worker",
            )?;
            drop(release);
            paste.finish()?;
            require(
                paste.stopped(),
                "clipboard reader or send waiter survived finish",
            )?;
            owner.shutdown().await;
            require(owner.stopped(), "paste article actor did not join")?;
            std::fs::write(directory.join("result.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "scope":"actual generated preview/cancel/confirm and production native worker; owned synthetic clipboard source, no OS clipboard or PTT",
            "peer_inputs":[[106],[65,13,164,164]],"cancelled_and_stale_bytes":0,
            "ime_read_attempts":0,"policy_revokes_read_and_preview":true,"failed_read_keeps_article":true,
            "geometry_repaint_retains_preview":true,"geometry_ack_required_to_send":true,
            "plain_navigation_character_is_raw":true,"multiline_requires_confirmation":true,
            "raw_hold_released_off_reading":true,"preload_commands":0,"pending_worker_and_article_joined":true
        })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            Ok(())
        },
    )
}
