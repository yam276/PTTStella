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
    ui.set_result("切換輸入法後點第 22 行。日文請選「あ」，輸入「こんにちは」後按 Enter。".into());
    let weak = ui.as_weak();
    let mut focused = false;
    let mut preediting = false;
    let mut commits = 0usize;
    ui.window().on_winit_window_event(move |_, event| {
        if let Some(ui) = weak.upgrade() {
            match event {
                WindowEvent::Focused(active) => {
                    focused = *active;
                    preediting = false;
                    if *active {
                        ui.invoke_focus_terminal();
                        #[cfg(target_os = "windows")]
                        ui.window()
                            .with_winit_window(|window| window.set_ime_allowed(true));
                    }
                    eprintln!("ime window focused: {active}");
                }
                WindowEvent::Ime(Ime::Enabled) => eprintln!("ime enabled"),
                WindowEvent::Ime(Ime::Disabled) => {
                    preediting = false;
                    eprintln!("ime disabled");
                }
                WindowEvent::Ime(Ime::Preedit(text, _))
                    if focused && ui.get_terminal_input_focused() =>
                {
                    let active = !text.is_empty();
                    if active != preediting {
                        preediting = active;
                        eprintln!("ime preedit active: {active}");
                    }
                }
                WindowEvent::Ime(Ime::Commit(text))
                    if focused && ui.get_terminal_input_focused() && !text.is_empty() =>
                {
                    preediting = false;
                    commits = commits.saturating_add(1);
                    let result = commit_status(commits, text);
                    ui.set_result(result.into());
                }
                _ => {}
            }
        }
        EventResult::Propagate
    });
}

fn commit_status(count: usize, text: &str) -> String {
    let preview: String = text.chars().filter(|c| !c.is_control()).take(24).collect();
    match pttstella::input::encode(text) {
        Ok(bytes) => {
            eprintln!("ime commit {count}: {} Big5-UAO bytes", bytes.len());
            format!(
                "原生提交 {count} 次 · 「{preview}」 · Big5-UAO {} bytes",
                bytes.len()
            )
        }
        Err(error) => {
            eprintln!("ime commit {count}: Big5-UAO encoding failed");
            format!("原生提交 {count} 次 · 「{preview}」 · {error}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_commit_distinguishes_native_input_from_station_encoding() {
        assert!(commit_status(1, "こんにちは").ends_with("Big5-UAO 10 bytes"));
        assert!(commit_status(2, "中").ends_with("Big5-UAO 2 bytes"));
        let rejected = commit_status(3, "こんにちは🙂");
        assert!(rejected.starts_with("原生提交 3 次 · 「こんにちは🙂」"));
        assert!(!rejected.contains("bytes"));
    }
}
