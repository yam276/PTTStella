//! Full native component station transitions. Toolkit intents are explicit;
//! native bytes and handshake order are independently checked by site_probe.
use super::*;
use crate::verification::{Verification, foreground, require, wait};
use pttstella::connection::{
    Phase,
    site_probe::{Case, Peers},
};
use std::{error::Error, path::PathBuf};
async fn ready(ui: &TerminalWindow, owner: &Manager, target: &Target) -> Result<(), String> {
    wait(
        || {
            foreground(ui, true);
            let view = owner.connection.updates.borrow();
            view.site == *target
                && view.phase == Phase::Connected
                && !view.output_pending
                && !view.input_pending
                && !ui.get_switching_station()
                && !ui.get_frame_paint_pending()
                && view.frame.glyphs.iter().any(|g| {
                    g.row == 1
                        && g.col == 0
                        && g.text == "中"
                        && g.width == 2
                        && g.left.fg == 1
                        && g.right.fg == 2
                })
        },
        "new station/frame/encoding/readiness did not settle",
    )
    .await
}
fn go(ui: &TerminalWindow, owner: &Rc<Manager>, target: &Target) -> Result<(), String> {
    foreground(ui, true);
    ui.invoke_sites_show();
    let window = owner
        .window
        .borrow()
        .as_ref()
        .ok_or("switch station window absent")?
        .as_weak()
        .upgrade()
        .ok_or("switch station window dropped")?;
    window
        .global::<crate::SitesState>()
        .set_address(target.endpoint.as_str().into());
    window
        .global::<crate::SitesState>()
        .set_encoding(index(target.encoding));
    window.global::<crate::SitesState>().invoke_action(3);
    require(
        !ui.get_sites_open(),
        "valid connection intent was not enqueued/closed",
    )
}
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<Manager>,
    peers: Rc<Peers>,
    directory: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(ui, directory, "site-switch", move |ui, dir| async move {
        require(
            peers.case != Case::Native,
            "native switch must use passive UI mode",
        )?;
        ready(&ui, &owner, &peers.first).await?;
        let first = owner.connection.updates.borrow().generation;
        let mut sequence =
            vec![serde_json::json!({"stage":"first","generation":first,"site":peers.first})];
        if peers.case == Case::Disconnected {
            ui.invoke_tools_toggle();
            ui.invoke_tools_category_choose(3);
            ui.invoke_tools_action_choose(1);
            wait(
                || owner.connection.updates.borrow().phase == Phase::Disconnected,
                "tool Disconnect did not complete",
            )
            .await?;
            sequence.push(serde_json::json!({"stage":"disconnected","generation":owner.connection.updates.borrow().generation}));
        }
        if peers.case == Case::CancelDial {
            go(&ui, &owner, &peers.hanging)?;
            wait(
                || {
                    peers
                        .audit()
                        .is_ok_and(|a| a.stage == "hanging_request_held")
                        && !ui.get_switching_station()
                },
                "pending dial did not reach held upgrade",
            )
            .await?;
            require(
                owner.connection.updates.borrow().phase == Phase::Connecting,
                "held dial unexpectedly completed",
            )?;
            sequence.push(serde_json::json!({"stage":"held_dial","generation":owner.connection.updates.borrow().generation}));
        }
        go(&ui, &owner, &peers.gbk)?;
        owner.connection.input(first, b"STALE-FIRST".to_vec())?;
        ready(&ui, &owner, &peers.gbk).await?;
        let gbk = owner.connection.updates.borrow().generation;
        require(
            gbk >= first + 2
                && !owner.connection.updates.borrow().site.ptt_features()
                && !owner.connection.updates.borrow().keep_alive,
            "GBK target kept old generation/PTT automation",
        )?;
        sequence.push(serde_json::json!({"stage":"gbk","generation":gbk,"site":peers.gbk}));
        go(&ui, &owner, &peers.uao)?;
        owner.connection.input(gbk, b"STALE-SECOND".to_vec())?;
        ready(&ui, &owner, &peers.uao).await?;
        let last = owner.connection.updates.borrow().generation;
        require(last >= gbk + 2, "UAO return did not advance generation")?;
        let state = peers.audit()?;
        require(
            state.closed == ["first", "gbk"]
                && state.inputs_hex == ["", "", ""]
                && (peers.case != Case::CancelDial || state.cancelled_dial_eof),
            "old Close/stale input/dial cancellation audit differs",
        )?;
        sequence.push(serde_json::json!({"stage":"uao","generation":last,"site":peers.uao}));
        crate::verification::capture(&ui, &dir, "site-switch.png")?;
        std::fs::write(dir.join("site-switch-controller.json"),serde_json::to_vec_pretty(&serde_json::json!({"status":"passed_before_final_close","case":format!("{:?}",peers.case),"sequence":sequence,"scope":"full widgets/controller/connection with toolkit intents and intentional stale-generation writes; raw native keys/paste not claimed. Peer finish after Connection shutdown independently proves final Close."})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())
    })
}
