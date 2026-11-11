//! Typed form conversion. Incomplete text stays in the native widgets; the
//! shared settings validator is the only authority for persisted values.
use crate::{SettingsState, TerminalWindow};
use pttstella::{
    i18n::Language,
    settings::{AppSettings, ArticleMode, Fonts, HoverPosition, Preset, ToolOpacity},
};
use slint::ComponentHandle;

const PRESETS: [Preset; 7] = [
    Preset::Windows,
    Preset::WindowsClassic,
    Preset::MacosBbs,
    Preset::Macos,
    Preset::Linux,
    Preset::MacosSerif,
    Preset::Bundled,
];
pub(super) fn populate(window: &TerminalWindow, app: &AppSettings) {
    window
        .global::<SettingsState>()
        .set_preset(match &app.display.fonts {
            Fonts::PlatformDefault {} => 0,
            Fonts::Preset { preset } => PRESETS
                .iter()
                .position(|p| p == preset)
                .map_or(8, |i| i as i32 + 1),
            Fonts::Custom { .. } => 8,
        });
    if let Ok(display) = app.display.resolve() {
        window
            .global::<SettingsState>()
            .set_ascii(display.ascii_font.into());
        window
            .global::<SettingsState>()
            .set_cjk(display.cjk_font.into());
    }
    window
        .global::<SettingsState>()
        .set_font_size(app.display.font_size.to_string().into());
    window
        .global::<SettingsState>()
        .set_baseline(app.display.baseline.to_string().into());
    window
        .global::<SettingsState>()
        .set_font_width(app.display.font_width.to_string().into());
    window
        .global::<SettingsState>()
        .set_language(if app.language == Language::English {
            1
        } else {
            0
        });
    window.global::<SettingsState>().set_article_mode(
        if app.article_mode == ArticleMode::Terminal {
            1
        } else {
            0
        },
    );
    window
        .global::<SettingsState>()
        .set_boundary(app.article_boundary_navigation);
    window
        .global::<SettingsState>()
        .set_automatic(app.auto_expand_images);
    window.global::<SettingsState>().set_hover(match app.hover {
        HoverPosition::FollowCursor => 0,
        HoverPosition::TopRight => 1,
        HoverPosition::Off => 2,
    });
    window
        .global::<SettingsState>()
        .set_tool_opacity(i32::from(app.tools_opacity.percent()));
    window
        .global::<SettingsState>()
        .set_right_click(app.tools_access.right_click_menu);
    window
        .global::<SettingsState>()
        .set_show_ball(app.tools_access.show_floating_button);
}
pub(super) fn read(window: &TerminalWindow, base: &AppSettings) -> Result<AppSettings, String> {
    let mut draft = base.clone();
    let number = |s: slint::SharedString| {
        s.trim()
            .parse::<f64>()
            .map_err(|_| pttstella::msg!("settings-autosave-invalid"))
    };
    draft.display.fonts = match window.global::<SettingsState>().get_preset() {
        0 => Fonts::PlatformDefault {},
        1..=7 => Fonts::Preset {
            preset: PRESETS[(window.global::<SettingsState>().get_preset() - 1) as usize].clone(),
        },
        _ => Fonts::Custom {
            ascii_font: window.global::<SettingsState>().get_ascii().to_string(),
            cjk_font: window.global::<SettingsState>().get_cjk().to_string(),
        },
    };
    draft.display.font_size = number(window.global::<SettingsState>().get_font_size())?;
    draft.display.baseline = number(window.global::<SettingsState>().get_baseline())?;
    draft.display.font_width = number(window.global::<SettingsState>().get_font_width())?;
    draft.language = if window.global::<SettingsState>().get_language() == 1 {
        Language::English
    } else {
        Language::TraditionalChinese
    };
    draft.article_mode = if window.global::<SettingsState>().get_article_mode() == 1 {
        ArticleMode::Terminal
    } else {
        ArticleMode::Web
    };
    draft.article_boundary_navigation = window.global::<SettingsState>().get_boundary();
    draft.auto_expand_images = window.global::<SettingsState>().get_automatic();
    draft.hover = match window.global::<SettingsState>().get_hover() {
        0 => HoverPosition::FollowCursor,
        1 => HoverPosition::TopRight,
        _ => HoverPosition::Off,
    };
    draft.tools_opacity = ToolOpacity::try_from(
        u8::try_from(window.global::<SettingsState>().get_tool_opacity())
            .map_err(|_| pttstella::msg!("settings-opacity-invalid"))?,
    )?;
    draft.tools_access.right_click_menu = window.global::<SettingsState>().get_right_click();
    draft.tools_access.show_floating_button = window.global::<SettingsState>().get_show_ball();
    draft.validate()
}

/// Native locations for the shared closed product destinations.
pub(super) fn destination(entry: pttstella::settings::catalog::Entry) -> Option<(i32, i32)> {
    use pttstella::settings::catalog::{Page, Target};
    Some(match entry.0 {
        Some(Target::InterfaceLanguage) => (0, 10),
        Some(Target::ZoomFit) => (1, 20),
        Some(Target::VimToggle) => (2, 21),
        Some(Target::StationMouse) => (2, 22),
        Some(Target::KeepAlive) => (4, 23),
        Some(Target::FontPreset) => (1, 11),
        Some(Target::FontAscii) => (1, 1),
        Some(Target::FontCjk) => (1, 2),
        Some(Target::FontSize) => (1, 3),
        Some(Target::FontBaseline) => (1, 4),
        Some(Target::FontWidth) => (1, 5),
        Some(Target::FontReset) => (1, 24),
        Some(Target::FontProfileName) => (1, 7),
        Some(Target::ToolsOpacity) => (2, 6),
        Some(Target::ToolsRightClick) => (2, 12),
        Some(Target::ToolsShowBall) => (2, 13),
        Some(Target::ArticleMode) => (3, 14),
        Some(Target::AutoExpandImages) => (3, 15),
        Some(Target::ArticleBoundaryNavigation) => (3, 16),
        Some(Target::SettingsHover) => (3, 17),
        None => match entry.1 {
            Page::Sites => (4, 18),
            Page::Filters => (5, 26),
            Page::Account => (6, 19),
            Page::About => (7, 25),
            _ => return None,
        },
    })
}
