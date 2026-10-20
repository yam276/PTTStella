//! Display preferences do not change cell geometry or terminal colours.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(feature = "desktop")]
pub mod store;

pub const MAX_SETTINGS_BYTES: usize = 8192;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    Windows,
    WindowsClassic,
    MacosBbs,
    Macos,
    Linux,
    MacosSerif,
    Bundled,
}
impl Preset {
    fn name(&self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::WindowsClassic => "windows_classic",
            Self::MacosBbs => "macos_bbs",
            Self::Macos => "macos",
            Self::Linux => "linux",
            Self::MacosSerif => "macos_serif",
            Self::Bundled => "bundled",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Fonts {
    PlatformDefault {},
    Preset {
        preset: Preset,
    },
    Custom {
        ascii_font: String,
        cjk_font: String,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayProfile {
    pub fonts: Fonts,
    pub font_size: f64,
    pub baseline: f64,
}
impl Default for DisplayProfile {
    fn default() -> Self {
        Self {
            fonts: Fonts::PlatformDefault {},
            font_size: 20.0,
            baseline: 20.0,
        }
    }
}
impl DisplayProfile {
    pub fn resolve(&self) -> Result<DisplaySettings, String> {
        let mut resolved = match &self.fonts {
            Fonts::PlatformDefault {} => DisplaySettings::default(),
            Fonts::Preset { preset } => DisplaySettings::preset(preset.name()),
            Fonts::Custom {
                ascii_font,
                cjk_font,
            } => DisplaySettings {
                ascii_font: ascii_font.clone(),
                cjk_font: cjk_font.clone(),
                ..DisplaySettings::default()
            },
        };
        resolved.font_size = self.font_size;
        resolved.baseline = self.baseline;
        resolved.validate()
    }
    pub fn validate(mut self) -> Result<Self, String> {
        let resolved = self.resolve()?;
        if matches!(self.fonts, Fonts::Custom { .. }) {
            self.fonts = Fonts::Custom {
                ascii_font: resolved.ascii_font,
                cjk_font: resolved.cjk_font,
            };
        }
        Ok(self)
    }
}
impl From<DisplaySettings> for DisplayProfile {
    fn from(old: DisplaySettings) -> Self {
        Self {
            fonts: Fonts::Custom {
                ascii_font: old.ascii_font,
                cjk_font: old.cjk_font,
            },
            font_size: old.font_size,
            baseline: old.baseline,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HoverPosition {
    Off,
    FollowCursor,
    #[default]
    TopRight,
}
/// Keep the tool affordance discoverable even at the lowest saved opacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct ToolOpacity(u8);
impl Default for ToolOpacity {
    fn default() -> Self {
        Self(100)
    }
}
impl ToolOpacity {
    pub fn percent(self) -> u8 {
        self.0
    }
}
impl TryFrom<u8> for ToolOpacity {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if (20..=100).contains(&value) {
            Ok(Self(value))
        } else {
            Err(crate::msg!("settings-opacity-invalid"))
        }
    }
}
impl From<ToolOpacity> for u8 {
    fn from(value: ToolOpacity) -> Self {
        value.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolAccess {
    pub right_click_menu: bool,
    pub show_floating_button: bool,
}
impl Default for ToolAccess {
    fn default() -> Self {
        Self {
            right_click_menu: true,
            show_floating_button: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppSettings {
    pub version: u32,
    pub display: DisplayProfile,
    pub hover: HoverPosition,
    #[serde(default)]
    pub language: crate::i18n::Language,
    #[serde(default)]
    pub tools_opacity: ToolOpacity,
    #[serde(default)]
    pub tools_access: ToolAccess,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: 5,
            language: crate::i18n::Language::default(),
            tools_opacity: ToolOpacity::default(),
            tools_access: ToolAccess::default(),
            display: DisplayProfile::default(),
            hover: HoverPosition::default(),
        }
    }
}
impl AppSettings {
    fn migrate(mut self) -> Self {
        if matches!(self.version, 2..=4) {
            self.version = 5;
        }
        self
    }
    pub fn validate(mut self) -> Result<Self, String> {
        if self.version != 5 {
            return Err(crate::msg!("settings-version"));
        }
        self.display = self.display.validate()?;
        Ok(self)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        bounded(bytes)?;
        #[derive(Deserialize)]
        struct Header {
            version: u32,
        }
        let head: Header = serde_json::from_slice(bytes)
            .map_err(|e| crate::msg!("settings-invalid", e = e.to_string()))?;
        match head.version {
            1 => {
                let old: DisplaySettings =
                    serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
                Ok(Self {
                    display: old.validate().map_err(|error| error.to_string())?.into(),
                    ..Self::default()
                })
            }
            2..=5 => serde_json::from_slice::<Self>(bytes)
                .map_err(|e| e.to_string())?
                .migrate()
                .validate()
                .map_err(|error| error.to_string()),
            _ => Err(crate::msg!("settings-version")),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableProfile {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub display: DisplayProfile,
}
fn bounded(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_SETTINGS_BYTES {
        Err(crate::msg!("settings-too-large"))
    } else {
        Ok(())
    }
}
impl PortableProfile {
    pub fn new(name: String, display: DisplayProfile) -> Result<Self, String> {
        Self {
            format: "PTTStella.DisplayProfile".into(),
            version: 1,
            name,
            display,
        }
        .validate()
    }
    fn validate(mut self) -> Result<Self, String> {
        if self.format != "PTTStella.DisplayProfile" || self.version != 1 {
            return Err(crate::msg!("profile-version"));
        }
        self.name = self.name.trim().to_owned();
        if self.name.is_empty() || self.name.len() > 256 || self.name.chars().any(char::is_control)
        {
            return Err(crate::msg!("profile-name-invalid"));
        }
        self.display = self.display.validate().map_err(|error| error.to_string())?;
        Ok(self)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        bounded(bytes)?;
        serde_json::from_slice::<Self>(bytes)
            .map_err(|e| e.to_string())?
            .validate()
    }
    pub fn encode(self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec_pretty(&self.validate()?).map_err(|e| e.to_string())?;
        bounded(&bytes)?;
        Ok(bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplaySettings {
    pub version: u32,
    pub ascii_font: String,
    pub cjk_font: String,
    pub font_size: f64,
    pub baseline: f64,
}
impl Default for DisplaySettings {
    fn default() -> Self {
        Self::preset(if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        })
    }
}
impl DisplaySettings {
    fn preset(name: &str) -> Self {
        let (ascii, cjk) = match name {
            "bundled" => ("Cascadia Mono", "Noto Sans CJK TC"),
            "windows" => ("Cascadia Mono", "Microsoft JhengHei"),
            "windows_classic" => ("MingLiU", "MingLiU"),
            "macos_bbs" => ("Songti TC", "Songti TC"),
            "macos" => ("Menlo", "PingFang TC"),
            "macos_serif" => ("Menlo", "Songti TC"),
            _ => ("DejaVu Sans Mono", "Noto Sans Mono CJK TC"),
        };
        Self {
            version: 1,
            ascii_font: ascii.into(),
            cjk_font: cjk.into(),
            font_size: 20.0,
            baseline: 20.0,
        }
    }
    pub fn presets() -> BTreeMap<&'static str, Self> {
        [
            "windows",
            "windows_classic",
            "macos_bbs",
            "macos",
            "linux",
            "macos_serif",
            "bundled",
        ]
        .into_iter()
        .map(|name| (name, Self::preset(name)))
        .collect()
    }
    pub fn validate(mut self) -> Result<Self, String> {
        if self.version != 1 {
            return Err(crate::msg!("settings-version"));
        }
        for name in [&mut self.ascii_font, &mut self.cjk_font] {
            *name = name.trim().to_owned();
            if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
                return Err(crate::msg!("font-name-invalid"));
            }
        }
        if !self.font_size.is_finite()
            || !(8.0..=24.0).contains(&self.font_size)
            || !self.baseline.is_finite()
            || !(0.0..=24.0).contains(&self.baseline)
        {
            return Err(crate::msg!("font-geometry-invalid"));
        }
        Ok(self)
    }
}
