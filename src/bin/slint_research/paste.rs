//! One explicit clipboard read and one confirmation owned by the terminal window.
use crate::{TerminalWindow, clipboard::Clipboard, session::FrameId};
use pttstella::{
    connection::{Connection, Phase, View},
    input::{self, PreparedPaste},
    msg,
    navigation::{self, Stamp},
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct Pending {
    stamp: Stamp,
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
        Ok(Self { stamp, prepared })
    }
}

pub struct Paste {
    connection: Rc<Connection>,
    presented: Rc<Cell<Option<FrameId>>>,
    reading: Cell<bool>,
    read_stamp: Cell<Option<Stamp>>,
    pending: RefCell<Option<Pending>>,
    clipboard: Clipboard,
}

impl Paste {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        presented: Rc<Cell<Option<FrameId>>>,
        clipboard: Clipboard,
    ) -> Rc<Self> {
        let paste = Rc::new(Self {
            connection,
            presented,
            reading: Cell::new(false),
            read_stamp: Cell::new(None),
            pending: RefCell::new(None),
            clipboard,
        });
        ui.set_paste_cancel_label(msg!("cancel").into());
        ui.set_paste_send_label(msg!("paste-send").into());
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
                    let current = paste.current(&ui);
                    let obsolete = paste
                        .pending
                        .borrow()
                        .as_ref()
                        .is_some_and(|pending| Some(pending.stamp) != current);
                    if obsolete
                        || paste
                            .read_stamp
                            .get()
                            .is_some_and(|stamp| Some(stamp) != current)
                    {
                        paste.cancel(&ui, !ui.get_switching_station());
                        ui.set_input_notice(msg!("paste-stale").into());
                    }
                }
            }
        });
        paste
    }

    pub fn request(self: &Rc<Self>, ui: &TerminalWindow) {
        if self.reading.get() || ui.get_paste_open() {
            return;
        }
        let Some(stamp) = self.current(ui) else {
            ui.set_input_notice(msg!("paste-stale").into());
            return;
        };
        self.reading.set(true);
        self.read_stamp.set(Some(stamp));
        ui.set_paste_reading(true);
        ui.invoke_keep_alive_changed();
        ui.set_input_notice("".into());
        let (weak, paste) = (ui.as_weak(), self.clone());
        if let Err(error) = slint::spawn_local(async move {
            if paste.read_stamp.get() != Some(stamp) {
                paste.reading.set(false);
                if let Some(ui) = weak.upgrade() {
                    ui.set_paste_reading(false);
                    ui.invoke_keep_alive_changed();
                }
                return;
            }
            let clipboard = paste.clipboard.clone();
            let result = tokio::task::spawn_blocking(move || clipboard.read_text())
                .await
                .unwrap_or_else(|_| Err(msg!("paste-unavailable")));
            paste.reading.set(false);
            let current = paste.read_stamp.take();
            if let Some(ui) = weak.upgrade() {
                if current == Some(stamp) {
                    paste.complete(&ui, stamp, result);
                }
                ui.set_paste_reading(false);
                ui.invoke_keep_alive_changed();
            }
        }) {
            self.reading.set(false);
            self.cancel_read();
            ui.set_paste_reading(false);
            ui.invoke_keep_alive_changed();
            ui.set_input_notice(error.to_string().into());
        }
    }

    pub fn cancel_read(&self) {
        self.read_stamp.set(None);
    }

    pub fn cancel(&self, ui: &TerminalWindow, restore_focus: bool) {
        self.cancel_read();
        if self.pending.borrow_mut().take().is_some() {
            Self::dismiss(ui, restore_focus);
        }
    }

    fn current(&self, ui: &TerminalWindow) -> Option<Stamp> {
        if ui.get_switching_station() {
            return None;
        }
        let view = self.connection.updates.borrow();
        paste_stamp(&view, self.presented.get())
    }

    fn complete(&self, ui: &TerminalWindow, stamp: Stamp, read: Result<String, String>) {
        let prepared = read.and_then(|text| {
            let view = self.connection.updates.borrow();
            if !ui.get_terminal_input_focused()
                || ui.get_switching_station()
                || paste_stamp(&view, self.presented.get()) != Some(stamp)
            {
                return Err(msg!("paste-stale"));
            }
            Pending::prepare(&view, stamp, &text)
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
            Err(error) => ui.set_input_notice(error.into()),
        }
    }

    fn send(&self, ui: &TerminalWindow, pending: Pending) {
        let stamp = pending.stamp;
        if self.current(ui) != Some(stamp) {
            ui.set_input_notice(msg!("paste-stale").into());
            return;
        }
        match self.connection.request_paste(stamp, pending.prepared.bytes) {
            Ok(reply) => {
                let (weak, connection) = (ui.as_weak(), self.connection.clone());
                if let Err(error) = slint::spawn_local(async move {
                    if let Err(error) = reply.wait().await
                        && connection.updates.borrow().generation == stamp.generation
                        && let Some(ui) = weak.upgrade()
                    {
                        ui.set_input_notice(error.into());
                    }
                }) {
                    ui.set_input_notice(error.to_string().into());
                }
            }
            Err(error) => ui.set_input_notice(error.into()),
        }
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
