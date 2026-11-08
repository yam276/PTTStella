//! Inline image targets use the same source snapshot/menu lifetime as text.
use super::*;
use pttstella::media::{hover::Lease, loader::Image};
use std::sync::Arc;
#[derive(Clone)]
pub struct Picture {
    pub scope: Lease,
    pub image: Option<Arc<Image>>,
    pub probe: bool,
    pub tag: String,
}
impl ContextMenu {
    pub(super) fn picture_current(&self, picture: &Picture) -> bool {
        self.media
            .borrow()
            .upgrade()
            .and_then(|m| m.inline_picture(picture.scope.index as i32, &picture.tag))
            .is_some_and(|now| {
                now.scope.stamp == picture.scope.stamp
                    && now.scope.index == picture.scope.index
                    && now.scope.link == picture.scope.link
                    && match (&now.image, &picture.image) {
                        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                        (None, None) => true,
                        _ => false,
                    }
            })
    }
    pub(super) fn picture_pointer(
        self: &Rc<Self>,
        kind: i32,
        index: i32,
        tag: &str,
        point: (f32, f32),
    ) {
        if kind == 3 {
            self.press.take();
            return;
        }
        let Some(ui) = self.ui.upgrade() else {
            return;
        };
        if ui.get_context_open() || ui.get_context_pending() {
            self.dismiss(true);
            return;
        }
        let Some(source) = self.source(&ui).filter(|s| s.mouse.route == Route::Local) else {
            self.press.take();
            return;
        };
        let Some(picture) = self
            .media
            .borrow()
            .upgrade()
            .and_then(|m| m.inline_picture(index, tag))
        else {
            self.press.take();
            return;
        };
        if kind == 0 {
            self.press.replace(Some(Press {
                source,
                point,
                shift: false,
                picture: Some(picture),
            }));
            return;
        }
        if kind == 1 {
            if self
                .press
                .borrow()
                .as_ref()
                .is_some_and(|p| !press_matches(&source, p, point))
            {
                self.press.take();
            }
            return;
        }
        let Some(press) = self.press.take() else {
            return;
        };
        if kind != 2
            || !press_matches(&source, &press, point)
            || press
                .picture
                .as_ref()
                .is_none_or(|p| !self.picture_current(p))
        {
            return;
        }
        self.output.prepare_context(&ui);
        ui.invoke_reset_keyboard();
        let Some(source) = self.source(&ui) else {
            return;
        };
        let Some(picture) = self
            .media
            .borrow()
            .upgrade()
            .and_then(|m| m.inline_picture(index, tag))
        else {
            return;
        };
        let entries = self.entries(
            &ui,
            &source,
            Some(picture.scope.index),
            Some(&picture),
            None,
        );
        ui.set_context_x(point.0);
        ui.set_context_y(point.1);
        self.install_menu(
            &ui,
            source,
            Some(picture.scope.index),
            Some(picture),
            None,
            entries,
        );
    }
    pub(super) fn picture_action(&self, ui: &TerminalWindow, picture: Picture, action: Action) {
        use crate::output::picture::PictureAction as A;
        let action = match action {
            Action::CopyPicture => A::Copy,
            Action::SavePicture => A::Save,
            Action::CopyLink => A::CopyUrl,
            Action::OpenLink => {
                self.output
                    .context_action(action, Some(picture.scope.index));
                return;
            }
            _ => return,
        };
        self.output.picture_output(
            ui,
            &picture.scope,
            picture.image,
            action,
            picture.probe,
            |ui, message| ui.set_input_notice(message.into()),
        );
    }
}
