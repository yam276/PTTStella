//! Native widgets use the same bundled CJK bytes as the terminal renderer.
use slint::fontique_011::{fontique, shared_collection};
use std::sync::Arc;

const FAMILY: &str = "Noto Sans CJK TC";

pub fn register() -> Result<(), &'static str> {
    configure(&mut shared_collection())
}

fn configure(collection: &mut fontique::Collection) -> Result<(), &'static str> {
    // Preserve the platform's existing choices, including cached system
    // fallbacks, before registration updates the shared collection version.
    let previous = [*b"Hani", *b"Bopo", *b"Hira", *b"Kana", *b"Hang"].map(|script| {
        let key = fontique::FallbackKey::new(fontique::Script::from_bytes(script), None);
        let families: Vec<_> = collection.fallback_families(key).collect();
        (key, families)
    });
    // Share the existing static asset; do not allocate another font byte buffer.
    let blob = fontique::Blob::new(Arc::new(pttstella::terminal_fonts::NOTO_SANS_CJK_TC));
    let families = collection.register_fonts(blob, None);
    let Some((family, _)) = families
        .iter()
        .find(|(id, fonts)| !fonts.is_empty() && collection.family_name(*id) == Some(FAMILY))
    else {
        return Err("bundled Traditional Chinese UI font did not register");
    };
    for (key, mut prior) in previous {
        if !prior.contains(family) {
            prior.push(*family);
        }
        collection.set_fallbacks(key, prior.into_iter());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_widget_fallback_reaches_existing_window_collections_without_system_fonts() {
        let mut collection = fontique::Collection::new(fontique::CollectionOptions {
            shared: true,
            system_fonts: false,
        });
        // Existing windows may already hold a clone when the root is created.
        let mut window_fonts = collection.clone();
        let han = fontique::FallbackKey::new(fontique::Script::from_bytes(*b"Hani"), None);
        let bopomofo = fontique::FallbackKey::new(fontique::Script::from_bytes(*b"Bopo"), None);
        assert!(window_fonts.fallback_families(han).next().is_none());
        configure(&mut collection).unwrap();
        for key in [han, bopomofo] {
            let families: Vec<_> = window_fonts.fallback_families(key).collect();
            assert!(
                families
                    .iter()
                    .any(|id| window_fonts.family_name(*id) == Some("Noto Sans CJK TC"))
            );
        }
    }

    #[test]
    fn installed_or_configured_fallback_keeps_priority_over_the_bundled_font() {
        let mut collection = fontique::Collection::new(fontique::CollectionOptions {
            shared: true,
            system_fonts: false,
        });
        let existing = collection.register_fonts(
            fontique::Blob::new(Arc::new(pttstella::terminal_fonts::CASCADIA_MONO)),
            None,
        )[0]
        .0;
        let key = fontique::FallbackKey::new(fontique::Script::from_bytes(*b"Hani"), None);
        collection.set_fallbacks(key, [existing].into_iter());
        configure(&mut collection).unwrap();
        let result: Vec<_> = collection.fallback_families(key).collect();
        assert_eq!(result.first(), Some(&existing));
        assert_eq!(result.len(), 2);
        assert_eq!(collection.family_name(result[1]), Some("Noto Sans CJK TC"));
    }
}
