use super::super::surface_state::{Identity, SurfaceState};
use super::*;
use pttstella::settings::{AppSettings, DisplayProfile, HoverPosition, store::Store};
use pttstella::{
    connection::{Phase, View},
    filters,
    screen::Screen,
    selection::Selection,
};
use std::time::Duration;
use tokio::sync::oneshot;

fn snapshot() -> (
    SurfaceState,
    tokio::sync::watch::Sender<View>,
    tokio::sync::watch::Receiver<View>,
    Identity,
) {
    let view = View {
        frame: Screen::new(80, 24).unwrap().frame(),
        generation: 7,
        input_epoch: 0,
        input_pending: false,
        login_pending: false,
        phase: Phase::Connected,
        message: String::new(),
        received: 1,
        keep_alive: false,
        keep_alive_ready: false,
        keep_alive_sent: 0,
        keep_alive_failure: None,
    };
    let (sender, receiver) = tokio::sync::watch::channel(view);
    let mut state = SurfaceState::new(filters::Rules::default());
    let packet = state
        .queue(
            &receiver.borrow(),
            false,
            &pttstella::settings::DisplaySettings::default(),
        )
        .unwrap();
    let identity = Identity {
        presentation: packet.presentation,
        generation: packet.generation,
        revision: packet.frame.revision,
    };
    state.acknowledge(identity).unwrap();
    (state, sender, receiver, identity)
}

#[tokio::test]
async fn slow_disk_keeps_ui_pollable_and_cleanup_retains_the_started_work() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("retained.txt");
    let output = path.clone();
    let (started, start) = oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let mut files = Files::default();
    files
        .start(|permit| async move {
            Completion::Probe(
                blocking(permit, move || {
                    started.send(()).unwrap();
                    gate.recv_timeout(Duration::from_secs(3)).unwrap();
                    std::fs::write(output, b"completed").unwrap();
                    Ok(serde_json::Value::Null)
                })
                .await,
            )
        })
        .unwrap();
    tokio::select! {
        _=files.next()=>panic!("disk finished before release"),
        result=start=>result.unwrap(),
    }
    tokio::select! {
        _=files.next()=>panic!("disk work unexpectedly completed"),
        _=tokio::time::sleep(Duration::from_millis(5))=>{},
    }
    assert!(!path.exists());
    let mut finish = Box::pin(files.finish());
    tokio::select! {
        _=&mut finish=>panic!("cleanup dropped the disk owner"),
        _=tokio::time::sleep(Duration::from_millis(5))=>{},
    }
    release.send(()).unwrap();
    finish.await.unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"completed");
    assert!(!files.busy());
}

#[tokio::test]
async fn busy_admission_never_starts_another_operation_and_unstarted_work_is_revoked() {
    let mut files = Files::default();
    files
        .start(|permit| save_hover(permit, Err("no store".into()), AppSettings::default()))
        .unwrap();
    let called = std::cell::Cell::new(false);
    assert!(
        files
            .start(|_| {
                called.set(true);
                async { Completion::Profile(Ok(ProfileResult::Cancelled)) }
            })
            .is_err()
    );
    assert!(!called.get());
    files.revoke();
    assert!(
        matches!(files.next().await,Completion::Hover(Err(message)) if message==Error::Cancelled)
    );
    assert!(
        files
            .start(|_| async { panic!("closed owner restarted") })
            .is_err()
    );
}

#[tokio::test]
async fn closing_during_a_picker_retains_the_result_but_does_not_start_a_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cancelled.txt");
    let output = path.clone();
    let (choose, chosen) = oneshot::channel();
    let (started, opening) = oneshot::channel();
    let mut files = Files::default();
    files
        .start(|permit| async move {
            started.send(()).unwrap();
            chosen.await.unwrap();
            Completion::Probe(
                blocking(permit, move || {
                    std::fs::write(output, b"unexpected").unwrap();
                    Ok(serde_json::Value::Null)
                })
                .await,
            )
        })
        .unwrap();
    tokio::select! {_=files.next()=>panic!("picker finished"),_=opening=>{}}
    let mut finish = Box::pin(files.finish());
    tokio::select! {_=&mut finish=>panic!("picker owner lost"),_=tokio::time::sleep(Duration::from_millis(5))=>{}}
    assert!(!choose.is_closed());
    choose.send(()).unwrap();
    finish.await.unwrap();
    assert!(!path.exists());
    assert!(!files.busy());
}

#[tokio::test]
async fn failed_save_preserves_effective_state_and_the_next_save_can_commit() {
    let dir = tempfile::tempdir().unwrap();
    let mut preferences = super::super::preferences::Preferences::load(Some(dir.path())).unwrap();
    let original = preferences.app().clone();
    let store = Store::at(dir.path().join("probe-settings.json"));
    store.save(original.clone()).unwrap();
    let old = std::fs::read(dir.path().join("probe-settings.json")).unwrap();
    std::fs::create_dir(dir.path().join("probe-settings.json.bak")).unwrap();
    let draft = preferences.display_draft(DisplayProfile {
        font_size: 18.0,
        ..Default::default()
    });
    let mut files = Files::default();
    files
        .start(|permit| save_display(permit, preferences.store(), draft.clone()))
        .unwrap();
    assert!(matches!(files.next().await, Completion::Display(Err(_))));
    assert_eq!(preferences.app(), &original);
    assert_eq!(
        std::fs::read(dir.path().join("probe-settings.json")).unwrap(),
        old
    );
    std::fs::remove_dir(dir.path().join("probe-settings.json.bak")).unwrap();
    files
        .start(|permit| save_display(permit, preferences.store(), draft))
        .unwrap();
    let Completion::Display(Ok(saved)) = files.next().await else {
        panic!("save failed")
    };
    assert_eq!(preferences.app(), &original); // Delivery alone never mutates UI state.
    preferences.commit(saved).unwrap();
    assert_eq!(preferences.display().font_size, 18.0);
    let draft = preferences.hover_draft(HoverPosition::Off);
    files
        .start(|permit| save_hover(permit, preferences.store(), draft))
        .unwrap();
    let Completion::Hover(Ok(saved)) = files.next().await else {
        panic!("hover save failed")
    };
    preferences.commit(saved).unwrap();
    assert_eq!(store.load().unwrap().display.font_size, 18.0);
    assert_eq!(preferences.hover(), HoverPosition::Off);
}

#[tokio::test]
async fn revoked_profile_and_invalid_png_never_write_and_release_the_file_slot() {
    let dir = tempfile::tempdir().unwrap();
    let mut files = Files::default();
    let (mut state, _sender, source, identity) = snapshot();
    files
        .start(|permit| {
            export_png(
                permit,
                ExportRequest {
                    lease: state.lease_export(identity, None, 0),
                    source,
                    png: "data:image/png;base64,AAAA".into(),
                    rows: 0..24,
                    selected: false,
                    probe: Some(dir.path().into()),
                },
            )
        })
        .unwrap();
    assert!(matches!(
        files.next().await,
        Completion::Export { result: Err(_), .. }
    ));
    assert!(!dir.path().join("export.png").exists());
    files
        .start(|permit| {
            profile(
                permit,
                ProfileRequest::Export {
                    name: "cancelled".into(),
                    display: DisplayProfile::default(),
                },
                Some(dir.path().into()),
            )
        })
        .unwrap();
    files.finish().await.unwrap();
    assert!(!dir.path().join("display.pttstella.json").exists());
    assert!(!files.busy());
}

#[tokio::test]
async fn deferred_picker_cannot_save_an_old_policy_source_or_selection_but_a_new_export_succeeds() {
    for change in [
        "rules",
        "reveal",
        "frame",
        "session",
        "selection",
        "clear",
        "input",
        "display",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let old_path = dir.path().join("old.png");
        let target = old_path.clone();
        let (mut state, sender, source, identity) = snapshot();
        state
            .select(identity, 11, &Selection::All, &source.borrow())
            .unwrap();
        let rows = state
            .export_rows(identity, Some(11), &source.borrow())
            .unwrap();
        let png = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD
                .encode(include_bytes!("../../tests/fixtures/export-black.png"))
        );
        let (opening, opened) = oneshot::channel();
        let (choose, chosen) = oneshot::channel();
        let mut files = Files::default();
        let request = ExportRequest {
            png: png.clone(),
            rows,
            selected: true,
            probe: None,
            lease: state.lease_export(identity, Some(11), 0),
            source: source.clone(),
        };
        files
            .start(|permit| {
                export_with_picker(permit, request, |_, _, _| async move {
                    opening.send(()).unwrap();
                    chosen.await.unwrap();
                    Some(target)
                })
            })
            .unwrap();
        tokio::select! {_=files.next()=>panic!("picker completed before release"),_=opened=>{}}
        match change {
            "rules" => {
                state.replace_rules(filters::Rules::default().edit("Alice", "", false).unwrap())
            }
            "reveal" => {
                assert!(state.reveal(identity, true, &source.borrow()));
            }
            "frame" => {
                sender.send_modify(|view| view.frame.revision += 1);
            } // Owner has not consumed this watch update yet.
            "session" => {
                sender.send_modify(|view| view.generation += 1);
            }
            "selection" => {
                state
                    .select(identity, 12, &Selection::Line { row: 1 }, &source.borrow())
                    .unwrap();
            }
            "clear" => {
                let event: super::super::protocol::SurfaceEvent = serde_json::from_str(
                    include_str!("../../tests/fixtures/selection-cleared.json"),
                )
                .unwrap();
                let super::super::protocol::SurfaceEvent::SelectionCleared {
                    presentation,
                    generation,
                    revision,
                    request,
                } = event
                else {
                    panic!("clear event")
                };
                state.clear_selection(
                    Identity {
                        presentation,
                        generation,
                        revision,
                    },
                    request,
                );
            }
            "input" => {
                sender.send_modify(|view| view.input_epoch += 1);
            }
            "display" => state.display_changed(),
            _ => unreachable!(),
        }
        choose.send(()).unwrap();
        assert!(
            matches!(
                files.next().await,
                Completion::Export {
                    result: Err(Error::Stale),
                    ..
                }
            ),
            "{change}"
        );
        assert!(!old_path.exists(), "{change}");
        state.update(&source.borrow());
        let packet = state
            .queue(
                &source.borrow(),
                false,
                &pttstella::settings::DisplaySettings::default(),
            )
            .unwrap();
        let fresh = Identity {
            presentation: packet.presentation,
            generation: packet.generation,
            revision: packet.frame.revision,
        };
        state.acknowledge(fresh).unwrap();
        let fresh_rows = state.export_rows(fresh, None, &source.borrow()).unwrap();
        let fresh_epoch = source.borrow().input_epoch;
        let request = ExportRequest {
            png,
            rows: fresh_rows,
            selected: false,
            probe: Some(dir.path().into()),
            lease: state.lease_export(fresh, None, fresh_epoch),
            source,
        };
        files.start(|permit| export_png(permit, request)).unwrap();
        assert!(
            matches!(
                files.next().await,
                Completion::Export {
                    result: Ok(true),
                    ..
                }
            ),
            "{change}"
        );
        assert_eq!(
            std::fs::read(dir.path().join("export.png")).unwrap(),
            include_bytes!("../../tests/fixtures/export-black.png")
        );
    }
}

#[tokio::test]
async fn cleanup_reports_a_started_file_failure_instead_of_claiming_success() {
    let (started, opening) = oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let mut files = Files::default();
    files
        .start(|permit| async move {
            Completion::Probe(
                blocking(permit, move || {
                    started.send(()).unwrap();
                    gate.recv_timeout(Duration::from_secs(3)).unwrap();
                    Err("synthetic disk failure".into())
                })
                .await,
            )
        })
        .unwrap();
    tokio::select! {_=files.next()=>panic!("file completed"),_=opening=>{}}
    files.revoke();
    release.send(()).unwrap();
    assert_eq!(
        files.finish().await,
        Err("關閉前的檔案操作失敗：synthetic disk failure".into())
    );
    assert!(!files.busy());
}
