//! Display preferences do not change cell geometry or terminal colours.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(feature = "desktop")]
pub mod store;

pub const MAX_SETTINGS_BYTES: usize = 8192;

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
        ["windows", "macos", "linux"]
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
