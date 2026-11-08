//! Typed form conversion. Incomplete text stays in the native widgets; the
//! shared settings validator is the only authority for persisted values.
use crate::SettingsWindow;
use pttstella::{
    i18n::Language,
    settings::{AppSettings, ArticleMode, Fonts, HoverPosition, Preset, ToolOpacity},
};

const PRESETS: [Preset; 7] = [
    Preset::Windows,
    Preset::WindowsClassic,
    Preset::MacosBbs,
    Preset::Macos,
    Preset::Linux,
    Preset::MacosSerif,
    Preset::Bundled,
];
pub(super) fn populate(window: &SettingsWindow, app: &AppSettings) {
    window.set_preset(match &app.display.fonts {
        Fonts::PlatformDefault {} => 0,
        Fonts::Preset { preset } => PRESETS
            .iter()
            .position(|p| p == preset)
            .map_or(8, |i| i as i32 + 1),
        Fonts::Custom { .. } => 8,
    });
    if let Ok(display) = app.display.resolve() {
        window.set_ascii(display.ascii_font.into());
        window.set_cjk(display.cjk_font.into());
    }
    window.set_font_size(app.display.font_size.to_string().into());
    window.set_baseline(app.display.baseline.to_string().into());
    window.set_font_width(app.display.font_width.to_string().into());
    window.set_language(if app.language == Language::English {
        1
    } else {
        0
    });
    window.set_article_mode(if app.article_mode == ArticleMode::Terminal {
        1
    } else {
        0
    });
    window.set_boundary(app.article_boundary_navigation);
    window.set_automatic(app.auto_expand_images);
    window.set_hover(match app.hover {
        HoverPosition::FollowCursor => 0,
        HoverPosition::TopRight => 1,
        HoverPosition::Off => 2,
    });
    window.set_tool_opacity(app.tools_opacity.percent().to_string().into());
    window.set_right_click(app.tools_access.right_click_menu);
    window.set_show_ball(app.tools_access.show_floating_button);
}
pub(super) fn read(window: &SettingsWindow, base: &AppSettings) -> Result<AppSettings, String> {
    let mut draft = base.clone();
    let number = |s: slint::SharedString| {
        s.trim()
            .parse::<f64>()
            .map_err(|_| pttstella::msg!("settings-autosave-invalid"))
    };
    draft.display.fonts = match window.get_preset() {
        0 => Fonts::PlatformDefault {},
        1..=7 => Fonts::Preset {
            preset: PRESETS[(window.get_preset() - 1) as usize].clone(),
        },
        _ => Fonts::Custom {
            ascii_font: window.get_ascii().to_string(),
            cjk_font: window.get_cjk().to_string(),
        },
    };
    draft.display.font_size = number(window.get_font_size())?;
    draft.display.baseline = number(window.get_baseline())?;
    draft.display.font_width = number(window.get_font_width())?;
    draft.language = if window.get_language() == 1 {
        Language::English
    } else {
        Language::TraditionalChinese
    };
    draft.article_mode = if window.get_article_mode() == 1 {
        ArticleMode::Terminal
    } else {
        ArticleMode::Web
    };
    draft.article_boundary_navigation = window.get_boundary();
    draft.auto_expand_images = window.get_automatic();
    draft.hover = match window.get_hover() {
        0 => HoverPosition::FollowCursor,
        1 => HoverPosition::TopRight,
        _ => HoverPosition::Off,
    };
    draft.tools_opacity = ToolOpacity::try_from(
        window
            .get_tool_opacity()
            .trim()
            .parse::<u8>()
            .map_err(|_| pttstella::msg!("settings-opacity-invalid"))?,
    )?;
    draft.tools_access.right_click_menu = window.get_right_click();
    draft.tools_access.show_floating_button = window.get_show_ball();
    draft.validate()
}

/// Native locations for the shared closed product destinations. The filter
/// manager is still a separate migration task and has no placeholder result.
pub(super) fn destination(entry: pttstella::settings::catalog::Entry) -> Option<(i32, i32)> {
    use pttstella::settings::catalog::{Page, Target};
    Some(match entry.0 {
        Some(Target::InterfaceLanguage) => (0, 10),
        Some(Target::ZoomFit) => (1, 20),
        Some(Target::VimToggle) => (2, 21),
        Some(Target::StationMouse) => (2, 22),
        Some(Target::KeepAlive) => (0, 23),
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
        Some(Target::ArticleMode) => (0, 14),
        Some(Target::AutoExpandImages) => (3, 15),
        Some(Target::ArticleBoundaryNavigation) => (0, 16),
        Some(Target::SettingsHover) => (3, 17),
        None => match entry.1 {
            Page::Sites => (0, 18),
            Page::Account => (4, 19),
            Page::About => (5, 25),
            _ => return None,
        },
    })
}
