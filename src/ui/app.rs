//! Dioxus view and native window lifecycle wiring.
use super::surface::{Options, PendingPaste, surface};
use base64::Engine;
use dioxus::prelude::*;
use dioxus_icons::lucide::{
    ALargeSmall, AlignVerticalJustifyEnd, ArrowDown, ArrowLeft, ArrowUp, Camera, Compass, Copy,
    ExternalLink, Eye, FileDown, FileText, FolderOpen, Globe, Image, Keyboard, Languages, Link,
    List, Plug, RefreshCw, RotateCcw, Save, Shield, Star, Trash2, Type, X,
};
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
    let tools_mark = use_hook(|| {
        format!(
            "data:image/svg+xml;base64,{}",
            base64::engine::general_purpose::STANDARD
                .encode(include_bytes!("../../assets/icons/constellation-flat.svg"))
        )
    });
    let probing = props.probe.is_some();
    let window_probe = props.window_probe;
    let login_probe = props.login_probe;
    let filters_probe = props.filters_probe;
    let surface_probe = props.surface_probe.clone();
    let surface_probing = surface_probe.is_some();
    let sample_case = surface_probe.clone();
    let boards_probing = matches!(surface_probe.as_deref(), Some("boards" | "boards_close"));
    let articles_probing = matches!(
        surface_probe.as_deref(),
        Some("articles" | "articles_close")
    );
    let verification_probing = surface_probe.as_deref() == Some("login_2fa");
    let dialogs_probing = matches!(
        surface_probe.as_deref(),
        Some(
            "dialogs"
                | "tools"
                | "sites"
                | "fonts"
                | "sgr66"
                | "sgr66_legacy"
                | "osc8"
                | "inline"
                | "context_menu"
        )
    );
    let reload_settings = props.reload_settings;
    let media_origin = props.media_origin.clone();
    let owner = use_hook(move || {
        super::probe::surface_sample(sample_case.as_deref(), probing, media_origin.as_deref())
            .and_then(|bytes| {
                if boards_probing {
                    Connection::start_board_probe()
                } else if articles_probing {
                    Connection::start_article_probe()
                } else if sample_case.as_deref() == Some("bookmarks") {
                    Connection::start_bookmarks_probe()
                } else if matches!(sample_case.as_deref(), Some("mouse" | "mouse_boundary")) {
                    Connection::start_mouse_probe(sample_case.as_deref() == Some("mouse_boundary"))
                } else if let Some(case) = match sample_case.as_deref() {
                    Some("mouse_drag") => Some(pttstella::connection::MouseSampleProbe::Drag),
                    Some("mouse_round_trip") => {
                        Some(pttstella::connection::MouseSampleProbe::RoundTrip)
                    }
                    Some("mouse_motion") => Some(pttstella::connection::MouseSampleProbe::Motion),
                    Some("mouse_wheel") => Some(pttstella::connection::MouseSampleProbe::Wheel),
                    _ => None,
                } {
                    Connection::start_mouse_samples_probe(case)
                } else if verification_probing {
                    Connection::start_verification_probe()
                } else if sample_case.as_deref() == Some("sites_restart") {
                    let screen = Screen::new(80, 24).map_err(str::to_owned)?;
                    Connection::start(screen)
                } else if surface_probing {
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
                button { id: "tools-ball", class: "tools-ball", r#type: "button",
                    "aria-label": pttstella::msg!("tools-toggle"), "aria-expanded": "false", "aria-controls": "tools-fan tools-panel",
                    title: pttstella::msg!("tools-drag-hint"),
                    img { class: "tools-mark", src: "{tools_mark}", alt: "", "aria-hidden": "true", draggable: "false" }
                    span { class: "tools-back-mark", "aria-hidden": "true", ArrowLeft { size: 28 } }
                }
                nav { id: "tools-fan", class: "tools-fan", hidden: true, "aria-label": pttstella::msg!("tools-title"),
                    ToolPetal { group: "navigate", label: pttstella::msg!("tools-navigate"), icon: rsx!{ Compass { size: 22 } } }
                    ToolPetal { group: "display", label: pttstella::msg!("tools-display"), icon: rsx!{ Type { size: 22 } } }
                    ToolPetal { group: "capture", label: pttstella::msg!("tools-capture"), icon: rsx!{ Camera { size: 22 } } }
                    ToolPetal { group: "input", label: pttstella::msg!("tools-input"), icon: rsx!{ Keyboard { size: 22 } } }
                    ToolPetal { group: "session", label: pttstella::msg!("tools-session"), icon: rsx!{ Plug { size: 22 } } }
                    ToolPetal { group: "filters", label: pttstella::msg!("tools-filters"), icon: rsx!{ Shield { size: 22 } } }
                }
                if fatal() || matches!(phase(), Phase::Error | Phase::Disconnected) {
                    output { class: "tools-alert", role: "status", "{message}" }
                }
            header { id: "tools-panel", hidden: true, "aria-label": pttstella::msg!("tools-title"),
                div { class: "brand", "PTTStella" }
                span { class: "status", title: "{message}", "{message}" }
                section { "data-tools-group": "session",
                button { disabled: fatal() || props.probe.is_some() || matches!(phase(), Phase::Connecting | Phase::Connected),
                    onclick: move |_| { if let Err(error) = connect.connect() { message.set(error); } }, {pttstella::msg!("connect")} }
                button { disabled: fatal() || props.probe.is_some() || !matches!(phase(), Phase::Connecting | Phase::Connected),
                    onclick: move |_| { if let Err(error) = disconnect.disconnect() { message.set(error); } }, {pttstella::msg!("disconnect")} }
                }
                section { "data-tools-group": "display",
                button { id: "zoom-out", "aria-label": pttstella::msg!("zoom-out"), "－" }
                span { id: "zoom-level", "100%" }
                button { id: "zoom-in", "aria-label": pttstella::msg!("zoom-in"), "＋" }
                button { id: "zoom-fit", {pttstella::msg!("zoom-fit")} }
                button { id: "font-settings", {pttstella::msg!("font-settings")} }
                }
                section { "data-tools-group": "capture",
                button { id: "select-all", {pttstella::msg!("select-all")} }
                button { id: "export-png", {pttstella::msg!("export-png")} }
                button { id: "export-rows", disabled: true, title: pttstella::msg!("export-rows-hint"), {pttstella::msg!("export-rows")} }
                }
                section { "data-tools-group": "filters",
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
                }
                section { "data-tools-group": "input",
                button { id: "vim-toggle", "aria-pressed": "false", {pttstella::msg!("vim-off")} }
                button { id: "vim-once", hidden: true, {pttstella::msg!("vim-once-button")} }
                output { id: "vim-status", role: "status" }
                button { id: "station-mouse", r#type: "button", "aria-pressed": "true", title: pttstella::msg!("station-mouse-help"), {pttstella::msg!("station-mouse-on")} }
                output { id: "station-mouse-status", role: "status" }
                }
                section { "data-tools-group": "session",
                button { id: "sites-open", SettingsCaption {label:pttstella::msg!("sites-title"),icon:rsx!{Globe{size:18}}} }
                output { id: "site-current" }
                  button { id: "keep-alive", r#type: "button", "aria-pressed": "true", {pttstella::msg!("keepalive-on")} }
                button { id: "login-open", "data-platform-supported":cfg!(target_os="macos").to_string(), disabled: !cfg!(target_os="macos"), {pttstella::msg!("login")} }
                button { id: "login-stop", hidden: true, {pttstella::msg!("login-cancel-attempt")} }
                }
                section { "data-tools-group": "navigate",
                button { id: "destination-open", {pttstella::msg!("destination-open")} }
                button { id: "bookmarks-open", {pttstella::msg!("bookmarks-title")} }
                button { id: "destination-stop", hidden: true, {pttstella::msg!("destination-stop")} }
                div { id: "browse-controls", hidden: true,
                    button { id: "browse-back", disabled: true, {pttstella::msg!("browse-back")} }
                    button { id: "browse-previous", disabled: true, {pttstella::msg!("browse-previous")} }
                    button { id: "browse-next", disabled: true, {pttstella::msg!("browse-next")} }
                }
                span { id: "browse-hint" }
                }
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
            {context_menu()}
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
                    button { id: "filters-save", class: "primary", {pttstella::msg!("filters-save")} }
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
                    button { id: "login-submit", class: "primary", {pttstella::msg!("login")} }
                }
            }
            dialog { id: "destination-dialog", class: "paste-preview", "aria-labelledby": "destination-title",
                h2 { id: "destination-title", {pttstella::msg!("destination-title")} }
                p { {pttstella::msg!("destination-dialog-hint")} }
                label { {pttstella::msg!("destination-name")} input { id: "destination-name", maxlength: (pttstella::article_target::MAX_INPUT+1).to_string(), autocomplete: "off", autocapitalize: "off", spellcheck: "false" } }
                p { id: "destination-note", role: "status" }
                h3 { {pttstella::msg!("recent-title")} }
                ul { id: "board-recent", "aria-label": pttstella::msg!("recent-title") }
                p { id: "board-recent-empty", {pttstella::msg!("recent-empty")} }
                p { id: "board-recent-note", role: "status" }
                button { id: "board-recent-clear", {pttstella::msg!("recent-clear")} }
                div { class: "dialog-actions",
                    button { id: "destination-cancel", {pttstella::msg!("cancel")} }
                    button { id: "destination-submit", class: "primary", {pttstella::msg!("destination-go")} }
                }
            }
            dialog { id: "bookmarks-dialog", class: "paste-preview font-dialog", "aria-labelledby": "bookmarks-title",
                h2 { id: "bookmarks-title", {pttstella::msg!("bookmarks-title")} }
                p { {pttstella::msg!("bookmarks-description")} }
                label { {pttstella::msg!("bookmarks-list")} select { id: "bookmarks-list", size: "6", "aria-label": pttstella::msg!("bookmarks-list") } }
                button { id: "bookmarks-new", {pttstella::msg!("bookmarks-new")} }
                label { {pttstella::msg!("bookmarks-target")} input { id: "bookmarks-target", maxlength: (pttstella::article_target::MAX_INPUT+1).to_string(), autocomplete: "off", autocapitalize: "off", spellcheck: "false" } }
                label { {pttstella::msg!("bookmarks-label")} input { id: "bookmarks-label", maxlength: (pttstella::bookmarks::LABEL_LIMIT*2+1).to_string(), autocomplete: "off" } }
                p { id: "bookmarks-note", role: "status" }
                div { class: "dialog-actions",
                    button { id: "bookmarks-remove", {pttstella::msg!("bookmarks-remove")} }
                    button { id: "bookmarks-save", class: "primary", {pttstella::msg!("bookmarks-save")} }
                    button { id: "bookmarks-go", class: "primary", {pttstella::msg!("bookmarks-go")} }
                    button { id: "bookmarks-close", {pttstella::msg!("close")} }
                }
            }
            dialog { id: "paste-dialog", class: "paste-preview",
                "data-active": if pending_paste.read().is_some() { "true" } else { "false" },
                "aria-labelledby": "paste-title",
                h2 { id: "paste-title", {pttstella::msg!("paste-title", paste_lines = paste_lines.to_string())} }
                pre { "{paste_text}" }
                div { class: "dialog-actions",
                    button { id: "paste-cancel", {pttstella::msg!("cancel")} }
                    button { id: "paste-send", class: "primary", {pttstella::msg!("paste-send")} }
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
            dialog { id: "sites-dialog", class: "paste-preview font-dialog", "aria-labelledby": "sites-title",
                h2 { id: "sites-title", SettingsCaption { label:pttstella::msg!("sites-title"),icon:rsx!{Globe{size:18}} } }
                label { {pttstella::msg!("sites-favourites")} select {id:"sites-list"} }
                output {id:"sites-default"}
                label { {pttstella::msg!("sites-name")} input {id:"site-name",r#type:"text",maxlength:"128"} }
                label { {pttstella::msg!("sites-address")} input {id:"site-address",r#type:"text",maxlength:"512",spellcheck:"false",autocapitalize:"off"} }
                label { {pttstella::msg!("sites-encoding")} select {id:"site-encoding",
                    option {value:"uao","Big5-UAO"} option {value:"big5","Big5"} option {value:"gbk","GBK"}
                } }
                p {id:"sites-transport"}
                p { {pttstella::msg!("sites-help")} }
                div {class:"dialog-actions settings-actions",
                    button {id:"site-save", SettingsCaption {label:pttstella::msg!("sites-save"),icon:rsx!{Star{size:18}}} }
                    button {id:"site-remove", SettingsCaption {label:pttstella::msg!("sites-remove"),icon:rsx!{Trash2{size:18}}} }
                    button {id:"site-default", SettingsCaption {label:pttstella::msg!("sites-set-default"),icon:rsx!{Save{size:18}}} }
                }
                output {id:"sites-note",role:"status"}
                div {class:"dialog-actions settings-actions",
                    button {id:"sites-close",SettingsCaption {label:pttstella::msg!("close"),icon:rsx!{X{size:18}}} }
                    button {id:"site-connect",class:"primary",SettingsCaption {label:pttstella::msg!("sites-connect"),icon:rsx!{Plug{size:18}}} }
                }
            }
            dialog { id: "font-dialog", class: "paste-preview font-dialog", "aria-labelledby": "font-title",
                h2 { id: "font-title", {pttstella::msg!("settings-title")} }
                section { class: "settings-section",
                    label {
                        SettingsCaption { label: pttstella::msg!("language-label"), icon: rsx! { Languages { size: 18 } } }
                        select { id: "interface-language", "aria-describedby": "settings-language-help",
                            option { value: "zh-TW", "繁體中文" }
                            option { value: "en-US", "English" }
                        }
                    }
                    p { id: "settings-language-help", class: "settings-help", {pttstella::msg!("language-restart")} }
                    label {
                        SettingsCaption { label: pttstella::msg!("settings-opacity"), icon: rsx! { Eye { size: 18 } } }
                        div { class: "settings-range",
                            input { id: "tools-opacity", r#type: "range", min: "20", max: "100", step: "1", value: "100", "aria-label": pttstella::msg!("settings-opacity"), "aria-describedby": "settings-opacity-help" }
                            output { id: "tools-opacity-value", "for": "tools-opacity" }
                        }
                    }
                    p { id: "settings-opacity-help", class: "settings-help", {pttstella::msg!("settings-opacity-help")} }
                }
                section { class: "settings-section",
                    label {
                        SettingsCaption { label: pttstella::msg!("font-preset"), icon: rsx! { Type { size: 18 } } }
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
                    label {
                        SettingsCaption { label: pttstella::msg!("font-ascii"), icon: rsx! { Type { size: 18 } } }
                        input { id: "font-ascii", r#type: "text", maxlength: "128" }
                    }
                    label {
                        SettingsCaption { label: pttstella::msg!("settings-local-ascii"), icon: rsx! { List { size: 18 } } }
                        select { id: "font-local-ascii", disabled: true,
                            option { value: "", {pttstella::msg!("font-local-choose")} }
                        }
                    }
                    label {
                        SettingsCaption { label: pttstella::msg!("font-cjk"), icon: rsx! { Type { size: 18 } } }
                        input { id: "font-cjk", r#type: "text", maxlength: "128" }
                    }
                    label {
                        SettingsCaption { label: pttstella::msg!("settings-local-cjk"), icon: rsx! { List { size: 18 } } }
                        select { id: "font-local-cjk", disabled: true,
                            option { value: "", {pttstella::msg!("font-local-choose")} }
                        }
                    }
                    div { class: "dialog-actions settings-actions",
                        button { id: "font-refresh", title: pttstella::msg!("font-refresh"),
                            SettingsCaption { label: pttstella::msg!("settings-refresh"), icon: rsx! { RefreshCw { size: 18 } } }
                        }
                    }
                    output { id: "font-local-status", role: "status" }
                }
                section { class: "settings-section",
                    label {
                        SettingsCaption { label: pttstella::msg!("font-size"), icon: rsx! { ALargeSmall { size: 18 } } }
                        input { id: "font-size", r#type: "number", min: "8", max: "24", step: "1" }
                    }
                    label {
                        SettingsCaption { label: pttstella::msg!("font-baseline"), icon: rsx! { AlignVerticalJustifyEnd { size: 18 } } }
                        input { id: "font-baseline", r#type: "number", min: "0", max: "24", step: "1", "aria-describedby": "settings-baseline-help" }
                    }
                    p { id: "settings-baseline-help", class: "settings-help", {pttstella::msg!("settings-baseline-help")} }
                    p { class: "settings-preview-label",
                        SettingsCaption { label: pttstella::msg!("settings-preview"), icon: rsx! { Eye { size: 18 } } }
                    }
                    canvas { id: "font-preview", width: "576", height: "192", "aria-label": pttstella::msg!("font-preview-label") }
                    output { id: "font-note", role: "status" }
                    output { id: "font-availability", role: "status" }
                    p { class: "settings-help", {pttstella::msg!("font-help")} }
                }
                details { id: "font-sources",
                    summary {
                        SettingsCaption { label: pttstella::msg!("font-source-title"), icon: rsx! { ExternalLink { size: 18 } } }
                    }
                    p { {pttstella::msg!("font-windows-explained")} }
                    p { {pttstella::msg!("font-source-help")} }
                    div { class: "dialog-actions font-sources settings-actions",
                        for source in crate::fonts::Source::ALL {
                            { let help=source.help(); rsx! { button {
                                id: "font-source-{help.id}",
                                "data-font-source": help.id,
                                "data-font-families": help.families.join("\n"),
                                hidden: true,
                                SettingsCaption { label: help.label, icon: rsx! { ExternalLink { size: 18 } } }
                            } } }
                        }
                    }
                }
                details {
                    summary {
                        SettingsCaption { label: pttstella::msg!("settings-licenses"), icon: rsx! { FileText { size: 18 } } }
                    }
                    p { class: "settings-help", {pttstella::msg!("font-license")} }
                    pre { class: "font-license", {include_str!("../../assets/fonts/NOTICE.txt")} }
                    pre { class: "font-license", {include_str!("../../assets/fonts/Cascadia-OFL.txt")} }
                    pre { class: "font-license", {include_str!("../../assets/fonts/NotoSansCJK-OFL.txt")} }
                }
                section { class: "settings-section",
                    label {
                        SettingsCaption { label: pttstella::msg!("profile-name"), icon: rsx! { FileText { size: 18 } } }
                        input { id: "font-profile-name", r#type: "text", maxlength: "128", value: pttstella::msg!("profile-default-name") }
                    }
                    div { class: "dialog-actions settings-actions",
                        button { id: "profile-import", title: pttstella::msg!("profile-import"), "aria-describedby": "settings-profile-help",
                            SettingsCaption { label: pttstella::msg!("settings-import"), icon: rsx! { FolderOpen { size: 18 } } }
                        }
                        button { id: "profile-export", title: pttstella::msg!("profile-export"), "aria-describedby": "settings-profile-help",
                            SettingsCaption { label: pttstella::msg!("settings-export"), icon: rsx! { FileDown { size: 18 } } }
                        }
                    }
                    p { id: "settings-profile-help", class: "settings-help", {pttstella::msg!("settings-profile-help")} }
                }
                div { class: "dialog-actions settings-actions settings-footer",
                    button { id: "font-defaults",
                        SettingsCaption { label: pttstella::msg!("settings-defaults"), icon: rsx! { RotateCcw { size: 18 } } }
                    }
                    button { id: "font-cancel",
                        SettingsCaption { label: pttstella::msg!("cancel"), icon: rsx! { X { size: 18 } } }
                    }
                    button { id: "font-save", class: "primary",
                        SettingsCaption { label: pttstella::msg!("settings-save"), icon: rsx! { Save { size: 18 } } }
                    }
                }
            }
        }
    }
}

#[component]
fn ToolPetal(group: &'static str, label: String, icon: Element) -> Element {
    rsx! {
        button { class: "tools-petal", r#type: "button", "data-tools-target": group,
            "aria-controls": "tools-panel", "aria-expanded": "false",
            {icon} span { "{label}" }
        }
    }
}

#[component]
fn SettingsCaption(label: String, icon: Element) -> Element {
    rsx! {
        span { class: "settings-caption",
            span { class: "settings-icon", "aria-hidden": "true", {icon} }
            span { "{label}" }
        }
    }
}

// Static, localized controls; the surface owner supplies context and admission.
fn context_menu() -> Element {
    rsx! {
        div { id: "surface-context-menu", role: "menu", hidden: true,
            "aria-label": pttstella::msg!("context-title"),
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "copy",
                Copy { size: 18 } span { {pttstella::msg!("context-copy")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "copy-link",
                Link { size: 18 } span { {pttstella::msg!("context-copy-link")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "open-link",
                ExternalLink { size: 18 } span { {pttstella::msg!("context-open-link")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "select-link",
                Link { size: 18 } span { {pttstella::msg!("context-select-link")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "select-all",
                FileText { size: 18 } span { {pttstella::msg!("context-select-all")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "export-rows",
                Camera { size: 18 } span { {pttstella::msg!("context-export-rows")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "export-page",
                Camera { size: 18 } span { {pttstella::msg!("context-export-page")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "inline",
                Image { size: 18 } span { {pttstella::msg!("context-inline")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "back",
                ArrowLeft { size: 18 } span { {pttstella::msg!("context-back")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "previous",
                ArrowUp { size: 18 } span { {pttstella::msg!("context-previous")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "next",
                ArrowDown { size: 18 } span { {pttstella::msg!("context-next")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "bookmarks",
                Star { size: 18 } span { {pttstella::msg!("context-bookmarks")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "filters",
                Shield { size: 18 } span { {pttstella::msg!("context-filters")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "vim",
                Keyboard { size: 18 } span { {pttstella::msg!("context-vim")} }
            }
            button { r#type: "button", role: "menuitem", tabindex: "-1", "data-context-action": "settings",
                ALargeSmall { size: 18 } span { {pttstella::msg!("context-settings")} }
            }
        }
    }
}
