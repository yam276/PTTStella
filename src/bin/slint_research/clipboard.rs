//! Retain the native clipboard owner for X11 selection ownership after a write.
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct Clipboard(Arc<Mutex<Option<arboard::Clipboard>>>);

impl Clipboard {
    fn with<T>(
        &self,
        work: impl FnOnce(&mut arboard::Clipboard) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut owner = self.0.lock().map_err(|_| pttstella::msg!("copy-failed"))?;
        if owner.is_none() {
            *owner =
                Some(arboard::Clipboard::new().map_err(|_| pttstella::msg!("paste-unavailable"))?);
        }
        let clipboard = owner
            .as_mut()
            .ok_or_else(|| pttstella::msg!("paste-unavailable"))?;
        work(clipboard)
    }
    pub fn read_text(&self) -> Result<String, String> {
        self.with(|clipboard| {
            clipboard
                .get_text()
                .map_err(|_| pttstella::msg!("paste-unavailable"))
        })
        .map_err(|_| pttstella::msg!("paste-unavailable"))
    }
    pub fn read_image(&self) -> Result<arboard::ImageData<'static>, String> {
        self.with(|clipboard| {
            clipboard
                .get_image()
                .map_err(|_| pttstella::msg!("copy-failed"))
        })
    }
    pub fn write_text(
        &self,
        text: String,
        verify: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        self.with(|clipboard| {
            verify()?;
            clipboard
                .set_text(text)
                .map_err(|_| pttstella::msg!("copy-failed"))
        })
    }
    pub fn write_image(
        &self,
        image: arboard::ImageData<'static>,
        verify: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        self.with(|clipboard| {
            verify()?;
            clipboard
                .set_image(image)
                .map_err(|_| pttstella::msg!("copy-failed"))
        })
    }
}
