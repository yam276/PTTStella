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
        ui.window().set_size(slint::LogicalSize::new(1100., 960.));
        let outcome = Rc::new(RefCell::new(None));
        let result = outcome.clone();
        let weak = ui.as_weak();
        let job = slint::spawn_local(async move {
            let checked = match weak.upgrade() {
                Some(ui) => run(ui, directory).await,
                None => Err(format!("{name} probe window closed before verification")),
            };
            result.replace(Some(checked));
            if let Err(error) = slint::quit_event_loop() {
                result.replace(Some(Err(error.to_string())));
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
pub fn capture(ui: &TerminalWindow, directory: &std::path::Path, name: &str) -> Result<(), String> {
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
