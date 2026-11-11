//! Fixed independent expectations exercise the real search/result/focus callbacks.
use super::*;
use crate::verification::{require, wait};
use pttstella::i18n::Language;
use slint::{Model, platform::WindowEvent};
use std::path::Path;

impl Preferences {
    pub(super) async fn verify_search(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        dir: &Path,
    ) -> Result<(), String> {
        let original = self.app();
        let initial = self
            .model
            .borrow()
            .store()?
            .read()?
            .ok_or("search seed missing")?;
        ui.invoke_settings_show();
        let window = self
            .window
            .borrow()
            .as_ref()
            .and_then(|w| w.as_weak().upgrade())
            .ok_or("search window missing")?;
        let mut checked = Vec::new();
        // Independently specified widget destinations, not generated from the index.
        const CASES: [(&str, i32, i32); 24] = [
            ("language", 0, 10),
            ("zoom fit", 1, 20),
            ("experimental keyboard", 2, 21),
            ("mouse reporting", 2, 22),
            ("keepalive", 0, 23),
            ("font preset", 1, 11),
            ("latin", 1, 1),
            ("CJK FONT", 1, 2),
            ("font size", 1, 3),
            ("width", 1, 5),
            ("reset font", 1, 24),
            ("ＢＡＳＥＬＩＮＥ", 1, 4),
            ("profile import", 1, 7),
            ("ﬂoating opacity", 2, 6),
            ("right click", 2, 12),
            ("visibility", 2, 13),
            ("article mode", 0, 14),
            ("auto expand images", 3, 15),
            ("previous next", 0, 16),
            ("hover", 3, 17),
            ("site connection", 4, 18),
            ("privacy", 6, 19),
            ("about license", 7, 25),
            ("blacklist", 5, 26),
        ];
        for (language, selector) in [(Language::English, 1), (Language::TraditionalChinese, 0)] {
            window
                .global::<crate::SettingsState>()
                .set_language(selector);
            window.global::<crate::SettingsState>().invoke_edit();
            wait(
                || self.app().language == language && !ui.get_output_busy(),
                "search language did not save",
            )
            .await?;
            for (query, page, field) in CASES {
                window
                    .window()
                    .dispatch_event(WindowEvent::WindowActiveChanged(true));
                window
                    .global::<crate::SettingsState>()
                    .set_query(query.into());
                window
                    .global::<crate::SettingsState>()
                    .invoke_search(query.into());
                let index = (0..window
                    .global::<crate::SettingsState>()
                    .get_matches()
                    .row_count())
                    .find(|index| {
                        window
                            .global::<crate::SettingsState>()
                            .get_matches()
                            .row_data(*index)
                            .is_some_and(|r| r.page == page && r.field == field)
                    })
                    .ok_or_else(|| format!("missing native settings result for {query}"))?;
                let before = self.app();
                window
                    .global::<crate::SettingsState>()
                    .invoke_choose_result(index as i32);
                wait(
                    || window.global::<crate::SettingsState>().get_focused_target() == field,
                    &format!("result failed to focus {query}: expected {field}"),
                )
                .await?;
                require(
                    window.global::<crate::SettingsState>().get_page() == page
                        && window
                            .global::<crate::SettingsState>()
                            .get_query()
                            .is_empty()
                        && self.app() == before,
                    "search activation changed preferences or failed to navigate",
                )?;
                require(
                    window.global::<crate::SettingsState>().invoke_edit_key(0)
                        == matches!(field, 1..=7),
                    "text shortcut swallowed by a non-text target",
                )?;
                eprintln!(
                    "settings search focused {} {} {}",
                    language.tag(),
                    query,
                    field
                );
                checked.push(serde_json::json!({"language":language.tag(),"query":query,"page":page,"focused_field":field}));
            }
            for query in [
                "",
                " ",
                "private-account-123",
                "<img src=x onerror=alert(1)>",
                "baseline unrelated",
            ] {
                window
                    .global::<crate::SettingsState>()
                    .set_query(query.into());
                window
                    .global::<crate::SettingsState>()
                    .invoke_search(query.into());
                require(
                    window
                        .global::<crate::SettingsState>()
                        .get_matches()
                        .row_count()
                        == 0,
                    "search matched a blank/private/unknown value",
                )?;
            }
            // A new query cancels an already scheduled focus request.
            window
                .global::<crate::SettingsState>()
                .set_query("baseline".into());
            window
                .global::<crate::SettingsState>()
                .invoke_search(window.global::<crate::SettingsState>().get_query());
            window
                .global::<crate::SettingsState>()
                .invoke_choose_result(0);
            window
                .global::<crate::SettingsState>()
                .set_query("opacity".into());
            window
                .global::<crate::SettingsState>()
                .invoke_search(window.global::<crate::SettingsState>().get_query());
            window
                .global::<crate::SettingsState>()
                .invoke_focus_search();
            tokio::time::sleep(Duration::from_millis(50)).await;
            require(
                window.global::<crate::SettingsState>().get_query() == "opacity"
                    && window.global::<crate::SettingsState>().get_focused_target() == 0
                    && !self.search_focus.running(),
                "obsolete result stole focus from the new query",
            )?;
            // Focus may also return through the native field's normal focus
            // path rather than the explicit focus-search callback.
            window.global::<crate::SettingsState>().set_query("".into());
            window.global::<crate::SettingsState>().set_page(1);
            window
                .global::<crate::SettingsState>()
                .invoke_focus_result(3);
            wait(
                || window.global::<crate::SettingsState>().get_focused_target() == 3,
                "native field focus setup failed",
            )
            .await?;
            window
                .global::<crate::SettingsState>()
                .set_query("baseline".into());
            window
                .global::<crate::SettingsState>()
                .invoke_search(window.global::<crate::SettingsState>().get_query());
            window
                .global::<crate::SettingsState>()
                .invoke_choose_result(0);
            window
                .global::<crate::SettingsState>()
                .invoke_focus_field(0);
            tokio::time::sleep(Duration::from_millis(50)).await;
            require(
                window.global::<crate::SettingsState>().get_focused_target() == 0
                    && !self.search_focus.running(),
                "pending result stole ordinary native search focus",
            )?;
        }
        // Search never repopulates or indexes an incomplete widget draft/name.
        window
            .global::<crate::SettingsState>()
            .set_font_size("".into());
        window.global::<crate::SettingsState>().invoke_edit();
        window
            .global::<crate::SettingsState>()
            .set_profile_name("private-profile-123".into());
        window
            .global::<crate::SettingsState>()
            .set_query("private-profile-123".into());
        window
            .global::<crate::SettingsState>()
            .invoke_search(window.global::<crate::SettingsState>().get_query());
        require(
            window
                .global::<crate::SettingsState>()
                .get_matches()
                .row_count()
                == 0,
            "search indexed a user profile name",
        )?;
        window
            .global::<crate::SettingsState>()
            .set_query("visibility".into());
        window
            .global::<crate::SettingsState>()
            .invoke_search(window.global::<crate::SettingsState>().get_query());
        window
            .global::<crate::SettingsState>()
            .invoke_choose_result(0);
        wait(
            || window.global::<crate::SettingsState>().get_focused_target() == 13,
            "search lost incomplete draft focus target",
        )
        .await?;
        require(
            window
                .global::<crate::SettingsState>()
                .get_font_size()
                .is_empty()
                && window.global::<crate::SettingsState>().get_profile_name()
                    == "private-profile-123",
            "search replaced the native draft",
        )?;
        window
            .global::<crate::SettingsState>()
            .set_font_size(original.display.font_size.to_string().into());
        window.global::<crate::SettingsState>().invoke_edit();
        wait(
            || self.valid.get() && self.pending.borrow().is_none() && !ui.get_output_busy(),
            "restored search draft did not drain",
        )
        .await?;
        require(
            self.app() == original
                && self.model.borrow().store()?.read()?.as_deref() == Some(initial.as_slice()),
            "search changed another setting or lost original saved state",
        )?;
        // Invalid names stop before picker/worker creation; the same typed
        // success consumer used by the real picker restores the imported name.
        window
            .global::<crate::SettingsState>()
            .set_profile_name("".into());
        self.profile(true);
        require(
            window.global::<crate::SettingsState>().get_notice() == msg!("profile-name-invalid")
                && !self.output.busy(),
            "invalid profile name entered OS output",
        )?;
        self.imported(pttstella::settings::PortableProfile::new(
            "自製具名測試".into(),
            original.display.clone(),
        )?);
        require(
            window.global::<crate::SettingsState>().get_profile_name() == "自製具名測試"
                && window.global::<crate::SettingsState>().get_notice()
                    == msg!("slint-profile-imported"),
            "import retained an obsolete export/error notice or lost the name",
        )?;
        wait(
            || self.pending.borrow().is_none() && !ui.get_output_busy(),
            "imported settings did not drain",
        )
        .await?;
        window
            .global::<crate::SettingsState>()
            .invoke_close_settings();
        wait(|| !ui.get_settings_open(), "search window did not close").await?;
        require(!self.search_focus.running(), "focus timer survived close")?;
        // Runtime controls are explicit activations after settings drain, not
        // side effects of search/category changes. Each toggle is restored.
        let zoom = ui.get_zoom();
        ui.set_zoom(1.5);
        ui.invoke_settings_show();
        window
            .global::<crate::SettingsState>()
            .invoke_runtime_action(0);
        wait(
            || !ui.get_settings_open() && ui.get_zoom() == 1.,
            "fit handoff did not apply",
        )
        .await?;
        for (action, expected) in [(6, 1.25), (7, 1.5)] {
            ui.invoke_settings_show();
            window
                .global::<crate::SettingsState>()
                .invoke_runtime_action(action);
            wait(
                || !ui.get_settings_open() && ui.get_zoom() == expected,
                "zoom preset handoff did not apply",
            )
            .await?;
        }
        ui.set_zoom(zoom);
        for action in 1..=4 {
            let value = || match action {
                1 => ui.get_vim_enabled().to_string(),
                2 => ui.get_station_mouse_enabled().to_string(),
                3 => ui.get_keep_alive_enabled().to_string(),
                _ => ui.get_pan_mode().to_string(),
            };
            let before = value();
            for restore in [false, true] {
                ui.invoke_settings_show();
                window
                    .global::<crate::SettingsState>()
                    .invoke_runtime_action(action);
                wait(
                    || !ui.get_settings_open() && (value() == before) == restore,
                    &format!("runtime settings action {action} did not toggle/restore"),
                )
                .await?;
            }
        }
        require(
            self.app() == original
                && self.model.borrow().store()?.read()?.as_deref() == Some(initial.as_slice()),
            "runtime controls changed persisted preferences",
        )?;
        std::fs::write(dir.join("settings-search.json"),serde_json::to_vec_pretty(&serde_json::json!({
            "status":"passed","cases":checked,"pending_filter_manager":"not included until native manager exists",
            "stale_focus_cancelled":true,"private_name_not_indexed":true,"incomplete_draft_preserved":true,
            "saved_settings_restored":true,"runtime_zoom_and_four_toggles_restored":true,"profile_name_validation_and_import_notice":true,"scope":"real native widgets and production callbacks, synthetic focus; not OS IME/accessibility or all-platform acceptance"
        })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
    }
}
