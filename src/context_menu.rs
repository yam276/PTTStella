//! The ordered content-menu policy is shared with the legacy WebView adapter.
//! Entries describe capabilities; their existing owners still admit every action.
use serde::Deserialize;
use std::sync::OnceLock;
pub const POLICY: &str = include_str!("../assets/context-menu-policy.json");
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Flag {
    Picture,
    PictureReady,
    Word,
    Ptt,
    Local,
    Reading,
    Authors,
    Ready,
    Mask,
    MaskRow,
    Allow,
    Reply,
    Focused,
    Background,
    Selection,
    SelectionReady,
    RowsReady,
    Editor,
    Pending,
    Link,
    ExportReady,
    ReplyNavigation,
    Updates,
    Images,
    InlineReady,
    Navigate,
    First,
    Last,
    ListBookmark,
    Bookmark,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    CopyPicture,
    SavePicture,
    CopyLink,
    OpenLink,
    AccountMask,
    AccountRemoveMask,
    AccountMaskRow,
    AccountRemoveMaskRow,
    AccountAllow,
    AccountRemoveAllow,
    ReplyFocus,
    ReplyFocusClear,
    Copy,
    CopyAnsi,
    ExportRows,
    CopyImageRows,
    Paste,
    EditorColor,
    SelectLink,
    SelectAll,
    ExportPage,
    CopyImagePage,
    ReplyNavigation,
    ArticleUpdates,
    Inline,
    Back,
    Previous,
    Next,
    BookmarkList,
    BookmarkCurrent,
    Bookmarks,
    Settings,
}
#[derive(Clone, Copy, Default)]
pub struct Capabilities(u32);
impl Capabilities {
    pub fn set(&mut self, flag: Flag, value: bool) {
        let bit = 1u32 << (flag as u8);
        if value {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }
    pub fn get(self, flag: Flag) -> bool {
        self.0 & (1u32 << (flag as u8)) != 0
    }
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Predicate {
    #[serde(default)]
    all: Vec<Flag>,
    #[serde(default)]
    any: Vec<Flag>,
    #[serde(default)]
    none: Vec<Flag>,
}
impl Predicate {
    fn matches(&self, flags: Capabilities) -> bool {
        self.all.iter().all(|f| flags.get(*f))
            && (self.any.is_empty() || self.any.iter().any(|f| flags.get(*f)))
            && self.none.iter().all(|f| !flags.get(*f))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Toggle {
    flag: Flag,
    action: Action,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    action: Action,
    #[serde(default)]
    when: Predicate,
    #[serde(default)]
    enabled: Predicate,
    toggle: Option<Toggle>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    when: Predicate,
    entries: Vec<Item>,
    #[serde(default)]
    stop: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub action: Action,
    pub enabled: bool,
}
pub fn items(flags: Capabilities) -> Result<Vec<Entry>, String> {
    static PARSED: OnceLock<Result<Vec<Step>, String>> = OnceLock::new();
    let policy = PARSED
        .get_or_init(|| {
            serde_json::from_str(POLICY).map_err(|e| format!("context menu policy: {e}"))
        })
        .as_ref()
        .map_err(Clone::clone)?;
    let mut result: Vec<Entry> = Vec::new();
    for step in policy {
        if !step.when.matches(flags) {
            continue;
        }
        for item in &step.entries {
            if !item.when.matches(flags) {
                continue;
            }
            let action = item
                .toggle
                .as_ref()
                .filter(|toggle| flags.get(toggle.flag))
                .map_or(item.action, |toggle| toggle.action);
            let entry = Entry {
                action,
                enabled: item.enabled.matches(flags),
            };
            if let Some(old) = result.iter_mut().find(|e| e.action == action) {
                *old = entry;
            } else {
                result.push(entry);
            }
        }
        if step.stop {
            break;
        }
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn flags(values: &[Flag]) -> Capabilities {
        let mut flags = Capabilities::default();
        for value in values {
            flags.set(*value, true);
        }
        flags
    }
    fn actions(flags: Capabilities) -> Vec<Action> {
        items(flags)
            .unwrap()
            .into_iter()
            .map(|e| e.action)
            .collect()
    }
    #[test]
    fn shared_policy_preserves_picture_link_selection_editor_and_background_priority() {
        use {Action as A, Flag as F};
        assert_eq!(
            actions(flags(&[F::Ready, F::ExportReady, F::Ptt, F::Local])),
            [
                A::SelectAll,
                A::ExportPage,
                A::CopyImagePage,
                A::Bookmarks,
                A::Settings
            ]
        );
        assert_eq!(
            actions(flags(&[
                F::Picture,
                F::Link,
                F::Selection,
                F::Word,
                F::Reading,
                F::Ptt
            ])),
            [A::CopyPicture, A::SavePicture, A::CopyLink, A::OpenLink]
        );
        assert_eq!(
            actions(flags(&[F::Selection, F::Link, F::Reading, F::Ptt])),
            [
                A::Copy,
                A::CopyAnsi,
                A::ExportRows,
                A::CopyImageRows,
                A::CopyLink,
                A::OpenLink,
                A::SelectLink
            ]
        );
        assert_eq!(
            actions(flags(&[F::Editor, F::Selection, F::Ptt])),
            [
                A::Copy,
                A::CopyAnsi,
                A::ExportRows,
                A::CopyImageRows,
                A::Paste,
                A::EditorColor
            ]
        );
        let entries = items(flags(&[F::Reading, F::Navigate, F::First, F::Last])).unwrap();
        assert!(entries.contains(&Entry {
            action: A::Back,
            enabled: true
        }));
        for action in [A::Previous, A::Next] {
            assert!(entries.contains(&Entry {
                action,
                enabled: false
            }));
        }
    }
    #[test]
    fn account_removal_and_station_routing_keep_their_original_capability_limits() {
        use {Action as A, Flag as F};
        let state = flags(&[F::Word, F::Authors, F::Ptt, F::Local, F::MaskRow, F::Ready]);
        assert_eq!(
            actions(state),
            [A::AccountMask, A::AccountRemoveMaskRow, A::AccountAllow]
        );
        let mut state = state;
        state.set(F::Local, false);
        assert!(actions(state).is_empty());
        let entries = items(flags(&[F::Link])).unwrap();
        assert!(entries.contains(&Entry {
            action: A::CopyLink,
            enabled: true
        }));
        assert!(entries.contains(&Entry {
            action: A::OpenLink,
            enabled: false
        }));
        assert!(entries.contains(&Entry {
            action: A::SelectLink,
            enabled: true
        }));
    }
}
