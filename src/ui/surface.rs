//! Serializes renderer events and coordinates their feature owners.
use base64::Engine;
use dioxus::prelude::*;
use pttstella::{
    connection::{CloseOutcome, Connection, Phase},
    input::{self, PreparedPaste},
    login::vault::{Action as LoginAction, Job as LoginJob, Outcome as LoginOutcome, Vault},
    media::{self, hover::Lease as HoverLease, preview::Presentation},
    navigation,
    settings::HoverPosition,
    vim::{self, Effect},
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::watch;

use super::protocol::{MediaAction, SurfaceEvent};
use super::{
    bridge::Bridge,
    commands::Kind as CommandKind,
    files::{self, Completion as FileCompletion, ProfileRequest},
    lifecycle::Resources,
    outbound::{
        ExportStatus, Feedback, FiltersOpen, FiltersResult, LinkStatus, LoginResult, MediaHover,
        MediaImage, MediaOpen, NavigationStatus, Packet, PasteStatus, SaveStatus, SelectionReply,
    },
    probe::Checks,
    surface_state::{Identity, SurfaceState},
};

/// Named launch options keep call sites independent of positional booleans.
pub(super) struct Options {
    pub(super) reload_settings: bool,
    pub(super) media_origin: Option<String>,
    pub(super) window_probe: bool,
    pub(super) login_probe: bool,
    pub(super) filters_probe: bool,
    pub(super) surface_probe: Option<String>,
}

#[derive(Clone)]
pub(super) struct PendingPaste {
    pub(super) generation: u64,
    pub(super) prepared: PreparedPaste,
}
struct Signals {
    message: Signal<String>,
    phase: Signal<Phase>,
    pending_paste: Signal<Option<PendingPaste>>,
}
pub(super) async fn surface(
    connection: Arc<Connection>,
    closing: watch::Receiver<bool>,
    probe: Option<PathBuf>,
    message: Signal<String>,
    phase: Signal<Phase>,
    mut pending_paste: Signal<Option<PendingPaste>>,
    options: Options,
) -> Result<(), String> {
    let mut resources = Resources::default();
    let reports = run(
        connection.clone(),
        closing,
        probe.clone(),
        Signals {
            message,
            phase,
            pending_paste,
        },
        options,
        &mut resources,
    )
    .await;
    pending_paste.set(None);
    // Every exit, including preflight, decoding and send errors, joins the same
    // owners. No branch can bypass renderer or credential cleanup.
    let cleanup = resources.finish(&connection, message).await;
    if let Some(path) = &probe {
        std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
        std::fs::write(
            path.join("cleanup.json"),
            serde_json::json!({
                "run_completed":reports.is_ok(), "cleanup_completed":cleanup.is_ok(),
                "disconnect":cleanup.as_ref().ok(),
            })
            .to_string(),
        )
        .map_err(|e| e.to_string())?;
    }
    let disconnect = match cleanup {
        Ok(outcome) => outcome,
        Err(cleanup) => {
            return Err(match reports {
                Ok(_) => cleanup,
                Err(error) => pttstella::msg!(
                    "cleanup-with-error",
                    error = error.to_string(),
                    cleanup = cleanup.to_string()
                ),
            });
        }
    };
    for (path, mut report) in reports? {
        report["listeners_released"] = true.into();
        report["connection_worker_joined"] = true.into();
        report["credential_worker_joined"] = true.into();
        report["file_worker_joined"] = (!resources.files.busy()).into();
        report["media_pending"] = resources.media.pending().into();
        report["media_preview_open"] = resources.media.active().is_some().into();
        report["disconnect"] = serde_json::to_value(disconnect).map_err(|e| e.to_string())?;
        if disconnect != CloseOutcome::Acknowledged {
            report["status"] = "failed".into();
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, report.to_string()).map_err(|e| e.to_string())?;
    }
    let window = dioxus::desktop::window();
    window.set_close_behavior(dioxus::desktop::WindowCloseBehaviour::WindowCloses);
    window.close();
    Ok(())
}

async fn run(
    connection: Arc<Connection>,
    mut closing: watch::Receiver<bool>,
    probe: Option<PathBuf>,
    signals: Signals,
    options: Options,
    resources: &mut Resources,
) -> Result<Vec<(PathBuf, serde_json::Value)>, String> {
    let Signals {
        mut message,
        mut phase,
        mut pending_paste,
    } = signals;
    let Options {
        reload_settings,
        media_origin,
        window_probe,
        login_probe,
        filters_probe,
        surface_probe,
    } = options;
    if *closing.borrow() {
        return Ok(Vec::new());
    }
    resources
        .files
        .start(|permit| files::load(permit, probe.clone()))?;
    let loaded = tokio::select! {
        result=resources.files.next()=>match result {FileCompletion::Loaded(result)=>result.map_err(|e|e.to_string())?,_=>return Err(pttstella::msg!("file-load-kind"))},
        changed=closing.changed()=>{changed.map_err(|e|e.to_string())?;return Ok(Vec::new());}
    };
    let files::Initial {
        mut preferences,
        filter_store,
        rules,
        mut filter_warning,
    } = *loaded;
    let mut screen = SurfaceState::new(rules);
    let mut receiver = connection.updates.clone();
    let Resources {
        eval: owned_eval,
        media: media_preview,
        login: login_job,
        commands,
        files,
    } = resources;
    let script = [
        "const fluent = (() => {const exports = {}; const module = {exports};\n",
        include_str!("../../assets/vendor/fluent/index.js"),
        "\nreturn module.exports;})();\n",
        include_str!("../../assets/i18n.js"),
        include_str!("../../assets/media.js"),
        include_str!("../../assets/surface-lifetime.js"),
        include_str!("../../assets/surface-packets.js"),
        include_str!("../../assets/surface-settings.js"),
        include_str!("../../assets/surface-login.js"),
        include_str!("../../assets/surface-filters.js"),
        include_str!("../../assets/surface.js"),
    ]
    .concat();
    let mut eval = Bridge::new(&script);
    *owned_eval = Some(eval.owner());
    let language = pttstella::i18n::application().language();
    eval.send(Packet::Localization {
        language: language.tag(),
        resource: language.resource(),
        fallback: pttstella::i18n::Language::TraditionalChinese.resource(),
    })
    .map_err(|e| e.to_string())?;
    let mut keyboard_policy = vim::Keyboard::default();
    let mut keyboard_generation = 0;

    let mut login_stamp: Option<navigation::Stamp> = None;
    let mut login_open = false;
    let first = receiver.borrow_and_update().clone();
    let mut last_status = first.message.clone();
    let painting = screen
        .queue(&first, false, preferences.display())
        .ok_or(pttstella::msg!("paint-missing-initial"))?;
    eval.send(Packet::Paint(painting))
        .map_err(|e| e.to_string())?;
    eval.send(Packet::HoverSettings(preferences.hover()))
        .map_err(|e| e.to_string())?;
    if let Some(warning) = preferences.warning() {
        eval.send(Packet::Notice(warning.into()))
            .map_err(|e| e.to_string())?;
    }
    if let Some(warning) = &filter_warning {
        eval.send(Packet::Notice(warning.into()))
            .map_err(|e| e.to_string())?;
    }
    connection.connect()?;
    let mut close_deadline = None;
    let mut checks = Checks::new();
    let mut pending_report: Option<serde_json::Value> = None;
    let mut surface_probe_started = false;
    let mut rejected_events = 0;
    let mut surface_probe_clean = false;
    let (complete, probe_checked) = loop {
        if *closing.borrow() {
            break (false, false);
        }
        let complete = probe.is_some() && {
            let view = receiver.borrow();
            screen.painted(&view)
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
            checks.probe_started && !screen.has_in_flight()
        } else {
            interactions_checked
        };
        if !surface_probe_started
            && surface_probe.is_some()
            && !screen.has_in_flight()
            && receiver.borrow().phase == Phase::Connected
            && receiver.borrow().received > 0
        {
            surface_probe_started = true;
            let case = surface_probe
                .as_deref()
                .ok_or("missing surface probe case")?;
            if case == "send" {
                eval.probe_send_failure().map_err(|e| e.to_string())?;
            }
            if case == "slow" {
                commands.start(CommandKind::Navigate, receiver.borrow().generation, || {
                    Ok(async {
                        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                        Err("synthetic delayed reply".into())
                    })
                })?;
            }
            if case == "files_slow" {
                let directory = probe.clone().ok_or("missing file probe directory")?;
                files.start(|permit| files::slow_probe(permit, directory))?;
            }
            eval.send(Packet::SurfaceProbe(case.into()))
                .map_err(|e| e.to_string())?;
        }
        if pending_report.is_some() && probe_checked {
            break (complete, probe_checked);
        }
        // One in-flight frame: slow font/paint work cannot grow the JS queue.
        if screen.dirty() && !screen.has_in_flight() {
            let view = receiver.borrow().clone();
            let run_probe = probe.is_some()
                && !window_probe
                && !login_probe
                && !filters_probe
                && surface_probe.is_none()
                && !checks.probe_started
                && view.phase == Phase::Connected
                && view.received > 0;
            checks.probe_started |= run_probe;
            if let Some(painting) = screen.queue(&view, run_probe, preferences.display()) {
                eval.send(Packet::Paint(painting))
                    .map_err(|e| e.to_string())?;
            }
        }
        tokio::select! {
            changed = closing.changed() => {
                changed.map_err(|e| e.to_string())?;
                if *closing.borrow_and_update() {
                    message.set(pttstella::msg!("closing"));
                    break (complete, probe_checked);
                }
            },
            done=files.next(), if !*closing.borrow()=>{
                let packet=match done {
                    FileCompletion::Probe(result)=>{pending_report=Some(result.map_err(|e|e.to_string())?);close_deadline=Some(tokio::time::Instant::now()+std::time::Duration::from_secs(45));continue;},
                    FileCompletion::Loaded(_)=>return Err(pttstella::msg!("file-load-duplicate")),
                    FileCompletion::Display(result)=>match result.map_err(|e|e.to_string()).and_then(|saved|preferences.commit(saved)) {
                        Ok(())=>{
                            checks.settings_saved |= probe.is_some() && preferences.display().font_size==18.0 && preferences.display().baseline==19.0;
                            screen.display_changed();
                            Packet::SettingsSaved(Feedback::new(SaveStatus::Saved,if preferences.app().language!=pttstella::i18n::application().language(){pttstella::msg!("settings-language-saved")}else{pttstella::msg!("settings-saved")}))
                        },
                        Err(error)=>Packet::SettingsSaved(Feedback::new(SaveStatus::Rejected,error)),
                    },
                    FileCompletion::Hover(result)=>{
                        if let Err(error)=result.map_err(|e|e.to_string()).and_then(|saved|preferences.commit(saved)) {
                            eval.send(Packet::Notice(error)).map_err(|e|e.to_string())?;
                        }
                        if preferences.hover()==HoverPosition::Off && screen.clear_hover() {
                            media_preview.close();eval.send(Packet::HoverReset{media:false}).map_err(|e|e.to_string())?;
                        }
                        Packet::HoverSettings(preferences.hover())
                    },
                    FileCompletion::Filters(result)=>match result {
                        Ok(rules)=>{
                            screen.replace_rules(rules);filter_warning=None;checks.filter_edits+=1;
                            media_preview.close();
                            eval.send(Packet::HoverReset{media:true}).map_err(|e|e.to_string())?;
                            Packet::FiltersResult(FiltersResult::Saved{entries:screen.rules().entries.clone(),message:pttstella::msg!("filters-saved")})
                        },
                        Err(error)=>Packet::FiltersResult(FiltersResult::Rejected{message:error.to_string()}),
                    },
                    FileCompletion::Profile(result)=>Packet::ProfileResult(result.unwrap_or_else(|error|super::outbound::ProfileResult::Rejected{notice:error.to_string()})),
                    FileCompletion::Export{selected,result}=>match result {
                        Ok(true)=>{checks.export_checked|=probe.is_some()&&!selected;checks.rows_export_checked|=probe.is_some()&&selected;Packet::ExportResult(Feedback::new(ExportStatus::Saved,pttstella::msg!("export-saved")))},
                        Ok(false)=>Packet::ExportResult(Feedback::new(ExportStatus::Cancelled,pttstella::msg!("export-cancelled"))),
                        Err(error)=>Packet::ExportResult(Feedback::new(ExportStatus::Failed,error.to_string())),
                    },
                };
                eval.send(packet).map_err(|e|e.to_string())?;
            },
            done = commands.next() => {
                if done.generation != receiver.borrow().generation {continue;}
                let packet = match done.kind {
                    CommandKind::Login => match done.result {
                        Ok(()) if login_open => {login_open=false;Packet::LoginResult(LoginResult::Sent)},
                        Ok(()) => continue,
                        Err(error) => Packet::LoginResult(LoginResult::Message{message:error}),
                    },
                    CommandKind::Navigate => match done.result {
                        Ok(()) => {if probe.is_some(){checks.navigation_requests+=1;}Packet::NavigationResult(Feedback::quiet(NavigationStatus::Sent))},
                        Err(error) => Packet::NavigationResult(Feedback::new(NavigationStatus::Rejected,error)),
                    },
                    CommandKind::Browse => match done.result {
                        Ok(()) => continue,
                        Err(error) => Packet::Notice(error),
                    },
                };
                eval.send(packet).map_err(|e|e.to_string())?;
            },
            result = async { match login_job.as_mut() {Some(job)=>job.wait().await,None=>std::future::pending().await} } => {
                *login_job=None;
                if !login_open {eval.send(Packet::LoginResult(LoginResult::Idle)).map_err(|e|e.to_string())?;continue;}
                let packet=match result {
                    Ok(LoginOutcome::Opened {account,saved})=>Packet::LoginResult(LoginResult::Opened{account,saved,message:if saved {pttstella::msg!("login-saved-help")} else {pttstella::msg!("login-unsaved-help")}}),
                    Ok(LoginOutcome::Forgotten)=>Packet::LoginResult(LoginResult::Opened{account:String::new(),saved:false,message:pttstella::msg!("login-forgotten")}),
                    Ok(LoginOutcome::Ready(credentials))=>{
                        let result=if let Some(stamp)=login_stamp.take() {
                            commands.start(CommandKind::Login,stamp.generation,|| connection.request_login(stamp,credentials).map(|reply|reply.wait()))
                        } else {Err(pttstella::msg!("login-request-cancelled"))};
                        match result {
                            Ok(())=>continue,
                            Err(error)=>Packet::LoginResult(LoginResult::Message{message:error}),
                        }
                    },
                    Err(error)=>Packet::LoginResult(LoginResult::Message{message:error}),
                };
                eval.send(packet).map_err(|e|e.to_string())?;
            },
            update = media_preview.update() => {
                if let Some(update) = update {
                    let reply = match update.result {
                        Ok((image,cached)) => {
                            Packet::MediaImage(MediaImage{request:update.request,
                                data:base64::engine::general_purpose::STANDARD.encode(&image.bytes),
                                mime:image.mime,width:image.width,height:image.height,
                                animated:image.animated_format,cached})
                        },
                        Err(error) => Packet::MediaError{request:update.request,message:error.to_string()},
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                }
            },
            event = eval.recv() => {
                let event = event.map_err(|e| e.to_string())?;
                if !event.admitted(receiver.borrow().generation) {
                    rejected_events += 1;
                    event.discard();
                    continue;
                }
                match event {
                    SurfaceEvent::SurfaceProbe { clean } => {
                        if !matches!(surface_probe.as_deref(), Some("untrusted" | "confirm")) {
                            return Err("unexpected surface probe result".into());
                        }
                        surface_probe_clean = clean;
                        break (complete, probe_checked);
                    }
                    SurfaceEvent::LoginOpen { trusted: _ } => {
                        if login_job.is_some() {
                            continue;
                        }
                        login_open = true;
                        login_stamp = None;
                        let vault = Vault::new(probe.is_some());
                        *login_job = Some(LoginJob::start(vault, LoginAction::Open)?);
                    }
                    SurfaceEvent::LoginSubmit {
                        account,
                        password,
                        remember,
                        stamp,
                        trusted: _,
                    } => {
                        let password = zeroize::Zeroizing::new(password.into_bytes());
                        if !login_open || login_job.is_some() {
                            continue;
                        }
                        login_stamp = Some(stamp);
                        let vault = Vault::new(probe.is_some());
                        *login_job = Some(LoginJob::start(
                            vault,
                            LoginAction::Submit {
                                account,
                                password,
                                remember,
                            },
                        )?);
                    }
                    SurfaceEvent::LoginForget { trusted: _ } => {
                        if !login_open || login_job.is_some() {
                            continue;
                        }
                        let vault = Vault::new(probe.is_some());
                        *login_job = Some(LoginJob::start(vault, LoginAction::Forget)?);
                    }
                    SurfaceEvent::LoginCancel { trusted: _ } => {
                        login_open = false;
                        login_stamp = None;
                        if commands.is(CommandKind::Login) {
                            commands.cancel();
                        }
                        if let Err(error) = connection.cancel_login() {
                            message.set(error);
                        }
                    }
                    SurfaceEvent::FiltersOpen { trusted: _ } => {
                        eval.send(Packet::FiltersOpen(FiltersOpen{entries:screen.rules().entries.clone(),warning:filter_warning.clone()})).map_err(|e|e.to_string())?;
                    }
                    SurfaceEvent::FiltersEdit {
                        id,
                        note,
                        remove,
                        trusted: _,
                    } => {
                        let rules=screen.rules().edit(&id,&note,remove);
                        if let Err(message)=files.start(|permit|files::save_filters(permit,filter_store.clone(),rules)) {
                            eval.send(Packet::FiltersResult(FiltersResult::Rejected{message})).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::FiltersReveal {
                        presentation,
                        generation,
                        revision,
                        reveal,
                        trusted: _,
                    } => {
                        let view = receiver.borrow();
                        if !screen.reveal(Identity{presentation,generation,revision},reveal,&view) {continue;}
                        checks.filter_reveals+=1;
                        media_preview.close();
                        eval.send(Packet::HoverReset{media:true})
                            .map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::HoverPreference { position, trusted: _ } => {
                        if let Err(error)=files.start(|permit|files::save_hover(permit,preferences.store(),preferences.hover_draft(position))) {
                            eval.send(Packet::Notice(error)).map_err(|e|e.to_string())?;
                            eval.send(Packet::HoverSettings(preferences.hover())).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::Hover {
                        presentation,
                        sequence,
                        stamp,
                        index,
                        trusted: _,
                    } => {
                        let Some(index) = index else {
                            if screen.cancel_hover(sequence) {media_preview.close();}
                            continue;
                        };
                        let view = receiver.borrow().clone();
                        let link=screen.current(Identity{presentation,generation:stamp.generation,revision:stamp.revision},&view)
                            .filter(|p|p.input_epoch==stamp.input_epoch).and_then(|p|p.links.get(index)).cloned();
                        let target = link.as_ref()
                            .and_then(|link| media::classify(&link.url).ok().flatten())
                            .filter(|target| target.kind == media::Kind::Image);
                        if preferences.hover() == HoverPosition::Off
                            || view.stamp() != stamp
                            || view.phase != Phase::Connected
                            || !view.frame.control_complete
                            || target.is_none()
                            || media_preview
                                .active()
                                .is_some_and(|active| active.presentation == Presentation::Inspect)
                        {
                            eval.send(Packet::HoverRejected(sequence))
                                .map_err(|e| e.to_string())?;
                            continue;
                        }
                        if let (Some(link), Some(target)) = (link, target) {
                            if filters_probe {
                                checks.filter_hover_requests += 1;
                                eval.send(Packet::HoverRejected(sequence))
                                    .map_err(|e| e.to_string())?;
                                continue;
                            }
                            let active = media_preview.hover(stamp.generation, target)?;
                            screen.begin_hover(HoverLease {
                                sequence,
                                stamp,
                                index,
                                link,
                            });
                            eval.send(Packet::MediaHover(MediaHover{sequence,request:active.request,target:active.target.clone(),position:preferences.hover()})).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::KeyboardState {
                        action,
                        code,
                        ..
                    } => {
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
                        eval.send(Packet::KeyboardState(keyboard_policy.status(&context)))
                            .map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::MediaDecoded { request, success } => {
                        media_preview.decoded(request, success);
                    }
                    SurfaceEvent::KeepAlive { enabled, trusted: _ } => {
                        if let Err(error) = connection.keep_alive(enabled) {
                            eval.send(Packet::Notice(error))
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    SurfaceEvent::KeepAliveGate { stamp, ready } => {
                        connection.keep_alive_gate(
                            stamp,
                            ready
                                && !keyboard_policy
                                    .status(&navigation::analyze(&receiver.borrow().frame))
                                    .pending,
                        );
                    }
                    SurfaceEvent::Navigate {
                        stamp,
                        intent,
                        trusted: _,
                    } => {
                        let shown=screen.navigation_shown(stamp);
                        let result = if !shown {
                            Err(navigation::Error::Stale.to_string())
                        } else if pending_paste.read().is_some() {
                            Err(navigation::Error::Unavailable.to_string())
                        } else {
                            commands.start(CommandKind::Navigate, stamp.generation, || {
                                connection.request_navigate(stamp, intent).map(|reply|reply.wait())
                            })
                        };
                        if let Err(error) = result {
                            eval.send(Packet::NavigationResult(Feedback::new(NavigationStatus::Rejected,error)))
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    SurfaceEvent::SettingsOpen { trusted: _ } => {
                        eval.send(preferences.open_packet())
                            .map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::SettingsPreview { request, values } => {
                        eval.send(preferences.preview_packet(request, values))
                            .map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::SettingsSave { values, language, trusted: _ } => {
                        if let Err(error)=files.start(|permit|files::save_display(permit,preferences.store(),{let mut draft=preferences.display_draft(values);draft.language=language;draft})) {
                            eval.send(Packet::SettingsSaved(Feedback::new(SaveStatus::Rejected,error))).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::SettingsProfile {
                        action,
                        name,
                        trusted: _,
                    } => {
                        let request=match action.as_str() {
                            "import"=>Some(ProfileRequest::Import),
                            "export"=>Some(ProfileRequest::Export{name,display:preferences.app().display.clone()}),
                            _=>None,
                        };
                        let result=request.ok_or_else(||pttstella::msg!("profile-unknown-action").to_owned()).and_then(|request|files.start(|permit|files::profile(permit,request,probe.clone().filter(|_|surface_probe.as_deref()!=Some("files")))));
                        if let Err(notice)=result {
                            eval.send(Packet::ProfileResult(super::outbound::ProfileResult::Rejected{notice})).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::ExportPng {
                        presentation,
                        generation,
                        revision,
                        selection_request,
                        png,
                        trusted: _,
                    } => {
                        let result = (|| -> Result<(),String> {
                            let rows=screen.export_rows(Identity{presentation,generation,revision},selection_request,&receiver.borrow())?;
                            files.start(|permit|{
                                let lease=screen.lease_export(Identity{presentation,generation,revision},selection_request,receiver.borrow().input_epoch);
                                let request=files::ExportRequest{lease,source:receiver.clone(),png,rows,selected:selection_request.is_some(),probe:probe.clone().filter(|_|surface_probe.as_deref()!=Some("files"))};
                                files::export_png(permit,request)
                            })
                        })();
                        if let Err(error)=result {
                            eval.send(Packet::ExportResult(Feedback::new(ExportStatus::Failed,error))).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::OpenLink {
                        presentation,
                        generation,
                        revision,
                        index,
                        trusted: _,
                    } => {
                        let link=screen.current(Identity{presentation,generation,revision},&receiver.borrow()).and_then(|p|p.links.get(index)).cloned();
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
                                    screen.clear_hover();
                                    eval.send(Packet::MediaOpen(MediaOpen{request:active.request,target:active.target.clone()})).map_err(|e| e.to_string())?;
                                    continue;
                                }
                                Err(error) => {
                                    eval.send(Packet::LinkResult(Feedback::new(LinkStatus::Failed,error)))
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
                                webbrowser::open(&link.url).map_err(|e| pttstella::msg!("link-open-failed", e = e.to_string()))
                            }
                            None => Err(pttstella::msg!("link-stale").to_owned()),
                        };
                        let reply = match result {
                            Ok(()) => Packet::LinkResult(Feedback::quiet(LinkStatus::Requested)),
                            Err(error) => Packet::LinkResult(Feedback::new(LinkStatus::Failed,error)),
                        };
                        eval.send(reply).map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::Media {
                        request,
                        action,
                        trusted: _,
                    } => {
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
                                        .map_err(|e| pttstella::msg!("link-open-failed", e = e.to_string()))
                                };
                                eval.send(Packet::MediaExternal{request,ok:result.is_ok(),message:result.err()}).map_err(|e| e.to_string())?;
                            }
                        }
                    }
                    SurfaceEvent::Paste {
                        text,
                        generation,
                        trusted: _,
                    } => {
                        let result = input::prepare_paste(&text)
                            .map_err(|e| e.to_string())
                            .and_then(|prepared| {
                                let current = receiver.borrow();
                                if current.phase != Phase::Connected || current.generation != generation {
                                    return Err(pttstella::msg!("paste-stale"));
                                }
                                if prepared.lines > 1 {
                                    pending_paste.set(Some(PendingPaste {
                                        generation,
                                        prepared,
                                    }));
                                    Ok(false)
                                } else {
                                    checks.paste_single |=
                                        probe.is_some() && prepared.text == "中";
                                    connection.input(generation, prepared.bytes).map(|()| true)
                                }
                            });
                        let reply = match result {
                            Ok(queued) => {
                                Packet::PasteResult(Feedback::quiet(if queued{PasteStatus::Queued}else{PasteStatus::Preview}))
                            }
                            Err(error) => {
                                if probe.is_some()  {
                                    checks.paste_rejected += 1;
                                }
                                Packet::PasteResult(Feedback::new(PasteStatus::Rejected,error))
                            }
                        };
                        eval.send(reply).map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::PasteCancel { trusted: _ } => {
                        checks.paste_cancel |= probe.is_some()

                            && pending_paste
                                .read()
                                .as_ref()
                                .is_some_and(|p| p.prepared.text == "A\nB");
                        pending_paste.set(None);
                        eval.send(Packet::PasteResult(Feedback::quiet(PasteStatus::Cancelled)))
                            .map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::PasteConfirm { trusted: _ } => {
                        let pending = pending_paste.write().take();
                        if let Some(pending) = pending {
                            let current = receiver.borrow();
                            let result = if current.phase == Phase::Connected
                                && current.generation == pending.generation
                            {
                                checks.paste_confirm |=
                                    probe.is_some() && pending.prepared.text == "A\nB";
                                connection.input(pending.generation, pending.prepared.bytes)
                            } else {
                                Err(pttstella::msg!("paste-stale").to_owned())
                            };
                            let reply = match result {
                                Ok(()) => Packet::PasteResult(Feedback::quiet(PasteStatus::Queued)),
                                Err(error) => Packet::PasteResult(Feedback::new(PasteStatus::Rejected,error)),
                            };
                            eval.send(reply).map_err(|e| e.to_string())?;
                        } else {
                            eval.send(Packet::PasteResult(Feedback::quiet(PasteStatus::Cancelled)))
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
                        let selected=screen.select(Identity{presentation,generation,revision},request,&selection,&receiver.borrow());
                        if probe.is_some() && selected.as_ref().is_some_and(|s| s.text == "中") {
                            checks.copied_selection = Some((generation, revision, request));
                        }
                        eval.send(Packet::Selection(SelectionReply{presentation,generation,revision,request,value:selected})).map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::SelectionCleared {presentation,generation,revision,request}=>{
                        screen.clear_selection(Identity{presentation,generation,revision},request);
                    },
                    SurfaceEvent::Copied {
                        presentation,
                        generation,
                        revision,
                        request,
                        trusted: _,
                    } => {
                        if filters_probe && screen.copied(Identity{presentation,generation,revision},request,&receiver.borrow()) {
                            checks.filter_copies += 1;
                        }
                        checks.copy_checked |=
                            checks.copied_selection == Some((generation, revision, request));
                    }
                    SurfaceEvent::Key {
                        key,
                        generation,
                        trusted: _,
                    } => {
                        let view = receiver.borrow().clone();
                        let context = navigation::analyze(&view.frame);
                        let effect = keyboard_policy.key(&key, &context);
                        eval.send(Packet::KeyboardState(keyboard_policy.status(&context)))
                            .map_err(|e| e.to_string())?;
                        let bytes = match effect {
                            Effect::None => continue,
                            Effect::Notice(notice) => {
                                eval.send(Packet::Notice(notice.into()))
                                    .map_err(|e| e.to_string())?;
                                continue;
                            }
                            Effect::Browse(intent) => {
                                let shown=screen.browse_shown(generation,&context);
                                let result = if !shown || pending_paste.read().is_some() {
                                    Err(navigation::Error::Stale.to_string())
                                } else {
                                    commands.start(CommandKind::Browse, generation, || {
                                        connection.request_browse_key(view.stamp(), intent).map(|reply|reply.wait())
                                    })
                                };
                                if let Err(error) = result {
                                    eval.send(Packet::Notice(error))
                                        .map_err(|e| e.to_string())?;
                                }
                                continue;
                            }
                            Effect::Native(bytes) => bytes,
                        };
                        if probe.is_some() {
                            let current = receiver.borrow().generation;
                            checks.arrow_checked |=
                                generation == current && bytes == [27, 91, 65];
                            if generation == current && bytes == [13] {
                                checks.enter_count += 1;
                            }
                            checks.closed_checked |= generation == current
                                && bytes == [27, 91, 67]
                                && receiver.borrow().phase == Phase::Disconnected;
                        }
                        message.set(match connection.input(generation, bytes) {
                            Ok(()) => receiver.borrow().message.clone(),
                            Err(error) => {
                                eval.send(Packet::InputRejected(error.clone()))
                                    .map_err(|e| e.to_string())?;
                                error
                            }
                        });
                    }
                    SurfaceEvent::Text {
                        text,
                        generation,
                        trusted: _,
                        composition_input,
                        composed,
                    } => {
                        if !keyboard_policy.accept_text(composed)
                        {
                            continue;
                        }
                        eval.send(Packet::KeyboardState(keyboard_policy.status(&navigation::analyze(&receiver.borrow().frame)))).map_err(|e|e.to_string())?;
                        let vim_input = keyboard_policy
                            .status(&navigation::analyze(&receiver.borrow().frame))
                            .enabled;
                        if probe.is_some()
                            && !vim_input
                            && checks.input_bytes.len() >= checks.expected_input.len()
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
                                eval.send(Packet::InputRejected(error.clone()))
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
                        screen.acknowledge(Identity{presentation,generation,revision})?;
                    }
                    SurfaceEvent::Error { message } => return Err(message),
                    SurfaceEvent::Probe(pixels) => {
                        let path=probe.clone().ok_or("unexpected probe response")?;
                        let settings=preferences.app().clone();
                        files.start(|permit|files::probe_report(permit,*pixels,path,reload_settings,settings))?;
                    }
                    SurfaceEvent::Stopped { clean } => {
                        if !clean {
                            return Err(pttstella::msg!("cleanup-listeners-failed"));
                        }
                        break (complete, probe_checked);
                    }
                }
            },

            changed = receiver.changed() => {
                changed.map_err(|e| e.to_string())?;
                let view = receiver.borrow_and_update().clone();
                if screen.update(&view) {
                    media_preview.close();
                    eval.send(Packet::HoverReset{media:false}).map_err(|e|e.to_string())?;
                }
                if keyboard_generation!=view.generation || view.phase!=Phase::Connected {
                    if commands.busy() {
                        commands.cancel();
                        eval.send(Packet::NavigationResult(Feedback::quiet(NavigationStatus::Rejected))).map_err(|e|e.to_string())?;
                        eval.send(Packet::LoginResult(LoginResult::Message{message:pttstella::msg!("command-stale")})).map_err(|e|e.to_string())?;
                    }
                    keyboard_policy.reset();keyboard_generation=view.generation;
                }
                keyboard_policy.context(&navigation::analyze(&view.frame));
                eval.send(Packet::KeyboardState(keyboard_policy.status(&navigation::analyze(&view.frame)))).map_err(|e|e.to_string())?;
                if media_preview.active().is_some_and(|preview| preview.generation != view.generation) {
                    media_preview.close();
                    eval.send(Packet::MediaReset).map_err(|e| e.to_string())?;
                }
                if view.phase != Phase::Connected || pending_paste.read().as_ref().is_some_and(|p| p.generation != view.generation) {
                    pending_paste.set(None);
                }
                phase.set(view.phase);
                if last_status != view.message {
                    last_status = view.message.clone(); message.set(view.message);
                }
            },
            _ = async { match close_deadline { Some(deadline) => tokio::time::sleep_until(deadline).await, None => std::future::pending().await } } => {
                return Err(pttstella::msg!("probe-timeout"));
            },
        }
    };
    let mut reports = Vec::new();
    if let (Some(case), Some(path)) = (surface_probe, probe.as_ref()) {
        reports.push((path.join("surface-probe.json"),serde_json::json!({
            "status":if case=="slow" && commands.busy() || case=="files_slow" && files.busy() || case=="files" && checks.export_checked || case=="untrusted" && surface_probe_clean && rejected_events==3
                || case=="confirm" && surface_probe_clean && rejected_events==1 && pending_paste.read().as_ref().is_some_and(|p|p.prepared.text=="A\nB")
                || case=="dialogs" && checks.settings_saved && checks.export_checked && checks.rows_export_checked && checks.media_external>=1 {"passed"} else {"failed"},
            "scope":"synthetic local surface only; peer rejects all station input",
            "case":case,"command_pending_on_close":commands.busy(),"file_pending_on_close":files.busy(),"synthetic_events_rejected":rejected_events,"js_admission":surface_probe_clean,
            "paste_preview_preserved":pending_paste.read().is_some(),
            "settings_saved":checks.settings_saved,"full_png":checks.export_checked,"rows_png":checks.rows_export_checked,"media_external_requests":checks.media_external,
        })));
    }
    if login_probe {
        let path = probe.as_ref().ok_or("missing login-probe directory")?;
        reports.push((path.join("login-probe.json"),serde_json::json!({"status":"passed","scope":"synthetic login only; peer checked one account and password, no extra input"})));
    }
    if filters_probe {
        let path = probe.as_ref().ok_or("missing filters probe directory")?;
        reports.push((path.join("filters-probe.json"),serde_json::json!({"status":if checks.filter_edits>=4 && checks.filter_reveals>=2 && checks.filter_copies>=1 && checks.filter_hover_requests==0 && checks.export_checked && checks.rows_export_checked {"passed"} else {"failed"},"scope":"synthetic local push masking and explicit CRUD, reveal, copy and PNG; no extra station bytes","filter_edits":checks.filter_edits,"filter_reveals":checks.filter_reveals,"copies":checks.filter_copies,"hover_requests":checks.filter_hover_requests,"full_png":checks.export_checked,"rows_png":checks.rows_export_checked})));
    }
    if window_probe {
        let path = probe.as_ref().ok_or("missing window-probe directory")?;
        let report = serde_json::json!({"status":"passed",
                                "scope":"local window probe; exact 40 Vim operations across main menu, board/favorite and article lists, including five switching cycles, no-redraw Home/count/enter and three continue prompts followed by dense ASCII-pointer article browsing; no extra station input",

                                "settings":preferences.app(),"vim_mode":keyboard_policy.status(&navigation::analyze(&receiver.borrow().frame))});
        reports.push((path.join("window-probe.json"), report));
    }
    if let Some(mut report) = pending_report.take() {
        report["media_external_requests"] = checks.media_external.into();
        checks.augment_report(&mut report, complete);
        if !probe_checked {
            report["status"] = "failed".into();
        }
        if reload_settings {
            report["scope"] =
                "settings reload in a second application process; synthetic data only".into();
        }
        let path = probe.as_ref().ok_or("missing probe directory")?;
        reports.push((
            path.join(if reload_settings {
                "reload.json"
            } else {
                "probe.json"
            }),
            report,
        ));
    }
    Ok(reports)
}
