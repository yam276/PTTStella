//! One set of localized swatch labels for local and station editors.
use slint::{ModelRc, SharedString, VecModel};
pub fn labels() -> ModelRc<SharedString> {
    let names = [
        pttstella::msg!("editor-color-name-0"),
        pttstella::msg!("editor-color-name-1"),
        pttstella::msg!("editor-color-name-2"),
        pttstella::msg!("editor-color-name-3"),
        pttstella::msg!("editor-color-name-4"),
        pttstella::msg!("editor-color-name-5"),
        pttstella::msg!("editor-color-name-6"),
        pttstella::msg!("editor-color-name-7"),
    ];
    ModelRc::new(VecModel::from(
        (0..24)
            .map(|i| {
                let group = if i < 16 {
                    pttstella::msg!("editor-color-foreground")
                } else {
                    pttstella::msg!("editor-color-background")
                };
                let bright = if (8..16).contains(&i) {
                    format!("{} · ", pttstella::msg!("editor-color-bright"))
                } else {
                    String::new()
                };
                format!("{group} · {bright}{}", names[i % 8]).into()
            })
            .collect::<Vec<_>>(),
    ))
}
