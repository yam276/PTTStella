//! Serializes renderer events and coordinates their feature owners.
use base64::Engine;
use dioxus::prelude::*;
use pttstella::{
    connection::{Connection, Phase},
    input::{self, PreparedPaste},
    login::vault::{Action as LoginAction, Job as LoginJob, Outcome as LoginOutcome, Vault},
    media::{self, hover::Lease as HoverLease, preview::Presentation},
    navigation, selection,
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
        DestinationStatus, ExportStatus, Feedback, FiltersOpen, FiltersResult, LinkStatus,
        LoginResult, MediaHover, MediaImage, MediaOpen, NavigationStatus, Packet, PasteStatus,
        SaveStatus, SelectionReply,
    },
    probe::Checks,
    surface_state::{Identity, SurfaceState},
};

/// Named launch options keep call sites independent of positional booleans.
pub(super) struct Options {
    pub(super) locale: Signal<pttstella::i18n::Language>,
    pub(super) settings_requests: watch::Receiver<(u64, bool)>,
    #[cfg(target_os = "macos")]
    pub(super) native_menu: crate::macos::NativeMenu,
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
    let main_window = dioxus::desktop::window();
    let mut resources = Resources::default();
    let mut reports = run(
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
    // Persist collected probe observations even when cleanup or its strict peer
    // fails. Such a report must never retain a passing status or claim joins.
    if let Ok(reports) = &mut reports {
        for (path, report) in reports {
            super::probe::finish_report(report, cleanup.as_ref().ok().copied());
            report["file_worker_joined"] =
                cleanup.is_ok().then_some(!resources.files.busy()).into();
            report["media_pending"] = resources.media.pending().into();
            report["media_preview_open"] = resources.media.active().is_some().into();
            if report.get("case").is_some() {
                report["tools"] = serde_json::json!(resources.tools);
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(path, report.to_string()).map_err(|e| e.to_string())?;
        }
    }
    match cleanup {
        Ok(_) => {}
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
    }
    reports?;
    let window = main_window;
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
        mut locale,
        mut settings_requests,
        #[cfg(target_os = "macos")]
        native_menu,
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
        mut sites,
        mut bookmarks,
        mut recent,
        mut preferences,
        filter_store,
        rules,
        mut filter_warning,
    } = *loaded;
    let initial_recent = recent.packet().boards;
    let initial_bookmarks = bookmarks.snapshot();
    let mut recent_clears = 0usize;
    let mut screen = SurfaceState::new(rules);
    let mut filter_import = super::filter_import::Import::default();
    let mut receiver = connection.updates.clone();
    let Resources {
        eval: owned_eval,
        media: media_preview,
        inline: inline_images,
        login: login_job,
        commands,
        files,
        tools: _,
        settings,
    } = resources;
    let script = [
        "const fluent = (() => {const exports = {}; const module = {exports};\n",
        include_str!("../../assets/vendor/fluent/index.js"),
        "\nreturn module.exports;})();\n",
        include_str!("../../assets/i18n.js"),
        include_str!("../../assets/gif-playback.js"),
        include_str!("../../assets/media.js"),
        include_str!("../../assets/inline-images.js"),
        include_str!("../../assets/surface-lifetime.js"),
        include_str!("../../assets/surface-packets.js"),
        include_str!("../../assets/surface-fonts.js"),
        include_str!("../../assets/surface-renderer.js"),
        include_str!("../../assets/settings-catalog.js"),
        include_str!("../../assets/surface-settings.js"),
        include_str!("../../assets/surface-settings-host.js"),
        include_str!("../../assets/surface-sites.js"),
        include_str!("../../assets/surface-login.js"),
        include_str!("../../assets/surface-navigation.js"),
        include_str!("../../assets/surface-bookmarks.js"),
        include_str!("../../assets/surface-filters.js"),
        include_str!("../../assets/surface-mouse.js"),
        include_str!("../../assets/surface-tools.js"),
        include_str!("../../assets/surface-context-menu.js"),
        include_str!("../../assets/surface.js"),
    ]
    .concat();
    let mut eval = Bridge::new(&script, settings);
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
    let mut login_guidance = pttstella::login::Guidance::Idle;

    let mut login_stamp: Option<navigation::Stamp> = None;
    let mut destination_request: Option<(u64, navigation::Stamp)> = None;
    let mut login_open = false;
    let mut settings_visible = false;
    let mut pending_settings: Option<super::preferences::Change> = None;
    screen.set_article_boundary_navigation(preferences.app().article_boundary_navigation);
    screen.set_article_mode(if window_probe || filters_probe {
        pttstella::settings::ArticleMode::Terminal
    } else {
        preferences.app().article_mode
    });
    let first = receiver.borrow_and_update().clone();
    let mut last_status = first.message.clone();
    let painting = screen
        .queue(&first, false, preferences.display())
        .ok_or(pttstella::msg!("paint-missing-initial"))?;
    eval.send(Packet::Paint(painting))
        .map_err(|e| e.to_string())?;
    eval.send(Packet::ToolsOpacity(preferences.app().tools_opacity))
        .map_err(|e| e.to_string())?;
    eval.send(Packet::ToolsAccess(preferences.app().tools_access))
        .map_err(|e| e.to_string())?;
    eval.send(Packet::HoverSettings(preferences.hover()))
        .map_err(|e| e.to_string())?;
    eval.send(Packet::RecentBoards(recent.packet()))
        .map_err(|e| e.to_string())?;
    if let Some(warning) = sites.warning() {
        eval.send(Packet::Notice(warning.into()))
            .map_err(|e| e.to_string())?;
    }
    if let Some(warning) = preferences.warning() {
        eval.send(Packet::Notice(warning.into()))
            .map_err(|e| e.to_string())?;
    }
    if let Some(warning) = &filter_warning {
        eval.send(Packet::Notice(warning.into()))
            .map_err(|e| e.to_string())?;
    }
    let sites_restart = surface_probe.as_deref() == Some("sites_restart");
    if sites_restart {
        let target = sites.default_target();
        let url = url::Url::parse(target.endpoint.as_str()).map_err(|e| e.to_string())?;
        if url.scheme() != "telnet" || url.host_str() != Some("127.0.0.1") {
            return Err(pttstella::msg!("sites-probe-local"));
        }
        connection.connect_to(target)?;
    } else if probe.is_some() {
        connection.connect()?;
    } else {
        connection.connect_to(sites.default_target())?;
    }
    let mut close_deadline = None;
    let mut checks = Checks::new();
    if sites_restart {
        checks.site_switches = 1;
    }
    let mut pending_report: Option<serde_json::Value> = None;
    let mut surface_probe_started = false;
    let mut rejected_events = 0;
    let mut surface_probe_clean = false;
    let mut reading_gate = None;
    let mut reading_paint_at = tokio::time::Instant::now();
    let mut reading_timing = super::reading_timing::ReadingTiming::new()?;
    let (complete, probe_checked) = loop {
        if *closing.borrow() {
            break (false, false);
        }
        screen.set_article_boundary_navigation(preferences.app().article_boundary_navigation);
        if screen.set_article_mode(if window_probe || filters_probe {
            pttstella::settings::ArticleMode::Terminal
        } else {
            preferences.app().article_mode
        }) {
            media_preview.close();
            eval.send(Packet::HoverReset { media: true })
                .map_err(|e| e.to_string())?;
            inline_images.close();
            eval.send(Packet::InlineReset(true))
                .map_err(|e| e.to_string())?;
        }
        let key_status = keyboard_policy.status(&receiver.borrow().context());
        let raw_reading = (key_status.enabled && matches!(key_status.mode, "input" | "once"))
            || (!key_status.enabled && key_status.pending);
        if screen.set_raw_reading(raw_reading) && raw_reading {
            inline_images.close();
            eval.send(Packet::InlineReset(true))
                .map_err(|e| e.to_string())?;
        }
        if commands.is(CommandKind::Preload) && !screen.full_load_active() {
            commands.cancel();
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
        if !commands.busy()
            && !settings_visible
            && !login_open
            && pending_paste.read().is_none()
            && reading_gate.is_some()
            && reading_gate == screen.reading_scope()
            && media_preview.active().is_none()
        {
            let view = receiver.borrow().clone();
            let preload = screen.full_load_active();
            let intent = if preload {
                screen.next_full_page(&view)
            } else {
                screen.continue_article_fetch(&view)
            };
            if let Some(intent) = intent {
                reading_timing.record(
                    "request",
                    view.generation,
                    view.frame.revision,
                    screen.reading_progress(),
                )?;
                let result = commands.start(
                    if preload {
                        CommandKind::Preload
                    } else {
                        CommandKind::Navigate
                    },
                    view.generation,
                    || {
                        connection
                            .request_navigate(view.stamp(), intent, navigation::Source::Button)
                            .map(|reply| reply.wait())
                    },
                );
                if let Err(error) = result {
                    screen.cancel_article_fetch();
                    if preload {
                        screen.stop_full_load();
                    }
                    eval.send(Packet::NavigationResult(Feedback::new(
                        NavigationStatus::Rejected,
                        error,
                    )))
                    .map_err(|e| e.to_string())?;
                } else if let Ok(bytes) = navigation::plan(&view.context(), intent) {
                    screen.article_input(&view.context(), &bytes);
                    if preload && intent == navigation::Intent::First {
                        // Restarting from the head changes display-row ownership
                        // when the initial station page contained wrapped lines.
                        inline_images.close();
                        eval.send(Packet::InlineReset(true))
                            .map_err(|e| e.to_string())?;
                    } else {
                        inline_images.input(&view.context(), &bytes);
                    }
                }
            }
        }
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
        // Autosave retains one authorized edit while an interactive file operation
        // finishes. Apply against the latest committed state only when admitted.
        if !files.busy()
            && let Some(change) = pending_settings.take()
        {
            let result = change.apply(preferences.app().clone()).and_then(|draft| {
                files.start(|permit| files::save_display(permit, preferences.store(), draft))
            });
            if let Err(error) = result {
                eval.send(Packet::SettingsSaved(Feedback::new(
                    SaveStatus::Rejected,
                    error,
                )))
                .map_err(|e| e.to_string())?;
            }
        }
        // One in-flight frame: slow font/paint work cannot grow the JS queue.
        let preload_paint = screen.full_load_active() && screen.reading_progress().is_some();
        let reading_paint_due = tokio::time::Instant::now() >= reading_paint_at;
        if screen.dirty()
            && !screen.has_in_flight()
            && (!preload_paint
                || (reading_paint_due
                    && !receiver.borrow().input_pending
                    && !receiver.borrow().output_pending))
        {
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
                reading_paint_at =
                    tokio::time::Instant::now() + std::time::Duration::from_millis(250);
            }
        }
        {
            let view = receiver.borrow();
            reading_timing.record(
                "cached",
                view.generation,
                view.frame.revision,
                screen.reading_progress(),
            )?;
            if screen.has_in_flight() {
                reading_timing.record(
                    "paint_pending",
                    view.generation,
                    view.frame.revision,
                    screen.reading_progress(),
                )?;
            }
        }
        #[cfg(target_os = "macos")]
        native_menu.settings_visible(settings_visible);
        let load_deadline = screen.full_load_deadline();
        let paint_deadline =
            (preload_paint && screen.dirty() && !screen.has_in_flight() && !reading_paint_due)
                .then_some(reading_paint_at);
        tokio::select! {
            _ = async {if let Some(at)=paint_deadline {tokio::time::sleep_until(at).await;}else{std::future::pending::<()>().await;}} => {},
            _ = async {if let Some(at)=load_deadline {tokio::time::sleep_until(at).await;}else{std::future::pending::<()>().await;}} => {
                screen.expire_full_load(tokio::time::Instant::now());
                if commands.is(CommandKind::Preload) {commands.cancel();}
            },
            request=settings_requests.changed()=>{
                request.map_err(|e|e.to_string())?;
                let close=settings_requests.borrow_and_update().1;
                if close {eval.send(Packet::SettingsClose(true)).map_err(|e|e.to_string())?;continue;}
                settings_visible=true;connection.keep_alive_gate(receiver.borrow().stamp(),false);eval.send(Packet::SettingsVisibility(true)).map_err(|e|e.to_string())?;
                eval.open_settings(preferences.store());eval.send(preferences.open_packet()).map_err(|e|e.to_string())?;
            },
            changed = closing.changed() => {
                changed.map_err(|e| e.to_string())?;
                if *closing.borrow_and_update() {
                    message.set(pttstella::msg!("closing"));
                    break (complete, probe_checked);
                }
            },
            done=files.next(), if !*closing.borrow()=>{
                let packet=match done {
                    FileCompletion::Sites {request,action,result}=>{
                        if result.is_ok(){checks.site_saves=checks.site_saves.saturating_add(1);if matches!(action,super::sites::Edit::Remove){checks.site_removes=checks.site_removes.saturating_add(1);}}else{checks.site_rejections=checks.site_rejections.saturating_add(1);}
                        let feedback=sites.complete(result.map_err(|e|e.to_string()));
                        Packet::SitesResult{request,data:sites.snapshot(receiver.borrow().site.clone()),feedback}
                    },
                    FileCompletion::Bookmarks {request,result}=>{
                        if result.is_ok() {checks.bookmark_saves=checks.bookmark_saves.saturating_add(1);} else {checks.bookmark_rejected=checks.bookmark_rejected.saturating_add(1);}
                        let feedback=bookmarks.complete(result.map_err(|e|e.to_string()));
                        Packet::BookmarksResult {request, data:bookmarks.snapshot(), feedback}
                    },
                    FileCompletion::Clipboard(result)=>Packet::Notice(result.map(|_|{checks.context_copies+=1;pttstella::msg!("context-copied")}).unwrap_or_else(|e|if e==files::Error::Stale {pttstella::msg!("copy-not-ready")}else{e.to_string()})),
                    FileCompletion::Recent(done)=>{recent.complete(done);Packet::RecentBoards(recent.packet())},
                    FileCompletion::Probe(result)=>{pending_report=Some(result.map_err(|e|e.to_string())?);close_deadline=Some(tokio::time::Instant::now()+std::time::Duration::from_secs(45));continue;},
                    FileCompletion::Fonts{request,result}=>{
                        if result.is_ok(){checks.font_catalog_reads+=1;}
                        let catalog=result.unwrap_or_else(|error|crate::fonts::Catalog{families:Vec::new(),notice:Some(error.to_string())});
                        Packet::FontCatalog{request,families:catalog.families,notice:catalog.notice}
                    },
                    FileCompletion::Loaded(_)=>return Err(pttstella::msg!("file-load-duplicate")),
                    FileCompletion::Display(result)=>match result.map_err(|e|e.to_string()).and_then(|saved|preferences.commit(saved)) {
                        Ok(())=>{
                            checks.settings_saved |= probe.is_some() && preferences.display().font_size==18.0 && preferences.display().baseline==19.0;
                            screen.display_changed();
                            eval.send(Packet::ToolsOpacity(preferences.app().tools_opacity)).map_err(|e|e.to_string())?;
                            eval.send(Packet::ToolsAccess(preferences.app().tools_access)).map_err(|e|e.to_string())?;
                            eval.send(Packet::HoverSettings(preferences.hover())).map_err(|e|e.to_string())?;
                            let language=preferences.app().language;
                            if language!=pttstella::i18n::application().language() {
                                let previous=pttstella::i18n::application().language();
                                pttstella::i18n::application().select(language);
                                let updated={let current=message.peek();pttstella::i18n::application().relocalize_status(previous,current.as_str())};
                                if let Some(updated)=updated {message.set(updated);}
                                locale.set(language);
                                eval.send(Packet::Localization {language:language.tag(),resource:language.resource(),fallback:pttstella::i18n::Language::TraditionalChinese.resource()}).map_err(|e|e.to_string())?;
                            }
                            Packet::SettingsSaved(Feedback::new(SaveStatus::Saved,pttstella::msg!("settings-saved")))
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
                    FileCompletion::FilterImport{request,result}=>{
                        if !filter_import.accepts(request) {continue;}
                        match result {
                            Ok(Some(parsed))=>match filter_import.preview(request,screen.rules(),&parsed.entries) {
                                Ok(rules)=>Packet::FiltersResult(FiltersResult::Preview{request,entries:rules.entries,message:pttstella::msg!("filters-import-preview", count=parsed.entries.len(), skipped=parsed.skipped, adjusted=parsed.adjusted)}),
                                Err(message)=>{filter_import.cancel(request);Packet::FiltersResult(FiltersResult::ImportRejected{request,message})},
                            },
                            Ok(None)=>{filter_import.cancel(request);Packet::FiltersResult(FiltersResult::ImportRejected{request,message:pttstella::msg!("filters-import-cancelled")})},
                            Err(error)=>{filter_import.cancel(request);Packet::FiltersResult(FiltersResult::ImportRejected{request,message:error.to_string()})},
                        }
                    },
                    FileCompletion::Filters(result)=>{
                        eval.send(Packet::Notice(match &result { Ok(_) => pttstella::msg!("filters-saved"), Err(error) => error.to_string() })).map_err(|e|e.to_string())?;
                        match result {
                        Ok(rules)=>{
                            inline_images.close();eval.send(Packet::InlineReset(true)).map_err(|e|e.to_string())?;
                            screen.replace_rules(rules);filter_warning=None;checks.filter_edits+=1;
                            media_preview.close();
                            eval.send(Packet::HoverReset{media:true}).map_err(|e|e.to_string())?;
                            Packet::FiltersResult(FiltersResult::Saved{allow_color:screen.rules().allow_color,entries:screen.rules().entries.clone(),message:pttstella::msg!("filters-saved")})
                        },
                        Err(error)=>Packet::FiltersResult(FiltersResult::Rejected{message:error.to_string()}),
                        }
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
                    CommandKind::Preload => {
                        if done.result.is_err() {screen.stop_full_load();}
                        if probe.is_some() && done.result.is_ok() {checks.navigation_requests+=1;}
                        continue;
                    },
                    CommandKind::Navigate => match done.result {
                        Ok(()) => {if probe.is_some(){checks.navigation_requests+=1;}Packet::NavigationResult(Feedback::quiet(NavigationStatus::Sent))},
                        Err(error) => {screen.cancel_article_fetch();Packet::NavigationResult(Feedback::new(NavigationStatus::Rejected,error))},
                    },
                    CommandKind::Browse => match done.result {
                        Ok(()) => continue,
                        Err(error) => Packet::Notice(error),
                    },
                    CommandKind::Destination => {
                        let Some((request,_))=destination_request.take() else {continue;};
                        Packet::DestinationResult {request,feedback:match done.result {
                            Ok(())=>Feedback::quiet(DestinationStatus::Started),
                            Err(error)=>Feedback::new(DestinationStatus::Rejected,error),
                        }}
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
            update = inline_images.update() => {
                let packet=match update.result {
                    Ok(loaded)=>{let image=loaded.image;Packet::InlineImage{batch:update.batch,index:update.index,image:MediaImage{
                        request:update.batch,data:base64::engine::general_purpose::STANDARD.encode(&image.bytes),
                        mime:image.mime,width:image.width,height:image.height,animated:image.animated_format,animation_pixels:(image.animated_format&&image.mime=="image/gif").then_some(image.animation_pixels),cached:false}}},
                    Err(message)=>Packet::InlineError{batch:update.batch,index:update.index,message},
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
                                animated:image.animated_format,animation_pixels:(image.animated_format&&image.mime=="image/gif").then_some(image.animation_pixels),cached})
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
                if event.requires_ptt() && !receiver.borrow().site.ptt_features() {
                    event.discard();eval.send(Packet::Notice(pttstella::msg!("site-ptt-only"))).map_err(|e|e.to_string())?;continue;
                }
                if matches!(&event,
                    SurfaceEvent::DestinationSubmit{..}|SurfaceEvent::SitesConnect{..}|SurfaceEvent::LoginSubmit{..}|SurfaceEvent::Paste{..}|SurfaceEvent::PasteConfirm{..}) {
                    screen.clear_article();
                    inline_images.close();eval.send(Packet::InlineReset(true)).map_err(|e|e.to_string())?;
                }
                match event {
                    SurfaceEvent::StationMouse { event, .. } => {
                        if super::probe::mouse_case(surface_probe.as_deref()) && checks.mouse_edges.len()<16 {checks.mouse_edges.push(event.report);}
                        let result = if !screen.mouse_shown(&event) || pending_paste.read().is_some() || keyboard_policy.status(&receiver.borrow().context()).pending {
                            Err(pttstella::msg!("station-mouse-stale"))
                        } else {
                            connection.mouse_report(event)
                        };
                        if let Err(error) = result {
                            connection.cancel_mouse(event.lease);
                            eval.send(Packet::StationMouseRejected {lease:event.lease,message:error}).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::StationMouseSample { sample, .. } => {
                        if super::probe::mouse_case(surface_probe.as_deref()) {
                            let round_trip = surface_probe.as_deref()==Some("mouse_round_trip");
                            if checks.mouse_samples.len()<if round_trip {256} else {64} {checks.mouse_samples.push(sample.report);} else {checks.mouse_sample_overflow=true;}
                            if round_trip && matches!(sample.report,pttstella::mouse::input::SampleReport::Motion(r) if (8..=23).contains(&r.col) && (7..=11).contains(&r.row) && r.button==Some(pttstella::mouse::Button::Left) && r.modifiers==pttstella::mouse::Modifiers::default()) {
                                let view=receiver.borrow();
                                let status=view.frame.glyphs.iter().filter(|g|g.row==1).map(|g|g.text.as_str()).collect::<String>();
                                if screen.painted(&view) && status.starts_with("HOLD: return to A") {checks.mouse_return_painted=Some(view.frame.revision);}
                            }
                        }
                        let result = if !screen.mouse_sample_shown(&sample) || pending_paste.read().is_some() || keyboard_policy.status(&receiver.borrow().context()).pending {
                            Err(pttstella::msg!("station-mouse-stale"))
                        } else {
                            connection.mouse_sample(sample)
                        };
                        if let Err(error) = result {
                            eval.send(Packet::Notice(error)).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::StationMouseCancel { lease } => {
                        if super::probe::mouse_case(surface_probe.as_deref()) {checks.mouse_cancels+=1;}
                        connection.cancel_mouse(lease);
                    },
                    SurfaceEvent::StationMouseRevoke { generation } => connection.revoke_mouse(generation),
                    SurfaceEvent::StationMouseEnabled { generation, enabled, .. } => {
                        if super::probe::mouse_case(surface_probe.as_deref()) && checks.mouse_toggles.len()<8 {checks.mouse_toggles.push(enabled);}
                        connection.mouse_enabled(generation, enabled);
                    },
                    SurfaceEvent::BookmarksOpen {request,..}=>{
                        checks.bookmark_opens=checks.bookmark_opens.saturating_add(1);
                        eval.send(Packet::BookmarksOpen {request,data:bookmarks.snapshot()}).map_err(|e|e.to_string())?;
                    },
                    SurfaceEvent::BookmarksEdit {request,value,label,remove,..}=>{
                        let result=bookmarks.edit(&value,&label,remove).and_then(|write|files.start(|permit|files::save_bookmarks(permit,request,write)));
                        if let Err(error)=result {
                            checks.bookmark_rejected=checks.bookmark_rejected.saturating_add(1);
                            eval.send(Packet::BookmarksResult {request,data:bookmarks.snapshot(),feedback:Feedback::new(SaveStatus::Rejected,error)}).map_err(|e|e.to_string())?;
                        }
                    },
                    SurfaceEvent::DestinationSubmit {request,stamp,value,trusted:_} => {
                        let result=(|| {
                            let target=pttstella::navigation_job::Destination::parse(&value)?;
                            commands.start(CommandKind::Destination,stamp.generation,||connection.request_go_to(stamp,target).map(|reply|reply.wait()))
                        })();
                        match result {
                            Ok(())=>destination_request=Some((request,stamp)),
                            Err(error)=>{checks.destination_rejected(surface_probe.as_deref());eval.send(Packet::DestinationResult {request,feedback:Feedback::new(DestinationStatus::Rejected,error)}).map_err(|error|error.to_string())?;},
                        }
                    },
                    SurfaceEvent::RecentBoardsClear { .. } => {
                        if surface_probe.as_deref()==Some("recent") {recent_clears=recent_clears.saturating_add(1);}
                        recent.clear(files);
                        eval.send(Packet::RecentBoards(recent.packet())).map_err(|e|e.to_string())?;
                    },
                    SurfaceEvent::DestinationCancel {operation,trusted:_} => {
                        checks.article_cancelled |= matches!(surface_probe.as_deref(),Some("articles"|"articles_close")) && matches!(&receiver.borrow().destination,pttstella::navigation_job::Progress::Active{operation:owner,target:pttstella::navigation_job::Destination::Article(_),..} if *owner==operation);
                        checks.board_cancelled |= matches!(surface_probe.as_deref(),Some("boards"|"boards_close")) && matches!(&receiver.borrow().destination,pttstella::navigation_job::Progress::Active{operation:owner,..} if *owner==operation);
                        if destination_request.is_some_and(|(_,stamp)|stamp==operation) {
                            if commands.is(CommandKind::Destination) {commands.cancel();}
                            destination_request=None;
                        }
                        if let Err(error)=connection.cancel_navigation(operation) {eval.send(Packet::Notice(error)).map_err(|error|error.to_string())?;}
                    },
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
                        filter_import.clear();
                        eval.send(Packet::FiltersOpen(FiltersOpen{allow_color:screen.rules().allow_color,entries:screen.rules().entries.clone(),warning:filter_warning.clone()})).map_err(|e|e.to_string())?;
                    }
                    SurfaceEvent::AccountRule { presentation, stamp, row, col, policy, remove, .. } => {
                        let rules = screen.account_rule(Identity{presentation,generation:stamp.generation,revision:stamp.revision},stamp,(row,col),policy,remove,&receiver.borrow());
                        let Some(rules) = rules else {eval.send(Packet::Notice(pttstella::msg!("account-action-stale"))).map_err(|e|e.to_string())?;continue;};
                        checks.account_rule_actions+=1;
                        filter_import.clear();
                        if let Err(message)=files.start(|permit|files::save_filters(permit,filter_store.clone(),rules)) {
                            eval.send(Packet::Notice(message)).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::ReplyFocus { presentation, stamp, row, .. } => {
                        if screen.focus_reply(Identity{presentation,generation:stamp.generation,revision:stamp.revision},stamp,row,&receiver.borrow()) {
                            checks.reply_focus_actions+=1;
                            media_preview.close();inline_images.close();
                            eval.send(Packet::InlineReset(true)).map_err(|e|e.to_string())?;
                            eval.send(Packet::HoverReset{media:true}).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::FiltersDefaultColor { color, .. } => {
                        filter_import.clear();
                        let rules = screen.rules().with_allow_color(color);
                        if let Err(message) = files.start(|permit|files::save_filters(permit,filter_store.clone(),rules)) {
                            eval.send(Packet::FiltersResult(FiltersResult::Rejected{message})).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::FiltersEdit {
                        id,
                        note,
                        policy,
                        color,
                        remove,
                        trusted: _,
                    } => {
                        filter_import.clear();
                        let rules=screen.rules().edit_color(&id,&note,policy,color,remove);
                        if let Err(message)=files.start(|permit|files::save_filters(permit,filter_store.clone(),rules)) {
                            eval.send(Packet::FiltersResult(FiltersResult::Rejected{message})).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::FiltersImport { request, trusted: _ } => {
                        if let Err(message)=files.start(|permit|{filter_import.begin(request);files::import_filters(permit,request,probe.clone().filter(|_|surface_probe.as_deref()==Some("account_lists")))}) {
                            eval.send(Packet::FiltersResult(FiltersResult::ImportRejected{request,message})).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::FiltersImportCancel { request, trusted: _ } => {filter_import.cancel(request);}
                    SurfaceEvent::FiltersImportApply { request, trusted: _ } => {
                        if let Err(message)=files.start(|permit|{
                            let rules=filter_import.apply(request,screen.rules());
                            files::save_filters(permit,filter_store.clone(),rules)
                        }) {
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
                        inline_images.close();eval.send(Packet::InlineReset(true)).map_err(|e|e.to_string())?;
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
                            || view.mouse.route != pttstella::mouse::input::Route::Local
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
                            "toggle" if receiver.borrow().site.ptt_features() => keyboard_policy.toggle(),
                            "once" if receiver.borrow().site.ptt_features() => keyboard_policy.arm(),
                            "release" => keyboard_policy.release(&code),
                            "composition_start" => keyboard_policy.composition_start(),
                            "composition_end" => keyboard_policy.composition_end(),
                            "independent_edit" => keyboard_policy.independent_edit(),
                            "reset" => keyboard_policy.reset(),
                            _ => continue,
                        }
                        let context = receiver.borrow().context();
                        eval.send(Packet::KeyboardState(keyboard_policy.status(&context)))
                            .map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::MediaDecoded { request, success } => {
                        if surface_probe.as_deref()==Some("osc8") && success {checks.osc8_media_decoded+=1;}
                        media_preview.decoded(request, success);
                    }
                    SurfaceEvent::KeepAlive { enabled, trusted: _ } => {
                        if let Err(error) = connection.keep_alive(enabled) {
                            eval.send(Packet::Notice(error))
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    SurfaceEvent::KeepAliveGate { stamp, ready, reading, article } => {
                        reading_gate=if reading {article.and_then(|id|screen.authorize_reading(stamp,id))}else{None};
                        connection.keep_alive_gate(
                            stamp,
                            ready
                                && !settings_visible
                                && !keyboard_policy
                                    .status(&receiver.borrow().context())
                                    .pending,
                        );
                    }
                    SurfaceEvent::ArticleLoad {generation,article,resume,trusted:_} => {
                        if screen.full_load_action(generation,article,resume,&receiver.borrow()) {
                            if probe.is_some() {if resume {checks.load_resumes+=1;}else{checks.load_cancels+=1;}}
                            if !resume && commands.is(CommandKind::Preload) {commands.cancel();}
                        }
                    },
                    SurfaceEvent::Navigate {
                        stamp,
                        intent,
                        source,
                        trusted: _,
                    } => {
                        if let navigation::Intent::AdjacentArticle { down } = intent
                            && (source != navigation::Source::Keyboard || !screen.adjacent_article_allowed(stamp, down, &receiver.borrow())) {
                            eval.send(Packet::NavigationResult(Feedback::new(NavigationStatus::Rejected,navigation::Error::Unavailable.to_string()))).map_err(|e|e.to_string())?;
                            continue;
                        }
                        screen.interrupt_full_load();if commands.is(CommandKind::Preload) {commands.cancel();}
                        let shown=screen.navigation_shown(stamp);
                        let result = if !shown {
                            Err(navigation::Error::Stale.to_string())
                        } else if pending_paste.read().is_some() {
                            Err(navigation::Error::Unavailable.to_string())
                        } else {
                            commands.start(CommandKind::Navigate, stamp.generation, || {
                                connection.request_navigate(stamp, intent, source).map(|reply|reply.wait())
                            })
                        };
                        if result.is_ok() {
                            screen.article_navigation(intent, &receiver.borrow());
                        }
                        if result.is_ok() && inline_images.navigation(&receiver.borrow().context(),intent) {
                            eval.send(Packet::InlineReset(true)).map_err(|e|e.to_string())?;
                        }
                        if let Err(error) = result {
                            eval.send(Packet::NavigationResult(Feedback::new(NavigationStatus::Rejected,error)))
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    SurfaceEvent::SitesOpen {request,trusted:_}=>{
                        eval.send(Packet::SitesOpen{request,data:sites.snapshot(receiver.borrow().site.clone())}).map_err(|e|e.to_string())?;
                    },
                    SurfaceEvent::SitesEdit {request,action,address,encoding,name,trusted:_}=>{
                        let result=sites.edit(action,address,encoding,name).and_then(|write|files.start(|permit|files::save_sites(permit,request,write)));
                        if let Err(error)=result {checks.site_rejections=checks.site_rejections.saturating_add(1);eval.send(Packet::SitesResult{request,data:sites.snapshot(receiver.borrow().site.clone()),feedback:Feedback::new(super::sites::Status::Rejected,error)}).map_err(|e|e.to_string())?;}
                    },
                    SurfaceEvent::SitesConnect {request,address,encoding,trusted:_}=>{
                        let result=pttstella::sites::Target::new(address,encoding).and_then(|target|{
                            if probe.is_some() && url::Url::parse(target.endpoint.as_str()).ok().and_then(|url|url.host_str().map(str::to_owned)).as_deref()!=Some("127.0.0.1") {return Err(pttstella::msg!("sites-probe-local"));}
                            connection.connect_to(target)
                        });
                        if result.is_ok(){checks.site_switches=checks.site_switches.saturating_add(1);login_open=false;login_stamp=None;pending_paste.set(None);commands.cancel();}
                        let feedback=match result {Ok(())=>Feedback::new(super::sites::Status::Connecting,pttstella::msg!("sites-connecting")),Err(error)=>{checks.site_rejections=checks.site_rejections.saturating_add(1);Feedback::new(super::sites::Status::Rejected,error)}};
                        eval.send(Packet::SitesResult{request,data:sites.snapshot(receiver.borrow().site.clone()),feedback}).map_err(|e|e.to_string())?;
                    },
                    SurfaceEvent::SettingsVisibility {open} => {settings_visible=open;eval.send(Packet::SettingsVisibility(open)).map_err(|e|e.to_string())?;},
                    SurfaceEvent::AboutRepository { .. } => {
                        if probe.is_none() && let Err(error) = webbrowser::open(env!("CARGO_PKG_REPOSITORY")) {
                            eval.send(Packet::Notice(pttstella::msg!("link-open-failed",e=error.to_string()))).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::SettingsOpen { trusted: _ } => {
                        settings_visible=true;connection.keep_alive_gate(receiver.borrow().stamp(),false);eval.send(Packet::SettingsVisibility(true)).map_err(|e|e.to_string())?;
                        eval.open_settings(preferences.store());
                        eval.send(preferences.open_packet())
                            .map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::SettingsFonts { request } => {
                        if let Err(error)=files.start(|permit|files::font_catalog(permit,request)) {
                            eval.send(Packet::FontCatalog{request,families:Vec::new(),notice:Some(error)}).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::FontSource { source, trusted: _ } => {
                        let help=source.help();
                        let result=if probe.is_some() {
                            if !checks.font_sources.iter().any(|id|id==help.id){checks.font_sources.push(help.id.to_owned());}
                            Ok(())
                        } else {webbrowser::open(help.url)};
                        let notice=match result {
                            Ok(())=>pttstella::msg!("font-source-opened"),
                            Err(error)=>pttstella::msg!("link-open-failed",e=error.to_string()),
                        };
                        eval.settings_notice(&notice).map_err(|e|e.to_string())?;
                    }
                    SurfaceEvent::SettingsPreview { request, values } => {
                        eval.send(preferences.preview_packet(request, values))
                            .map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::SettingsSave { change, trusted: _ } => {pending_settings=Some(change);}
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
                            let rows=if rows.end>pttstella::export::ROWS {0..rows.len()} else {rows};
                            files.start(|permit|{
                                let lease=screen.lease_output(Identity{presentation,generation,revision},selection_request,receiver.borrow().input_epoch);
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
                        let link=screen.current(Identity{presentation,generation,revision},&receiver.borrow())
                            .filter(|_|receiver.borrow().mouse.route==pttstella::mouse::input::Route::Local)
                            .and_then(|p|p.links.get(index)).cloned();
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
                                if link.url != "https://example.com/pttstella" && !(surface_probe.as_deref()==Some("osc8") && link.url==super::probe::OSC8_URL) {
                                    return Err("unexpected probe URL".into());
                                }
                                checks.link_requests += 1;
                                if surface_probe.as_deref()==Some("osc8") {checks.osc8_open_rows.push(link.row);}
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
                        // Release the watch read lock before the result closure
                        // reads state again; a waiting writer makes nested reads
                        // deadlock on platforms with writer-preferring locks.
                        let encoding = receiver.borrow().site.encoding;
                        let result = input::prepare_paste_with(&text,encoding)
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
                        if surface_probe.as_deref()==Some("osc8") && matches!(selection,selection::Selection::Range{..}) && selected.as_ref().is_some_and(|value|value.rows==(8..10) && value.text.contains('\n')) {checks.osc8_dragged=true;}
                        if surface_probe.as_deref()==Some("osc8") && matches!(selection,selection::Selection::Link{..}) && let Some(value)=selected.as_ref().filter(|value|value.text==super::probe::OSC8_URL) {
                            checks.osc8_selected=Some((generation,revision,request,value.rows.start));
                        }
                        eval.send(Packet::Selection(SelectionReply{presentation,generation,revision,request,value:selected})).map_err(|e| e.to_string())?;
                    }
                    SurfaceEvent::SelectionCleared {presentation,generation,revision,request}=>{
                        screen.clear_selection(Identity{presentation,generation,revision},request);
                    },
                    SurfaceEvent::CopyText {presentation,generation,revision,target,trusted:_} => {
                        let view=receiver.borrow().clone();
                        let identity=Identity{presentation,generation,revision};
                        if let Some(text)=screen.copy_text(identity,&target,&view) {
                            let selected=match target {super::protocol::CopyTarget::Selection{request}=>Some(request),super::protocol::CopyTarget::Link{..}=>None};
                            let clipboard=files.clipboard();
                            if let Err(error)=files.start(|permit|files::copy_text(permit,clipboard,text,screen.lease_output(identity,selected,view.input_epoch),receiver.clone())) {
                                eval.send(Packet::Notice(error)).map_err(|e|e.to_string())?;
                            }
                        } else {eval.send(Packet::Notice(pttstella::msg!("copy-not-ready"))).map_err(|e|e.to_string())?;}
                    }
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
                        if surface_probe.as_deref()==Some("osc8") && screen.copied(Identity{presentation,generation,revision},request,&receiver.borrow()) && let Some((_,_,_,row))=checks.osc8_selected.filter(|(g,r,q,_)|(*g,*r,*q)==(generation,revision,request)) {
                            checks.osc8_copy_rows.push(row);
                        }
                        if super::probe::mouse_case(surface_probe.as_deref()) && screen.copied(Identity{presentation,generation,revision},request,&receiver.borrow()) {
                            checks.mouse_copies+=1;
                        }
                        checks.copy_checked |=
                            checks.copied_selection == Some((generation, revision, request));
                    }
                    SurfaceEvent::InlineExpand {presentation,stamp,key,target,trusted:_} => {
                        if matches!(surface_probe.as_deref(),Some("inline"|"inline_pages"|"full_load"|"long_load"|"short_overlap"|"short_links"|"context_menu")) {checks.inline_requests+=1;if checks.inline_targets.len()>=128{return Err("too many inline probe requests".into());}checks.inline_targets.push(target);}
                        let view=receiver.borrow().clone();
                        if !view.site.ptt_features() || !keyboard_policy.inline_images_key(&key,&view.context())
                            || pending_paste.read().is_some() || commands.busy() || login_open {continue;}
                        if let Some(paint)=screen.inline_page(Identity{presentation,generation:stamp.generation,revision:stamp.revision},stamp,&view) {
                            // Synthetic probes must never reach a public image host.
                            if probe.is_some() && paint.image_links.iter().any(|index|media_origin.as_ref().is_none_or(|origin|!paint.links[*index].url.starts_with(&format!("{origin}/")))) {continue;}
                            let old_batch=inline_images.batch();
                            if let Some(open)=inline_images.open(paint,target)? {
                                if matches!(surface_probe.as_deref(),Some("inline"|"inline_pages"|"full_load"|"long_load"|"short_overlap"|"short_links"|"context_menu"))  && old_batch!=Some(open.batch) {checks.inline_expands+=1;}
                                eval.send(Packet::InlineOpen(open)).map_err(|e|e.to_string())?;
                            }
                        }
                    }
                    SurfaceEvent::Key {
                        key,
                        generation,
                        trusted: _,
                    } => {
                        let view = receiver.borrow().clone();
                        let context = view.context();
                        let reading_context=screen.reading_context(&view);
                        let policy_context=reading_context.as_ref().unwrap_or(&context);
                        let effect = keyboard_policy.key(&key, policy_context);
                        eval.send(Packet::KeyboardState(keyboard_policy.status(policy_context)))
                            .map_err(|e| e.to_string())?;
                        if screen.reading_context(&view).is_none()
                            && let Some(bytes)=super::article_cache::terminal_arrow_boundary(&key,&context,preferences.app().article_boundary_navigation)
                        {
                            if !screen.navigation_shown(view.stamp()) || generation != view.generation || view.input_pending || view.output_pending
                                || view.mouse.route != pttstella::mouse::input::Route::Local {continue;}
                            if !bytes.is_empty() {
                                screen.article_input(&context,&bytes);
                                inline_images.close();
                                eval.send(Packet::InlineReset(true)).map_err(|e|e.to_string())?;
                                if let Err(error)=connection.input(generation,bytes) {eval.send(Packet::InputRejected(error)).map_err(|e|e.to_string())?;}
                            }
                            continue;
                        }
                        let bytes = match effect {
                            Effect::None => continue,
                            Effect::Notice(notice) => {
                                eval.send(Packet::Notice(notice))
                                    .map_err(|e| e.to_string())?;
                                continue;
                            }
                            Effect::Browse(intent) => {
                                if let Some(packet)=screen.reading_move(&view,intent) {
                                    eval.send(packet).map_err(|e|e.to_string())?;
                                    continue;
                                }
                                screen.interrupt_full_load();if commands.is(CommandKind::Preload) {commands.cancel();}
                                let shown=screen.browse_shown(generation,&context);
                                let result = if !shown || pending_paste.read().is_some() {
                                    Err(navigation::Error::Stale.to_string())
                                } else {
                                    commands.start(CommandKind::Browse, generation, || {
                                        connection.request_browse_key(view.stamp(), intent).map(|reply|reply.wait())
                                    })
                                };
                                if result.is_ok() {
                            screen.article_navigation(intent, &receiver.borrow());
                        }
                        if result.is_ok() && inline_images.navigation(&context,intent) {
                                    eval.send(Packet::InlineReset(true)).map_err(|e|e.to_string())?;
                                }
                                if let Err(error) = result {
                                    eval.send(Packet::Notice(error))
                                        .map_err(|e| e.to_string())?;
                                }
                                continue;
                            }
                            Effect::Native(bytes) => bytes,
                        };
                        if let Some(intent)=super::article_cache::reading_intent(&bytes) && let Some(packet)=screen.reading_move(&view,intent) {
                            eval.send(packet).map_err(|e|e.to_string())?;continue;
                        }
                        screen.article_input(&context,&bytes);
                        if inline_images.input(&context,&bytes) {
                            eval.send(Packet::InlineReset(true)).map_err(|e|e.to_string())?;
                        }
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
                        eval.send(Packet::KeyboardState(keyboard_policy.status(&receiver.borrow().context()))).map_err(|e|e.to_string())?;
                        let vim_input = keyboard_policy
                            .status(&receiver.borrow().context())
                            .enabled;
                        if probe.is_some()
                            && !vim_input
                            && checks.input_bytes.len() >= checks.expected_input.len()
                        {
                            return Err("unexpected probe input".into());
                        }
                        checks.composition_checked |= composition_input && text == "中";
                        let encoded = input::encode_with(&text,receiver.borrow().site.encoding).map_err(|e| e.to_string());
                        let result = encoded.and_then(|bytes| {
                            screen.article_input(&receiver.borrow().context(),&bytes);
                            if inline_images.input(&receiver.borrow().context(),&bytes) {
                                eval.send(Packet::InlineReset(true)).map_err(|e|e.to_string())?;
                            }
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
                        reading_timing.record("painted",generation,revision,screen.reading_progress())?;
                        let view=receiver.borrow();
                        if let Some(paint)=screen.inline_page(Identity{presentation,generation,revision},view.stamp(),&view)
                            && (probe.is_none() || paint.image_links.iter().all(|index|media_origin.as_ref().is_some_and(|origin|paint.links[*index].url.starts_with(&format!("{origin}/")))))
                            && let Some(open)=inline_images.follow(paint)? {
                            eval.send(Packet::InlineOpen(open)).map_err(|e|e.to_string())?;
                        }
                    }
                    SurfaceEvent::Error { message } => return Err(message),
                    SurfaceEvent::Probe(pixels) => {
                        let path=probe.clone().ok_or("unexpected probe response")?;
                        let settings=preferences.app().clone();
                        files.start(|permit|files::probe_report(permit,*pixels,path,reload_settings,settings))?;
                    }
                    SurfaceEvent::Stopped { clean, tools } => {
                        if surface_probe.is_some() {checks.tools=tools;}
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
                if recent.observe(&view, files) {
                    eval.send(Packet::RecentBoards(recent.packet())).map_err(|e|e.to_string())?;
                }
                if screen.update(&view) {
                    media_preview.close();
                    eval.send(Packet::HoverReset{media:false}).map_err(|e|e.to_string())?;
                }
                if inline_images.observe(&view) {eval.send(Packet::InlineReset(true)).map_err(|e|e.to_string())?;}
                if keyboard_generation!=view.generation || view.phase!=Phase::Connected {
                    if let Some((request,_))=destination_request.take() {
                        eval.send(Packet::DestinationResult {request,feedback:Feedback::new(DestinationStatus::Rejected,pttstella::msg!("command-stale"))}).map_err(|error|error.to_string())?;
                    }
                    if commands.busy() {
                        commands.cancel();
                        eval.send(Packet::NavigationResult(Feedback::quiet(NavigationStatus::Rejected))).map_err(|e|e.to_string())?;
                        eval.send(Packet::LoginResult(LoginResult::Message{message:pttstella::msg!("command-stale")})).map_err(|e|e.to_string())?;
                    }
                    keyboard_policy.reset();keyboard_generation=view.generation;
                    if !view.site.ptt_features() && keyboard_policy.status(&view.context()).enabled {keyboard_policy.toggle();}
                }
                if view.login_guidance != login_guidance && view.login_guidance.needs_input() {
                    keyboard_policy.use_native_input();
                }
                if matches!(surface_probe.as_deref(),Some("boards"|"boards_close")) && checks.board_reached.len()<3 && let pttstella::navigation_job::Progress::Reached{serial,target}=&view.destination && checks.board_reached.last().is_none_or(|(seen,_)|seen!=serial) {checks.board_reached.push((*serial,target.board().as_str().to_owned()));}
                if matches!(surface_probe.as_deref(),Some("articles"|"articles_close")) {
                    if checks.article_reached.len()<3 && let pttstella::navigation_job::Progress::Reached{serial,target:pttstella::navigation_job::Destination::Article(target)}=&view.destination && checks.article_reached.last().is_none_or(|(seen,_)|seen!=serial) { checks.article_reached.push((*serial,target.aid().as_str().to_owned())); }
                    checks.article_missing |= view.message==pttstella::msg!("article-open-missing");
                }
                login_guidance = view.login_guidance;
                checks.verification_required |= login_guidance == pttstella::login::Guidance::CodeRequired;
                checks.verification_rejected |= login_guidance == pttstella::login::Guidance::CodeRejected;
                checks.verification_browsing |= login_guidance == pttstella::login::Guidance::Browsing;
                keyboard_policy.context(&view.context());
                eval.send(Packet::KeyboardState(keyboard_policy.status(&view.context()))).map_err(|e|e.to_string())?;
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
        let reading = {
            let view = receiver.borrow();
            screen
                .inline_identity(&view)
                .and_then(|identity| screen.current(identity, &view))
                .and_then(|paint| paint.article.clone())
        };
        let mut mouse_report = checks.mouse_observations();
        mouse_report["final"] = serde_json::json!(receiver.borrow().mouse);
        let mut surface_report = serde_json::json!({
            "status":if case=="slow" && commands.busy() || case=="files_slow" && files.busy() || case=="files" && checks.export_checked || case=="untrusted" && surface_probe_clean && rejected_events==3
                || case=="confirm" && surface_probe_clean && rejected_events==1 && pending_paste.read().as_ref().is_some_and(|p|p.prepared.text=="A\nB")
                || case=="recent" && recent_clears>=1 && recent.packet().boards.is_empty() && recent.packet().notice==Some(pttstella::msg!("recent-cleared"))
                || checks.bookmarks_passed(&case,&initial_bookmarks,&bookmarks.snapshot(),&receiver.borrow().destination)
                || case=="boards_close" && checks.board_reached==[(1,"Stella".to_owned()),(2,"Other".to_owned())] && matches!(&receiver.borrow().destination,pttstella::navigation_job::Progress::Active{target,..} if target.board().as_str()=="Beta")
                || matches!(case.as_str(),"articles"|"articles_close") && checks.article_reached==[(1,"00000101".to_owned()),(2,"00000102".to_owned())] && checks.article_missing && checks.article_rejected>=2
                    && keyboard_policy.status(&receiver.borrow().context()).mode=="normal"
                    && (case=="articles" && checks.article_cancelled || case=="articles_close" && matches!(&receiver.borrow().destination,pttstella::navigation_job::Progress::Active{serial:4,target:pttstella::navigation_job::Destination::Article(target),..} if target.aid().as_str()=="00000104"))
                || case=="boards" && checks.board_reached==[(1,"Stella".to_owned()),(2,"Other".to_owned())] && checks.board_cancelled && checks.board_rejected>=1
                || matches!(case.as_str(),"mouse"|"mouse_boundary") && checks.mouse_passed(case=="mouse_boundary") && receiver.borrow().mouse.active.is_none()
                || matches!(case.as_str(),"mouse_drag"|"mouse_round_trip"|"mouse_motion"|"mouse_wheel") && checks.mouse_samples_passed(&case) && receiver.borrow().mouse.active.is_none() && receiver.borrow().mouse.modes.tracking()==pttstella::mouse::Tracking::Highlight
                || case=="pointer_rows" && checks.navigation_requests==7 && checks.export_checked && receiver.borrow().context().kind()=="article_list"
                || case=="filter_colors" && checks.filter_edits>=4 && checks.export_checked && screen.rules().allow_color==10
                || case=="account_actions" && checks.account_rule_actions>=3 && checks.filter_edits>=3 && checks.reply_focus_actions>=2 && checks.export_checked
                || case=="merged_replies" && checks.context_copies>=1 && checks.export_checked
                || matches!(case.as_str(),"account_lists"|"account_articles") && checks.filter_edits>=3 && checks.context_copies>=1 && checks.export_checked
                || case=="short_overlap" && checks.navigation_requests==1 && checks.inline_requests>=2 && checks.inline_expands==1 && checks.link_requests>=1 && checks.export_checked && reading.as_ref().is_some_and(|r| r.first_line==1 && r.last_line==32 && r.end && r.load==Some(super::article_cache::LoadState::Complete))
                || case=="terminal_pages" && checks.export_checked && !screen.full_load_active()
                || case=="long_load" && checks.navigation_requests==37 && checks.export_checked && checks.inline_requests>=2 && checks.inline_expands==1 && reading.as_ref().is_some_and(|r|r.first_line==1 && r.last_line==837 && r.end && r.load==Some(super::article_cache::LoadState::Complete))
                || case=="full_load" && checks.load_cancels>=1 && checks.load_resumes>=1 && checks.navigation_requests==19 && checks.export_checked && checks.inline_requests>=2 && checks.inline_expands==1
                || matches!(case.as_str(),"inline"|"inline_pages"|"short_links"|"context_menu") && checks.inline_requests>=2 && checks.inline_expands==1 && checks.export_checked && (case!="inline_pages"||checks.navigation_requests==1) && (case!="context_menu"||checks.context_copies>=2)
                || case=="osc8" && checks.osc8_open_rows==[8,9] && checks.osc8_copy_rows==[8,9] && checks.osc8_dragged && checks.osc8_media_decoded>=2 && checks.export_checked
                || case=="login_2fa" && checks.verification_required && checks.verification_rejected && checks.verification_browsing && keyboard_policy.status(&receiver.borrow().context()).mode=="normal"
                || case=="fonts" && checks.font_catalog_reads>=2 && !checks.font_sources.is_empty() && checks.export_checked
                || checks.sites_passed(&case,&receiver.borrow().site,&sites.default_target(),&receiver.borrow().frame)
                || matches!(case.as_str(),"tools"|"short_reading"|"sgr66"|"sgr66_legacy") && checks.export_checked
                || case=="dialogs" && checks.settings_saved && checks.export_checked && checks.rows_export_checked && checks.media_external>=1 {"passed"} else {"failed"},
            "recent_initial":initial_recent,"recent_final":recent.packet().boards,"recent_clears":recent_clears,
            "bookmarks_initial":initial_bookmarks,"bookmarks_final":bookmarks.snapshot(),"bookmark_opens":checks.bookmark_opens,"bookmark_saves":checks.bookmark_saves,"bookmark_rejected":checks.bookmark_rejected,
            "article_reached":checks.article_reached,"article_missing":checks.article_missing,"article_rejected":checks.article_rejected,"article_cancelled":checks.article_cancelled,
            "board_reached":checks.board_reached,"board_rejected":checks.board_rejected,"board_cancelled":checks.board_cancelled,
            "scope":if matches!(case.as_str(),"sites"|"sites_restart") {"synthetic loopback station UI; owned external test peers independently verify Big5/GBK bytes and EOF; explicit TCP shutdown is not a WebSocket acknowledgement"} else if case=="long_load" {"synthetic 38-page reading: thirty-seven exact PageDown commands, leading ink reset at page 31, retained 837 rows, explicit images, PNG and Close"} else if case=="full_load" {"synthetic full reading: nineteen exact PageDown commands, cancellation and explicit resume, retained 441 rows, explicit image expansion, PNG and Close"} else if case=="terminal_pages" {"synthetic terminal reading: no automatic load or image requests; one manual PageDown, PNG and Close"} else if case=="inline_pages" {"synthetic cached reading: one exact PageDown, fragmented response, local history without further station bytes, PNG and Close"} else if case=="tools" {"local tools playground: PNG and Close smoke only; gestures are observations, not an A/B acceptance test; peer rejects all station input"} else if matches!(case.as_str(),"mouse_drag"|"mouse_round_trip"|"mouse_motion"|"mouse_wheel") {"synthetic native pointer samples: bounded independent exact cells, required final gesture, no local keys or duplicate path, PNG and Close"} else if case=="mouse_boundary" {"synthetic station mouse boundary: one exact press, no release or local keys after leaving the grid; owned cancellation and Close"} else if case=="mouse" {"synthetic station mouse: exact three-button edges, unsupported mode, local copy and explicit fallback; middle release cancelled by mode epoch; peer rejects extra bytes"} else if case=="bookmarks" {"synthetic local bookmark CRUD and one verified article open through the existing destination flow; peer rejects extra input"} else if case.starts_with("bookmarks_") {"synthetic bookmark restart, empty or corrupt data; peer rejects all station input"} else if matches!(case.as_str(),"articles"|"articles_close") {"synthetic article UI: two verified opens, missing article with explicit manual recovery, two local rejections, then cancellation or close while query is delayed; peer rejects extra input"} else if matches!(case.as_str(),"boards"|"boards_close") {"synthetic board switch; peer checks two complete workflows and a cancelled third workflow, then Close without extra input"} else if case=="login_2fa" {"synthetic local verification; peer checks one account/password and two explicit codes, then Close without extra input"} else {"synthetic local surface only; peer rejects all station input"},
            "mouse":mouse_report,
            "tools":checks.tools,
            "inline_requests":checks.inline_requests,"inline_expands":checks.inline_expands,
            "osc8_dragged":checks.osc8_dragged,"osc8_media_decoded":checks.osc8_media_decoded,
            "osc8_open_rows":checks.osc8_open_rows,"osc8_copy_rows":checks.osc8_copy_rows,
            "verification_required":checks.verification_required,"verification_rejected":checks.verification_rejected,"verification_browsing":checks.verification_browsing,
            "font_sources":checks.font_sources,"font_catalog_reads":checks.font_catalog_reads,
            "case":case,"command_pending_on_close":commands.busy(),"file_pending_on_close":files.busy(),"synthetic_events_rejected":rejected_events,"js_admission":surface_probe_clean,
            "freeplay_only":case=="tools",
            "paste_preview_preserved":pending_paste.read().is_some(),
            "settings_saved":checks.settings_saved,"full_png":checks.export_checked,"rows_png":checks.rows_export_checked,"media_external_requests":checks.media_external,
        });
        surface_report["article_reading"] =
            serde_json::to_value(reading).map_err(|e| e.to_string())?;
        surface_report["inline_targets"] =
            serde_json::to_value(&checks.inline_targets).map_err(|e| e.to_string())?;
        surface_report["link_requests"] = checks.link_requests.into();
        surface_report["load_cancels"] = checks.load_cancels.into();
        surface_report["load_resumes"] = checks.load_resumes.into();
        surface_report["account_rule_actions"] = checks.account_rule_actions.into();
        surface_report["reply_focus_actions"] = checks.reply_focus_actions.into();
        surface_report["context_copies"] = checks.context_copies.into();
        surface_report["final_site"] = serde_json::json!(receiver.borrow().site);
        surface_report["sites_final"] =
            serde_json::json!(sites.snapshot(receiver.borrow().site.clone()));
        surface_report["site_removes"] = checks.site_removes.into();
        surface_report["site_switches"] = checks.site_switches.into();
        surface_report["site_saves"] = checks.site_saves.into();
        surface_report["site_rejections"] = checks.site_rejections.into();
        reports.push((path.join("surface-probe.json"), surface_report));
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
                                "scope":"local window probe; exact 46 Vim operations across main menu, board/favorite and article lists, including five switching cycles, no-redraw Home/count/enter, three continue prompts, dense ASCII-pointer articles, command help and empty-list return; split synchronized output and no extra station input",

                                "settings":preferences.app(),"vim_mode":keyboard_policy.status(&receiver.borrow().context())});
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
