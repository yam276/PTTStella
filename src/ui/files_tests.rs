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

#[tokio::test]
async fn bookmark_files_keep_request_identity_commit_only_on_delivery_and_revoke_unstarted_saves() {
    use super::super::bookmarks::State;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("probe-bookmarks.json");
    let mut state = State::load(Some(dir.path()));
    let write = state.edit("#00000101@Stella", "自製", false).unwrap();
    let mut files = Files::default();
    files
        .start(|permit| save_bookmarks(permit, 17, write))
        .unwrap();
    files.finish().await.unwrap();
    assert!(!path.exists());
    let mut files = Files::default();
    let write = state.edit("#00000101@Stella", "自製", false).unwrap();
    files
        .start(|permit| save_bookmarks(permit, 18, write))
        .unwrap();
    let Completion::Bookmarks {
        request,
        result: Ok(saved),
    } = files.next().await
    else {
        panic!("bookmark save failed")
    };
    assert_eq!(request, 18);
    assert!(state.snapshot().entries.is_empty());
    state.complete(Ok(saved));
    assert_eq!(state.snapshot().entries.len(), 1);
    let old = std::fs::read(&path).unwrap();
    std::fs::create_dir(path.with_extension("json.bak")).unwrap();
    let write = state.edit("#00000101@Stella", "", true).unwrap();
    files
        .start(|permit| save_bookmarks(permit, 19, write))
        .unwrap();
    let Completion::Bookmarks {
        request,
        result: Err(error),
    } = files.next().await
    else {
        panic!("backup failure was swallowed")
    };
    assert_eq!(request, 19);
    state.complete(Err(error.to_string()));
    assert_eq!(state.snapshot().entries.len(), 1);
    assert_eq!(std::fs::read(path).unwrap(), old);
    files.finish().await.unwrap();
    assert!(!files.busy());
}

fn snapshot() -> (
    SurfaceState,
    tokio::sync::watch::Sender<View>,
    tokio::sync::watch::Receiver<View>,
    Identity,
) {
    let view = View {
        site: pttstella::sites::Target::default(),
        mouse: pttstella::mouse::input::Snapshot::default(),
        output_pending: false,
        frame: Screen::new(80, 24).unwrap().frame(),
        generation: 7,
        input_epoch: 0,
        input_pending: false,
        login_pending: false,
        login_guidance: pttstella::login::Guidance::Idle,
        destination: pttstella::navigation_job::Progress::Idle,
        board_visit: None,
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
        .start(|permit| {
            save_hover(
                permit,
                Err("no store".into()),
                AppSettings::default(),
                AppSettings::default(),
            )
        })
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
    let mut draft = preferences.app().clone();
    draft.display = DisplayProfile {
        font_size: 18.0,
        ..Default::default()
    };
    draft.language = pttstella::i18n::Language::English;
    let mut files = Files::default();
    files
        .start(|permit| {
            save_display(
                permit,
                preferences.store(),
                preferences.app().clone(),
                draft.clone(),
            )
        })
        .unwrap();
    assert!(matches!(
        files.next().await,
        Completion::Display { result: Err(_), .. }
    ));
    assert_eq!(preferences.app(), &original);
    assert_eq!(
        std::fs::read(dir.path().join("probe-settings.json")).unwrap(),
        old
    );
    std::fs::remove_dir(dir.path().join("probe-settings.json.bak")).unwrap();
    files
        .start(|permit| {
            save_display(
                permit,
                preferences.store(),
                preferences.app().clone(),
                draft,
            )
        })
        .unwrap();
    let Completion::Display {
        result: Ok(saved), ..
    } = files.next().await
    else {
        panic!("save failed")
    };
    assert_eq!(preferences.app(), &original); // Delivery alone never mutates UI state.
    preferences.commit(saved).unwrap();
    assert_eq!(preferences.display().font_size, 18.0);
    let draft = preferences.hover_draft(HoverPosition::Off);
    files
        .start(|permit| {
            save_hover(
                permit,
                preferences.store(),
                preferences.app().clone(),
                draft,
            )
        })
        .unwrap();
    let Completion::Hover(Ok(saved)) = files.next().await else {
        panic!("hover save failed")
    };
    preferences.commit(saved).unwrap();
    assert_eq!(store.load().unwrap().display.font_size, 18.0);
    assert_eq!(preferences.hover(), HoverPosition::Off);
    assert_eq!(
        store.load().unwrap().language,
        pttstella::i18n::Language::English
    );
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
                    lease: state.lease_output(identity, None, 0),
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
            lease: state.lease_output(identity, Some(11), 0),
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
            lease: state.lease_output(fresh, None, fresh_epoch),
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

#[tokio::test]
async fn recent_writes_run_alongside_a_picker_and_close_drains_the_latest_snapshot() {
    use super::super::files_recent;
    use pttstella::{
        board_navigation::BoardName,
        recent_boards::{RecentBoards, store::Store as RecentStore},
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recent.json");
    let store = RecentStore::at(path.clone());
    let mut files = Files::default();
    let (started, start) = oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel();
    files
        .start(|permit| async move {
            Completion::Probe(
                blocking(permit, move || {
                    started.send(()).unwrap();
                    gate.recv_timeout(Duration::from_secs(3)).unwrap();
                    Ok(serde_json::Value::Null)
                })
                .await,
            )
        })
        .unwrap();
    tokio::select! { _=files.next()=>panic!("picker returned"), _=start=>{} }
    for (revision, name, clear) in [
        (1, "First", false),
        (2, "Middle", true),
        (3, "Latest", false),
    ] {
        let mut history = RecentBoards::default();
        history.visit(BoardName::parse(name).unwrap());
        files
            .save_recent(files_recent::Write {
                revision,
                store: store.clone(),
                history,
                clear,
            })
            .unwrap();
    }
    let mut finish = Box::pin(files.finish());
    tokio::select! { _=&mut finish=>panic!("picker was dropped on close"), _=tokio::time::sleep(Duration::from_millis(10))=>{} }
    release.send(()).unwrap();
    finish.await.unwrap();
    assert!(!files.busy());
    assert_eq!(store.load().unwrap().boards()[0].as_str(), "Latest");
    assert!(
        RecentStore::at(path.with_extension("json.bak"))
            .load()
            .unwrap()
            .boards()
            .is_empty()
    );
    assert!(
        files
            .save_recent(files_recent::Write {
                revision: 4,
                store,
                history: RecentBoards::default(),
                clear: true
            })
            .is_err()
    );
}

#[tokio::test]
async fn recent_failure_during_close_is_reported_and_joined() {
    use super::super::files_recent;
    use pttstella::recent_boards::{RecentBoards, store::Store as RecentStore};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recent.json");
    std::fs::create_dir(path.with_extension("json.bak")).unwrap();
    let mut files = Files::default();
    files
        .save_recent(files_recent::Write {
            revision: 1,
            store: RecentStore::at(path),
            history: RecentBoards::default(),
            clear: true,
        })
        .unwrap();
    assert!(files.finish().await.is_err());
    assert!(!files.busy());
}

#[tokio::test]
async fn clipboard_write_rechecks_revocation_and_source_before_touching_os() {
    for mode in ["valid", "revised", "filtered", "closed"] {
        let (mut state, updates, source, identity) = snapshot();
        let lease = state.lease_output(identity, None, 0);
        let mut files = Files::default();
        if mode == "revised" {
            updates.send_modify(|view| view.frame.revision += 1);
        }
        if mode == "filtered" {
            state.replace_rules(filters::Rules::default());
        }
        if mode == "closed" {
            files.revoke();
        }
        let writes = Arc::new(Mutex::new(Vec::new()));
        let written = writes.clone();
        let result = copy_with_writer(
            files.permit.clone(),
            "完整網址".into(),
            lease,
            source,
            move |text| {
                written.lock().unwrap().push(text);
                Ok(())
            },
        )
        .await;
        assert!(matches!(result, Completion::Clipboard(Ok(()))) == (mode == "valid"));
        assert_eq!(writes.lock().unwrap().len(), usize::from(mode == "valid"));
        files.finish().await.unwrap();
    }
}

#[tokio::test]
async fn clipboard_worker_is_bounded_nonblocking_and_joined_on_close() {
    let (mut state, _updates, source, identity) = snapshot();
    let lease = state.lease_output(identity, None, 0);
    let (started, start) = oneshot::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let mut files = Files::default();
    files
        .start(|permit| {
            copy_with_writer(permit, "text".into(), lease, source, move |_| {
                started.send(()).unwrap();
                gate.recv_timeout(Duration::from_secs(3)).unwrap();
                Err("synthetic clipboard failure".into())
            })
        })
        .unwrap();
    tokio::select! {_=files.next()=>panic!("worker returned early"),_=start=>{}}
    assert!(
        files
            .start(|_| async { Completion::Clipboard(Ok(())) })
            .is_err()
    );
    let mut finish = Box::pin(files.finish());
    tokio::select! {_=&mut finish=>panic!("worker was not joined"),_=tokio::time::sleep(Duration::from_millis(10))=>{}}
    release.send(()).unwrap();
    assert!(
        finish
            .await
            .unwrap_err()
            .contains("synthetic clipboard failure")
    );
    assert!(!files.busy());
}

#[test]
fn png_clipboard_decodes_rgb_and_rgba_with_exact_bounded_dimensions() {
    let image = clipboard_pixels(black_png_data(), &(0..24)).unwrap();
    assert_eq!((image.width, image.height), (960, 576));
    assert!(
        image
            .bytes
            .chunks_exact(4)
            .all(|pixel| pixel == [0, 0, 0, 255])
    );
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 960, 24);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&[17, 34, 51, 128].repeat(960 * 24))
            .unwrap();
    }
    let encoded = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );
    let image = clipboard_pixels(encoded.clone(), &(0..1)).unwrap();
    assert_eq!((image.width, image.height), (960, 24));
    assert!(image.bytes.chunks_exact(4).all(|p| p == [17, 34, 51, 128]));
    assert!(clipboard_pixels(encoded, &(0..24)).is_err());
    bytes.truncate(40);
    let encoded = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    );
    assert!(clipboard_pixels(encoded, &(0..1)).is_err());
}

#[tokio::test]
async fn png_clipboard_rejects_cancelled_revised_or_filtered_output_before_os_write() {
    for mode in [
        "valid", "revised", "filtered", "closed", "invalid", "session",
    ] {
        let (mut state, updates, source, identity) = snapshot();
        let lease = state.lease_output(identity, None, 0);
        let mut files = Files::default();
        if mode == "revised" {
            updates.send_modify(|v| v.frame.revision += 1);
        }
        if mode == "session" {
            updates.send_modify(|v| v.generation += 1);
        }

        if mode == "filtered" {
            state.replace_rules(filters::Rules::default());
        }
        if mode == "closed" {
            files.revoke();
        }
        let writes = Arc::new(Mutex::new(0));
        let written = writes.clone();
        let request = ExportRequest {
            lease,
            source,
            png: if mode == "invalid" {
                "data:image/png;base64,AAAA".into()
            } else {
                black_png_data()
            },
            rows: 0..24,
            selected: false,
            probe: None,
        };
        let done = copy_png_with_writer(files.permit.clone(), request, move |image| {
            assert_eq!((image.width, image.height), (960, 576));
            *written.lock().unwrap() += 1;
            Ok(())
        })
        .await;
        assert_eq!(
            matches!(
                done,
                Completion::Export {
                    destination: ExportDestination::Clipboard,
                    result: Ok(true),
                    ..
                }
            ),
            mode == "valid"
        );
        assert_eq!(*writes.lock().unwrap(), usize::from(mode == "valid"));
        files.finish().await.unwrap();
    }
}

fn black_png_data() -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD
            .encode(include_bytes!("../../tests/fixtures/export-black.png"))
    )
}

#[tokio::test]
async fn original_picture_save_preserves_bytes_and_epoch_revocation_never_touches_clipboard() {
    use std::sync::Arc;
    let bytes = include_bytes!("../../tests/fixtures/media/colours.png");
    let image = Arc::new(pttstella::media::loader::Image {
        bytes: bytes.to_vec(),
        mime: "image/png",
        width: 64,
        height: 32,
        animated_format: false,
        animation_pixels: 0,
    });
    let (mut state, sender, receiver, identity) = snapshot();
    let directory = tempfile::tempdir().unwrap();
    let mut files = Files::default();
    let clipboard = files.clipboard();
    files
        .start(|permit| {
            picture::output(
                permit,
                picture::Request {
                    image: image.clone(),
                    lease: state.lease_picture(identity, receiver.borrow().input_epoch),
                    source: receiver.clone(),
                    destination: ExportDestination::File,
                    probe: Some(directory.path().to_path_buf()),
                },
                clipboard.clone(),
            )
        })
        .unwrap();
    assert!(matches!(
        files.next().await,
        Completion::Picture {
            result: Ok(true),
            ..
        }
    ));
    assert_eq!(
        std::fs::read(directory.path().join("picture.png")).unwrap(),
        bytes
    );
    files
        .start(|permit| {
            picture::output(
                permit,
                picture::Request {
                    image,
                    lease: state.lease_picture(identity, receiver.borrow().input_epoch),
                    source: receiver.clone(),
                    destination: ExportDestination::Clipboard,
                    probe: None,
                },
                clipboard.clone(),
            )
        })
        .unwrap();
    sender.send_modify(|view| view.input_epoch += 1);
    assert!(matches!(
        files.next().await,
        Completion::Picture {
            result: Err(Error::Stale),
            ..
        }
    ));
    assert!(clipboard.lock().unwrap().is_none());
    files.finish().await.unwrap();
}

#[test]
fn viewport_oracle_rejects_missing_text_and_last_row_background() {
    let (width, height) = (800u32, 480u32);
    let mut pixels = vec![0u8; (width * height * 3) as usize];
    let colours = [
        [128, 0, 0],
        [0, 128, 0],
        [128, 128, 0],
        [0, 0, 128],
        [128, 0, 128],
        [0, 128, 128],
    ];
    for row in 0..24 {
        for y in row * 20..(row + 1) * 20 {
            for x in 0..800 {
                let i = (y * 800 + x) * 3;
                pixels[i..i + 3].copy_from_slice(&colours[row % 6]);
            }
        }
        for col in [0, 1, 2, 4, 5] {
            let i = ((row * 20 + 3) * 800 + col * 10 + 3) * 3;
            pixels[i..i + 3].fill(192);
        }
    }
    let encode = |pixels: &[u8]| {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(pixels)
                .unwrap();
        }
        bytes
    };
    assert_eq!(
        super::viewport_ink(&encode(&pixels), width, height).unwrap(),
        vec![[1; 5]; 24]
    );
    let missing = ((23 * 20 + 3) * 800 + 5 * 10 + 3) * 3;
    pixels[missing..missing + 3].copy_from_slice(&colours[5]);
    assert!(
        super::viewport_ink(&encode(&pixels), width, height)
            .unwrap_err()
            .contains("label missing")
    );
    pixels[missing..missing + 3].fill(192);
    let missing = (23 * 20 * 800 + 600) * 3;
    pixels[missing..missing + 3].fill(0);
    assert!(
        super::viewport_ink(&encode(&pixels), width, height)
            .unwrap_err()
            .contains("background missing")
    );
}

#[tokio::test]
async fn editor_clipboard_read_rechecks_lease_and_rejects_cancel_stale_controls_and_failure() {
    for mode in ["valid", "cancel", "before", "during", "control", "failed"] {
        let (mut state, sender, source, identity) = snapshot();
        let lease = state.lease_picture(identity, 0);
        let permit = Permit::default();
        if mode == "cancel" {
            permit.0.store(true, Ordering::Release);
        }
        if mode == "before" {
            sender.send_modify(|v| v.input_epoch += 1);
        }
        let result = read_editor_with_reader(permit, lease, source, move || {
            assert!(
                !matches!(mode, "cancel" | "before"),
                "revoked work must not read clipboard"
            );
            if mode == "during" {
                sender.send_modify(|v| v.input_epoch += 1);
            }
            if mode == "failed" {
                return Err(Error::Failed("synthetic unavailable".into()));
            }
            Ok(if mode == "control" {
                "\x1b[2J"
            } else {
                "\x1b[31mA\x1b[0m"
            }
            .into())
        })
        .await;
        match mode {
            "valid" => assert_eq!(result.unwrap().bytes, b"\x15[31mA\x15[0m"),
            "cancel" => assert!(matches!(result, Err(Error::Cancelled))),
            "before" | "during" => assert!(matches!(result, Err(Error::Stale))),
            _ => assert!(matches!(result, Err(Error::Failed(_)))),
        }
    }
}
