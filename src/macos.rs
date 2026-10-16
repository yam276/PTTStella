//! Native macOS commands; quitting uses the same owned shutdown as closing.
use dioxus::desktop::muda::{
    self, Menu, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers},
};

pub const QUIT: &str = "pttstella-quit";

pub fn menu() -> Result<Menu, muda::Error> {
    let menu = Menu::new();
    let application = Submenu::new("PTTStella", true);
    application.append_items(&[
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(
            QUIT,
            "Quit PTTStella",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyQ)),
        ),
    ])?;
    let edit = Submenu::new("Edit", true);
    edit.append_items(&[
        &PredefinedMenuItem::undo(None),
        &PredefinedMenuItem::redo(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::cut(None),
        &PredefinedMenuItem::copy(None),
        &PredefinedMenuItem::paste(None),
        &PredefinedMenuItem::select_all(None),
    ])?;
    let window = Submenu::new("Window", true);
    window.append_items(&[
        &PredefinedMenuItem::minimize(None),
        &PredefinedMenuItem::maximize(None),
        &PredefinedMenuItem::fullscreen(None),
        &PredefinedMenuItem::close_window(None),
    ])?;
    window.set_as_windows_menu_for_nsapp();
    menu.append_items(&[&application, &edit, &window])?;
    Ok(menu)
}
