//! Final autosave snapshot from the same surface; disk ownership stays with Files.
use super::protocol::SurfaceEvent;
#[derive(Default)]
pub(super) struct RetainedEdits {
    store: Option<Result<pttstella::settings::store::Store, String>>,
}
#[derive(Default, serde::Deserialize)]
pub(super) struct Snapshot {
    last: Option<SurfaceEvent>,
    error: Option<String>,
}
impl RetainedEdits {
    pub(super) fn open(&mut self, store: Result<pttstella::settings::store::Store, String>) {
        self.store = Some(store);
    }
    pub(super) async fn finish(&mut self, snapshot: Snapshot) -> Result<(), String> {
        if let Some(error) = snapshot.error {
            return Err(error);
        }
        if let Some(edit) = snapshot.last {
            let store = self.store.take().ok_or("settings store unavailable")??;
            super::files::flush_settings(store, final_change(edit)?).await?;
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
        retained.open(Ok(store.clone()));
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
}
