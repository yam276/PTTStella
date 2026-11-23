//! The OS source remains live while System Settings owns foreground focus.
use crate::{
    TerminalWindow,
    system_motion::Owner,
    verification::{self, Verification, require, wait},
};
use pttstella::{connection::Connection, motion_preferences::Preference};
use std::{
    error::Error,
    path::{Path, PathBuf},
    rc::Rc,
    time::Duration,
};

fn record(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(file, value).map_err(|e| e.to_string())
}
async fn notice(ui: &TerminalWindow, owner: &Owner, dir: &Path, name: &str) -> Result<(), String> {
    let reduced = owner.preference().reduced();
    require(
        !ui.get_notice_hovered() && !ui.get_notice_focused(),
        "motion notice is paused by pointer or control focus",
    )?;
    crate::notice::copied(ui, "OS motion preference / 系統動畫偏好");
    tokio::time::sleep(Duration::from_millis(1700)).await;
    require(
        !ui.get_input_notice().is_empty() && !ui.get_notice_fading(),
        "OS motion setting removed readable time",
    )?;
    if reduced {
        wait(
            || ui.get_input_notice().is_empty(),
            "OS reduced-motion notice did not expire",
        )
        .await?;
        require(!ui.get_notice_fading(), "OS reduced-motion notice animated")?;
    } else {
        wait(
            || ui.get_notice_fading(),
            "OS no-preference notice did not fade",
        )
        .await?;
        tokio::time::sleep(Duration::from_millis(150)).await;
        require(
            !ui.get_input_notice().is_empty(),
            "OS normal fade ended prematurely",
        )?;
        verification::capture(ui, dir, name)?;
        wait(
            || ui.get_input_notice().is_empty(),
            "OS normal fade did not finish",
        )
        .await?;
    }
    require(
        owner.preference().reduced() == reduced,
        "OS preference changed during its notification check",
    )
}
async fn setting(owner: &Owner, expected: bool) -> Result<(), String> {
    tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            if matches!(owner.preference(),Preference::Known{reduced,..} if reduced==expected) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .map_err(|_| "waiting for an actual OS motion preference change timed out".into())
}
pub fn start(
    ui: &TerminalWindow,
    owner: Rc<Owner>,
    connection: Rc<Connection>,
    dir: PathBuf,
) -> Result<Verification, Box<dyn Error>> {
    Verification::start(ui, dir, "system-motion", move |ui, dir| async move {
        wait(
            || dir.join("prepared.json").is_file(),
            "motion shell was not prepared",
        )
        .await?;
        wait(
            || !matches!(owner.preference(), Preference::Pending),
            "motion source did not answer",
        )
        .await?;
        let initial = owner.preference();
        if !matches!(&initial,Preference::Known{source,..} if *source!="controlled_synthetic_probe")
        {
            record(
                &dir.join("motion-not-ready.json"),
                &serde_json::json!({"status":"not_ready","reading":initial}),
            )?;
            return Err("actual OS motion preference is unavailable".into());
        }
        require(
            ui.get_tools_reduced_motion() == initial.reduced(),
            "OS reading did not reach the Slint property",
        )?;
        #[cfg(target_os = "macos")]
        {
            let before = owner.deliveries();
            owner.verify_notification()?;
            wait(
                || owner.deliveries() > before,
                "workspace notification did not reach the live UI listener",
            )
            .await?;
        }
        let changes = std::env::var_os("PTTSTELLA_SLINT_MOTION_EXPECT_CHANGE").is_some();
        record(
            &dir.join("motion-ready.json"),
            &serde_json::json!({"status":"ready","initial":initial,"expect_os_change_and_restore":changes,"delivered":owner.deliveries()}),
        )?;
        if changes {
            setting(&owner, !initial.reduced()).await?;
            require(
                ui.get_tools_reduced_motion() == !initial.reduced(),
                "changed OS setting did not reach the root",
            )?;
            notice(&ui, &owner, &dir, "motion-changed-fade.png").await?;
            record(
                &dir.join("motion-change-observed.json"),
                &serde_json::json!({"status":"awaiting_restore","reading":owner.preference(),"notice_policy_verified":true}),
            )?;
            setting(&owner, initial.reduced()).await?;
            require(
                ui.get_tools_reduced_motion() == initial.reduced(),
                "restored OS setting did not reach the root",
            )?;
        }
        notice(&ui, &owner, &dir, "motion-initial-fade.png").await?;
        require(
            connection.updates.borrow().input_epoch == 0,
            "motion preference source sent BBS input",
        )?;
        record(
            &dir.join("motion-result.json"),
            &serde_json::json!({"status":"passed_pending_exit_audit","initial":initial,"final":owner.preference(),
            "actual_os_change_and_restore":changes,"notice_policy_verified":true,"zero_bbs_input":true,
            "scope":if changes {"actual OS motion read/change/restore and production notification, no synthetic foreground"}else{"actual OS initial read and notification; macOS process-local workspace callback, no actual OS value change"}}),
        )
    })
}
