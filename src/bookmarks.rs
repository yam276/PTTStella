//! Explicit local article identities, independent of the current screen/session.
use crate::article_target::ArticleTarget;
use serde::{Deserialize, Serialize};

#[cfg(feature = "desktop")]
pub mod store;

pub const LIMIT: usize = 500;
pub const MAX_BYTES: usize = 512 * 1024;
pub const LABEL_LIMIT: usize = 128;
pub const MAX_CREATED_AT: u64 = 253_402_300_799; // Last Unix second in year 9999.

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Entry {
    #[serde(flatten)]
    target: ArticleTarget,
    label: Option<String>,
    created_at: u64,
}
impl Entry {
    pub fn target(&self) -> &ArticleTarget {
        &self.target
    }
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }
    pub fn created_at(&self) -> u64 {
        self.created_at
    }
    fn matches(&self, target: &ArticleTarget) -> bool {
        self.target.board().same(target.board()) && self.target.aid() == target.aid()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bookmarks {
    entries: Vec<Entry>,
}
impl Bookmarks {
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    /// Updating the same identity preserves its position and creation time.
    /// The caller commits the returned draft only after its file save succeeds.
    pub fn upsert(
        &self,
        target: ArticleTarget,
        label: &str,
        created_at: u64,
    ) -> Result<Self, String> {
        let label = normalize_label(label)?;
        if !(1..=MAX_CREATED_AT).contains(&created_at) {
            return Err(crate::msg!("bookmarks-time-invalid"));
        }
        let mut next = self.clone();
        if let Some(old) = next.entries.iter_mut().find(|entry| entry.matches(&target)) {
            old.target = target;
            old.label = label;
        } else {
            if next.entries.len() >= LIMIT {
                return Err(crate::msg!("bookmarks-full"));
            }
            next.entries.insert(
                0,
                Entry {
                    target,
                    label,
                    created_at,
                },
            );
        }
        Ok(next)
    }
    pub fn remove(&self, target: &ArticleTarget) -> Result<Self, String> {
        let mut next = self.clone();
        let position = next
            .entries
            .iter()
            .position(|entry| entry.matches(target))
            .ok_or(crate::msg!("bookmarks-not-found"))?;
        next.entries.remove(position);
        Ok(next)
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        // Shared saves can merge individually valid drafts past the item cap.
        // Enforce it at the serialization owner before either file is replaced.
        if self.entries.len() > LIMIT {
            return Err(crate::msg!("bookmarks-full"));
        }
        let bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "version": 1,
            "entries": self.entries,
        }))
        .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err(crate::msg!("bookmarks-invalid"));
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES {
            return Err(crate::msg!("bookmarks-invalid"));
        }
        let document: Document =
            serde_json::from_slice(bytes).map_err(|_| crate::msg!("bookmarks-invalid"))?;
        if document.version != 1 || document.entries.len() > LIMIT {
            return Err(crate::msg!("bookmarks-invalid"));
        }
        let mut entries = Vec::with_capacity(document.entries.len());
        for stored in document.entries {
            let target = ArticleTarget::new(&stored.board, &stored.aid)
                .map_err(|_| crate::msg!("bookmarks-invalid"))?;
            let label = normalize_label(stored.label.as_deref().unwrap_or_default())
                .map_err(|_| crate::msg!("bookmarks-invalid"))?;
            if target.board().as_str() != stored.board
                || target.aid().as_str() != stored.aid
                || label != stored.label
                || !(1..=MAX_CREATED_AT).contains(&stored.created_at)
                || entries.iter().any(|entry: &Entry| entry.matches(&target))
            {
                return Err(crate::msg!("bookmarks-invalid"));
            }
            entries.push(Entry {
                target,
                label,
                created_at: stored.created_at,
            });
        }
        Ok(Self { entries })
    }
}

fn normalize_label(value: &str) -> Result<Option<String>, String> {
    // Check before trim, so a truncated over-limit draft cannot save a prefix.
    if value.len() > LABEL_LIMIT * 4
        || value.chars().count() > LABEL_LIMIT
        || value.chars().any(char::is_control)
    {
        return Err(crate::msg!("bookmarks-label-invalid"));
    }
    let value = value.trim();
    Ok((!value.is_empty()).then(|| value.to_owned()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u32,
    entries: Vec<StoredEntry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredEntry {
    board: String,
    aid: String,
    label: Option<String>,
    created_at: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target(board: &str, aid: &str) -> ArticleTarget {
        ArticleTarget::new(board, aid).unwrap()
    }
    #[test]
    fn explicit_edits_preserve_identity_case_time_order_and_the_original_draft() {
        let first = Bookmarks::default()
            .upsert(target("Stella", "00000101"), " 中文書籤 ", 100)
            .unwrap();
        let second = first.upsert(target("Other", "1AbCdE-_"), "", 200).unwrap();
        let updated = second
            .upsert(target("stella", "00000101"), "改名", 300)
            .unwrap();
        assert_eq!(first.entries.len(), 1);
        assert_eq!(first.entries[0].label(), Some("中文書籤"));
        assert_eq!(updated.entries.len(), 2);
        assert_eq!(updated.entries[0].target().board().as_str(), "Other");
        assert_eq!(updated.entries[1].target().board().as_str(), "stella");
        assert_eq!(updated.entries[1].label(), Some("改名"));
        assert_eq!(updated.entries[1].created_at(), 100);
        let distinct = updated
            .upsert(target("other", "1ABCDE-_"), "不同 AID", 400)
            .unwrap();
        assert_eq!(distinct.entries.len(), 3);
        let removed = distinct.remove(&target("STELLA", "00000101")).unwrap();
        assert_eq!(removed.entries.len(), 2);
        assert!(removed.remove(&target("STELLA", "00000101")).is_err());
        assert_eq!(distinct.entries.len(), 3);
        assert_eq!(
            Bookmarks::decode(&distinct.encode().unwrap()).unwrap(),
            distinct
        );
    }
    #[test]
    fn capacity_updates_and_label_time_boundaries_are_explicit() {
        let mut bookmarks = Bookmarks::default();
        for n in 0..LIMIT {
            bookmarks = bookmarks
                .upsert(
                    target(&format!("Board{n}"), "00000101"),
                    &"😀".repeat(128),
                    n as u64 + 1,
                )
                .unwrap();
        }
        assert!(
            bookmarks
                .upsert(target("Extra", "00000101"), "", 1000)
                .is_err()
        );
        let updated = bookmarks
            .upsert(target("board0", "00000101"), &"字".repeat(128), 1000)
            .unwrap();
        assert_eq!(updated.entries.len(), LIMIT);
        assert!(updated.encode().unwrap().len() < MAX_BYTES);
        for label in [
            "字".repeat(129),
            "x".repeat(129),
            "bad\nlabel".into(),
            "bad\0label".into(),
        ] {
            assert!(
                updated
                    .upsert(target("board0", "00000101"), &label, 1000)
                    .is_err()
            );
        }
        for time in [0, MAX_CREATED_AT + 1] {
            assert!(
                updated
                    .upsert(target("board0", "00000101"), "", time)
                    .is_err()
            );
        }
        assert!(
            Bookmarks::default()
                .upsert(target("Stella", "00000101"), "", MAX_CREATED_AT)
                .is_ok()
        );
    }
    #[test]
    fn document_has_independent_expected_values_and_rejects_noncanonical_data() {
        let fixture = include_bytes!("../tests/fixtures/bookmarks.json");
        let bookmarks = Bookmarks::decode(fixture).unwrap();
        assert_eq!(bookmarks.entries.len(), 2);
        assert_eq!(bookmarks.entries[0].target(), &target("Stella", "00000101"));
        assert_eq!(bookmarks.entries[0].label(), Some("自製合成書籤"));
        assert_eq!(bookmarks.entries[0].created_at(), 100);
        assert_eq!(bookmarks.entries[1].label(), None);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bookmarks.encode().unwrap()).unwrap(),
            serde_json::from_slice::<serde_json::Value>(fixture).unwrap()
        );
        let original: serde_json::Value = serde_json::from_slice(fixture).unwrap();
        for (key, value) in [
            ("board", serde_json::json!(" Stella ")),
            ("aid", serde_json::json!("#00000101")),
            ("aid", serde_json::json!("00000000")),
            ("label", serde_json::json!(" ")),
            ("label", serde_json::json!("x\ny")),
            ("label", serde_json::json!("x".repeat(129))),
            ("created_at", serde_json::json!(0)),
            ("created_at", serde_json::json!(-1)),
            ("created_at", serde_json::json!(MAX_CREATED_AT + 1)),
            ("unknown", serde_json::json!(true)),
        ] {
            let mut bad = original.clone();
            bad["entries"][0][key] = value;
            assert!(
                Bookmarks::decode(&serde_json::to_vec(&bad).unwrap()).is_err(),
                "{key}"
            );
        }
        let mut duplicate = original.clone();
        duplicate["entries"][1] = duplicate["entries"][0].clone();
        duplicate["entries"][1]["board"] = serde_json::json!("stella");
        assert!(Bookmarks::decode(&serde_json::to_vec(&duplicate).unwrap()).is_err());
        for bad in [
            b"broken".to_vec(),
            vec![b' '; MAX_BYTES + 1],
            br#"{"version":2,"entries":[]}"#.to_vec(),
            br#"{"version":1,"entries":[],"account":"unused"}"#.to_vec(),
        ] {
            assert!(Bookmarks::decode(&bad).is_err());
        }
        let too_many = serde_json::json!({"version":1,"entries":vec![original["entries"][0].clone(); LIMIT+1]});
        assert!(Bookmarks::decode(&serde_json::to_vec(&too_many).unwrap()).is_err());
    }
}
