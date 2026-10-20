//! Embedded Fluent catalogs. The UI owner selects a language after a successful save;
//! one atomic choice publishes the same selection to background work.
use i18n_embed::{LanguageLoader, fluent::FluentLanguageLoader};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "zh-TW")]
    TraditionalChinese,
    #[serde(rename = "en-US")]
    English,
}
impl Language {
    pub fn tag(self) -> &'static str {
        match self {
            Self::TraditionalChinese => "zh-TW",
            Self::English => "en-US",
        }
    }
    pub fn resource(self) -> &'static str {
        match self {
            Self::TraditionalChinese => include_str!("../locales/zh-TW/pttstella.ftl"),
            Self::English => include_str!("../locales/en-US/pttstella.ftl"),
        }
    }
}
#[derive(RustEmbed)]
#[folder = "locales"]
struct Catalogs;

pub struct Localizer {
    chinese: FluentLanguageLoader,
    english: FluentLanguageLoader,
    english_selected: AtomicBool,
}
impl Localizer {
    pub fn new(language: Language) -> Result<Self, String> {
        fn load(language: Language) -> Result<FluentLanguageLoader, String> {
            let loader = i18n_embed::fluent::fluent_language_loader!();
            let identifier = language.tag().parse().map_err(|e| format!("locale: {e}"))?;
            loader
                .load_languages(&Catalogs, &[identifier])
                .map_err(|e| format!("embedded Fluent catalog: {e}"))?;
            loader.set_use_isolating(false);
            Ok(loader)
        }
        Ok(Self {
            chinese: load(Language::TraditionalChinese)?,
            english: load(Language::English)?,
            english_selected: AtomicBool::new(language == Language::English),
        })
    }
    pub fn language(&self) -> Language {
        if self.english_selected.load(Ordering::Acquire) {
            Language::English
        } else {
            Language::TraditionalChinese
        }
    }
    /// Publish only the selected enum. Both immutable catalogs are already
    /// validated and configured, so workers cannot observe half-loaded bundles.
    pub fn select(&self, language: Language) {
        self.english_selected
            .store(language == Language::English, Ordering::Release);
    }
    fn loader_for(&self, language: Language) -> &FluentLanguageLoader {
        if language == Language::English {
            &self.english
        } else {
            &self.chinese
        }
    }
    pub fn loader(&self) -> &FluentLanguageLoader {
        self.loader_for(self.language())
    }
    /// Refresh only canonical App status labels. Preserve diagnostic details.
    pub fn relocalize_status(&self, previous: Language, text: &str) -> Option<String> {
        [
            "status-offline-preview",
            "status-offline",
            "status-connecting",
            "status-connected",
            "status-keepalive-on",
            "status-keepalive-off",
        ]
        .into_iter()
        .find(|key| self.loader_for(previous).get(key) == text)
        .map(|key| self.loader().get(key))
    }
}

// One process owner, immutable embedded catalogs and one atomic language choice.
// The UI changes the choice only after persistence succeeds.
static APPLICATION: OnceLock<Localizer> = OnceLock::new();
static FALLBACK: OnceLock<Localizer> = OnceLock::new();
pub fn initialize(language: Language) -> Result<(), String> {
    APPLICATION
        .set(Localizer::new(language)?)
        .map_err(|_| "application language was already initialized".to_owned())
}
pub fn application() -> &'static Localizer {
    APPLICATION.get().unwrap_or_else(|| {
        FALLBACK.get_or_init(|| {
            // Embedded assets and locale identifiers are validated by the
            // catalog gate; corruption is a build defect, never user input.
            Localizer::new(Language::default()).expect("validated embedded Fluent catalog")
        })
    })
}

/// Fluent's macro checks IDs and named arguments against the fallback catalog.
#[macro_export]
macro_rules! msg {
    ($id:literal $(, $name:ident = $value:expr)* $(,)?) => {
        i18n_embed_fl::fl!($crate::i18n::application().loader(), $id $(, $name = $value)*)
    };
}

#[cfg(test)]
#[path = "i18n_tests.rs"]
mod tests;
