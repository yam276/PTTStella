//! Explicit native conformance tasks; owns cancellation and synthetic toolkit input.
use crate::TerminalWindow;
use slint::{
    ComponentHandle,
    platform::{Key, PointerEventButton as Button, WindowEvent as Event},
};
use std::{
    cell::RefCell, error::Error, fs::OpenOptions, future::Future, path::PathBuf, rc::Rc,
    time::Duration,
};
pub struct Verification {
    job: slint::JoinHandle<()>,
    outcome: Rc<RefCell<Option<Result<(), String>>>>,
    name: &'static str,
}
impl Verification {
    pub fn start<F, Fut>(
        ui: &TerminalWindow,
        directory: PathBuf,
        name: &'static str,
        run: F,
    ) -> Result<Self, Box<dyn Error>>
    where
        F: FnOnce(TerminalWindow, PathBuf) -> Fut + 'static,
        Fut: Future<Output = Result<(), String>> + 'static,
    {
        Self::start_inner(ui, directory, name, true, run)
    }
    /// A successful preparation leaves the native window for actual OS close.
    /// Preparation errors still use the normal exit owner and remain failures.
    #[cfg(feature = "native-video")]
    pub fn prepare_for_native_close<F, Fut>(
        ui: &TerminalWindow,
        directory: PathBuf,
        name: &'static str,
        run: F,
    ) -> Result<Self, Box<dyn Error>>
    where
        F: FnOnce(TerminalWindow, PathBuf) -> Fut + 'static,
        Fut: Future<Output = Result<(), String>> + 'static,
    {
        Self::start_inner(ui, directory, name, false, run)
    }
    pub(crate) fn start_inner<F, Fut>(
        ui: &TerminalWindow,
        directory: PathBuf,
        name: &'static str,
        automatic_close: bool,
        run: F,
    ) -> Result<Self, Box<dyn Error>>
    where
        F: FnOnce(TerminalWindow, PathBuf) -> Fut + 'static,
        Fut: Future<Output = Result<(), String>> + 'static,
    {
        ui.window().set_size(slint::LogicalSize::new(1100., 960.));
        let outcome = Rc::new(RefCell::new(None));
        let result = outcome.clone();
        let weak = ui.as_weak();
        let job = slint::spawn_local(async move {
            let checked = match weak.upgrade() {
                Some(ui) => run(ui, directory).await,
                None => Err(format!("{name} probe window closed before verification")),
            };
            if let Err(error) = &checked {
                eprintln!("{name} probe failed: {error}");
            }
            let should_close = automatic_close || checked.is_err();
            result.replace(Some(checked));
            if should_close && let Some(ui) = weak.upgrade() {
                ui.invoke_app_quit();
            }
        })?;
        Ok(Self { job, outcome, name })
    }
    pub fn finish(self) -> Result<(), String> {
        self.job.abort();
        self.outcome
            .take()
            .ok_or_else(|| format!("{} probe closed before completion", self.name))??;
        eprintln!(
            "Full native app {} verification passed. Toolkit events and synthetic foreground/buttons only; OS input, clipboard, picker and browser actions remain untested.",
            self.name
        );
        Ok(())
    }
}
pub fn require(value: bool, message: &str) -> Result<(), String> {
    value.then_some(()).ok_or_else(|| message.to_owned())
}
/// The owned history fixture has a thin glyph over blue/magenta half-cells.
/// Font rasterizers can put ink on a cell corner, so verify each half's opaque
/// background area instead of assuming that two corner pixels contain no ink.
pub fn check_dual_backgrounds(rgba: &[u8], first_col: usize) -> Result<(), String> {
    require(
        rgba.len() == 960 * 24 * 4 && first_col <= 78,
        "dual-colour fixture dimensions differ",
    )?;
    for (col, expected) in [
        (first_col, [0, 0, 128, 255]),
        (first_col + 1, [128, 0, 128, 255]),
    ] {
        let matching = (0..24)
            .flat_map(|y| (col * 12..(col + 1) * 12).map(move |x| (y * 960 + x) * 4))
            .filter(|&offset| rgba[offset..offset + 4] == expected)
            .count();
        require(
            matching > 12 * 24 / 2,
            "dual-colour fixture background differs",
        )?;
    }
    Ok(())
}
/// An explicit toolkit activation, never an OS focus or a production bypass.
/// Synthetic foreground alone cannot make a deactivated TextInput own focus.
pub fn foreground(ui: &TerminalWindow, active: bool) {
    ui.window()
        .dispatch_event(Event::WindowActiveChanged(active));
    ui.set_native_foreground(active);
}
pub async fn wait(mut condition: impl FnMut() -> bool, message: &str) -> Result<(), String> {
    for _ in 0..150 {
        if condition() {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Err(message.into())
}
pub fn key(ui: &TerminalWindow, key: Key) {
    ui.window()
        .dispatch_event(Event::KeyPressed { text: key.into() });
    ui.window()
        .dispatch_event(Event::KeyReleased { text: key.into() });
}
pub fn click(ui: &TerminalWindow, position: slint::LogicalPosition, button: Button) {
    ui.set_native_pointer_buttons(if button == Button::Right { 2 } else { 1 });
    ui.set_native_pointer_down(true);
    ui.window()
        .dispatch_event(Event::PointerPressed { position, button });
    ui.set_native_pointer_buttons(0);
    ui.set_native_pointer_down(false);
    ui.window()
        .dispatch_event(Event::PointerReleased { position, button });
}
pub fn capture<C: ComponentHandle>(
    ui: &C,
    directory: &std::path::Path,
    name: &str,
) -> Result<(), String> {
    let pixels = ui.window().take_snapshot().map_err(|e| e.to_string())?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(name))
        .map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(file, pixels.width(), pixels.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels.as_bytes())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::check_dual_backgrounds;

    fn fixture() -> Vec<u8> {
        let mut image = [0, 0, 0, 255].repeat(960 * 24);
        for y in 0..24 {
            for (range, colour) in [(72..84, [0, 0, 128, 255]), (84..96, [128, 0, 128, 255])] {
                for x in range {
                    image[(y * 960 + x) * 4..(y * 960 + x) * 4 + 4].copy_from_slice(&colour);
                }
            }
        }
        image
    }

    #[test]
    fn background_fixture_allows_glyph_ink_at_a_corner() {
        let mut image = fixture();
        image[84 * 4..84 * 4 + 4].copy_from_slice(&[255, 255, 255, 255]);
        assert!(check_dual_backgrounds(&image, 6).is_ok());
    }

    #[test]
    fn two_correct_corner_pixels_cannot_hide_a_missing_background() {
        let mut image = [0, 0, 0, 255].repeat(960 * 24);
        image[72 * 4..72 * 4 + 4].copy_from_slice(&[0, 0, 128, 255]);
        image[84 * 4..84 * 4 + 4].copy_from_slice(&[128, 0, 128, 255]);
        assert!(check_dual_backgrounds(&image, 6).is_err());
    }

    #[test]
    fn wrong_half_colour_and_invalid_fixture_bounds_are_rejected() {
        let mut image = fixture();
        for y in 0..24 {
            for x in 84..96 {
                image[(y * 960 + x) * 4..(y * 960 + x) * 4 + 4].copy_from_slice(&[0, 0, 128, 255]);
            }
        }
        assert!(check_dual_backgrounds(&image, 6).is_err());
        assert!(check_dual_backgrounds(&fixture()[..960 * 24 * 4 - 1], 6).is_err());
        assert!(check_dual_backgrounds(&fixture(), usize::MAX).is_err());
    }
}
