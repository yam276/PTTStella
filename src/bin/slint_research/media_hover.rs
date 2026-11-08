//! One dwell timer and the same preview owner; hover never acquires input focus.
use super::*;
use pttstella::settings::HoverPosition;
use std::time::Duration;

fn position(index: i32) -> Option<HoverPosition> {
    match index {
        0 => Some(HoverPosition::TopRight),
        1 => Some(HoverPosition::FollowCursor),
        2 => Some(HoverPosition::Off),
        _ => None,
    }
}
pub(super) fn index(position: HoverPosition) -> i32 {
    match position {
        HoverPosition::TopRight => 0,
        HoverPosition::FollowCursor => 1,
        HoverPosition::Off => 2,
    }
}
impl Media {
    pub(super) fn mount_hover(self: &Rc<Self>, ui: &TerminalWindow) {
        ui.set_media_hover_label(msg!("hover-position").into());
        ui.set_media_hover_help(msg!("hover-help").into());
        ui.set_media_hover_choices(slint::ModelRc::new(slint::VecModel::from(vec![
            msg!("hover-top-right").into(),
            msg!("hover-follow").into(),
            msg!("close").into(),
        ])));
        let value = index(self.settings.borrow().hover);
        ui.set_media_hover_position(value);
        ui.set_media_hover_choice(value);
        ui.on_media_hover_changed({
            let owner = self.clone();
            move || owner.hover_changed()
        });
        ui.on_media_hover_preference({
            let owner = self.clone();
            move |value| owner.preference(value)
        });
        ui.set_media_available(true);
    }
    pub(super) fn hover_changed(self: &Rc<Self>) {
        if self.closed.get()
            || self.presentation.get() == Presentation::Inspect && self.active.borrow().is_some()
        {
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if ui.get_media_hover_blocked() {
            self.output.forget_hover();
            self.close(false);
            return;
        }
        if ui.get_media_hover_position() == 2 {
            self.close(false);
            return;
        }
        let Some((scope, point)) = self.output.media_hover_source(&ui) else {
            self.close(false);
            return;
        };
        if !matches!(media::classify(&scope.link.url),Ok(Some(target)) if target.kind==Kind::Image)
        {
            self.close(false);
            return;
        }
        ui.set_media_hover_x(point.0);
        ui.set_media_hover_y(point.1);
        let active = self.active.borrow().clone();
        if let Some(active) = active
            && active.same_source(&scope)
            && self.valid(&ui, &active)
        {
            return;
        }
        let pending = self.candidate.borrow().clone();
        if pending.as_ref().is_some_and(|old| old.same_source(&scope)) {
            self.candidate.replace(Some(scope));
            return;
        }
        self.close(false);
        self.candidate.replace(Some(scope));
        let weak = Rc::downgrade(self);
        self.dwell.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(350),
            move || {
                let Some(owner) = weak.upgrade() else {
                    return;
                };
                let candidate = owner.candidate.borrow_mut().take();
                let Some(ui) = owner.ui.upgrade() else {
                    return;
                };
                if let Some(candidate) = candidate
                    && !owner.closed.get()
                    && !ui.get_media_hover_blocked()
                    && ui.get_media_hover_position() != 2
                    && let Some((current, _)) = owner.output.media_hover_source(&ui)
                    && candidate.same_source(&current)
                {
                    owner.begin(&ui, current, Presentation::Hover);
                }
            },
        );
    }
    fn preference(self: &Rc<Self>, value: i32) {
        let Some(position) = position(value) else {
            return;
        };
        self.save_preference(super::preferences::Change::Hover(position));
    }
}
