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

/// Static product metadata only. Never searches terminal text, family catalog,
/// saved site names, credentials or any other user data.
pub(super) fn search(query: &str) -> Vec<(i32, &'static str)> {
    const INDEX: [(i32, &str, &str); 12] = [
        (
            0,
            "language-label",
            "語言 繁體中文 英文 language locale english chinese",
        ),
        (
            0,
            "article-mode",
            "文章 網頁 終端機 article mode web terminal",
        ),
        (
            0,
            "article-boundary-navigation",
            "上篇 下篇 邊界 boundary previous next arrow",
        ),
        (
            1,
            "font-preset",
            "字型 字體 預設 fonts preset windows macos linux",
        ),
        (1, "font-ascii", "英數 ascii latin symbols"),
        (1, "font-cjk", "中文 漢字 chinese cjk"),
        (1, "font-size", "字高 字級 大小 size height"),
        (1, "font-baseline", "基線 排版 baseline alignment"),
        (1, "font-width", "字寬 字距 寬度 spacing width"),
        (
            2,
            "settings-opacity",
            "透明 浮球 滑鼠 右鍵 opacity ball mouse right click",
        ),
        (
            3,
            "hover-position",
            "圖片 自動 預覽 image automatic media hover preview",
        ),
        (
            4,
            "settings-privacy-description",
            "帳號 密碼 登入 隱私 account password login privacy",
        ),
    ];
    let query = query.chars().take(256).collect::<String>().to_lowercase();
    let query = query.trim();
    if query.is_empty() {
        return Vec::new();
    }
    INDEX
        .iter()
        .filter(|(_, _, terms)| query.split_whitespace().all(|q| terms.contains(q)))
        .map(|(page, key, _)| (*page, *key))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn search_is_bounded_static_bilingual_metadata() {
        assert_eq!(search("baseline"), [(1, "font-baseline")]);
        assert_eq!(search("字寬"), [(1, "font-width")]);
        assert!(search("my-private-user-account-123").is_empty());
        assert!(search("").is_empty());
        assert!(search(&"x".repeat(20_000)).is_empty());
    }
}
