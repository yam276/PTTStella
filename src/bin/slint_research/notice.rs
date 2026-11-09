//! A single weak-root notification owner; no idle polling or queued notices.
use crate::TerminalWindow;
use pttstella::notification::{Kind, Policy, State};
use slint::{ComponentHandle, Timer, TimerMode};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

pub fn show(ui: &TerminalWindow, text: impl Into<slint::SharedString>, kind: Kind) {
    ui.invoke_notice_publish(
        text.into(),
        match kind {
            Kind::Persistent => 0,
            Kind::Error => 1,
            Kind::Copy => 2,
            Kind::Success => 3,
        },
    );
}
pub fn error(ui: &TerminalWindow, text: impl Into<slint::SharedString>) {
    show(ui, text, Kind::Error);
}
pub fn persistent(ui: &TerminalWindow, text: impl Into<slint::SharedString>) {
    show(ui, text, Kind::Persistent);
}
pub fn copied(ui: &TerminalWindow, text: impl Into<slint::SharedString>) {
    show(ui, text, Kind::Copy);
}
pub fn success(ui: &TerminalWindow, text: impl Into<slint::SharedString>) {
    show(ui, text, Kind::Success);
}
pub fn clear(ui: &TerminalWindow) {
    ui.invoke_notice_publish("".into(), 0);
}

pub struct Owner {
    ui: slint::Weak<TerminalWindow>,
    state: RefCell<State>,
    text: RefCell<slint::SharedString>,
    timer: Timer,
    ticket: RefCell<Rc<()>>,
    clock: Instant,
    stopped: Cell<bool>,
}
impl Owner {
    pub fn mount(ui: &TerminalWindow) -> Result<Rc<Self>, String> {
        let owner = Rc::new(Self {
            ui: ui.as_weak(),
            state: RefCell::new(State::new(
                Policy::embedded()?,
                ui.get_tools_reduced_motion(),
            )),
            text: RefCell::new("".into()),
            timer: Timer::default(),
            ticket: RefCell::new(Rc::new(())),
            clock: Instant::now(),
            stopped: Cell::new(false),
        });
        ui.on_notice_publish({
            let weak = Rc::downgrade(&owner);
            move |text, kind| {
                if let Some(owner) = weak.upgrade() {
                    owner.publish(text, kind);
                }
            }
        });
        ui.on_notice_interaction({
            let weak = Rc::downgrade(&owner);
            move || {
                if let Some(owner) = weak.upgrade() {
                    owner.interaction();
                }
            }
        });
        ui.on_notice_dismiss({
            let weak = Rc::downgrade(&owner);
            move || {
                if let Some(owner) = weak.upgrade() {
                    if let Some(ui) = owner.ui.upgrade() {
                        ui.set_result(ui.get_connection_status());
                    }
                    owner.publish("".into(), 0);
                }
            }
        });
        Ok(owner)
    }
    fn publish(self: &Rc<Self>, text: slint::SharedString, kind: i32) {
        if self.stopped.get() {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            self.stop();
            return;
        };
        if ui.get_app_closing() {
            self.quiesce();
            return;
        }
        let now = self.clock.elapsed();
        let mut state = self.state.borrow_mut();
        if text.is_empty() {
            state.clear();
            self.text.replace("".into());
        } else {
            if text.trim().is_empty() {
                return;
            }
            // Direct diagnostic setters are persistent; never let a timer for
            // an older owner text dismiss their replacement.
            if ui.get_input_notice() != *self.text.borrow() {
                state.clear();
            }
            let kind = match kind {
                1 => Kind::Error,
                2 => Kind::Copy,
                3 => Kind::Success,
                _ => Kind::Persistent,
            };
            if !state.show(now, kind) {
                return;
            }
            state.interaction(
                now,
                ui.get_notice_hovered(),
                ui.get_notice_focused(),
                ui.get_tools_reduced_motion(),
            );
            self.text.replace(text);
        }
        drop(state);
        self.render(&ui);
        self.arm();
    }
    fn interaction(self: &Rc<Self>) {
        let Some(ui) = self.ui.upgrade() else {
            self.stop();
            return;
        };
        if ui.get_app_closing() {
            self.quiesce();
            return;
        }
        if self.stopped.get() {
            return;
        }
        if ui.get_input_notice() != *self.text.borrow() {
            let text = ui.get_input_notice();
            let mut state = self.state.borrow_mut();
            state.clear();
            if !text.is_empty() {
                state.show(self.clock.elapsed(), Kind::Persistent);
            }
            self.text.replace(text);
        }
        self.state.borrow_mut().interaction(
            self.clock.elapsed(),
            ui.get_notice_hovered(),
            ui.get_notice_focused(),
            ui.get_tools_reduced_motion(),
        );
        self.render(&ui);
        self.arm();
    }
    fn render(&self, ui: &TerminalWindow) {
        let state = self.state.borrow();
        if !state.visible() {
            self.text.replace("".into());
        }
        ui.set_input_notice(self.text.borrow().clone());
        ui.set_notice_error(state.error());
        ui.set_notice_fading(state.fading());
        ui.set_notice_fade_duration(
            state
                .delay()
                .map_or(0, |d| d.as_millis().min(i64::MAX as u128) as i64),
        );
    }
    fn arm(self: &Rc<Self>) {
        self.timer.stop();
        let ticket = Rc::new(());
        self.ticket.replace(ticket.clone());
        if self.stopped.get() {
            return;
        }
        if let Some(delay) = self.state.borrow().delay() {
            let weak = Rc::downgrade(self);
            self.timer.start(TimerMode::SingleShot, delay, move || {
                let Some(owner) = weak.upgrade() else {
                    return;
                };
                if owner.stopped.get() || !Rc::ptr_eq(&ticket, &owner.ticket.borrow()) {
                    return;
                }
                let Some(ui) = owner.ui.upgrade() else {
                    owner.stop();
                    return;
                };
                if ui.get_app_closing() {
                    owner.quiesce();
                    return;
                }
                if ui.get_input_notice() != *owner.text.borrow() {
                    owner.stop_timer();
                    owner.state.borrow_mut().clear();
                    ui.set_notice_fading(false);
                    ui.set_notice_error(false);
                    return;
                }
                owner.state.borrow_mut().expire(owner.clock.elapsed());
                owner.render(&ui);
                owner.arm();
            });
        }
    }
    fn stop_timer(&self) {
        self.timer.stop();
        self.ticket.replace(Rc::new(()));
    }
    fn quiesce(&self) {
        // A failed spawn in Exit can restore the window and allow retry.
        // Revoke the current deadline immediately without permanently
        // disabling feedback before that exit has actually completed.
        self.stop_timer();
        self.state.borrow_mut().clear();
    }
    pub fn stop(&self) {
        self.stopped.set(true);
        self.stop_timer();
        self.state.borrow_mut().stop();
    }
    pub fn clean(&self) -> bool {
        self.stopped.get() && !self.timer.running() && self.state.borrow().delay().is_none()
    }
    pub fn timer_active(&self) -> bool {
        self.timer.running()
    }
    pub fn audit_closed(&self, path: &std::path::Path) -> Result<(), String> {
        if !self.clean() {
            return Err("notification timer did not stop".into());
        }
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.join("notice-closed.json"))
            .map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(file,&serde_json::json!({"status":"passed","stopped":true,"timer_running":self.timer.running(),"model_deadline":false})).map_err(|e|e.to_string())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.timer.stop();
    }
}
