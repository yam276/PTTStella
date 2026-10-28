//! Native macOS commands; quitting uses the same owned shutdown as closing.
use dioxus::desktop::muda::{
    self, Menu, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, Code, Modifiers},
};

pub const SETTINGS: &str = "pttstella-settings";
pub const CLOSE_SETTINGS: &str = "pttstella-settings-close";
pub const QUIT: &str = "pttstella-quit";
pub const FULLSCREEN: &str = "pttstella-fullscreen";

fn build_menu(visible: bool) -> Result<(Menu, MenuItem), muda::Error> {
    let menu = Menu::new();
    let application = Submenu::new("PTTStella", true);
    let close = MenuItem::with_id(
        CLOSE_SETTINGS,
        pttstella::msg!("settings-close-command"),
        visible,
        Some(Accelerator::new(None, Code::Escape)),
    );
    application.append_items(&[
        &MenuItem::with_id(
            SETTINGS,
            pttstella::msg!("context-settings"),
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::Comma)),
        ),
        &close,
        &PredefinedMenuItem::separator(),
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
        &MenuItem::with_id(
            FULLSCREEN,
            pttstella::msg!("menu-fullscreen"),
            true,
            Some(Accelerator::new(
                Some(Modifiers::SUPER | Modifiers::CONTROL),
                Code::KeyF,
            )),
        ),
        &PredefinedMenuItem::close_window(Some(&pttstella::msg!("menu-close"))),
    ])?;
    window.set_as_windows_menu_for_nsapp();
    menu.append_items(&[&application, &edit, &window])?;
    Ok((menu, close))
}

/// Shared only by this window and its surface; no process-global key monitor.
#[derive(Clone, Default)]
pub struct NativeMenu(std::rc::Rc<std::cell::RefCell<MenuState>>);
#[derive(Default)]
struct MenuState {
    current: Option<(Menu, MenuItem)>,
    settings_visible: bool,
}
impl NativeMenu {
    pub fn install(&self) -> Result<(), muda::Error> {
        let mut state = self.0.borrow_mut();
        let next = build_menu(state.settings_visible)?;
        next.0.init_for_nsapp();
        state.current = Some(next);
        Ok(())
    }
    pub fn settings_visible(&self, visible: bool) {
        let mut state = self.0.borrow_mut();
        if state.settings_visible == visible {
            return;
        }
        state.settings_visible = visible;
        if let Some((_, close)) = &state.current {
            close.set_enabled(visible);
        }
    }
}

pub fn menu() -> Result<Menu, muda::Error> {
    build_menu(false).map(|(menu, _)| menu)
}

/// Tao lowers floating windows before entering native fullscreen. The direct
/// AppKit menu selector skips that step, so it cannot enter from our test window.
/// Restore the floating level only after the original window size returns;
/// `set_fullscreen(None)` updates Tao's state before the exit animation finishes.
#[derive(Default)]
pub struct NativeFullscreen {
    floating_size: std::cell::Cell<Option<dioxus::desktop::tao::dpi::PhysicalSize<u32>>>,
}
impl NativeFullscreen {
    pub fn toggle(&self, window: &dioxus::desktop::tao::window::Window) {
        use dioxus::desktop::tao::window::Fullscreen;
        if window.fullscreen().is_some() {
            window.set_fullscreen(None);
        } else {
            if window.is_always_on_top() {
                self.floating_size.set(Some(window.inner_size()));
            }
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
    }

    pub fn restore_if_windowed(&self, window: &dioxus::desktop::tao::window::Window) {
        if let Some(size) = self.floating_size.get()
            && window.fullscreen().is_none()
            && window.inner_size() == size
        {
            self.floating_size.set(None);
            window.set_always_on_top(true);
        }
    }
}
