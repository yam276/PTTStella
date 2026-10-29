//! Final autosave snapshot from the same surface; disk ownership stays with Files.
use super::protocol::SurfaceEvent;
#[derive(Default)]
pub(super) struct RetainedEdits {
    store: Option<(
        Result<pttstella::settings::store::Store, String>,
        pttstella::settings::AppSettings,
    )>,
}
#[derive(Default, serde::Deserialize)]
pub(super) struct Snapshot {
    last: Option<SurfaceEvent>,
    error: Option<String>,
}
impl RetainedEdits {
    pub(super) fn open(
        &mut self,
        store: Result<pttstella::settings::store::Store, String>,
        base: pttstella::settings::AppSettings,
    ) {
        self.store = Some((store, base));
    }
    /// A successful save drained during shutdown belongs to this editor, even
    /// when its UI acknowledgement never ran. Keep submitted fields, not the
    /// merged disk result, so unseen changes from other windows stay external.
    pub(super) fn submitted(&mut self, submitted: pttstella::settings::AppSettings) {
        if let Some((_, base)) = self.store.as_mut() {
            *base = submitted;
        }
    }
    pub(super) async fn finish(&mut self, snapshot: Snapshot) -> Result<(), String> {
        if let Some(error) = snapshot.error {
            return Err(error);
        }
        if let Some(edit) = snapshot.last {
            let (store, base) = self.store.take().ok_or("settings store unavailable")?;
            super::files::flush_settings(store?, base, final_change(edit)?).await?;
        }
        Ok(())
    }
}
fn final_change(edit: SurfaceEvent) -> Result<super::preferences::Change, String> {
    match edit {
        SurfaceEvent::SettingsSave {
            change,
            trusted: true,
        } => Ok(change),
        _ => Err("invalid settings snapshot".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn final_edit_preserves_other_preferences_and_rejects_non_save_events() {
        let directory = tempfile::tempdir().unwrap();
        let store = pttstella::settings::store::Store::at(directory.path().join("settings.json"));
        let app = pttstella::settings::AppSettings {
            hover: pttstella::settings::HoverPosition::Off,
            ..Default::default()
        };
        store.save(app).unwrap();
        let event=serde_json::from_value(serde_json::json!({"kind":"settings_save","values":{"fonts":{"kind":"platform_default"},"font_size":18,"baseline":19},"language":"en-US","hover":null,"tools_opacity":40,"tools_access":{"right_click_menu":false,"show_floating_button":false},"trusted":true})).unwrap();
        let mut retained = RetainedEdits::default();
        retained.open(Ok(store.clone()), store.load().unwrap());
        retained
            .finish(Snapshot {
                last: Some(event),
                error: None,
            })
            .await
            .unwrap();
        let saved = store.load().unwrap();
        assert_eq!(saved.display.font_size, 18.0);
        assert_eq!(saved.hover, pttstella::settings::HoverPosition::Off);
        assert!(!saved.tools_access.show_floating_button);
        let untrusted =
            serde_json::from_value(serde_json::json!({"kind":"settings_open","trusted":false}))
                .unwrap();
        assert!(final_change(untrusted).is_err());
    }
    #[tokio::test]
    async fn final_snapshot_merges_external_changes_and_rejects_conflicting_edits() {
        use pttstella::settings::{AppSettings, store::Store};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let store = Store::at(path.clone());
        let base = store.load().unwrap();
        let external = Store::at(path);
        let mut remote = base.clone();
        remote.hover = pttstella::settings::HoverPosition::Off;
        external.save_from(&base, remote.clone()).unwrap();
        let snapshot = || {
            serde_json::from_value(serde_json::json!({"last":{"kind":"settings_save","values":{ "fonts":{"kind":"platform_default"},"font_size":18,"baseline":20},"language":"zh-TW","tools_opacity":100,"tools_access":AppSettings::default().tools_access,"trusted":true}})).unwrap()
        };
        let mut retained = RetainedEdits::default();
        retained.open(Ok(store.clone()), base.clone());
        retained.finish(snapshot()).await.unwrap();
        assert_eq!(external.load().unwrap().hover, remote.hover);
        let mut external_draft = external.load().unwrap();
        external_draft.display.font_size = 19.0;
        external.save(external_draft.clone()).unwrap();
        retained.open(Ok(store), base);
        assert!(retained.finish(snapshot()).await.is_err());
        assert_eq!(external.load().unwrap(), external_draft);
    }
}
