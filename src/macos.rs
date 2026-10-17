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
        &PredefinedMenuItem::hide(Some(&pttstella::msg!("menu-hide"))),
        &PredefinedMenuItem::hide_others(Some(&pttstella::msg!("menu-hide-others"))),
        &PredefinedMenuItem::show_all(Some(&pttstella::msg!("menu-show-all"))),
        &PredefinedMenuItem::separator(),
        &MenuItem::with_id(
            QUIT,
            pttstella::msg!("menu-quit"),
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyQ)),
        ),
    ])?;
    let edit = Submenu::new(pttstella::msg!("menu-edit"), true);
    edit.append_items(&[
        &PredefinedMenuItem::undo(Some(&pttstella::msg!("menu-undo"))),
        &PredefinedMenuItem::redo(Some(&pttstella::msg!("menu-redo"))),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::cut(Some(&pttstella::msg!("menu-cut"))),
        &PredefinedMenuItem::copy(Some(&pttstella::msg!("menu-copy"))),
        &PredefinedMenuItem::paste(Some(&pttstella::msg!("menu-paste"))),
        &PredefinedMenuItem::select_all(Some(&pttstella::msg!("menu-select-all"))),
    ])?;
    let window = Submenu::new(pttstella::msg!("menu-window"), true);
    window.append_items(&[
        &PredefinedMenuItem::minimize(Some(&pttstella::msg!("menu-minimize"))),
        &PredefinedMenuItem::maximize(Some(&pttstella::msg!("menu-maximize"))),
        &PredefinedMenuItem::fullscreen(Some(&pttstella::msg!("menu-fullscreen"))),
        &PredefinedMenuItem::close_window(Some(&pttstella::msg!("menu-close"))),
    ])?;
    window.set_as_windows_menu_for_nsapp();
    menu.append_items(&[&application, &edit, &window])?;
    Ok(menu)
}
