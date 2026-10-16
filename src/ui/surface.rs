//! Serializes renderer events and coordinates their feature owners.
use base64::Engine;
use dioxus::prelude::*;
use pttstella::{
    connection::{CloseOutcome, Connection, Phase},
    export, filters,
    input::{self, PreparedPaste},
    links,
    login::vault::{Action as LoginAction, Job as LoginJob, Outcome as LoginOutcome, Vault},
    media::{
        self,
        hover::Lease as HoverLease,
        preview::{Presentation, Preview},
    },
    navigation, selection,
    settings::HoverPosition,
    storage,
    vim::{self, Effect},
};
use std::{ops::Range, path::PathBuf, sync::Arc};
use tokio::sync::watch;

use super::{
    preferences::Preferences,
    probe::{self, Checks},
};
use super::{
    presentation::{Paint, current_presentation, paint},
    protocol::{MediaAction, SurfaceEvent},
};

/// Named launch options keep call sites independent of positional booleans.
pub(super) struct Options {
    pub(super) reload_settings: bool,
    pub(super) media_origin: Option<String>,
    pub(super) window_probe: bool,
    pub(super) login_probe: bool,
    pub(super) filters_probe: bool,
}

#[derive(Clone)]
pub(super) struct PendingPaste {
    pub(super) generation: u64,
    pub(super) prepared: PreparedPaste,
}
struct SelectedRows {
    presentation: u64,
    generation: u64,
    revision: u64,
    request: u64,
    rows: Range<usize>,
}
pub(super) async fn surface(
    connection: Arc<Connection>,
    mut closing: watch::Receiver<bool>,
    probe: Option<PathBuf>,
    mut message: Signal<String>,
    mut phase: Signal<Phase>,
    mut pending_paste: Signal<Option<PendingPaste>>,
    options: Options,
) -> Result<(), String> {
    let Options {
        reload_settings,
        media_origin,
        window_probe,
        login_probe,
        filters_probe,
    } = options;
    let filter_store = probe
        .as_ref()
        .map_or_else(filters::store::Store::system, |path| {
            Ok(filters::store::Store::at(path.join("probe-filters.json")))
        });
    let (rules, mut filter_warning) = match filter_store
        .as_ref()
        .map_err(Clone::clone)
        .and_then(filters::store::Store::load)
    {
        Ok(rules) => (rules, None),
        Err(error) => (filters::Rules::default(), Some(error)),
    };
    let mut filters = filters::State::new(rules);
    let mut receiver = connection.updates.clone();
    let mut preferences = Preferences::load(probe.as_deref())?;
    let script = [
        include_str!("../../assets/media.js"),
        include_str!("../../assets/surface.js"),
    ]
    .concat();
    let mut eval = document::eval(&script);
    let mut media_preview = Preview::default();
    let mut hover_lease: Option<HoverLease> = None;
    let mut keyboard_policy = vim::Keyboard::default();
    let mut keyboard_generation = 0;
    let mut login_job: Option<LoginJob> = None;
    let mut login_stamp = None;
    let mut login_open = false;
    let first = receiver.borrow_and_update().clone();
    let mut last_status = first.message.clone();
    let mut painting = paint(&first, false, preferences.display(), &filters);
    let mut displayed: Option<Paint> = None;
    eval.send(&painting).map_err(|e| e.to_string())?;
    eval.send(serde_json::json!({"hover_settings":preferences.hover()}))
        .map_err(|e| e.to_string())?;
    if let Some(warning) = preferences.warning() {
        eval.send(serde_json::json!({"notice":warning}))
            .map_err(|e| e.to_string())?;
    }
    if let Some(warning) = &filter_warning {
        eval.send(serde_json::json!({"notice":warning}))
            .map_err(|e| e.to_string())?;
    }
    connection.connect()?;
    let mut in_flight = Some((first.generation, first.frame.revision, filters.version()));
    let mut dirty = false;
    let mut stopping = false;
    let mut close_deadline = None;
    let mut checks = Checks::new();
    let mut selected_rows: Option<SelectedRows> = None;
    let mut painted = (first.generation, first.frame.revision);
    let mut pending_report: Option<serde_json::Value> = None;
    loop {
        let complete = probe.is_some() && {
            let view = receiver.borrow();
            painted == (view.generation, view.frame.revision)
                && view
                    .frame
                    .glyphs
                    .iter()
                    .take("INPUT ROUND TRIP COMPLETE".len())
                    .map(|g| g.text.as_str())
                    .collect::<String>()
                    == "INPUT ROUND TRIP COMPLETE"
        };
        let interactions_checked = checks.interactions_complete(complete);
        let probe_checked = if reload_settings {
            checks.probe_started && in_flight.is_none()
        } else {
            interactions_checked
        };
        if pending_report.is_some() && probe_checked && !stopping {
            connection.keep_alive_gate(receiver.borrow().stamp(), false);
            eval.send(serde_json::json!({"stop":true}))
                .map_err(|e| e.to_string())?;
            stopping = true;
            close_deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(5));
        }
        // One in-flight frame: slow font/paint work cannot grow the JS queue.
        if dirty && in_flight.is_none() && !stopping {
            let view = receiver.borrow().clone();
            let run_probe = probe.is_some()
                && !window_probe
                && !login_probe
                && !filters_probe
                && !checks.probe_started
                && view.phase == Phase::Connected
                && view.received > 0;
            checks.probe_started |= run_probe;
            painting = paint(&view, run_probe, preferences.display(), &filters);
            painting.hover = hover_lease
                .as_ref()
                .filter(|lease| lease.stamp == view.stamp())
                .cloned();
            eval.send(&painting).map_err(|e| e.to_string())?;
            in_flight = Some((view.generation, view.frame.revision, filters.version()));
            dirty = false;
        }
        tokio::select! {
            result = async { match login_job.as_mut() {Some(job)=>job.wait().await,None=>std::future::pending().await} } => {
                login_job=None;
                if stopping {continue;}
                if !login_open {eval.send(serde_json::json!({"login_result":{"busy":false}})).map_err(|e|e.to_string())?;continue;}
                let packet=match result {
                    Ok(LoginOutcome::Opened {account,saved})=>serde_json::json!({"login_result":{"account":account,"saved":saved,"busy":false,"message":if saved {"已有密碼保存在鑰匙圈；留空可使用已存密碼"} else {"密碼僅用於此次登入；勾選後才保存至鑰匙圈"}}}),
                    Ok(LoginOutcome::Forgotten)=>serde_json::json!({"login_result":{"account":"","saved":false,"busy":false,"message":"已移除鑰匙圈中的帳密"}}),
                    Ok(LoginOutcome::Ready(credentials))=>{
                        let result=if let Some(stamp)=login_stamp.take() {connection.login(stamp,credentials).await} else {Err("登入請求已取消".into())};
                        match result {
                            Ok(())=>{login_open=false;serde_json::json!({"login_result":{"close":true,"busy":false}})},
                            Err(error)=>serde_json::json!({"login_result":{"busy":false,"message":error}}),
                        }
                    },
                    Err(error)=>serde_json::json!({"login_result":{"busy":false,"message":error}}),
                };
                eval.send(packet).map_err(|e|e.to_string())?;
            },
            update = media_preview.update() => {
                if let Some(update) = update {
                    let reply = match update.result {
                        Ok((image,cached)) => {
                            serde_json::json!({"media_image": {"request":update.request,
                                "data":base64::engine::general_purpose::STANDARD.encode(&image.bytes),
                                "mime":image.mime,"width":image.width,"height":image.height,
                                "animated":image.animated_format,"cached":cached}})
                        },
                        Err(error) => serde_json::json!({"media_error":{"request":update.request,"message":error.to_string()}}),
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                }
            },
            event = eval.recv::<SurfaceEvent>() => match event.map_err(|e| e.to_string())? {
                SurfaceEvent::LoginOpen { trusted } => {
                    if !trusted || stopping || login_job.is_some() {
                        continue;
                    }
                    login_open = true;
                    login_stamp = None;
                    let vault = Vault::new(probe.is_some());
                    login_job = Some(LoginJob::start(vault, LoginAction::Open)?);
                }
                SurfaceEvent::LoginSubmit {
                    account,
                    password,
                    remember,
                    stamp,
                    trusted,
                } => {
                    let password = zeroize::Zeroizing::new(password.into_bytes());
                    if !trusted || stopping || !login_open || login_job.is_some() {
                        continue;
                    }
                    login_stamp = Some(stamp);
                    let vault = Vault::new(probe.is_some());
                    login_job = Some(LoginJob::start(
                        vault,
                        LoginAction::Submit {
                            account,
                            password,
                            remember,
                        },
                    )?);
                }
                SurfaceEvent::LoginForget { trusted } => {
                    if !trusted || stopping || !login_open || login_job.is_some() {
                        continue;
                    }
                    let vault = Vault::new(probe.is_some());
                    login_job = Some(LoginJob::start(vault, LoginAction::Forget)?);
                }
                SurfaceEvent::LoginCancel { trusted } => {
                    if !trusted || stopping {
                        continue;
                    }
                    login_open = false;
                    login_stamp = None;
                    if let Err(error) = connection.cancel_login() {
                        message.set(error);
                    }
                }
                SurfaceEvent::FiltersOpen { trusted } => {
                    if stopping || !trusted {
                        continue;
                    }
                    eval.send(serde_json::json!({"filters_open":{"entries":filters.rules().entries,"warning":filter_warning}})).map_err(|e|e.to_string())?;
                }
                SurfaceEvent::FiltersEdit {
                    id,
                    note,
                    remove,
                    trusted,
                } => {
                    if stopping || !trusted {
                        continue;
                    }
                    let result = filters.rules().edit(&id, &note, remove).and_then(|rules| {
                        filter_store
                            .as_ref()
                            .map_err(Clone::clone)
                            .and_then(|store| store.save(rules))
                    });
                    match result {
                        Ok(rules) => {
                            filters.replace(rules);
                            filter_warning = None;
                            checks.filter_edits += 1;
                            selected_rows = None;
                            dirty = true;
                            hover_lease = None;
                            media_preview.close();
                            eval.send(serde_json::json!({"media_reset":true,"hover_reset":true}))
                                .map_err(|e| e.to_string())?;
                            eval.send(serde_json::json!({"filters_result":{"ok":true,"entries":filters.rules().entries,"message":"已套用並儲存一般黑名單"}})).map_err(|e|e.to_string())?;
                        }
                        Err(error) => eval
                            .send(serde_json::json!({"filters_result":{"ok":false,"message":error}}))
                            .map_err(|e| e.to_string())?,
                    }
                }
                SurfaceEvent::FiltersReveal {
                    presentation,
                    generation,
                    revision,
                    reveal,
                    trusted,
                } => {
                    if stopping || !trusted {
                        continue;
                    }
                    let view = receiver.borrow();
                    if filters.version() != presentation
                        || (view.generation, view.frame.revision) != (generation, revision)
                    {
                        continue;
                    }
                    filters.reveal(generation, revision, reveal);
                    checks.filter_reveals += 1;
                    selected_rows = None;
                    dirty = true;
                    hover_lease = None;
                    media_preview.close();
                    eval.send(serde_json::json!({"media_reset":true,"hover_reset":true}))
                        .map_err(|e| e.to_string())?;
                }
                SurfaceEvent::HoverPreference { position, trusted } => {
                    if stopping || !trusted {
                        continue;
                    }
                    if let Err(error) = preferences.set_hover(position) {
                        eval.send(serde_json::json!({"notice":error}))
                            .map_err(|e| e.to_string())?;
                    }
                    if preferences.hover() == HoverPosition::Off && hover_lease.take().is_some() {
                        media_preview.close();
                        eval.send(serde_json::json!({"hover_reset":true}))
                            .map_err(|e| e.to_string())?;
                    }
                    eval.send(serde_json::json!({"hover_settings":preferences.hover()}))
                        .map_err(|e| e.to_string())?;
                }
                SurfaceEvent::Hover {
                    presentation,
                    sequence,
                    stamp,
                    index,
                    trusted,
                } => {
                    if stopping {
                        continue;
                    }
                    let Some(index) = index else {
                        if hover_lease
                            .as_ref()
                            .is_some_and(|lease| lease.sequence == sequence)
                        {
                            hover_lease = None;
                            media_preview.close();
                        }
                        continue;
                    };
                    let view = receiver.borrow().clone();
                    let link = displayed
                        .as_ref()
                        .filter(|p| {
                            current_presentation(
                                p,
                                presentation,
                                stamp.generation,
                                stamp.revision,
                                &filters,
                                &view,
                            ) && p.input_epoch == stamp.input_epoch
                        })
                        .and_then(|p| p.links.get(index));
                    let target = link
                        .and_then(|link| media::classify(&link.url).ok().flatten())
                        .filter(|target| target.kind == media::Kind::Image);
                    if !trusted
                        || preferences.hover() == HoverPosition::Off
                        || view.stamp() != stamp
                        || view.phase != Phase::Connected
                        || !view.frame.control_complete
                        || target.is_none()
                        || media_preview
                            .active()
                            .is_some_and(|active| active.presentation == Presentation::Inspect)
                    {
                        eval.send(serde_json::json!({"hover_rejected":sequence}))
                            .map_err(|e| e.to_string())?;
                        continue;
                    }
                    if let (Some(link), Some(target)) = (link, target) {
                        if filters_probe {
                            checks.filter_hover_requests += 1;
                            eval.send(serde_json::json!({"hover_rejected":sequence}))
                                .map_err(|e| e.to_string())?;
                            continue;
                        }
                        let active = media_preview.hover(stamp.generation, target)?;
                        hover_lease = Some(HoverLease {
                            sequence,
                            stamp,
                            index,
                            link: link.clone(),
                        });
                        eval.send(serde_json::json!({"media_hover":{"sequence":sequence,"request":active.request,"target":active.target,"position":preferences.hover()}})).map_err(|e|e.to_string())?;
                    }
                }
                SurfaceEvent::KeyboardState {
                    action,
                    code,
                    generation,
                    trusted,
                } => {
                    // Ending an owned edit only revokes permission; WebView CDP can
                    // report its compositionend as untrusted, including cancellation.
                    if stopping
                        || (!trusted && action != "composition_end")
                        || generation != receiver.borrow().generation
                    {
                        continue;
                    }
                    match action.as_str() {
                        "toggle" => keyboard_policy.toggle(),
                        "once" => keyboard_policy.arm(),
                        "release" => keyboard_policy.release(&code),
                        "composition_start" => keyboard_policy.composition_start(),
                        "composition_end" => keyboard_policy.composition_end(),
                        "independent_edit" => keyboard_policy.independent_edit(),
                        "reset" => keyboard_policy.reset(),
                        _ => continue,
                    }
                    let context = navigation::analyze(&receiver.borrow().frame);
                    eval.send(serde_json::json!({"keyboard_state":keyboard_policy.status(&context)}))
                        .map_err(|e| e.to_string())?;
                }
                SurfaceEvent::MediaDecoded { request, success } => {
                    media_preview.decoded(request, success);
                }
                SurfaceEvent::KeepAlive { enabled, trusted } => {
                    if stopping || !trusted {
                        continue;
                    }
                    if let Err(error) = connection.keep_alive(enabled) {
                        eval.send(serde_json::json!({"notice":error}))
                            .map_err(|e| e.to_string())?;
                    }
                }
                SurfaceEvent::KeepAliveGate { stamp, ready } => {
                    if !stopping {
                        connection.keep_alive_gate(
                            stamp,
                            ready
                                && !keyboard_policy
                                    .status(&navigation::analyze(&receiver.borrow().frame))
                                    .pending,
                        );
                    }
                }
                SurfaceEvent::Navigate {
                    stamp,
                    intent,
                    trusted,
                } => {
                    if stopping || !trusted {
                        continue;
                    }
                    let shown = displayed.as_ref().is_some_and(|p| {
                        p.generation == stamp.generation
                            && p.frame.revision == stamp.revision
                            && p.input_epoch == stamp.input_epoch
                    });
                    let result = if !shown {
                        Err(navigation::Error::Stale.to_string())
                    } else if pending_paste.read().is_some() {
                        Err(navigation::Error::Unavailable.to_string())
                    } else {
                        connection.navigate(stamp, intent).await
                    };
                    if result.is_ok() && probe.is_some() {
                        checks.navigation_requests += 1;
                    }
                    eval.send(match result {
                        Ok(()) => serde_json::json!({"navigation_result":"sent"}),
                        Err(error) => {
                            serde_json::json!({"navigation_result":"rejected","notice":error})
                        }
                    })
                    .map_err(|e| e.to_string())?;
                }
                SurfaceEvent::SettingsOpen { trusted } => {
                    if stopping || !trusted {
                        continue;
                    }
                    eval.send(preferences.open_packet())
                        .map_err(|e| e.to_string())?;
                }
                SurfaceEvent::SettingsPreview { request, values } => {
                    eval.send(preferences.preview_packet(request, values))
                        .map_err(|e| e.to_string())?;
                }
                SurfaceEvent::SettingsSave { values, trusted } => {
                    if stopping || !trusted {
                        continue;
                    }
                    let reply = match preferences.save_display(values) {
                        Ok(()) => {
                            checks.settings_saved |= probe.is_some()
                                && preferences.display().font_size == 18.0
                                && preferences.display().baseline == 19.0;
                            dirty = true;
                            serde_json::json!({"settings_saved":true,"notice":"顯示設定已儲存"})
                        }
                        Err(error) => serde_json::json!({"settings_saved":false,"notice":error}),
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                }
                SurfaceEvent::SettingsProfile {
                    action,
                    name,
                    trusted,
                } => {
                    if stopping || !trusted {
                        continue;
                    }
                    eval.send(preferences.profile_packet(&action, name, probe.as_deref()))
                        .map_err(|e| e.to_string())?;
                }
                SurfaceEvent::ExportPng {
                    presentation,
                    generation,
                    revision,
                    selection_request,
                    png,
                    trusted,
                } => {
                    if stopping || !trusted {
                        continue;
                    }
                    let result = (|| -> Result<Option<PathBuf>, String> {
                        let shown = displayed
                            .as_ref()
                            .filter(|p| {
                                current_presentation(
                                    p,
                                    presentation,
                                    generation,
                                    revision,
                                    &filters,
                                    &receiver.borrow(),
                                )
                            })
                            .ok_or("畫面已更新，請重新選取後匯出")?;
                        if (shown.frame.cols, shown.frame.rows) != (80, export::ROWS) {
                            return Err("此畫面尺寸不支援 PNG 匯出".into());
                        }
                        let rows = match selection_request {
                            Some(request) => selected_rows
                                .as_ref()
                                .filter(|s| {
                                    s.presentation == presentation
                                        && s.generation == generation
                                        && s.revision == revision
                                        && s.request == request
                                })
                                .map(|s| s.rows.clone())
                                .ok_or("選取已變更，請重新選取後匯出")?,
                            None => 0..export::ROWS,
                        };
                        let encoded = png
                            .strip_prefix("data:image/png;base64,")
                            .ok_or("PNG 格式不正確")?;
                        if encoded.len() > export::MAX_BASE64_BYTES {
                            return Err("PNG 超過匯出大小上限".into());
                        }
                        let bytes = base64::engine::general_purpose::STANDARD
                            .decode(encoded)
                            .map_err(|_| "PNG 編碼不正確")?;
                        export::check_export_header(&bytes, &rows).map_err(str::to_owned)?;
                        let path = if let Some(directory) = &probe {
                            Some(directory.join(if selection_request.is_some() {
                                "export-rows.png"
                            } else {
                                "export.png"
                            }))
                        } else {
                            // Parent-owned synchronous dialog: no detached native dialog task.
                            let window = dioxus::desktop::window();
                            rfd::FileDialog::new()
                                .set_parent(window.window.as_ref())
                                .set_title("匯出 PTT 畫面")
                                .add_filter("PNG", &["png"])
                                .set_file_name(if selection_request.is_some() {
                                    format!("PTTStella-lines-{}-{}.png", rows.start + 1, rows.end)
                                } else {
                                    "PTTStella.png".into()
                                })
                                .save_file()
                        };
                        if let Some(path) = &path {
                            if !path
                                .extension()
                                .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
                            {
                                return Err("請使用 .png 副檔名；尚未儲存".into());
                            }
                            storage::replace(path, &bytes)?;
                        }
                        Ok(path)
                    })();
                    let reply = match result {
                        Ok(Some(_)) => {
                            checks.export_checked |= probe.is_some() && selection_request.is_none();
                            checks.rows_export_checked |=
                                probe.is_some() && selection_request.is_some();
                            serde_json::json!({"export_result":"saved","notice":"PNG 已儲存"})
                        }
                        Ok(None) => {
                            serde_json::json!({"export_result":"cancelled","notice":"未選擇儲存位置"})
                        }
                        Err(error) => serde_json::json!({"export_result":"failed","notice":error}),
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                }
                SurfaceEvent::OpenLink {
                    presentation,
                    generation,
                    revision,
                    index,
                    trusted,
                } => {
                    if stopping || !trusted {
                        continue;
                    }
                    let link = displayed
                        .as_ref()
                        .filter(|p| {
                            current_presentation(
                                p,
                                presentation,
                                generation,
                                revision,
                                &filters,
                                &receiver.borrow(),
                            )
                        })
                        .and_then(|p| p.links.get(index))
                        .cloned();
                    if let Some(link) = &link {
                        match media::classify(&link.url) {
                            Ok(Some(mut target)) => {
                                if probe.is_some() {
                                    let origin = media_origin
                                        .as_deref()
                                        .ok_or("missing media probe origin")?;
                                    if target.kind == media::Kind::Youtube {
                                        target.image_url = Some(format!("{origin}/colours.png"));
                                    } else if !target.url.starts_with(&format!("{origin}/")) {
                                        return Err("unexpected media probe URL".into());
                                    }
                                }
                                let active = media_preview.open(generation, target)?;
                                hover_lease = None;
                                eval.send(serde_json::json!({"media_open":{"request":active.request,"target":active.target}})).map_err(|e| e.to_string())?;
                                continue;
                            }
                            Err(error) => {
                                eval.send(serde_json::json!({"notice":error,"link_result":"failed"}))
                                    .map_err(|e| e.to_string())?;
                                continue;
                            }
                            Ok(None) => {}
                        }
                    }
                    let result = match link {
                        Some(link) if probe.is_some() => {
                            if link.url != "https://example.com/pttstella" {
                                return Err("unexpected probe URL".into());
                            }
                            checks.link_requests += 1;
                            Ok(())
                        }
                        Some(link) => {
                            webbrowser::open(&link.url).map_err(|e| format!("無法開啟網址：{e}"))
                        }
                        None => Err("畫面已更新，請重新選取網址".to_owned()),
                    };
                    let reply = match result {
                        Ok(()) => serde_json::json!({"link_result":"requested"}),
                        Err(error) => serde_json::json!({"link_result":"failed","notice":error}),
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                }
                SurfaceEvent::Media {
                    request,
                    action,
                    trusted,
                } => {
                    if stopping || !trusted {
                        continue;
                    }
                    let Some(preview) = media_preview.active() else {
                        continue;
                    };
                    if preview.request != request {
                        continue;
                    }
                    match action {
                        MediaAction::Close => media_preview.close(),
                        MediaAction::Retry => media_preview.retry(),
                        MediaAction::External => {
                            let result = if probe.is_some() {
                                checks.media_external += 1;
                                Ok(())
                            } else {
                                webbrowser::open(&preview.target.url)
                                    .map_err(|e| format!("無法開啟網址：{e}"))
                            };
                            eval.send(serde_json::json!({"media_external":{"request":request,"ok":result.is_ok(),"message":result.err()}})).map_err(|e| e.to_string())?;
                        }
                    }
                }
                SurfaceEvent::Paste {
                    text,
                    generation,
                    trusted,
                } => {
                    if stopping {
                        continue;
                    }
                    let result = input::prepare_paste(&text)
                        .map_err(|e| e.to_string())
                        .and_then(|prepared| {
                            let current = receiver.borrow();
                            if current.phase != Phase::Connected || current.generation != generation {
                                return Err("連線已變更；貼上未送出".into());
                            }
                            if prepared.lines > 1 {
                                pending_paste.set(Some(PendingPaste {
                                    generation,
                                    prepared,
                                }));
                                Ok(false)
                            } else {
                                checks.paste_single |=
                                    probe.is_some() && trusted && prepared.text == "中";
                                connection.input(generation, prepared.bytes).map(|()| true)
                            }
                        });
                    let reply = match result {
                        Ok(queued) => {
                            serde_json::json!({"paste_result":if queued {"queued"} else {"preview"}})
                        }
                        Err(error) => {
                            if probe.is_some() && trusted {
                                checks.paste_rejected += 1;
                            }
                            serde_json::json!({"notice":error,"paste_result":"rejected"})
                        }
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                }
                SurfaceEvent::PasteCancel { trusted } => {
                    checks.paste_cancel |= probe.is_some()
                        && trusted
                        && pending_paste
                            .read()
                            .as_ref()
                            .is_some_and(|p| p.prepared.text == "A\nB");
                    pending_paste.set(None);
                    eval.send(serde_json::json!({"paste_result":"cancelled"}))
                        .map_err(|e| e.to_string())?;
                }
                SurfaceEvent::PasteConfirm { trusted } => {
                    let pending = pending_paste.write().take();
                    if let Some(pending) = pending {
                        let current = receiver.borrow();
                        let result = if current.phase == Phase::Connected
                            && current.generation == pending.generation
                        {
                            checks.paste_confirm |=
                                probe.is_some() && trusted && pending.prepared.text == "A\nB";
                            connection.input(pending.generation, pending.prepared.bytes)
                        } else {
                            Err("連線已變更；貼上未送出".to_owned())
                        };
                        let reply = match result {
                            Ok(()) => serde_json::json!({"paste_result":"queued"}),
                            Err(error) => serde_json::json!({"notice":error,"paste_result":"rejected"}),
                        };
                        eval.send(reply).map_err(|e| e.to_string())?;
                    } else {
                        eval.send(serde_json::json!({"paste_result":"cancelled"}))
                            .map_err(|e| e.to_string())?;
                    }
                }
                SurfaceEvent::Select {
                    presentation,
                    generation,
                    revision,
                    request,
                    selection,
                } => {
                    let selected = displayed
                        .as_ref()
                        .filter(|p| {
                            current_presentation(
                                p,
                                presentation,
                                generation,
                                revision,
                                &filters,
                                &receiver.borrow(),
                            )
                        })
                        .and_then(|p| selection::extract(&p.frame, &selection).ok());
                    if probe.is_some() && selected.as_ref().is_some_and(|s| s.text == "中") {
                        checks.copied_selection = Some((generation, revision, request));
                    }
                    selected_rows = selected.as_ref().map(|s| SelectedRows {
                        presentation,
                        generation,
                        revision,
                        request,
                        rows: s.rows.clone(),
                    });
                    eval.send(serde_json::json!({"selection": {
                            "presentation":presentation,"generation":generation,"revision":revision,"request":request,"value":selected
                        }})).map_err(|e| e.to_string())?;
                }
                SurfaceEvent::Copied {
                    presentation,
                    generation,
                    revision,
                    request,
                    trusted,
                } => {
                    if filters_probe
                        && trusted
                        && selected_rows
                            .as_ref()
                            .is_some_and(|s| s.presentation == presentation && s.request == request)
                        && displayed.as_ref().is_some_and(|p| {
                            current_presentation(
                                p,
                                presentation,
                                generation,
                                revision,
                                &filters,
                                &receiver.borrow(),
                            )
                        })
                    {
                        checks.filter_copies += 1;
                    }
                    checks.copy_checked |=
                        trusted && checks.copied_selection == Some((generation, revision, request));
                }
                SurfaceEvent::Key {
                    key,
                    generation,
                    trusted,
                } => {
                    if stopping || !trusted || generation != receiver.borrow().generation {
                        continue;
                    }
                    let view = receiver.borrow().clone();
                    let context = navigation::analyze(&view.frame);
                    let effect = keyboard_policy.key(&key, &context);
                    eval.send(serde_json::json!({"keyboard_state":keyboard_policy.status(&context)}))
                        .map_err(|e| e.to_string())?;
                    let bytes = match effect {
                        Effect::None => continue,
                        Effect::Notice(notice) => {
                            eval.send(serde_json::json!({"notice":notice}))
                                .map_err(|e| e.to_string())?;
                            continue;
                        }
                        Effect::Browse(intent) => {
                            let shown = displayed.as_ref().is_some_and(|p| {
                                p.generation == generation && p.navigation.kind() == context.kind()
                            });
                            let result = if !shown || pending_paste.read().is_some() {
                                Err(navigation::Error::Stale.to_string())
                            } else {
                                connection.browse_key(view.stamp(), intent).await
                            };
                            if let Err(error) = result {
                                eval.send(serde_json::json!({"notice":error}))
                                    .map_err(|e| e.to_string())?;
                            }
                            continue;
                        }
                        Effect::Native(bytes) => bytes,
                    };
                    if probe.is_some() {
                        let current = receiver.borrow().generation;
                        checks.arrow_checked |=
                            trusted && generation == current && bytes == [27, 91, 65];
                        if trusted && generation == current && bytes == [13] {
                            checks.enter_count += 1;
                        }
                        checks.closed_checked |= trusted
                            && generation == current
                            && bytes == [27, 91, 67]
                            && receiver.borrow().phase == Phase::Disconnected;
                    }
                    message.set(match connection.input(generation, bytes) {
                        Ok(()) => receiver.borrow().message.clone(),
                        Err(error) => {
                            eval.send(serde_json::json!({"notice":error,"input_result":"rejected"}))
                                .map_err(|e| e.to_string())?;
                            error
                        }
                    });
                }
                SurfaceEvent::Text {
                    text,
                    generation,
                    trusted,
                    composition_input,
                    composed,
                } => {
                    if stopping {
                        continue;
                    }
                    if generation != receiver.borrow().generation
                        || !(trusted || composition_input)
                        || !keyboard_policy.accept_text(composed)
                    {
                        continue;
                    }
                    eval.send(serde_json::json!({"keyboard_state":keyboard_policy.status(&navigation::analyze(&receiver.borrow().frame))})).map_err(|e|e.to_string())?;
                    let vim_input = keyboard_policy
                        .status(&navigation::analyze(&receiver.borrow().frame))
                        .enabled;
                    if probe.is_some()
                        && !vim_input
                        && (!(trusted || composition_input)
                            || generation != receiver.borrow().generation
                            || checks.input_bytes.len() >= checks.expected_input.len())
                    {
                        return Err("unexpected probe input".into());
                    }
                    checks.composition_checked |= composition_input && text == "中";
                    let encoded = input::encode(&text).map_err(|e| e.to_string());
                    let result = encoded.and_then(|bytes| {
                        if probe.is_some() && !vim_input {
                            checks.input_bytes.push(bytes.clone());
                        }
                        connection.input(generation, bytes)
                    });
                    message.set(match result {
                        Ok(()) => receiver.borrow().message.clone(),
                        Err(error) => {
                            eval.send(serde_json::json!({"notice":error,"input_result":"rejected"}))
                                .map_err(|e| e.to_string())?;
                            error
                        }
                    });
                }
                SurfaceEvent::Painted {
                    presentation,
                    revision,
                    generation,
                } => {
                    if in_flight != Some((generation, revision, presentation)) {
                        return Err("畫面回報未知版本".into());
                    }
                    in_flight = None;
                    painted = (generation, revision);
                    displayed = Some(painting.clone());
                }
                SurfaceEvent::Error { message } => return Err(message),
                SurfaceEvent::Probe(pixels) => {
                    let path = probe.as_deref().ok_or("unexpected probe response")?;
                    let report =
                        probe::pixels_report(*pixels, path, reload_settings, preferences.app())?;
                    pending_report = Some(report);
                    close_deadline =
                        Some(tokio::time::Instant::now() + std::time::Duration::from_secs(45));
                }
                SurfaceEvent::Stopped { clean } => {
                    // Native Keychain calls cannot be aborted. Keep ownership and
                    // join the one worker; discard any late secret without sending.
                    if let Some(mut job) = login_job.take() {
                        let _ = job.wait().await;
                    }
                    media_preview.close();
                    let disconnect = connection.shutdown().await?;
                    if login_probe {
                        let path = probe.as_ref().ok_or("missing login-probe directory")?;
                        std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
                        std::fs::write(path.join("login-probe.json"),serde_json::json!({"status":if clean && disconnect==CloseOutcome::Acknowledged {"passed"} else {"failed"},"scope":"synthetic login only; peer checked one account and password, no extra input","disconnect":disconnect,"listeners_released":clean,"connection_worker_joined":true,"credential_worker_joined":true}).to_string()).map_err(|e|e.to_string())?;
                    }
                    if filters_probe {
                        let path = probe.as_ref().ok_or("missing filters probe directory")?;
                        std::fs::write(path.join("filters-probe.json"),serde_json::json!({"status":if clean && disconnect==CloseOutcome::Acknowledged && checks.filter_edits>=4 && checks.filter_reveals>=2 && checks.filter_copies>=1 && checks.filter_hover_requests==0 && checks.export_checked && checks.rows_export_checked {"passed"} else {"failed"},"scope":"synthetic local push masking and explicit CRUD, reveal, copy and PNG; no extra station bytes","disconnect":disconnect,"listeners_released":clean,"connection_worker_joined":true,"filter_edits":checks.filter_edits,"filter_reveals":checks.filter_reveals,"copies":checks.filter_copies,"hover_requests":checks.filter_hover_requests,"full_png":checks.export_checked,"rows_png":checks.rows_export_checked}).to_string()).map_err(|e|e.to_string())?;
                    }
                    if window_probe {
                        let path = probe.as_ref().ok_or("missing window-probe directory")?;
                        std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
                        let report = serde_json::json!({"status":if clean && disconnect==CloseOutcome::Acknowledged {"passed"} else {"failed"},
                                "scope":"local window probe; exact 40 Vim operations across main menu, board/favorite and article lists, including five switching cycles, no-redraw Home/count/enter and three continue prompts followed by dense ASCII-pointer article browsing; no extra station input",
                                "listeners_released":clean,"connection_worker_joined":true,"disconnect":disconnect,
                                "settings":preferences.app(),"vim_mode":keyboard_policy.status(&navigation::analyze(&receiver.borrow().frame))});
                        std::fs::write(path.join("window-probe.json"), report.to_string())
                            .map_err(|e| e.to_string())?;
                    }
                    if let Some(mut report) = pending_report.take() {
                        report["listeners_released"] = clean.into();
                        report["connection_worker_joined"] = true.into();
                        report["disconnect"] =
                            serde_json::to_value(disconnect).map_err(|e| e.to_string())?;
                        report["media_pending"] = media_preview.pending().into();
                        report["media_preview_open"] = media_preview.active().is_some().into();
                        report["media_external_requests"] = checks.media_external.into();
                        checks.augment_report(&mut report, complete);
                        if !clean || !probe_checked {
                            report["status"] = "failed".into();
                        }
                        if reload_settings {
                            report["scope"] =
                                "settings reload in a second application process; synthetic data only"
                                    .into();
                        }
                        let path = probe.as_ref().ok_or("missing probe directory")?;
                        std::fs::write(
                            path.join(if reload_settings {
                                "reload.json"
                            } else {
                                "probe.json"
                            }),
                            report.to_string(),
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    let window = dioxus::desktop::window();
                    window.set_close_behavior(dioxus::desktop::WindowCloseBehaviour::WindowCloses);
                    window.close();
                    return Ok(());
                }
            },

            changed = receiver.changed(), if !stopping => {
                changed.map_err(|e| e.to_string())?;
                let view = receiver.borrow_and_update().clone();
                let policy=filters.version();filters.sync(view.generation,view.frame.revision);
                if filters.version()!=policy {selected_rows=None;}
                let filtered=filters.project(view.generation,&view.frame);
                if hover_lease.as_mut().is_some_and(|lease|!lease.renew(view.stamp(),view.phase==Phase::Connected && view.frame.control_complete,&links::detect(&filtered.frame))) {
                    hover_lease=None;media_preview.close();
                    eval.send(serde_json::json!({"hover_reset":true})).map_err(|e|e.to_string())?;
                }
                if keyboard_generation!=view.generation {keyboard_policy.reset();keyboard_generation=view.generation;}
                keyboard_policy.context(&navigation::analyze(&view.frame));
                eval.send(serde_json::json!({"keyboard_state":keyboard_policy.status(&navigation::analyze(&view.frame))})).map_err(|e|e.to_string())?;
                if media_preview.active().is_some_and(|preview| preview.generation != view.generation) {
                    media_preview.close();
                    eval.send(serde_json::json!({"media_reset":true})).map_err(|e| e.to_string())?;
                }
                if view.phase != Phase::Connected || pending_paste.read().as_ref().is_some_and(|p| p.generation != view.generation) {
                    pending_paste.set(None);
                }
                phase.set(view.phase);
                if last_status != view.message {
                    last_status = view.message.clone(); message.set(view.message);
                }
                dirty = true;
            },
            changed = closing.changed() => {
                changed.map_err(|e| e.to_string())?;
                if *closing.borrow_and_update() && !stopping {
                    login_open=false;login_stamp=None;
                    connection.cancel_login()?;
                    media_preview.close();
                    connection.keep_alive_gate(receiver.borrow().stamp(),false);
                    eval.send(serde_json::json!({"stop":true})).map_err(|e| e.to_string())?;
                    stopping = true;
                    // Notify and join the network owner now, rather than leaving
                    // it alive while waiting for renderer cleanup acknowledgement.
                    message.set("正在中斷 PTT 連線並關閉視窗…".into());
                    let disconnect=connection.shutdown().await?;
                    eprintln!("PTTStella disconnect: {disconnect:?}");
                    close_deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(5));
                }
            },
            _ = async { match close_deadline { Some(deadline) => tokio::time::sleep_until(deadline).await, None => std::future::pending().await } } => {
                return Err("畫面監聽器清理逾時".into());
            },
        }
    }
}
