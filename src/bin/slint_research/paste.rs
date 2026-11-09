//! One explicit clipboard read and one confirmation owned by the terminal window.
use crate::{TerminalWindow, clipboard::Clipboard, session::FrameId};
use pttstella::{
    connection::{Connection, Phase, View},
    input::{self, PreparedPaste},
    msg,
    navigation::{self, Stamp},
    surface_state::Identity,
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    thread::JoinHandle,
};
#[path = "article_paste_probe.rs"]
pub mod probe;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Source {
    stamp: Stamp,
    identity: Option<Identity>,
}
struct Pending {
    stamp: Stamp,
    identity: Option<Identity>,
    prepared: PreparedPaste,
}

impl Pending {
    fn prepare(view: &View, stamp: Stamp, text: &str) -> Result<Self, String> {
        let prepared = if view.site.ptt_features() && navigation::article_editor(&view.frame) {
            input::prepare_editor_paste(text, view.site.encoding)
        } else {
            input::prepare_paste_with(text, view.site.encoding)
        }
        .map_err(|error| error.to_string())?;
        Ok(Self {
            stamp,
            identity: None,
            prepared,
        })
    }
    fn source(&self) -> Source {
        Source {
            stamp: self.stamp,
            identity: self.identity,
        }
    }
}

pub struct Paste {
    connection: Rc<Connection>,
    presented: Rc<Cell<Option<FrameId>>>,
    reading: Cell<bool>,
    read_source: Cell<Option<Source>>,
    pending: RefCell<Option<Pending>>,
    clipboard: Clipboard,
    article: Option<Rc<crate::article::session::ArticleSession>>,
    worker: RefCell<Option<JoinHandle<()>>>,
    read_job: RefCell<Option<slint::JoinHandle<()>>>,
    send_job: RefCell<Option<slint::JoinHandle<()>>>,
    closed: Cell<bool>,
}

impl Paste {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        presented: Rc<Cell<Option<FrameId>>>,
        clipboard: Clipboard,
        article: Option<Rc<crate::article::session::ArticleSession>>,
    ) -> Rc<Self> {
        let paste = Rc::new(Self {
            connection,
            presented,
            reading: Cell::new(false),
            read_source: Cell::new(None),
            pending: RefCell::new(None),
            clipboard,
            article,
            worker: RefCell::new(None),
            read_job: RefCell::new(None),
            send_job: RefCell::new(None),
            closed: Cell::new(false),
        });
        ui.on_paste_confirm({
            let (weak, paste) = (ui.as_weak(), paste.clone());
            move || {
                if let Some(ui) = weak.upgrade() {
                    let pending = paste.pending.borrow_mut().take();
                    if let Some(pending) = pending {
                        paste.send(&ui, pending);
                        Self::dismiss(&ui, true);
                    }
                }
            }
        });
        ui.on_paste_cancel({
            let (weak, paste) = (ui.as_weak(), paste.clone());
            move || {
                if let Some(ui) = weak.upgrade() {
                    paste.cancel(&ui, true);
                }
            }
        });
        ui.on_session_updated({
            let (weak, paste) = (ui.as_weak(), paste.clone());
            move || {
                if let Some(ui) = weak.upgrade() {
                    paste.updated(&ui);
                }
            }
        });
        ui.on_paste_updated({
            let (weak, paste) = (ui.as_weak(), paste.clone());
            move || {
                if let Some(ui) = weak.upgrade() {
                    paste.updated(&ui)
                }
            }
        });
        paste
    }

    pub fn request(self: &Rc<Self>, ui: &TerminalWindow) {
        let clipboard = self.clipboard.clone();
        self.request_with(ui, move || clipboard.read_text());
    }

    // Production and explicit synthetic conformance use the same owned worker;
    // only the source of the clipboard bytes differs. Automated conformance
    // supplies owned text; ordinary user paste reads the native clipboard.
    fn request_with(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        read: impl FnOnce() -> Result<String, String> + Send + 'static,
    ) {
        if self.closed.get()
            || self.reading.get()
            || self.send_job.borrow().is_some()
            || ui.get_paste_open()
        {
            return;
        }
        if !ui.get_context_focus_restorable() || !self.field_focused(ui) {
            crate::notice::error(ui, msg!("paste-stale"));
            return;
        }
        let Some(source) = self.current(ui) else {
            crate::notice::error(ui, msg!("paste-stale"));
            return;
        };
        self.reading.set(true);
        self.read_source.set(Some(source));
        ui.set_paste_reading(true);
        ui.invoke_keep_alive_changed();
        crate::notice::clear(ui);
        let (send, receive) = tokio::sync::oneshot::channel();
        let worker = match std::thread::Builder::new()
            .name("pttstella-slint-paste".into())
            .spawn(move || {
                let _ = send.send(read());
            }) {
            Ok(worker) => worker,
            Err(error) => {
                self.reading.set(false);
                self.cancel_read();
                ui.set_paste_reading(false);
                ui.invoke_keep_alive_changed();
                crate::notice::error(ui, error.to_string());
                return;
            }
        };
        self.worker.replace(Some(worker));
        let (weak, paste) = (ui.as_weak(), Rc::downgrade(self));
        match slint::spawn_local(async move {
            let result = receive
                .await
                .unwrap_or_else(|_| Err(msg!("paste-unavailable")));
            let Some(paste) = paste.upgrade() else { return };
            let joined = paste.worker.borrow_mut().take().map(|worker| worker.join());
            paste.read_job.borrow_mut().take();
            paste.reading.set(false);
            let current = paste.read_source.take();
            if paste.closed.get() {
                return;
            }
            if let Some(ui) = weak.upgrade() {
                if current == Some(source) {
                    let result = if joined.is_some_and(|r| r.is_err()) {
                        Err(msg!("paste-unavailable"))
                    } else {
                        result
                    };
                    paste.complete(&ui, source, result);
                }
                ui.set_paste_reading(false);
                ui.invoke_keep_alive_changed();
            }
        }) {
            Ok(job) => {
                self.read_job.replace(Some(job));
            }
            Err(error) => {
                self.cancel_read();
                self.reading.set(false);
                self.worker.borrow_mut().take().map(|worker| worker.join());
                ui.set_paste_reading(false);
                ui.invoke_keep_alive_changed();
                crate::notice::error(ui, error.to_string());
            }
        }
    }

    fn field_focused(&self, ui: &TerminalWindow) -> bool {
        if ui.get_article_active() {
            ui.get_article_focused()
        } else {
            ui.get_terminal_input_focused()
        }
    }
    fn updated(&self, ui: &TerminalWindow) {
        if self.closed.get() || self.pending.borrow().is_none() && self.read_source.get().is_none()
        {
            return;
        }
        let current = self.current(ui);
        let obsolete = self
            .pending
            .borrow()
            .as_ref()
            .is_some_and(|p| Some(p.source()) != current)
            || self
                .read_source
                .get()
                .is_some_and(|source| Some(source) != current);
        if obsolete {
            self.cancel(ui, !ui.get_switching_station());
            crate::notice::error(ui, msg!("paste-stale"));
        }
    }

    pub fn cancel_read(&self) {
        self.read_source.set(None);
    }

    pub fn cancel(&self, ui: &TerminalWindow, restore_focus: bool) {
        self.cancel_read();
        if self.pending.borrow_mut().take().is_some() {
            Self::dismiss(ui, restore_focus);
        }
    }

    fn current(&self, ui: &TerminalWindow) -> Option<Source> {
        if self.closed.get()
            || ui.get_app_closing()
            || ui.get_manager_open()
            || ui.get_settings_open()
            || ui.get_editor_open()
            || ui.get_station_color_open()
            || ui.get_switching_station()
            || ui.get_frame_paint_pending()
        {
            return None;
        }
        let Some(article) = &self.article else {
            return paste_stamp(&self.connection.updates.borrow(), self.presented.get()).map(
                |stamp| Source {
                    stamp,
                    identity: None,
                },
            );
        };
        let view = self.connection.updates.borrow().clone();
        if view.phase != Phase::Connected
            || view.input_pending
            || view.output_pending
            || view.login_pending
        {
            return None;
        }
        let identity = article.output_identity(&view)?;
        Some(Source {
            stamp: view.stamp(),
            identity: Some(identity),
        })
    }

    fn complete(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        source: Source,
        read: Result<String, String>,
    ) {
        let prepared = read.and_then(|text| {
            if !self.field_focused(ui)
                || !ui.get_native_foreground()
                || ui.get_native_composing()
                || self.current(ui) != Some(source)
            {
                return Err(msg!("paste-stale"));
            }
            let mut pending =
                Pending::prepare(&self.connection.updates.borrow(), source.stamp, &text)?;
            pending.identity = source.identity;
            Ok(pending)
        });
        match prepared {
            Ok(pending) if pending.prepared.requires_confirmation() => {
                ui.set_paste_title(
                    msg!(
                        "paste-title",
                        paste_lines = pending.prepared.lines.to_string()
                    )
                    .into(),
                );
                ui.set_paste_preview(pending.prepared.text.replace('\x1b', "␛").into());
                self.pending.replace(Some(pending));
                ui.set_paste_open(true);
                ui.invoke_show_paste_preview();
            }
            Ok(pending) => self.send(ui, pending),
            Err(error) => crate::notice::error(ui, error),
        }
    }

    fn send(self: &Rc<Self>, ui: &TerminalWindow, pending: Pending) {
        let stamp = pending.stamp;
        if self.closed.get()
            || self.send_job.borrow().is_some()
            || !ui.get_native_foreground()
            || ui.get_native_composing()
            || self.current(ui) != Some(pending.source())
        {
            crate::notice::error(ui, msg!("paste-stale"));
            return;
        }
        if let Some(article) = &self.article {
            article.station_handoff();
        }
        match self.connection.request_paste(stamp, pending.prepared.bytes) {
            Ok(reply) => {
                let (weak, paste) = (ui.as_weak(), Rc::downgrade(self));
                match slint::spawn_local(async move {
                    let result = reply.wait().await;
                    let Some(paste) = paste.upgrade() else { return };
                    paste.send_job.borrow_mut().take();
                    if paste.closed.get() {
                        return;
                    }
                    if let Err(error) = result
                        && paste.connection.updates.borrow().generation == stamp.generation
                        && let Some(ui) = weak.upgrade()
                    {
                        crate::notice::error(&ui, error);
                    }
                }) {
                    Ok(job) => {
                        self.send_job.replace(Some(job));
                    }
                    Err(error) => crate::notice::error(ui, error.to_string()),
                }
            }
            Err(error) => crate::notice::error(ui, error),
        }
    }

    pub fn finish(&self) -> Result<(), String> {
        self.closed.set(true);
        self.cancel_read();
        self.pending.borrow_mut().take();
        if let Some(job) = self.read_job.borrow_mut().take() {
            job.abort();
        }
        if let Some(job) = self.send_job.borrow_mut().take() {
            job.abort();
        }
        let joined = self.worker.borrow_mut().take().map(|worker| worker.join());
        self.reading.set(false);
        if joined.is_some_and(|r| r.is_err()) {
            return Err(msg!("paste-unavailable"));
        }
        Ok(())
    }
    pub fn stopped(&self) -> bool {
        self.closed.get()
            && !self.reading.get()
            && self.worker.borrow().is_none()
            && self.read_job.borrow().is_none()
            && self.send_job.borrow().is_none()
            && self.pending.borrow().is_none()
            && self.read_source.get().is_none()
    }

    fn dismiss(ui: &TerminalWindow, restore_focus: bool) {
        ui.set_paste_open(false);
        ui.invoke_hide_paste_preview(restore_focus);
        ui.set_paste_preview("".into());
        ui.invoke_keep_alive_changed();
    }
}

fn paste_stamp(view: &View, presented: Option<FrameId>) -> Option<Stamp> {
    let stamp = view.stamp();
    (view.phase == Phase::Connected
        && !view.input_pending
        && !view.login_pending
        && !view.output_pending
        && presented
            == Some(FrameId {
                generation: stamp.generation,
                revision: stamp.revision,
            }))
    .then_some(stamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::{connection::CloseOutcome, replay, text};
    use std::time::Duration;

    async fn editor_stage(connection: &Connection, index: u32) -> View {
        let mut updates = connection.updates.clone();
        let prefix = format!("EDITOR CLIPBOARD {index}/4");
        tokio::time::timeout(
            Duration::from_secs(5),
            updates.wait_for(|view| {
                text::rows(&view.frame)
                    .and_then(|rows| rows.into_iter().next())
                    .is_some_and(|row| row.text.starts_with(&prefix))
            }),
        )
        .await
        .unwrap()
        .unwrap()
        .clone()
    }

    #[tokio::test]
    async fn editor_clipboard_checks_scope_and_sends_confirmed_ansi_once() {
        let connection =
            Connection::start_editor_clipboard_probe(replay::editor_clipboard().unwrap()).unwrap();
        connection.connect().unwrap();
        let initial = editor_stage(&connection, 0).await;
        let id = FrameId {
            generation: initial.generation,
            revision: initial.frame.revision,
        };
        let stamp = paste_stamp(&initial, Some(id)).unwrap();
        assert!(
            paste_stamp(
                &initial,
                Some(FrameId {
                    revision: id.revision - 1,
                    ..id
                })
            )
            .is_none()
        );
        assert!(navigation::article_editor(&initial.frame));
        let fixture = "ANSI clipboard check:\x1b[31m中\x1b[0m\r\nEND";
        let pending = Pending::prepare(&initial, stamp, fixture).unwrap();
        assert!(pending.prepared.requires_confirmation());
        assert_eq!(pending.prepared.lines, 2);
        let ansi_only = Pending::prepare(&initial, stamp, "\x1b[31m中").unwrap();
        assert_eq!(ansi_only.prepared.lines, 1);
        assert!(ansi_only.prepared.requires_confirmation());
        let plain_multiline = input::prepare_paste("A\r\nB").unwrap();
        assert!(plain_multiline.requires_confirmation());
        assert_eq!(plain_multiline.bytes, b"A\rB");
        assert!(Pending::prepare(&initial, stamp, "\x1b[2J").is_err());
        assert!(Pending::prepare(&initial, stamp, "🙂").is_err());
        assert!(
            Pending::prepare(&initial, stamp, &"x".repeat(input::MAX_INPUT_BYTES + 1)).is_err()
        );

        connection
            .input(initial.generation, b"\x16".to_vec())
            .unwrap();
        let first = editor_stage(&connection, 1).await;
        assert_eq!(
            connection
                .request_paste(pending.stamp, pending.prepared.bytes)
                .unwrap()
                .wait()
                .await,
            Err(msg!("paste-stale"))
        );
        connection
            .input(first.generation, b"\x16".to_vec())
            .unwrap();
        let second = editor_stage(&connection, 2).await;
        let fresh = Pending::prepare(&second, second.stamp(), fixture).unwrap();
        let bytes = fresh.prepared.bytes;
        connection
            .request_paste(fresh.stamp, bytes.clone())
            .unwrap()
            .wait()
            .await
            .unwrap();
        assert_eq!(
            connection
                .request_paste(fresh.stamp, bytes)
                .unwrap()
                .wait()
                .await,
            Err(msg!("paste-stale"))
        );
        let third = editor_stage(&connection, 3).await;
        connection
            .input(third.generation, b"\x16".to_vec())
            .unwrap();
        editor_stage(&connection, 4).await;
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
    }
}
