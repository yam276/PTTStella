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
    Macos,
    Linux,
    MacosSerif,
}
impl Preset {
    fn name(&self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Macos => "macos",
            Self::Linux => "linux",
            Self::MacosSerif => "macos_serif",
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
    pub fn resolve(&self) -> Result<DisplaySettings, &'static str> {
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
    pub fn validate(mut self) -> Result<Self, &'static str> {
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppSettings {
    pub version: u32,
    pub display: DisplayProfile,
    pub hover: HoverPosition,
}
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            version: 2,
            display: DisplayProfile::default(),
            hover: HoverPosition::default(),
        }
    }
}
impl AppSettings {
    pub fn validate(mut self) -> Result<Self, &'static str> {
        if self.version != 2 {
            return Err("不支援此設定版本");
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
        let head: Header =
            serde_json::from_slice(bytes).map_err(|e| format!("設定檔格式不正確：{e}"))?;
        match head.version {
            1 => {
                let old: DisplaySettings =
                    serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
                Ok(Self {
                    display: old.validate().map_err(str::to_owned)?.into(),
                    ..Self::default()
                })
            }
            2 => serde_json::from_slice::<Self>(bytes)
                .map_err(|e| e.to_string())?
                .validate()
                .map_err(str::to_owned),
            _ => Err("不支援此設定版本".into()),
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
        Err("設定檔超過 8 KiB；原檔保留".into())
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
            return Err("不支援此顯示設定檔格式或版本".into());
        }
        self.name = self.name.trim().to_owned();
        if self.name.is_empty() || self.name.len() > 256 || self.name.chars().any(char::is_control)
        {
            return Err("設定檔名稱需為 1 至 256 bytes 且不含控制字元".into());
        }
        self.display = self.display.validate().map_err(str::to_owned)?;
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
            "windows" => ("Cascadia Mono", "Microsoft JhengHei"),
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
        ["windows", "macos", "linux", "macos_serif"]
            .into_iter()
            .map(|name| (name, Self::preset(name)))
            .collect()
    }
    pub fn validate(mut self) -> Result<Self, &'static str> {
        if self.version != 1 {
            return Err("不支援此設定版本");
        }
        for name in [&mut self.ascii_font, &mut self.cjk_font] {
            *name = name.trim().to_owned();
            if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
                return Err("字型名稱需為 1 至 256 bytes，且不含控制字元");
            }
        }
        if !self.font_size.is_finite()
            || !(8.0..=24.0).contains(&self.font_size)
            || !self.baseline.is_finite()
            || !(0.0..=24.0).contains(&self.baseline)
        {
            return Err("字高需介於 8 至 24，基線需介於 0 至 24");
        }
        Ok(self)
    }
}
