//! Observes the native IME boundary while Slint owns preedit, caret and rendering.
use crate::TerminalWindow;
use slint::{
    ComponentHandle,
    winit_030::{
        EventResult, WinitWindowAccessor,
        winit::event::{Ime, WindowEvent},
    },
};

pub fn mount(ui: &TerminalWindow) {
    ui.set_terminal_input(true);
    ui.set_result("點終端第 22 行的輸入區，測試原生組字與候選窗".into());
    let weak = ui.as_weak();
    let mut focused = false;
    let mut commits = 0usize;
    ui.window().on_winit_window_event(move |_, event| {
        if let Some(ui) = weak.upgrade() {
            match event {
                WindowEvent::Focused(active) => focused = *active,
                WindowEvent::Ime(Ime::Commit(text))
                    if focused && ui.get_terminal_input_focused() && !text.is_empty() =>
                {
                    let result = match pttstella::input::encode(text) {
                        Ok(bytes) => {
                            commits = commits.saturating_add(1);
                            eprintln!("ime commit {commits}: {} bytes", bytes.len());
                            format!("組字提交 {commits} 次 · {} bytes", bytes.len())
                        }
                        Err(error) => format!("組字編碼失敗 · {error}"),
                    };
                    ui.set_result(result.into());
                }
                _ => {}
            }
        }
        EventResult::Propagate
    });
}
