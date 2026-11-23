//! Preview and inline menus share one lease, picker, clipboard and output worker.
use super::*;
use pttstella::media::{hover::Lease as MediaLease, loader::Image, picture};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(crate) enum PictureAction {
    Copy,
    Save,
    CopyUrl,
}
impl Output {
    pub(crate) fn picture_output(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        scope: &MediaLease,
        image: Option<Arc<Image>>,
        action: PictureAction,
        probe: bool,
        complete: impl FnOnce(&TerminalWindow, String, pttstella::notification::Kind) + 'static,
    ) {
        if self.closed.get() || self.busy() {
            complete(ui, msg!("file-busy"), pttstella::notification::Kind::Error);
            return;
        }
        let mut picker_opened = false;
        let mut cancelled = false;
        let prepared = (|| {
            if !matches!(action, PictureAction::CopyUrl) && image.is_none() {
                return Err(msg!("copy-not-ready"));
            }
            let (lease, phase) = self.media_output_lease(ui, scope)?;
            let path = if matches!(action, PictureAction::Save) {
                let extension =
                    picture::extension(image.as_ref().ok_or_else(|| msg!("copy-not-ready"))?)?;
                if !self.begin_picker(ui) {
                    return Err(msg!("file-busy"));
                }
                picker_opened = true;
                let path = ui
                    .window()
                    .with_winit_window(|window| {
                        rfd::FileDialog::new()
                            .set_parent(window)
                            .set_title(msg!("picture-save-title"))
                            .add_filter("Image", &[extension])
                            .set_file_name(format!("PTTStella-image.{extension}"))
                            .save_file()
                    })
                    .flatten();
                cancelled = path.is_none();
                Some(path.ok_or_else(|| msg!("export-cancelled"))?)
            } else {
                None
            };
            if self.closed.get() {
                return Err(msg!("export-stale"));
            }
            Ok((lease, phase, path))
        })();
        if picker_opened {
            self.end_picker(ui);
        }
        let (lease, phase, path) = match prepared {
            Ok(p) => p,
            Err(e) => {
                complete(
                    ui,
                    e,
                    if cancelled {
                        pttstella::notification::Kind::Persistent
                    } else {
                        pttstella::notification::Kind::Error
                    },
                );
                return;
            }
        };
        let clipboard = self.clipboard();
        let source = self.connection.updates.clone();
        let url = scope.link.url.clone();
        self.work(
            ui,
            move || {
                let verify = || check_output(&lease, &source.borrow(), phase);
                verify()?;
                if matches!(action, PictureAction::CopyUrl) {
                    clipboard.write_text(url.clone(), verify)?;
                    if probe {
                        crate::media_fixture::verify_url(&clipboard, &url)?;
                    }
                } else {
                    let image = image.ok_or_else(|| msg!("copy-not-ready"))?;
                    if let Some(path) = path {
                        picture::save(&path, &image, verify)?;
                    } else {
                        let mime = image.mime;
                        let pixels = picture::pixels(image)?;
                        clipboard.write_image(pixels, verify)?;
                        if probe {
                            crate::media_fixture::verify_picture(&clipboard, mime)?;
                        }
                    }
                }
                Ok(())
            },
            move |_, ui, result| {
                use pttstella::notification::Kind;
                let (message, kind) = match result {
                    Ok(()) => match action {
                        PictureAction::Save => (msg!("picture-saved"), Kind::Success),
                        PictureAction::CopyUrl => (msg!("context-copied"), Kind::Copy),
                        PictureAction::Copy => (msg!("export-copied"), Kind::Copy),
                    },
                    Err(e) => (e, Kind::Error),
                };
                complete(ui, message, kind)
            },
        );
    }
}
