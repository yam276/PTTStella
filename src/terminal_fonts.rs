//! Application-owned fallback fonts shared by desktop frontends.
pub const CASCADIA_MONO: &[u8] = include_bytes!("../assets/fonts/CascadiaMono.ttf");
// One linker-owned asset shared across the terminal renderer and native widgets.
pub static NOTO_SANS_CJK_TC: &[u8] = include_bytes!("../assets/fonts/NotoSansCJKtc-Regular.otf");
