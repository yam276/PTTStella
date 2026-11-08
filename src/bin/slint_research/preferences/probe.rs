//! Explicit loopback-only settings acceptance. Native widgets and production
//! save path, synthetic values/foreground; never a claim about OS IME/pickers.
use super::*;
use crate::verification::{require, wait};
use slint::{Model, winit_030::WinitWindowAccessor};
use std::{fs::OpenOptions, path::Path};

impl Preferences {
    pub(crate) fn prepare_shutdown(self: &Rc<Self>, dir: &Path) -> Result<(), String> {
        let mut expected = self.model.borrow().store()?.load()?;
        expected.display.font_size = 22.;
        std::fs::write(
            dir.join("shutdown-expected.json"),
            serde_json::to_vec_pretty(&expected).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        // The verification future exits in this same poll, so the completion
        // callback cannot consume the first result before the loop quits.
        let mut first = self.app();
        first.display.font_size = 21.;
        self.queue(first, Duration::ZERO);
        self.flush();
        require(
            self.inflight.borrow().is_some(),
            "shutdown fixture did not start its first write",
        )?;
        let mut latest = self.app();
        latest.display.font_size = 22.;
        self.queue(latest, Duration::from_millis(300));
        Ok(())
    }
    pub(crate) fn audit_shutdown(&self, dir: &Path) -> Result<(), String> {
        let expected = AppSettings::decode(
            &std::fs::read(dir.join("shutdown-expected.json")).map_err(|e| e.to_string())?,
        )?;
        let saved = self.model.borrow().store()?.load()?;
        require(
            self.stopped.get()
                && self.inflight.borrow().is_none()
                && self.pending.borrow().is_none()
                && !self.timer.running()
                && self.window.borrow().is_none()
                && saved == expected
                && self.app() == expected,
            "shutdown lost a write/result/latest draft or retained native work",
        )?;
        std::fs::write(
            dir.join("shutdown-saved.json"),
            serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub(crate) async fn verify(
        self: &Rc<Self>,
        ui: &TerminalWindow,
        dir: &Path,
        settings_path: &Path,
    ) -> Result<(), String> {
        let original = self.app();
        let store = self.model.borrow().store()?;
        let initial = store.read()?.ok_or("isolated settings seed missing")?;
        require(
            AppSettings::decode(&initial)? == original,
            "isolated file/model seeds differ",
        )?;
        ui.invoke_settings_show();
        let window = self
            .window
            .borrow()
            .as_ref()
            .and_then(|w| w.as_weak().upgrade())
            .ok_or("settings window missing")?;
        require(
            ui.get_settings_open() && !ui.get_keep_alive_surface_ready(),
            "settings did not suspend reading/keepalive",
        )?;
        let identity = window.window().with_winit_window(|w| w.id());
        ui.invoke_settings_show();
        require(
            identity == window.window().with_winit_window(|w| w.id()),
            "settings opened a second native window",
        )?;
        window.set_page(1);
        window.set_font_size("18".into());
        window.invoke_edit();
        tokio::time::sleep(Duration::from_millis(120)).await;
        require(
            store.read()?.as_deref() == Some(initial.as_slice()) && self.app() == original,
            "settings saved/applied before debounce",
        )?;
        window.set_font_width("110".into());
        window.invoke_edit();
        tokio::time::sleep(Duration::from_millis(150)).await;
        require(
            store.read()?.as_deref() == Some(initial.as_slice()),
            "new edit did not restart debounce",
        )?;
        wait(
            || !ui.get_output_busy() && self.app().display.font_width == 110.,
            "debounced settings never applied",
        )
        .await?;
        require(
            store.load()?.display.font_size == 18. && self.app().display.font_size == 18.,
            "coalescing lost one field",
        )?;

        // Hold only our synthetic Output job; two edits must collapse into one
        // latest draft, then use the shared worker after it becomes available.
        let (release, held) = std::sync::mpsc::sync_channel(1);
        self.output.work(
            ui,
            move || {
                held.recv_timeout(Duration::from_secs(3))
                    .map_err(|e| e.to_string())
            },
            |_, _, _| {},
        );
        window.set_font_size("21".into());
        window.invoke_edit();
        tokio::time::sleep(Duration::from_millis(320)).await;
        window.set_font_size("22".into());
        window.invoke_edit();
        require(
            self.pending
                .borrow()
                .as_ref()
                .is_some_and(|d| d.display.font_size == 22.)
                && self.inflight.borrow().is_none()
                && store.load()?.display.font_size == 18.,
            "busy file worker dropped/applied a pending edit",
        )?;
        release.send(()).map_err(|e| e.to_string())?;
        wait(
            || !ui.get_output_busy() && self.app().display.font_size == 22.,
            "latest edit did not follow owned file work",
        )
        .await?;

        require(
            store.load()?.display.fonts == original.display.fonts
                && self.app().display.fonts == original.display.fonts,
            "numeric edits changed the requested font intent",
        )?;
        let backup_dir = dir.join("held-settings-backup.json");
        let path = dir.join("settings-window.json");
        // Only the probe-owned TempDir Store is used. Recover its path from a
        // separately supplied owner method, never a product settings directory.
        require(!ui.get_live_session(), "settings probe refused Live")?;
        let backup = settings_path.with_extension("json.bak");
        std::fs::rename(&backup, &backup_dir).map_err(|e| e.to_string())?;
        std::fs::create_dir(&backup).map_err(|e| e.to_string())?;
        window.set_font_size("19".into());
        window.invoke_edit();
        wait(
            || self.failed.get() && !ui.get_output_busy(),
            "backup failure did not reach retry UI",
        )
        .await?;
        require(
            window.get_font_size() == "19"
                && self.app().display.font_size == 22.
                && store.load()?.display.font_size == 22.,
            "failed save replaced file/effective value or lost draft",
        )?;
        capture(&window, dir, "settings-save-failed.png")?;
        require(
            std::fs::read_dir(&backup)
                .map_err(|e| e.to_string())?
                .next()
                .is_none(),
            "injected backup directory acquired contents",
        )?;
        std::fs::remove_dir(&backup).map_err(|e| e.to_string())?;
        std::fs::rename(&backup_dir, &backup).map_err(|e| e.to_string())?;
        window.invoke_retry();
        wait(
            || !ui.get_output_busy() && self.app().display.font_size == 19.,
            "settings retry did not recover",
        )
        .await?;
        require(
            store.load()?.display.font_size == 19.,
            "retry did not persist",
        )?;

        window.set_font_size("-".into());
        window.invoke_edit();
        tokio::time::sleep(Duration::from_millis(360)).await;
        require(
            store.load()?.display.font_size == 19. && self.pending.borrow().is_none(),
            "invalid intermediate was saved",
        )?;
        window.set_font_size("20".into());
        self.composing.set(true);
        window.invoke_edit();
        tokio::time::sleep(Duration::from_millis(360)).await;
        require(
            store.load()?.display.font_size == 19.,
            "synthetic preedit was saved",
        )?;
        self.composing.set(false);
        window.invoke_edit();
        wait(
            || !ui.get_output_busy() && self.app().display.font_size == 20.,
            "committed valid edit was not saved",
        )
        .await?;
        window.invoke_search("baseline".into());
        require(
            window.get_matches().row_count() == 1
                && window
                    .get_matches()
                    .row_data(0)
                    .is_some_and(|m| m.page == 1),
            "settings search failed",
        )?;

        window.invoke_close_settings();
        wait(|| !ui.get_settings_open(), "settings close did not drain").await?;
        ui.invoke_settings_show();
        require(
            window.get_font_size() == "20" && window.get_font_width() == "110",
            "reopen did not show persisted values",
        )?;
        capture(&window, dir, "settings-saved.png")?;

        // This explicit probe restores its initial settings before handing
        // control back to the unchanged media workload and strict peer.
        self.model.borrow_mut().open();
        form::populate(&window, &original);
        window.invoke_edit();
        wait(
            || !ui.get_output_busy() && self.app() == original,
            "settings probe restore failed",
        )
        .await?;
        window.invoke_close_settings();
        wait(
            || !ui.get_settings_open(),
            "restored settings window remained open",
        )
        .await?;
        crate::verification::foreground(ui, true);
        ui.invoke_focus_terminal();
        std::fs::write(path,serde_json::to_vec_pretty(&serde_json::json!({"status":"passed","scope":"actual native settings component and production file worker; synthetic properties/foreground/composition only","isolated_store":true,"single_native_window":true,"debounce_restart_and_latest_edit":true,"busy_worker_retains_latest":true,"backup_failure_preserves_draft_file_effective":true,"explicit_retry":true,"invalid_and_synthetic_preedit_not_saved":true,"close_and_reopen":true,"restored_initial_settings":true,"os_ime_picker_keyboard_focus":"not_ready"})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        Ok(())
    }
}
fn capture(window: &crate::SettingsWindow, dir: &Path, name: &str) -> Result<(), String> {
    let pixels = window.window().take_snapshot().map_err(|e| e.to_string())?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(name))
        .map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(file, pixels.width(), pixels.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder
        .write_header()
        .map_err(|e| e.to_string())?
        .write_image_data(pixels.as_bytes())
        .map_err(|e| e.to_string())
}
