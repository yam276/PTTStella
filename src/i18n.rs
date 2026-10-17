//! Fluent catalogs are embedded. The process language is selected once before
//! the desktop event loop; saving a preference cannot change an active session.
use i18n_embed::{LanguageLoader, fluent::FluentLanguageLoader};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

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
    language: Language,
    loader: FluentLanguageLoader,
}
impl Localizer {
    pub fn new(language: Language) -> Result<Self, String> {
        let loader = i18n_embed::fluent::fluent_language_loader!();
        let identifier = language.tag().parse().map_err(|e| format!("locale: {e}"))?;
        loader
            .load_languages(&Catalogs, &[identifier])
            .map_err(|e| format!("embedded Fluent catalog: {e}"))?;
        // Both supported languages are LTR. Preserve exact IDs/paths and the
        // existing clipboard/status contracts without invisible bidi marks.
        loader.set_use_isolating(false);
        Ok(Self { language, loader })
    }
    pub fn language(&self) -> Language {
        self.language
    }
    pub fn loader(&self) -> &FluentLanguageLoader {
        &self.loader
    }
}

// Immutable after startup. There is deliberately no runtime locale setter or
// per-thread language: background work uses the same language as the UI.
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
