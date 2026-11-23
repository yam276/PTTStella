//! Fixed product search metadata shared with the legacy WebView adapter.
//! No terminal text, credentials, saved names or installed fonts enter this index.
use icu_normalizer::ComposingNormalizer;
use serde::Deserialize;
use std::sync::OnceLock;

pub const DATA: &str = include_str!("../../assets/settings-catalog.json");
pub const QUERY_UNITS: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    InterfaceLanguage,
    ZoomFit,
    VimToggle,
    StationMouse,
    KeepAlive,
    FontPreset,
    FontAscii,
    FontCjk,
    FontSize,
    FontWidth,
    FontReset,
    FontBaseline,
    FontProfileName,
    ToolsOpacity,
    ToolsRightClick,
    ToolsShowBall,
    ArticleMode,
    AutoExpandImages,
    ArticleBoundaryNavigation,
    SettingsHover,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Page {
    General,
    Appearance,
    Input,
    Media,
    Sites,
    Filters,
    Account,
    About,
}
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Entry(
    pub Option<Target>,
    pub Page,
    pub &'static str,
    pub &'static str,
);

pub fn entries() -> Result<&'static [Entry], String> {
    static ENTRIES: OnceLock<Result<Vec<Entry>, String>> = OnceLock::new();
    ENTRIES
        .get_or_init(|| {
            let entries: Vec<Entry> = serde_json::from_str(DATA).map_err(|e| e.to_string())?;
            if entries.len() > 64 {
                return Err("embedded settings catalog exceeds its bound".into());
            }
            let mut seen = std::collections::HashSet::new();
            if entries.iter().any(|e| !seen.insert((e.0, e.1))) {
                return Err("embedded settings catalog has duplicate destinations".into());
            }
            Ok(entries)
        })
        .as_ref()
        .map(|entries| entries.as_slice())
        .map_err(Clone::clone)
}

/// Match the legacy field's 128 UTF-16-unit limit without splitting a scalar.
pub fn bounded_query(query: &str) -> String {
    let mut used = 0;
    query
        .chars()
        .take_while(|ch| {
            used += ch.len_utf16();
            used <= QUERY_UNITS
        })
        .collect()
}
fn normalized(text: &str) -> String {
    ComposingNormalizer::new_nfkc()
        .normalize_iter(text.chars())
        .flat_map(char::to_lowercase)
        .collect()
}

pub struct Index(Vec<(Entry, String)>);
impl Index {
    pub fn new(mut translate: impl FnMut(&str) -> String) -> Result<Self, String> {
        Ok(Self(
            entries()?
                .iter()
                .map(|entry| {
                    (
                        *entry,
                        normalized(&format!("{} {}", translate(entry.2), translate(entry.3))),
                    )
                })
                .collect(),
        ))
    }
    pub fn search(&self, query: &str) -> Vec<Entry> {
        let query = normalized(&bounded_query(query));
        let words: Vec<_> = query.split_whitespace().collect();
        if words.is_empty() {
            return Vec::new();
        }
        self.0
            .iter()
            .filter(|(_, terms)| words.iter().all(|word| terms.contains(word)))
            .map(|(entry, _)| *entry)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Language, Localizer};

    #[test]
    fn bilingual_fixed_catalog_normalizes_and_never_indexes_user_data() {
        for language in [Language::English, Language::TraditionalChinese] {
            let tr = Localizer::new(language).unwrap();
            let index = Index::new(|key| {
                let label = tr.loader().get(key);
                assert_ne!(label, key, "missing product translation");
                label
            })
            .unwrap();
            assert_eq!(entries().unwrap().len(), 24);
            for (query, target) in [
                ("ＢＡＳＥＬＩＮＥ", Target::FontBaseline),
                ("CJK FONT", Target::FontCjk),
                ("自動開圖", Target::AutoExpandImages),
                ("right click", Target::ToolsRightClick),
                ("letter spacing", Target::FontWidth),
                ("reset font", Target::FontReset),
                ("ﬂoating opacity", Target::ToolsOpacity),
                ("keepalive", Target::KeepAlive),
                ("experimental keyboard", Target::VimToggle),
            ] {
                assert!(
                    index.search(query).iter().any(|e| e.0 == Some(target)),
                    "{query}"
                );
            }
            assert!(
                index
                    .search("黑白名單")
                    .iter()
                    .any(|e| e.0.is_none() && e.1 == Page::Filters)
            );
            for query in [
                "",
                " \t\n",
                "private-account-123",
                "<img src=x onerror=alert(1)>",
                &"x".repeat(20_000),
            ] {
                assert!(
                    index.search(query).is_empty(),
                    "private/unknown query matched"
                );
            }
            assert!(index.search("baseline unrelated").is_empty());
        }
    }

    #[test]
    fn utf16_limit_precedes_normalization_and_preserves_complete_scalars() {
        assert_eq!(
            bounded_query(&format!("{}😀baseline", "x".repeat(127))),
            "x".repeat(127)
        );
        assert_eq!(
            bounded_query(&format!("{}😀baseline", "x".repeat(126))),
            format!("{}😀", "x".repeat(126))
        );
        assert_eq!(bounded_query(&"字".repeat(200)).chars().count(), 128);
    }
}
