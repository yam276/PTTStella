//! One persisted media preference baseline, merged by the shared settings store.
use super::*;
use pttstella::settings::{AppSettings, HoverPosition};

pub(super) enum Change {
    Hover(HoverPosition),
    Automatic(bool),
}
impl Media {
    pub(super) fn mount_preferences(self: &Rc<Self>, ui: &TerminalWindow) {
        ui.set_media_auto_label(msg!("auto-expand-images").into());
        ui.set_media_auto_help(msg!("auto-expand-images-help").into());
        let enabled = self.settings.borrow().auto_expand_images;
        ui.set_media_auto_enabled(enabled);
        ui.set_media_auto_choice(enabled);
        ui.on_media_auto_preference({
            let owner = self.clone();
            move |enabled| owner.save_preference(Change::Automatic(enabled))
        });
    }
    pub(super) fn save_preference(self: &Rc<Self>, change: Change) {
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if self.closed.get() || self.output.busy() || !ui.get_native_foreground() {
            return;
        }
        self.output.forget_hover();
        if self.presentation.get() == Presentation::Hover {
            self.close(false);
        }
        let baseline = self.settings.borrow().clone();
        let mut draft = baseline.clone();
        match change {
            Change::Hover(value) => draft.hover = value,
            Change::Automatic(value) => draft.auto_expand_images = value,
        }
        if draft == baseline {
            self.apply_preferences(&ui, baseline);
            return;
        }
        let store = self.store.clone();
        let weak = Rc::downgrade(self);
        ui.set_input_notice(msg!("settings-saving").into());
        self.output.work(
            &ui,
            move || store.save_from(&baseline, draft),
            move |_, ui, result| {
                let Some(owner) = weak.upgrade().filter(|o| !o.closed.get()) else {
                    return;
                };
                match result {
                    Ok(saved) => {
                        owner.apply_preferences(ui, saved);
                        ui.set_input_notice(msg!("settings-saved").into());
                    }
                    Err(error) => ui.set_input_notice(
                        format!("{} {error}", msg!("settings-save-failed")).into(),
                    ),
                }
            },
        );
    }
    fn apply_preferences(self: &Rc<Self>, ui: &TerminalWindow, saved: AppSettings) {
        let automatic = saved.auto_expand_images;
        let hover = super::hover::index(saved.hover);
        self.settings.replace(saved);
        if let Some(worker) = self.worker.borrow().as_ref() {
            worker.inline_automatic(automatic);
        }
        ui.set_media_hover_position(hover);
        ui.set_media_hover_choice(hover);
        ui.set_media_auto_enabled(automatic);
        ui.set_media_auto_choice(automatic);
        let inline = self.inline.borrow().clone();
        if let Some(inline) = inline {
            inline.set_automatic(automatic);
        }
    }
}
