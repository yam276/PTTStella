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
        let enabled = self.preferences.app().auto_expand_images;
        ui.set_media_auto_enabled(enabled);
        ui.set_media_auto_choice(enabled);
        self.preferences.subscribe({
            let weak = Rc::downgrade(self);
            move |saved| {
                if let Some(owner) = weak.upgrade()
                    && let Some(ui) = owner.ui.upgrade()
                {
                    owner.apply_preferences(&ui, saved.clone())?;
                }
                Ok(())
            }
        });
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
        self.preferences.change(move |draft| match change {
            Change::Hover(value) => draft.hover = value,
            Change::Automatic(value) => draft.auto_expand_images = value,
        });
    }
    fn apply_preferences(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        saved: AppSettings,
    ) -> Result<(), String> {
        let automatic = saved.auto_expand_images;
        let hover = super::hover::index(saved.hover);
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
        Ok(())
    }
}
