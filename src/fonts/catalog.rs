//! Native family discovery and fixed official acquisition pages. No font files leave the OS.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Source {
    Cascadia,
    Noto,
    Dejavu,
    MicrosoftJhenghei,
    Apple,
}

pub(crate) struct Help {
    pub id: &'static str,
    pub families: &'static [&'static str],
    pub url: &'static str,
    pub label: String,
}
impl Source {
    pub const ALL: [Self; 5] = [
        Self::Cascadia,
        Self::Noto,
        Self::Dejavu,
        Self::MicrosoftJhenghei,
        Self::Apple,
    ];
    pub fn help(self) -> Help {
        let (id, families, url, label): (_, &'static [&'static str], _, _) = match self {
            Self::Cascadia => (
                "cascadia",
                &["Cascadia Mono"],
                "https://github.com/microsoft/cascadia-code/releases",
                pttstella::msg!("font-source-download", name = "Cascadia Mono"),
            ),
            Self::Noto => (
                "noto",
                &["Noto Sans CJK TC", "Noto Sans Mono CJK TC"],
                "https://github.com/notofonts/noto-cjk/blob/main/Sans/README.md",
                pttstella::msg!("font-source-download", name = "Noto CJK"),
            ),
            Self::Dejavu => (
                "dejavu",
                &["DejaVu Sans Mono"],
                "https://dejavu-fonts.github.io/Download.html",
                pttstella::msg!("font-source-download", name = "DejaVu"),
            ),
            Self::MicrosoftJhenghei => (
                "microsoft_jhenghei",
                &["Microsoft JhengHei"],
                "https://learn.microsoft.com/en-us/typography/font-list/microsoft-jhenghei",
                pttstella::msg!("font-source-license", name = "Microsoft JhengHei"),
            ),
            Self::Apple => (
                "apple",
                &["Menlo", "PingFang TC", "Songti TC"],
                "https://support.apple.com/guide/font-book/install-and-validate-fonts-fntbk1000/mac",
                pttstella::msg!("font-source-apple"),
            ),
        };
        Help {
            id,
            families,
            url,
            label,
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct Catalog {
    pub families: Vec<String>,
    pub notice: Option<String>,
}
// A family name has the same 256-byte bound as saved display settings. The
// bridge caps the list at 4096 names (~1 MiB), independent of host inventory.
const MAX_FAMILIES: usize = 4096;
fn normalize(names: Vec<String>) -> Catalog {
    let mut families: Vec<_> = names
        .into_iter()
        .map(|name| name.trim().to_owned())
        .filter(|name| {
            !name.is_empty()
                && !name.starts_with('.')
                && name.len() <= 256
                && !name.chars().any(char::is_control)
        })
        .collect();
    families.sort_unstable();
    families.dedup();
    let truncated = families.len() > MAX_FAMILIES;
    families.truncate(MAX_FAMILIES);
    Catalog {
        families,
        notice: truncated.then(|| pttstella::msg!("font-list-truncated")),
    }
}
/// Call from the owned blocking pool, never the UI event loop. Only family
/// names are queried; no glyph rasterization, font copying or upload occurs.
pub(crate) fn installed() -> Result<Catalog, String> {
    font_kit::source::SystemSource::new()
        .all_families()
        .map(normalize)
        .map_err(|e| pttstella::msg!("font-list-failed", error = e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_are_bounded_sorted_unique_and_valid_for_settings() {
        let catalog = normalize(vec![
            "Menlo".into(),
            " Arial ".into(),
            "Menlo".into(),
            ".Hidden".into(),
            "".into(),
            "bad\nname".into(),
            "x".repeat(257),
            "新細明體".into(),
            "A \"quote\"".into(),
        ]);
        assert_eq!(
            catalog.families,
            ["A \"quote\"", "Arial", "Menlo", "新細明體"]
        );
        assert!(catalog.notice.is_none());
        let catalog = normalize(
            (0..MAX_FAMILIES + 1)
                .map(|i| format!("Font{i:04}"))
                .collect(),
        );
        assert_eq!(catalog.families.len(), MAX_FAMILIES);
        assert!(catalog.notice.is_some());
    }
    #[test]
    fn source_ids_only_open_known_official_pages() {
        let urls = [
            "https://github.com/microsoft/cascadia-code/releases",
            "https://github.com/notofonts/noto-cjk/blob/main/Sans/README.md",
            "https://dejavu-fonts.github.io/Download.html",
            "https://learn.microsoft.com/en-us/typography/font-list/microsoft-jhenghei",
            "https://support.apple.com/guide/font-book/install-and-validate-fonts-fntbk1000/mac",
        ];
        for (source, url) in Source::ALL.into_iter().zip(urls) {
            let help = source.help();
            assert_eq!(help.url, url);
            assert_eq!(
                serde_json::from_value::<Source>(serde_json::json!(help.id)).unwrap(),
                source
            );
        }
        for unknown in [
            "https://example.com/font.ttf",
            "file:///font.ttf",
            "unknown",
        ] {
            assert!(serde_json::from_value::<Source>(serde_json::json!(unknown)).is_err());
        }
        assert!(
            Source::MicrosoftJhenghei
                .help()
                .label
                .contains("Microsoft JhengHei")
        );
    }
}
