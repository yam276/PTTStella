//! Only the explicit loopback article probe calls this, with its owned store path.
use super::*;
use crate::verification::{self, require, wait};
use slint::{
    Model,
    platform::{Key, PointerEventButton as Button, WindowEvent as Event},
};
use std::{path::Path, time::Duration};

impl Media {
    pub(crate) async fn verify_automatic(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        path: &Path,
        directory: &Path,
    ) -> Result<(), String> {
        require(
            self.probe && !ui.get_media_auto_enabled() && self.worker.borrow().is_none(),
            "auto default started a worker",
        )?;
        let original = self
            .output
            .with_frame(ui, serde_json::to_vec)
            .ok_or("automatic source missing")?
            .map_err(|e| e.to_string())?;
        let baseline = self.preferences.model.borrow().app().clone();
        let mut external = baseline.clone();
        external.hover = pttstella::settings::HoverPosition::Off;
        external.display.font_width = 110.;
        self.preferences
            .model
            .borrow()
            .store()?
            .save_from(&baseline, external)?;
        ui.set_media_auto_choice(true);
        ui.invoke_media_auto_preference(true);
        require(
            !ui.get_media_auto_enabled(),
            "auto applied before persisted completion",
        )?;
        wait(
            || !ui.get_output_busy() && ui.get_media_auto_enabled(),
            "automatic preference did not save",
        )
        .await?;
        wait(
            || {
                self.inline_snapshot()
                    .is_some_and(|s| s.automatic && s.resources.len() == 3)
                    && ui.get_inline_pictures().row_count() == 3
            },
            "automatic images did not render",
        )
        .await?;
        require(
            ui.get_inline_pictures().iter().all(|picture| {
                (picture.x + picture.width / 2. - (ui.get_view_width() - 12.) / 2.).abs() < 0.5
            }),
            "automatically expanded pictures were not centered in the article",
        )?;
        require(
            ui.get_media_hover_position() == 2 && ui.get_media_hover_choice() == 2,
            "merged hover preference did not apply",
        )?;
        let reopened = pttstella::settings::store::Store::at(path.to_owned()).load()?;
        require(
            reopened.auto_expand_images
                && reopened.hover == pttstella::settings::HoverPosition::Off
                && reopened.display.font_width == 110.,
            "fresh store lost automatic or unrelated settings",
        )?;
        let initial = self
            .inline_snapshot()
            .ok_or("automatic resources missing")?;
        verification::capture(ui, directory, "automatic-on.png")?;
        self.auto_probe_save(ui, false).await?;
        wait(
            || {
                self.inline_snapshot()
                    .is_some_and(|s| !s.automatic && s.entries.is_empty() && s.resources.is_empty())
                    && ui.get_inline_pictures().row_count() == 0
            },
            "automatic opt-out retained images or queue",
        )
        .await?;
        verification::capture(ui, directory, "automatic-off.png")?;
        ui.invoke_focus_article();
        wait(
            || ui.get_inline_enabled(),
            "manual inline controls did not recover",
        )
        .await?;
        let arrow = ui
            .get_inline_arrows()
            .row_data(0)
            .ok_or("manual arrow missing")?;
        let point = slint::LogicalPosition::new(
            ui.get_terminal_origin_x() + arrow.x + arrow.size / 2.,
            ui.get_terminal_origin_y() + arrow.y + arrow.size / 2.,
        );
        ui.window()
            .dispatch_event(Event::PointerMoved { position: point });
        verification::click(ui, point, Button::Left);
        wait(
            || {
                self.inline_snapshot()
                    .is_some_and(|s| !s.automatic && s.resources.len() == 1)
            },
            "manual image unavailable after opt-out",
        )
        .await?;
        let manual = self.inline_snapshot().ok_or("manual resource missing")?;
        require(
            std::sync::Arc::ptr_eq(
                initial.resources[&0]
                    .original
                    .as_ref()
                    .ok_or("initial original missing")?,
                manual.resources[&0]
                    .original
                    .as_ref()
                    .ok_or("manual original missing")?,
            ),
            "opt-out discarded loader cache original",
        )?;
        self.auto_probe_save(ui, true).await?;
        wait(
            || {
                self.inline_snapshot()
                    .is_some_and(|s| s.automatic && s.resources.len() == 3)
                    && ui.get_inline_enabled()
            },
            "automatic re-enable did not extend same article",
        )
        .await?;
        ui.invoke_focus_article();
        ui.window().dispatch_event(Event::KeyPressed {
            text: Key::Return.into(),
        });
        wait(
            || {
                self.inline_snapshot()
                    .is_some_and(|s| !s.automatic && s.resources.len() == 3)
            },
            "manual All did not promote automatic batch",
        )
        .await?;
        ui.window().dispatch_event(Event::KeyReleased {
            text: Key::Return.into(),
        });
        self.auto_probe_save(ui, false).await?;
        tokio::time::sleep(Duration::from_millis(80)).await;
        require(
            self.inline_snapshot()
                .is_some_and(|s| !s.automatic && s.resources.len() == 3),
            "opt-out removed explicitly promoted manual batch",
        )?;
        let before = self
            .preferences
            .model
            .borrow()
            .store()?
            .read()?
            .ok_or("saved settings missing")?;
        let bad = br#"{"version":999}"#;
        std::fs::write(path, bad).map_err(|e| e.to_string())?;
        ui.set_media_auto_choice(true);
        ui.invoke_media_auto_preference(true);
        wait(
            || !ui.get_output_busy(),
            "failed automatic save did not join",
        )
        .await?;
        require(
            !ui.get_media_auto_enabled()
                && ui.get_media_auto_choice()
                && ui
                    .get_input_notice()
                    .starts_with(&msg!("settings-save-failed"))
                && std::fs::read(path).map_err(|e| e.to_string())? == bad
                && self
                    .inline_snapshot()
                    .is_some_and(|s| s.resources.len() == 3 && !s.automatic),
            "failed automatic save applied, erased retry draft, overwrote data or changed manual batch",
        )?;
        verification::capture(ui, directory, "automatic-save-failed.png")?;
        std::fs::write(path, &before).map_err(|e| e.to_string())?;
        ui.invoke_media_auto_preference(true);
        wait(
            || !ui.get_output_busy() && ui.get_media_auto_enabled(),
            "explicit automatic retry did not recover",
        )
        .await?;
        self.auto_probe_save(ui, false).await?;
        wait(
            || self.output.current(ui).is_some(),
            "automatic output source did not recover",
        )
        .await?;
        require(
            self.output
                .with_frame(ui, serde_json::to_vec)
                .ok_or("final source missing")?
                .map_err(|e| e.to_string())?
                == original,
            "automatic preferences changed terminal source",
        )?;
        let record = serde_json::json!({"status":"passed","scope":"actual Slint widget with typed preference callbacks, synthetic pointer/keyboard/focus; isolated TempDir settings and loopback only","default_no_worker":true,"save_before_apply":true,"fresh_store_compatible":true,"unrelated_merge_and_hover_apply":true,"opt_out_clears_auto_images":true,"manual_arrow_after_opt_out":true,"cache_original_arc_reused":true,"manual_all_survives_opt_out":true,"failed_save_preserves_file_draft_and_runtime":true,"explicit_retry_recovers":true,"terminal_source_unchanged":true,"pending":["native OS checkbox and restart acceptance on four targets","picture menu","resource performance"]});
        std::fs::write(
            directory.join("result.json"),
            serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn inline_snapshot(&self) -> Option<crate::inline_engine::State> {
        self.worker
            .borrow()
            .as_ref()
            .map(|w| w.inline_updates().borrow().clone())
    }
    async fn auto_probe_save(&self, ui: &TerminalWindow, enabled: bool) -> Result<(), String> {
        ui.set_media_auto_choice(enabled);
        ui.invoke_media_auto_preference(enabled);
        wait(
            || !ui.get_output_busy() && ui.get_media_auto_enabled() == enabled,
            "automatic save/apply failed",
        )
        .await
    }
}
