//! Native mouse reports reuse Connection's single writer and priority controls.
#[path = "station_mouse/state.rs"]
mod state;
#[cfg(test)]
#[path = "station_mouse/tests.rs"]
mod tests;
use crate::{TerminalWindow, pointer::Geometry, renderer::Size, session::FrameId};
use pttstella::{
    connection::Connection,
    mouse::{Button, Modifiers, input::Route},
};
use slint::{ComponentHandle, Timer, TimerMode};
use state::{Context, Effect, State};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Instant, SystemTime},
};

pub struct StationMouse {
    ui: slint::Weak<TerminalWindow>,
    connection: Rc<Connection>,
    size: Rc<Cell<Option<Size>>>,
    last_size: Cell<Option<Size>>,
    presented: Rc<Cell<Option<FrameId>>>,
    state: RefCell<State>,
    timer: Timer,
    closed: Cell<bool>,
    status: Cell<Option<(bool, Route)>>,
}
impl StationMouse {
    pub fn mount(
        ui: &TerminalWindow,
        connection: Rc<Connection>,
        size: Rc<Cell<Option<Size>>>,
        presented: Rc<Cell<Option<FrameId>>>,
    ) -> Rc<Self> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            connection,
            size,
            last_size: Cell::new(None),
            presented,
            state: RefCell::new(State::default()),
            timer: Timer::default(),
            closed: Cell::new(false),
            status: Cell::new(None),
        });
        ui.set_station_mouse_available(true);
        ui.on_station_pointer({
            let owner = owner.clone();
            move |kind, button, x, y, shift, alt, ctrl, meta| {
                let button = match button {
                    1 => Some(Button::Left),
                    2 => Some(Button::Middle),
                    3 => Some(Button::Right),
                    _ => None,
                };
                owner.pointer(kind, button, (x, y), Modifiers { shift, alt, ctrl }, meta)
            }
        });
        ui.on_station_wheel({
            let owner = owner.clone();
            move |x, y, delta, shift, alt, ctrl, meta| {
                owner.wheel((x, y), delta, Modifiers { shift, alt, ctrl }, meta)
            }
        });
        ui.on_station_mouse_updated({
            let owner = owner.clone();
            move || owner.update()
        });
        ui.on_station_mouse_cancel({
            let owner = owner.clone();
            move || owner.cancel()
        });
        ui.on_toggle_station_mouse({
            let owner = owner.clone();
            move || {
                if owner.closed.get() {
                    return;
                }
                if let Some(ui) = owner.ui.upgrade() {
                    owner.cancel();
                    let (generation, enabled) = {
                        let view = owner.connection.updates.borrow();
                        (view.generation, view.mouse.enabled)
                    };
                    owner.connection.mouse_enabled(generation, !enabled);
                    ui.invoke_clear_selection();
                    ui.invoke_focus_terminal();
                }
            }
        });
        owner.update();
        owner
    }
    fn reduce<R>(
        &self,
        ui: &TerminalWindow,
        reduce: impl FnOnce(&mut State, &Context<'_>) -> R,
    ) -> R {
        let size = self.size.get();
        if size.is_some() {
            self.last_size.set(size);
        }
        // Evaluate reactive UI geometry before borrowing the connection or reducer.
        let geometry = Geometry::for_ui(ui, size.or(self.last_size.get()));
        let presented = self.presented.get();
        let painting = ui.get_frame_paint_pending();
        let blocked = ui.get_station_pointer_blocked();
        let selected = ui.get_selection_active();
        let buttons = ui.get_native_pointer_buttons().clamp(0, 7) as u8;
        let view = self.connection.updates.borrow();
        let context = Context {
            view: &view,
            geometry,
            presented: size.is_some()
                && !painting
                && presented
                    == Some(FrameId {
                        generation: view.generation,
                        revision: view.frame.revision,
                    }),
            blocked,
            selected,
            buttons,
            now: Instant::now(),
            wall: SystemTime::now(),
        };
        reduce(&mut self.state.borrow_mut(), &context)
    }
    fn pointer(
        self: &Rc<Self>,
        kind: i32,
        button: Option<Button>,
        point: (f32, f32),
        modifiers: Modifiers,
        meta: bool,
    ) -> bool {
        let Some(ui) = self.ui.upgrade().filter(|_| !self.closed.get()) else {
            return true;
        };
        let dpi = ui.window().scale_factor();
        let (consumed, effects) = self.reduce(&ui, |state, c| {
            state.pointer(
                c,
                kind,
                button,
                (point.0 * dpi, point.1 * dpi),
                modifiers,
                meta,
            )
        });
        self.apply(&ui, effects);
        consumed
    }
    fn wheel(
        self: &Rc<Self>,
        point: (f32, f32),
        delta: f32,
        modifiers: Modifiers,
        meta: bool,
    ) -> bool {
        let Some(ui) = self.ui.upgrade().filter(|_| !self.closed.get()) else {
            return true;
        };
        let dpi = ui.window().scale_factor();
        let (consumed, effects) = self.reduce(&ui, |state, c| {
            state.wheel(c, (point.0 * dpi, point.1 * dpi), delta, modifiers, meta)
        });
        self.apply(&ui, effects);
        consumed
    }
    fn update(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade().filter(|_| !self.closed.get()) else {
            return;
        };
        let effects = self.reduce(&ui, |state, c| state.observe(c));
        self.apply(&ui, effects);
    }
    fn apply(self: &Rc<Self>, ui: &TerminalWindow, effects: Vec<Effect>) {
        for effect in effects {
            let result = match effect {
                Effect::Prepare => {
                    ui.invoke_clear_selection();
                    ui.invoke_reset_keyboard();
                    ui.invoke_focus_terminal();
                    Ok(())
                }
                Effect::Edge(event) => self.connection.mouse_report(event),
                Effect::Sample(sample) => self.connection.mouse_sample(sample),
                Effect::Cancel(lease) => {
                    self.connection.cancel_mouse(lease);
                    Ok(())
                }
                Effect::Revoke(generation) => {
                    self.connection.revoke_mouse(generation);
                    Ok(())
                }
            };
            if let Err(error) = result {
                let generation = self.connection.updates.borrow().generation;
                self.state.borrow_mut().revoke(generation);
                self.connection.revoke_mouse(generation);
                crate::notice::error(ui, error);
                break;
            }
        }
        let status = {
            let view = self.connection.updates.borrow();
            (view.mouse.enabled, view.mouse.route)
        };
        ui.set_station_mouse_enabled(status.0);
        if self.status.replace(Some(status)) != Some(status) {
            let label = if status.0 {
                pttstella::msg!("station-mouse-on")
            } else {
                pttstella::msg!("station-mouse-off")
            };
            let message = match status.1 {
                Route::Local => pttstella::msg!("station-mouse-local"),
                Route::Station => pttstella::msg!("station-mouse-active"),
                Route::Blocked => pttstella::msg!("station-mouse-blocked"),
            };
            ui.set_station_mouse_label(label.into());
            ui.set_station_mouse_status(message.into());
        }
        let held = self.state.borrow().active();
        if ui.get_station_mouse_held() != held {
            ui.set_station_mouse_held(held);
            ui.invoke_keep_alive_changed();
        }
        self.timer.stop();
        let after = self.state.borrow().wake_after(Instant::now());
        if let Some(after) = after {
            let weak = Rc::downgrade(self);
            self.timer.start(TimerMode::SingleShot, after, move || {
                if let Some(owner) = weak.upgrade()
                    && let Some(ui) = owner.ui.upgrade()
                    && !owner.closed.get()
                {
                    let effects = owner.reduce(&ui, |state, c| state.flush(c, false));
                    owner.apply(&ui, effects);
                }
            });
        }
    }
    fn cancel(self: &Rc<Self>) {
        if self.closed.get() {
            return;
        }
        if let Some(ui) = self.ui.upgrade() {
            let (generation, local) = {
                let view = self.connection.updates.borrow();
                (view.generation, view.mouse.route == Route::Local)
            };
            let effects = self.state.borrow_mut().revoke(generation);
            // Off/disabled modes have already invalidated every station lease.
            // Ordinary local keys and geometry updates need no additional watch write.
            self.apply(&ui, if local { Vec::new() } else { effects });
        }
    }
    pub fn finish(&self) {
        self.closed.set(true);
        self.timer.stop();
        let generation = self.connection.updates.borrow().generation;
        self.state.borrow_mut().revoke(generation);
        self.connection.revoke_mouse(generation);
        if let Some(ui) = self.ui.upgrade() {
            ui.set_station_mouse_held(false);
        }
    }
}
