//! Native station-colour preview leases an acknowledged selection. Connection
//! remains the sole owner of verification, station bytes and partial failures.
pub mod probe;
use crate::{StationColorState, TerminalWindow, output::Output, pointer::SurfaceGeometry};
use pttstella::{
    connection::{Connection, Phase},
    editor_color::Color,
    navigation::Stamp,
    screen::Frame,
    selection::{self, Position, Selection, Span},
};
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    rc::Rc,
};

#[derive(Clone, PartialEq)]
struct Source {
    stamp: Stamp,
    policy: u64,
    geometry: SurfaceGeometry,
    key: (Range<usize>, Vec<Span>),
}
pub struct StationColor {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    output: Rc<Output>,
    source: RefCell<Option<Source>>,
    job: RefCell<Option<slint::JoinHandle<()>>>,
    cancel: RefCell<Option<tokio::sync::oneshot::Sender<()>>>,
    closed: Cell<bool>,
}
fn stale() -> String {
    pttstella::msg!("editor-color-stale")
}
fn derive(frame: &Frame, key: &(Range<usize>, Vec<Span>)) -> Result<Selection, String> {
    let first = key.1.first().ok_or_else(stale)?;
    let last = key.1.last().ok_or_else(stale)?;
    let selection = Selection::Range {
        anchor: Position {
            row: first.row,
            col: first.start,
        },
        focus: Position {
            row: last.row,
            col: last.end.checked_sub(1).ok_or_else(stale)?,
        },
    };
    let selected = selection::extract(frame, &selection)?;
    if (selected.rows, selected.spans) != *key {
        return Err(stale());
    }
    Ok(selection)
}
impl StationColor {
    pub fn mount(ui: &TerminalWindow, connection: Rc<Connection>, output: Rc<Output>) -> Rc<Self> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            connection,
            output,
            source: RefCell::new(None),
            job: RefCell::new(None),
            cancel: RefCell::new(None),
            closed: Cell::new(false),
        });
        let state = ui.global::<StationColorState>();
        state.on_action({
            let o = owner.clone();
            move |code| match code {
                0 => o.open(),
                1 => o.apply(),
                2 => o.stop(true),
                3 => o.reset(),
                _ => {}
            }
        });
        state.on_choose({
            let o = owner.clone();
            move |index| o.choose(index)
        });
        ui.on_station_color_updated({
            let o = owner.clone();
            move || o.updated()
        });
        owner.paint_choice(ui);
        owner
    }
    fn current(&self, ui: &TerminalWindow) -> Option<Source> {
        let view = self.connection.updates.borrow();
        if self.closed.get()
            || !ui.get_native_foreground()
            || ui.get_native_composing()
            || ui.get_editor_open()
            || ui.get_media_open()
            || ui.get_find_open()
            || ui.get_switching_station()
            || ui.get_login_open()
            || ui.get_login_busy()
            || ui.get_paste_open()
            || ui.get_paste_reading()
            || ui.get_output_busy()
            || ui.get_keyboard_read_only()
            || ui.get_keyboard_pending()
            || view.phase != Phase::Connected
            || !view.site.ptt_features()
            || view.input_pending
            || view.output_pending
            || view.login_pending
            || view.mouse.route != pttstella::mouse::input::Route::Local
        {
            return None;
        }
        let stamp = self.output.current(ui)?;
        if view.stamp() != stamp {
            return None;
        }
        let key = self.output.selection_key()?;
        self.output.with_frame(ui, |frame| {
            let selection = derive(frame, &key).ok()?;
            pttstella::editor_color::Flow::new(
                frame,
                &selection,
                Color {
                    foreground: 15,
                    background: 0,
                },
            )
            .ok()?;
            Some(())
        })??;
        Some(Source {
            stamp,
            key,
            geometry: self.output.geometry(ui)?,
            policy: self.output.policy(ui),
        })
    }
    fn open(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        if self.job.borrow().is_some() || ui.get_terminal_pointer_blocked() {
            return;
        }
        match self.current(&ui) {
            Some(source) => {
                self.output.prepare_context(&ui);
                self.source.replace(Some(source));
                ui.invoke_reset_keyboard();
                let state = ui.global::<StationColorState>();
                state.set_status("".into());
                state.set_selected(state.get_foreground());
                state.set_open(true);
                ui.invoke_focus_station_color();
            }
            None => crate::notice::error(&ui, stale()),
        }
    }
    fn valid(&self, ui: &TerminalWindow) -> bool {
        self.source
            .borrow()
            .as_ref()
            .is_some_and(|source| self.current(ui).as_ref() == Some(source))
    }
    fn choose(&self, index: i32) {
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<StationColorState>();
        if !(0..24).contains(&index) || !state.get_open() || state.get_busy() {
            return;
        }
        if !self.valid(&ui) {
            self.dismiss(false);
            crate::notice::error(&ui, stale());
            return;
        }
        if index < 16 {
            state.set_foreground(index);
        } else {
            state.set_background(index - 16);
        }
        state.set_selected(index);
        self.paint_choice(&ui);
    }
    fn reset(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let s = ui.global::<StationColorState>();
        if s.get_busy() {
            return;
        }
        s.set_foreground(7);
        s.set_background(0);
        s.set_selected(7);
        self.paint_choice(&ui);
    }
    fn paint_choice(&self, ui: &TerminalWindow) {
        let state = ui.global::<StationColorState>();
        if let Ok(code) = (Color {
            foreground: state.get_foreground() as u8,
            background: state.get_background() as u8,
        })
        .sgr()
        {
            state.set_code(code.replace('\x1b', "ESC").into());
        }
    }
    fn apply(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<StationColorState>();
        if !state.get_open() || state.get_busy() {
            return;
        }
        if !self.valid(&ui) {
            self.dismiss(false);
            crate::notice::error(&ui, stale());
            return;
        }
        let Some(source) = self.source.borrow().clone() else {
            return;
        };
        let selection = self
            .output
            .with_frame(&ui, |frame| derive(frame, &source.key))
            .ok_or_else(stale)
            .and_then(|r| r);
        let reply = selection.and_then(|selection| {
            self.connection.request_editor_color(
                source.stamp,
                selection,
                Color {
                    foreground: state.get_foreground() as u8,
                    background: state.get_background() as u8,
                },
            )
        });
        let reply = match reply {
            Ok(r) => r,
            Err(e) => {
                crate::notice::error(&ui, e);
                self.dismiss(true);
                return;
            }
        };
        let (send, receive) = tokio::sync::oneshot::channel();
        self.cancel.replace(Some(send));
        state.set_busy(true);
        state.set_cancelling(false);
        state.set_status(pttstella::msg!("editor-color-working").into());
        ui.invoke_focus_station_color_actions();
        let weak = Rc::downgrade(self);
        match slint::spawn_local(async move {
            let result = tokio::select! {biased;_=receive=>None,result=reply.wait()=>Some(result)};
            if let Some(owner) = weak.upgrade() {
                owner.job.borrow_mut().take();
                owner.cancel.borrow_mut().take();
                if !owner.closed.get()
                    && let Some(ui) = owner.ui.upgrade()
                {
                    let same =
                        owner.connection.updates.borrow().generation == source.stamp.generation;
                    owner.dismiss(same && ui.get_native_foreground());
                    if same {
                        match result {
                            Some(Ok(())) => {
                                crate::notice::success(&ui, pttstella::msg!("editor-color-done"))
                            }
                            Some(Err(e)) => crate::notice::error(&ui, e),
                            None => crate::notice::persistent(
                                &ui,
                                pttstella::msg!("editor-color-stopped"),
                            ),
                        }
                    }
                }
            }
        }) {
            Ok(job) => {
                self.job.replace(Some(job));
            }
            Err(e) => {
                self.cancel.borrow_mut().take();
                self.dismiss(true);
                crate::notice::error(&ui, e.to_string());
            }
        }
    }
    fn stop(&self, focus: bool) {
        if let Some(send) = self.cancel.borrow_mut().take() {
            let _ = send.send(());
            if let Some(ui) = self.ui.upgrade() {
                let s = ui.global::<StationColorState>();
                s.set_cancelling(true);
                s.set_status(pttstella::msg!("editor-color-stop-requested").into());
            }
        } else if self.job.borrow().is_none() {
            self.dismiss(focus);
        }
    }
    fn dismiss(&self, focus: bool) {
        self.source.borrow_mut().take();
        if let Some(ui) = self.ui.upgrade() {
            let s = ui.global::<StationColorState>();
            s.set_busy(false);
            s.set_cancelling(false);
            s.set_open(false);
            s.set_status("".into());
            if focus && ui.get_context_focus_restorable() {
                ui.invoke_focus_terminal();
            }
        }
    }
    fn updated(&self) {
        let Some(ui) = self.ui.upgrade() else { return };
        let state = ui.global::<StationColorState>();
        if !state.get_open() {
            return;
        }
        if state.get_busy() {
            if !ui.get_native_foreground()
                || ui.get_switching_station()
                || self.source.borrow().as_ref().is_some_and(|s| {
                    s.stamp.generation != self.connection.updates.borrow().generation
                })
            {
                self.stop(false);
            }
        } else if !self.valid(&ui) {
            self.dismiss(ui.get_context_focus_restorable());
            crate::notice::error(&ui, stale());
        }
    }
    pub fn finish(&self) {
        self.closed.set(true);
        self.cancel.borrow_mut().take();
        if let Some(job) = self.job.borrow_mut().take() {
            job.abort();
        }
        self.dismiss(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconstruction_requires_exact_whole_glyph_ranges_and_all_middle_rows() {
        let wire = pttstella::editor_color::fixture::Editor::new(
            &["P中文Q", "", "第三行"],
            true,
            false,
            true,
        )
        .wire();
        let mut screen = pttstella::screen::Screen::new(80, 24).unwrap();
        screen.feed(&wire);
        let frame = screen.frame();
        let original = Selection::Range {
            anchor: Position { row: 2, col: 5 },
            focus: Position { row: 0, col: 2 },
        };
        let selected = selection::extract(&frame, &original).unwrap();
        let key = (selected.rows, selected.spans);
        let reconstructed = derive(&frame, &key).unwrap();
        let actual = selection::extract(&frame, &reconstructed).unwrap();
        assert_eq!(actual.rows, 0..3);
        assert_eq!(actual.spans, key.1);
        assert_eq!(&actual.text[..3], "中");
        let mut cut = key.clone();
        cut.1[0].start = 2;
        assert!(derive(&frame, &cut).is_err());
        let mut gap = key.clone();
        gap.1.remove(1);
        assert!(derive(&frame, &gap).is_err());
        let mut rows = key.clone();
        rows.0 = 0..4;
        assert!(derive(&frame, &rows).is_err());
    }
}
