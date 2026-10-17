//! Dioxus view and native window lifecycle wiring.
use super::probe::preview;
use super::surface::{Options, PendingPaste, surface};
use dioxus::prelude::*;
use pttstella::{
    connection::{Connection, Phase},
    screen::Screen,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::watch;

#[derive(Clone, PartialEq, Props)]
pub struct AppProps {
    pub probe: Option<PathBuf>,
    pub window_probe: bool,
    pub login_probe: bool,
    pub filters_probe: bool,
    pub surface_probe: Option<String>,
    pub reload_settings: bool,
    pub media_origin: Option<String>,
}

pub fn app(props: AppProps) -> Element {
    dioxus::desktop::use_asset_handler("pttstella-fonts", |request, responder| {
        responder.respond(crate::fonts::response(&request));
    });
    let probing = props.probe.is_some();
    let window_probe = props.window_probe;
    let login_probe = props.login_probe;
    let filters_probe = props.filters_probe;
    let surface_probe = props.surface_probe.clone();
    let surface_probing = surface_probe.is_some();
    let dialogs_probing = matches!(surface_probe.as_deref(), Some("dialogs" | "fonts"));
    let reload_settings = props.reload_settings;
    let media_origin = props.media_origin.clone();
    let owner = use_hook(move || {
        preview(probing, media_origin.as_deref())
            .and_then(|bytes| {
                if surface_probing {
                    if dialogs_probing {
                        Connection::start_ui_probe(bytes)
                    } else {
                        Connection::start_surface_probe()
                    }
                } else if filters_probe {
                    Connection::start_filters_probe()
                } else if login_probe {
                    Connection::start_login_probe()
                } else if window_probe {
                    Connection::start_window_probe()
                } else if probing {
                    Connection::start_probe(bytes, reload_settings)
                } else {
                    let mut screen = Screen::new(80, 24).map_err(|error| error.to_string())?;
                    screen.feed(&bytes);
                    Connection::start(screen)
                }
            })
            .map(Arc::new)
    });
    let connection = match owner {
        Ok(connection) => connection,
        Err(error) => {
            if let Some(path) = &props.probe {
                let report = serde_json::json!({"status":"failed","reason":error});
                let saved = std::fs::create_dir_all(path)
                    .and_then(|_| std::fs::write(path.join("probe.json"), report.to_string()));
                if let Err(error) = saved {
                    eprintln!("probe setup failed: {error}");
                }
                let window = dioxus::desktop::window();
                window.set_close_behavior(dioxus::desktop::WindowCloseBehaviour::WindowCloses);
                window.close();
            }
            return rsx! { p { "{error}" } };
        }
    };
    let mut message = use_signal(|| pttstella::msg!("status-offline-preview"));
    let phase = use_signal(|| Phase::Disconnected);
    let mut started = use_signal(|| false);
    let mut fatal = use_signal(|| false);
    let pending_paste = use_signal(|| None::<PendingPaste>);
    let paste_lines = pending_paste
        .read()
        .as_ref()
        .map_or(0, |p| p.prepared.lines);
    let paste_text = pending_paste
        .read()
        .as_ref()
        .map(|p| p.prepared.text.clone())
        .unwrap_or_default();
    let closing = use_hook(|| watch::channel(false));
    let close_sender = closing.0.clone();
    dioxus::desktop::use_wry_event_handler(move |event, _| {
        let close_requested = matches!(
            event,
            dioxus::desktop::tao::event::Event::WindowEvent {
                event: dioxus::desktop::WindowEvent::CloseRequested,
                ..
            }
        );
        if close_requested {
            close_sender.send_replace(true);
        }
    });
    #[cfg(target_os = "macos")]
    {
        let close_sender = closing.0.clone();
        dioxus::desktop::use_muda_event_handler(move |event| {
            if event.id().0 == crate::macos::QUIT {
                if fatal() {
                    // The common cleanup already joined every owner before
                    // entering this state, so no surface loop remains to wake.
                    dioxus::desktop::window().close();
                } else {
                    close_sender.send_replace(true);
                }
            }
        });
    }
    let probe = props.probe.clone();
    let media_origin = props.media_origin.clone();
    let connect = connection.clone();
    let disconnect = connection.clone();
    rsx! {
        style { {include_str!("../../assets/app.css")} }
        main {
            div { class: "chrome", id: "chrome",
                "data-attention": if fatal() || matches!(phase(), Phase::Error | Phase::Disconnected) { "true" } else { "false" },
            header {
                div { class: "brand", "PTTStella" }
                span { class: "status", title: "{message}", "{message}" }
                button { disabled: fatal() || props.probe.is_some() || matches!(phase(), Phase::Connecting | Phase::Connected),
                    onclick: move |_| { if let Err(error) = connect.connect() { message.set(error); } }, {pttstella::msg!("connect")} }
                button { disabled: fatal() || props.probe.is_some() || !matches!(phase(), Phase::Connecting | Phase::Connected),
                    onclick: move |_| { if let Err(error) = disconnect.disconnect() { message.set(error); } }, {pttstella::msg!("disconnect")} }
                button { id: "zoom-out", "aria-label": pttstella::msg!("zoom-out"), "－" }
                span { id: "zoom-level", "100%" }
                button { id: "zoom-in", "aria-label": pttstella::msg!("zoom-in"), "＋" }
                button { id: "zoom-fit", {pttstella::msg!("zoom-fit")} }
                button { id: "select-all", {pttstella::msg!("select-all")} }
                button { id: "export-png", {pttstella::msg!("export-png")} }
                button { id: "export-rows", disabled: true, title: pttstella::msg!("export-rows-hint"), {pttstella::msg!("export-rows")} }
                button { id: "font-settings", {pttstella::msg!("font-settings")} }
                button { id: "filters-open", {pttstella::msg!("filters-title")} }
                button { id: "filters-reveal", hidden: true, {pttstella::msg!("filters-reveal")} }
                output { id: "filters-status", role: "status" }
                label { title: pttstella::msg!("hover-help"),
                    {pttstella::msg!("hover-label")}
                    select { id: "hover-position", "aria-label": pttstella::msg!("hover-position"),
                        option { value: "top_right", {pttstella::msg!("hover-top-right")} }
                        option { value: "follow_cursor", {pttstella::msg!("hover-follow")} }
                        option { value: "off", {pttstella::msg!("close")} }
                    }
                }
                button { id: "vim-toggle", "aria-pressed": "false", {pttstella::msg!("vim-off")} }
                button { id: "vim-once", hidden: true, {pttstella::msg!("vim-once-button")} }
                output { id: "vim-status", role: "status" }
                  button { id: "keep-alive", r#type: "button", "aria-pressed": "true", {pttstella::msg!("keepalive-on")} }
                button { id: "login-open", disabled: !cfg!(target_os="macos"), {pttstella::msg!("login")} }
                button { id: "login-stop", hidden: true, {pttstella::msg!("login-cancel-attempt")} }
                div { id: "browse-controls", hidden: true,
                    button { id: "browse-back", disabled: true, {pttstella::msg!("browse-back")} }
                    button { id: "browse-previous", disabled: true, {pttstella::msg!("browse-previous")} }
                    button { id: "browse-next", disabled: true, {pttstella::msg!("browse-next")} }
                }
                span { id: "browse-hint" }
            }
            }
            section { class: "workspace",
                div { class: "surface", id: "surface",
                    onmounted: move |_| {
                        if started() { return; }
                        started.set(true);
                        let connection = connection.clone(); let probe = probe.clone(); let closing = closing.1.clone(); let media_origin = media_origin.clone();
                        let surface_probe = surface_probe.clone();
                        spawn(async move {
                            let result = surface(connection.clone(), closing, probe, message, phase, pending_paste, Options { reload_settings, media_origin, window_probe, login_probe, filters_probe, surface_probe:surface_probe.clone() }).await;
                            if let Err(error) = result {
                                fatal.set(true);
                                message.set(pttstella::msg!("fatal-reopen", error = error));
                                let window = dioxus::desktop::window();
                                window.set_close_behavior(dioxus::desktop::WindowCloseBehaviour::WindowCloses);
                                window.set_visible(true);
                            }
                        });
                    },
                    canvas { id: "screen", "aria-label": pttstella::msg!("screen-label") }
                    div { id: "selection", "aria-hidden": "true" }
                    div { id: "navigation-marker", "aria-hidden": "true", hidden: true }
                    output { id: "surface-note", role: "status" }
                    textarea { id: "keyboard", "aria-label": pttstella::msg!("keyboard-label"), autocomplete: "off", autocapitalize: "off", "autocorrect": "off", wrap: "off", spellcheck: "false" }
                }
            }
            dialog { id: "filters-dialog", class: "paste-preview font-dialog", "aria-labelledby": "filters-title",
                h2 { id: "filters-title", {pttstella::msg!("filters-title")} }
                p { {pttstella::msg!("filters-description")} }
                p { {pttstella::msg!("filters-limitations")} }
                label { {pttstella::msg!("filters-saved-ids")} select { id: "filters-list", size: "5", "aria-label": pttstella::msg!("filters-saved-ids") } }
                label { {pttstella::msg!("filters-account-id")} input { id: "filters-id", maxlength: "12", autocomplete: "off", autocapitalize: "off", spellcheck: "false" } }
                label { {pttstella::msg!("filters-note")} input { id: "filters-note", maxlength: "128", autocomplete: "off" } }
                output { id: "filters-note-status", role: "status" }
                if filters_probe {
                    label { {pttstella::msg!("probe-clipboard")} textarea { id: "filters-copy-check", "aria-label": pttstella::msg!("probe-clipboard") } }
                }
                div { class: "dialog-actions",
                    button { id: "filters-remove", {pttstella::msg!("filters-remove")} }
                    button { id: "filters-save", {pttstella::msg!("filters-save")} }
                    button { id: "filters-close", {pttstella::msg!("close")} }
                }
            }
            dialog { id: "login-dialog", class: "paste-preview font-dialog", "aria-labelledby": "login-title",
                h2 { id: "login-title", {pttstella::msg!("login-title")} }
                p { {pttstella::msg!("login-description")} }
                label { {pttstella::msg!("account")} input { id: "login-account", maxlength: "12", autocomplete: "off", autocapitalize: "off", spellcheck: "false" } }
                label { {pttstella::msg!("password")} input { id: "login-password", r#type: "password", maxlength: "72", autocomplete: "off" } }
                label { input { id: "login-remember", r#type: "checkbox" } {pttstella::msg!("login-remember")} }
                p { id: "login-note", role: "status" }
                div { class: "dialog-actions",
                    button { id: "login-forget", {pttstella::msg!("login-forget")} }
                    button { id: "login-cancel", {pttstella::msg!("cancel")} }
                    button { id: "login-submit", {pttstella::msg!("login")} }
                }
            }
            dialog { id: "paste-dialog", class: "paste-preview",
                "data-active": if pending_paste.read().is_some() { "true" } else { "false" },
                "aria-labelledby": "paste-title",
                h2 { id: "paste-title", {pttstella::msg!("paste-title", paste_lines = paste_lines.to_string())} }
                pre { "{paste_text}" }
                div { class: "dialog-actions",
                    button { id: "paste-cancel", {pttstella::msg!("cancel")} }
                    button { id: "paste-send", {pttstella::msg!("paste-send")} }
                }
            }
            dialog { id: "media-dialog", class: "media-dialog", "aria-labelledby": "media-title",
                div { class: "media-toolbar",
                    h2 { id: "media-title", {pttstella::msg!("media-title")} }
                    button { id: "media-minus", "aria-label": pttstella::msg!("media-zoom-out"), "－" }
                    button { id: "media-plus", "aria-label": pttstella::msg!("media-zoom-in"), "＋" }
                    button { id: "media-fit", {pttstella::msg!("media-fit")} }
                    button { id: "media-original", {pttstella::msg!("media-original")} }
                    button { id: "media-motion", hidden: true, {pttstella::msg!("media-play")} }
                    button { id: "media-retry", {pttstella::msg!("retry")} }
                    button { id: "media-external", {pttstella::msg!("media-external")} }
                    button { id: "media-close", {pttstella::msg!("close")} }
                }
                p { id: "media-url" }
                output { id: "media-status", role: "status" }
                div { id: "media-viewport" }
            }
            aside { id: "media-hover", hidden:true, "aria-hidden":"true",
                output { id: "media-hover-status" }
                div { id: "media-hover-viewport" }
            }
            dialog { id: "font-dialog", class: "paste-preview font-dialog", "aria-labelledby": "font-title",
                h2 { id: "font-title", {pttstella::msg!("settings-title")} }
                label { {pttstella::msg!("language-label")}
                    select { id: "interface-language", "aria-label": pttstella::msg!("language-label"),
                        option { value: "zh-TW", "繁體中文" }
                        option { value: "en-US", "English" }
                    }
                }
                p { {pttstella::msg!("language-restart")} }
                label { {pttstella::msg!("font-preset")}
                    select { id: "font-preset",
                        option { value: "platform", {pttstella::msg!("font-platform")} }
                        option { value: "custom", {pttstella::msg!("font-custom")} }
                        option { value: "bundled", {pttstella::msg!("font-bundled")} }
                        option { value: "windows", {pttstella::msg!("font-windows-modern")} }
                        option { value: "windows_classic", {pttstella::msg!("font-windows-classic")} }
                        option { value: "macos_bbs", {pttstella::msg!("font-macos-bbs")} }
                        option { value: "macos", {pttstella::msg!("font-macos-sans")} }
                        option { value: "linux", "Linux" }
                        option { value: "macos_serif", {pttstella::msg!("font-macos-serif")} }
                    }
                }
                label { {pttstella::msg!("font-ascii")} input { id: "font-ascii", r#type: "text", maxlength: "128" } }
                label { {pttstella::msg!("font-local-ascii")} select { id: "font-local-ascii", disabled: true,
                    option { value: "", {pttstella::msg!("font-local-choose")} }
                } }
                label { {pttstella::msg!("font-cjk")} input { id: "font-cjk", r#type: "text", maxlength: "128" } }
                label { {pttstella::msg!("font-local-cjk")} select { id: "font-local-cjk", disabled: true,
                    option { value: "", {pttstella::msg!("font-local-choose")} }
                } }
                div { class: "dialog-actions", button { id: "font-refresh", {pttstella::msg!("font-refresh")} } }
                output { id: "font-local-status", role: "status" }
                label { {pttstella::msg!("font-size")} input { id: "font-size", r#type: "number", min: "8", max: "24", step: "1" } }
                label { {pttstella::msg!("font-baseline")} input { id: "font-baseline", r#type: "number", min: "0", max: "24", step: "1" } }
                canvas { id: "font-preview", width: "576", height: "192", "aria-label": pttstella::msg!("font-preview-label") }
                output { id: "font-note", role: "status" }
                output { id: "font-availability", role: "status" }
                p { {pttstella::msg!("font-help")} }
                details { id: "font-sources",
                    summary { {pttstella::msg!("font-source-title")} }
                    p { {pttstella::msg!("font-windows-explained")} }
                    p { {pttstella::msg!("font-source-help")} }
                    div { class: "dialog-actions font-sources",
                        for source in crate::fonts::Source::ALL {
                            { let help=source.help(); rsx! { button {
                                id: "font-source-{help.id}",
                                "data-font-source": help.id,
                                "data-font-families": help.families.join("\n"),
                                hidden: true,
                                {help.label}
                            } } }
                        }
                    }
                }
                details {
                    summary { {pttstella::msg!("font-license")} }
                    pre { class: "font-license", {include_str!("../../assets/fonts/NOTICE.txt")} }
                    pre { class: "font-license", {include_str!("../../assets/fonts/Cascadia-OFL.txt")} }
                    pre { class: "font-license", {include_str!("../../assets/fonts/NotoSansCJK-OFL.txt")} }
                }
                label { {pttstella::msg!("profile-name")} input { id: "font-profile-name", r#type: "text", maxlength: "128", value: pttstella::msg!("profile-default-name") } }
                div { class: "dialog-actions",
                    button { id: "profile-import", {pttstella::msg!("profile-import")} }
                    button { id: "profile-export", {pttstella::msg!("profile-export")} }
                }
                div { class: "dialog-actions",
                    button { id: "font-defaults", {pttstella::msg!("settings-defaults")} }
                    button { id: "font-cancel", {pttstella::msg!("cancel")} }
                    button { id: "font-save", {pttstella::msg!("settings-save")} }
                }
            }
        }
    }
}
