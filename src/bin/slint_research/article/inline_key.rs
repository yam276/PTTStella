//! One physical held Enter, one deadline; no repeat can become station input.
use super::Inline;
use crate::pointer::SurfaceGeometry;
use pttstella::media::inline::Target;
use std::{rc::Rc, time::Duration};

pub(super) struct Press {
    epoch: u64,
    geometry: SurfaceGeometry,
    fired: bool,
}

impl Inline {
    fn enter_allowed(&self) -> bool {
        self.ui.upgrade().is_some_and(|ui| {
            !self.closed.get()
                && ui.get_inline_enabled()
                && ui.get_native_foreground()
                && ui.get_article_focused()
                && !ui.get_native_composing()
                && !ui.get_keyboard_pending()
                && (!ui.get_vim_enabled() || ui.get_keyboard_read_only())
                && !self.candidates.borrow().is_empty()
        })
    }
    fn valid_enter(&self, press: &Press) -> bool {
        self.enter_allowed()
            && press.epoch == self.epoch.get()
            && self.article.upgrade().and_then(|a| a.output_geometry())
                == Some(press.geometry.clone())
    }
    pub(super) fn cancel_enter(&self) {
        self.enter_timer.stop();
        self.enter_press.take();
    }
    pub(super) fn validate_enter(&self) {
        let invalid = self
            .enter_press
            .borrow()
            .as_ref()
            .is_some_and(|p| !self.valid_enter(p));
        if invalid {
            self.cancel_enter();
        }
    }
    pub(super) fn enter_key(self: &Rc<Self>, down: bool, repeat: bool, eligible: bool) -> bool {
        if !down {
            let held = self.enter_held.replace(false);
            let press = self.enter_press.take();
            self.enter_timer.stop();
            if let Some(press) = press
                && !press.fired
                && eligible
                && self.valid_enter(&press)
            {
                self.single_visible();
            }
            return held;
        }
        if self.enter_held.get() {
            if !eligible {
                self.cancel_enter();
            }
            return true;
        }
        if !eligible || !self.enter_allowed() {
            return false;
        }
        // A repeat after cancellation is consumed without arming another press.
        if repeat {
            return true;
        }
        let Some(geometry) = self.article.upgrade().and_then(|a| a.output_geometry()) else {
            return false;
        };
        self.enter_held.set(true);
        self.enter_press.replace(Some(Press {
            epoch: self.epoch.get(),
            geometry,
            fired: false,
        }));
        let weak = Rc::downgrade(self);
        self.enter_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(500),
            move || {
                let Some(owner) = weak.upgrade() else {
                    return;
                };
                let fire = owner
                    .enter_press
                    .borrow()
                    .as_ref()
                    .is_some_and(|p| owner.valid_enter(p));
                if !fire {
                    owner.cancel_enter();
                    return;
                }
                if let Some(press) = owner.enter_press.borrow_mut().as_mut() {
                    press.fired = true;
                }
                if let Err(error) = owner.request(Some(Target::All))
                    && let Some(ui) = owner.ui.upgrade()
                {
                    ui.set_input_notice(error.into());
                }
            },
        );
        true
    }
    fn single_visible(self: &Rc<Self>) {
        use slint::Model;
        let (Some(ui), Some(article)) = (self.ui.upgrade(), self.article.upgrade()) else {
            return;
        };
        let native = article.view.borrow().clone();
        let Some(native) = native else {
            return;
        };
        let (Some(layout), Some(flow)) = (native.layout.get(), native.flow()) else {
            return;
        };
        let offset = native.position(&flow);
        let mut visible: Vec<_> = self
            .candidates
            .borrow()
            .iter()
            .copied()
            .filter(|(index, row, _, _)| {
                let top = (flow.top(*row) as f64 - offset) / f64::from(layout.dpi);
                top < f64::from(layout.body_height) && top + f64::from(layout.line_height()) > 0.
                    || (0..self.pictures.row_count())
                        .filter_map(|n| self.pictures.row_data(n))
                        .any(|p| p.index == *index as i32)
            })
            .collect();
        visible.sort_by_key(|(index, row, _, end)| (*row, *end, *index));
        let state = self.state.borrow();
        // A queued but unacknowledged request still selects the same index.
        let next = visible
            .iter()
            .find(|(index, _, _, _)| !state.entries.iter().any(|e| e.index == *index));
        if let Some((index, _, _, _)) = next {
            let index = *index;
            drop(state);
            if let Err(error) = self.request(Some(Target::Link { index })) {
                ui.set_input_notice(error.into());
            }
            return;
        }
        let key = visible.iter().find_map(|(index, _, _, _)| {
            let entry = state.entries.iter().find(|e| e.index == *index)?;
            (!self.wide.borrow().contains(&entry.key)
                && state
                    .resources
                    .get(&entry.key)
                    .is_none_or(|r| r.error.is_none()))
            .then_some(entry.key)
        });
        drop(state);
        if let Some(key) = key {
            self.wide.borrow_mut().insert(key);
            self.update();
            self.schedule();
        }
    }
}
