//! Native text shortcuts and IME ownership for auxiliary manager windows.
use crate::TerminalWindow;
use slint::{
    ComponentHandle,
    winit_030::{
        EventResult, WinitWindowAccessor,
        winit::{
            event::{ElementState, Ime, WindowEvent},
            keyboard::ModifiersState,
        },
    },
};
use std::cell::Cell;

pub trait TextWindow: ComponentHandle + 'static {
    fn composing(&self) -> bool;
    fn composition(&self, active: bool);
    fn edit_key(&self, code: i32) -> bool;
}
macro_rules! text_window {
    ($($window:ty),+) => {$(
        impl TextWindow for $window {
            fn composing(&self)->bool {self.get_composing()}
            fn composition(&self,active:bool) {self.set_composing(active);}
            fn edit_key(&self,code:i32)->bool {self.invoke_edit_key(code)}
        }
    )+};
}
text_window!(
    crate::SitesWindow,
    crate::FiltersWindow,
    crate::BookmarksWindow
);

pub fn install<W: TextWindow>(root: &TerminalWindow, window: &W) {
    let root = root.as_weak();
    let weak = window.as_weak();
    let modifiers = Cell::new(ModifiersState::empty());
    window.window().on_winit_window_event(move |_, event| {
        if let WindowEvent::ModifiersChanged(m) = event {
            modifiers.set(m.state());
        }
        if let Some(w) = weak.upgrade() {
            match event {
                WindowEvent::Focused(true) => {
                    if let Some(root) = root.upgrade() {
                        root.invoke_native_menu_refresh();
                    }
                }
                WindowEvent::Focused(false) => {
                    w.composition(false);
                    modifiers.set(ModifiersState::empty());
                }
                WindowEvent::Ime(Ime::Preedit(text, _)) => w.composition(!text.is_empty()),
                WindowEvent::Ime(Ime::Commit(_) | Ime::Disabled) => w.composition(false),
                WindowEvent::KeyboardInput {
                    event,
                    is_synthetic: false,
                    ..
                } if !w.composing() && event.state == ElementState::Pressed && !event.repeat => {
                    if let Some(code) =
                        crate::native_input::text_edit_shortcut(event.physical_key, modifiers.get())
                        && w.edit_key(code)
                    {
                        return EventResult::PreventDefault;
                    }
                }
                _ => {}
            }
        }
        EventResult::Propagate
    });
}
