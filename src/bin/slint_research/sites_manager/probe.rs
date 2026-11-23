//! Real site widgets/shared transaction, explicit synthetic loopback namespace.
use super::*;
use crate::verification::{Verification, require, wait};
use slint::{Model, winit_030::WinitWindowAccessor};
use std::{
    error::Error,
    path::{Path, PathBuf},
};
async fn idle(owner: &Manager) -> Result<(), String> {
    wait(
        || {
            owner.result.borrow().is_none()
                && !owner.output.busy()
                && owner
                    .window
                    .borrow()
                    .as_ref()
                    .is_none_or(|w| !w.global::<crate::SitesState>().get_busy())
        },
        "station file operation did not settle",
    )
    .await
}
fn fill(w: &TerminalWindow, name: &str, address: &str, encoding: i32) {
    w.global::<crate::SitesState>().set_name(name.into());
    w.global::<crate::SitesState>().set_address(address.into());
    w.global::<crate::SitesState>().set_encoding(encoding);
}
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<Manager>,
    directory: PathBuf,
    path: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(ui, directory, "stations", move |ui, dir| async move {
        wait(
            || owner.connection.updates.borrow().phase == pttstella::connection::Phase::Connected,
            "site probe never connected",
        )
        .await?;
        crate::verification::foreground(&ui, true);
        let initial = (
            owner.connection.updates.borrow().generation,
            owner.connection.updates.borrow().site.clone(),
        );
        ui.invoke_sites_show();
        let w = owner
            .window
            .borrow()
            .as_ref()
            .ok_or("native station window absent")?
            .as_weak()
            .upgrade()
            .ok_or("site window dropped")?;
        wait(
            || w.window().with_winit_window(|w| w.id()).is_some(),
            "station native window not realized",
        )
        .await?;
        let id = w.window().with_winit_window(|w| w.id());
        require(
            ui.get_sites_open() && !ui.get_keep_alive_surface_ready() && !ui.get_vim_available(),
            "stations did not suspend terminal/background input",
        )?;
        fill(&w, "unsaved window", "wss://unsaved.example/bbs", 1);
        ui.invoke_sites_show();
        require(
            w.window().with_winit_window(|w| w.id()) == id
                && w.global::<crate::SitesState>().get_name() == "unsaved window"
                && w.global::<crate::SitesState>().get_encoding() == 1,
            "opening stations twice replaced its window/draft",
        )?;
        let original = std::fs::read(&path).map_err(|e| e.to_string())?;
        fill(&w, "Rejected", "telnet://legacy.example:2323", 2);
        w.global::<crate::SitesState>().invoke_action(0);
        require(
            owner.result.borrow().is_none()
                && std::fs::read(&path).map_err(|e| e.to_string())? == original,
            "new plaintext favourite was written",
        )?;
        w.global::<crate::SitesState>().invoke_action(3);
        require(
            owner.connection.updates.borrow().generation == initial.0,
            "rejected nonlocal probe address changed connection",
        )?;
        w.global::<crate::SitesState>().invoke_preset(true);
        require(
            w.global::<crate::SitesState>().get_address() == pttstella::sites::PTT_SSH
                && w.global::<crate::SitesState>().get_encoding() == 0,
            "SSH preset is not PTT/UAO",
        )?;
        w.global::<crate::SitesState>().set_encoding(2);
        w.global::<crate::SitesState>().invoke_action(2);
        require(
            owner.result.borrow().is_none()
                && std::fs::read(&path).map_err(|e| e.to_string())? == original,
            "SSH accepted non-UAO default",
        )?;
        fill(&w, "原生測試", "wss://native.example/bbs", 2);
        w.global::<crate::SitesState>().invoke_action(0);
        idle(&owner).await?;
        let store = pttstella::sites::store::Store::at(path.clone());
        let saved = store.load()?;
        let target = Target::new("wss://native.example/bbs".into(), TextEncoding::Gbk)?;
        require(
            saved
                .favourites
                .iter()
                .any(|f| f.target == target && f.name == "原生測試")
                && owner.state.borrow().sites() == &saved,
            "favourite file/effective state differs",
        )?;
        require(
            std::fs::read(path.with_extension("json.bak")).map_err(|e| e.to_string())? == original,
            "first favourite did not preserve exact original backup",
        )?;
        let mut other = saved.clone();
        let external = Target::new("wss://external.example/bbs".into(), TextEncoding::Big5)?;
        other.save_favourite("External".into(), external.clone())?;
        store.save_from(&saved, other)?;
        w.global::<crate::SitesState>().set_name("改名".into());
        w.global::<crate::SitesState>().invoke_action(0);
        idle(&owner).await?;
        require(
            store
                .load()?
                .favourites
                .iter()
                .any(|f| f.target == external),
            "station edit lost disjoint external favourite",
        )?;
        let before_failure = std::fs::read(&path).map_err(|e| e.to_string())?;
        let before_model = owner.state.borrow().sites().clone();
        let backup = path.with_extension("json.bak");
        let held = dir.join("held-sites-backup.json");
        std::fs::rename(&backup, &held).map_err(|e| e.to_string())?;
        std::fs::create_dir(&backup).map_err(|e| e.to_string())?;
        w.global::<crate::SitesState>()
            .set_name("保存失敗後重試".into());
        w.global::<crate::SitesState>().invoke_action(0);
        idle(&owner).await?;
        require(
            std::fs::read(&path).map_err(|e| e.to_string())? == before_failure
                && owner.state.borrow().sites() == &before_model
                && w.global::<crate::SitesState>().get_name() == "保存失敗後重試"
                && !w.global::<crate::SitesState>().get_message().is_empty(),
            "failed save changed file/model or lost draft/error",
        )?;
        crate::verification::capture(&ui, &dir, "station-root-suspended.png")?;
        std::fs::remove_dir(&backup).map_err(|e| e.to_string())?;
        std::fs::rename(&held, &backup).map_err(|e| e.to_string())?;
        w.global::<crate::SitesState>().invoke_action(0);
        idle(&owner).await?;
        require(
            store
                .load()?
                .favourites
                .iter()
                .any(|f| f.target == target && f.name == "保存失敗後重試"),
            "retry did not save retained form",
        )?;
        w.global::<crate::SitesState>().invoke_action(2);
        idle(&owner).await?;
        require(
            store.load()?.default == target,
            "explicit default did not persist",
        )?;
        require(
            owner.connection.updates.borrow().generation == initial.0
                && owner.connection.updates.borrow().site == initial.1,
            "saving favourite/default dialled or changed encoding",
        )?;
        let index = owner
            .state
            .borrow()
            .sites()
            .favourites
            .iter()
            .position(|f| f.target == target)
            .ok_or("saved favourite absent")? as i32
            + 1;
        w.global::<crate::SitesState>().set_choice(index);
        w.global::<crate::SitesState>().invoke_choose(index);
        w.global::<crate::SitesState>()
            .set_address("invalid".into());
        w.global::<crate::SitesState>().invoke_action(1);
        idle(&owner).await?;
        require(
            store.load()?.default == target
                && !store.load()?.favourites.iter().any(|f| f.target == target),
            "removal used edited address or reset separate default",
        )?;
        let legacy = owner
            .state
            .borrow()
            .sites()
            .favourites
            .iter()
            .position(|f| f.name == "Legacy synthetic")
            .ok_or("legacy fixture absent")? as i32
            + 1;
        w.global::<crate::SitesState>().set_choice(legacy);
        w.global::<crate::SitesState>().invoke_choose(legacy);
        w.global::<crate::SitesState>().invoke_action(1);
        idle(&owner).await?;
        require(
            !store
                .load()?
                .favourites
                .iter()
                .any(|f| f.name == "Legacy synthetic"),
            "legacy transport could not be removed",
        )?;
        let valid = std::fs::read(&path).map_err(|e| e.to_string())?;
        std::fs::write(&path, b"synthetic-corrupt-sites").map_err(|e| e.to_string())?;
        w.global::<crate::SitesState>().invoke_action(4);
        idle(&owner).await?;
        require(
            !w.global::<crate::SitesState>().get_writable()
                && owner.state.borrow().warning().is_some(),
            "corrupt site file not visibly read-only",
        )?;
        w.global::<crate::SitesState>().invoke_action(0);
        require(
            std::fs::read(&path).map_err(|e| e.to_string())? == b"synthetic-corrupt-sites",
            "corrupt source overwritten",
        )?;
        std::fs::write(&path, &valid).map_err(|e| e.to_string())?;
        w.global::<crate::SitesState>().invoke_action(4);
        idle(&owner).await?;
        require(
            w.global::<crate::SitesState>().get_writable(),
            "explicit reload did not recover writable model",
        )?;
        fill(&w, "Close drains write", "wss://close.example/bbs", 1);
        w.global::<crate::SitesState>().invoke_action(0);
        require(
            owner.result.borrow().is_some(),
            "close case did not start worker",
        )?;
        w.global::<crate::SitesState>().invoke_close_sites();
        wait(
            || !ui.get_sites_open() && owner.result.borrow().is_none() && !owner.output.busy(),
            "closing station editor did not drain write",
        )
        .await?;
        require(
            store
                .load()?
                .favourites
                .iter()
                .any(|f| f.name == "Close drains write"),
            "close lost started station save",
        )?;
        ui.invoke_sites_show();
        wait(|| ui.get_sites_open(), "stations did not reopen").await?;
        wait(
            || w.window().with_winit_window(|w| w.id()).is_some(),
            "reopened station window not realized",
        )
        .await?;
        require(
            w.global::<crate::SitesState>().get_favourites().row_count()
                == store.load()?.favourites.len() + 1,
            "reopen lost merged favourite model",
        )?;
        w.global::<crate::SitesState>().invoke_close_sites();
        wait(|| !ui.get_sites_open(), "station close failed").await?;
        std::fs::write(dir.join("sites-saved.json"), store.load()?.encode()?)
            .map_err(|e| e.to_string())?;
        std::fs::write(dir.join("sites-result.json"),br#"{"status":"passed","scope":"native widgets/production model/real file worker with synthetic intents; no OS IME or valid transport switch claim","save_default_no_dial":true,"plaintext_ssh_encoding_rejected":true,"backup_exact":true,"failure_retry":true,"external_merge":true,"legacy_remove":true,"remove_keeps_default":true,"corrupt_read_only_reload":true,"close_drains_started_save":true,"single_window_while_open":true}"#).map_err(|e|e.to_string())
    })
}
pub fn audit_closed(owner: &Manager, directory: &Path) -> Result<(), String> {
    require(
        owner.stopped.get() && owner.result.borrow().is_none() && owner.window.borrow().is_none(),
        "station owner retained result/window after finish",
    )?;
    let stored = owner.state.borrow().store()?.load()?;
    require(
        owner.state.borrow().sites() == &stored,
        "station finish file/effective model differs",
    )?;
    std::fs::write(directory.join("sites-closed-saved.json"), stored.encode()?)
        .map_err(|e| e.to_string())?;
    std::fs::write(directory.join("sites-closed.json"),br#"{"status":"passed","stopped":true,"pending_result":false,"window_retained":false,"file_effective_equal":true}"#).map_err(|e|e.to_string())
}
