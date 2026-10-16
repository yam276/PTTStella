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
    let reload_settings = props.reload_settings;
    let media_origin = props.media_origin.clone();
    let owner = use_hook(move || {
        preview(probing, media_origin.as_deref())
            .and_then(|bytes| {
                if surface_probing {
                    Connection::start_surface_probe()
                } else if filters_probe {
                    Connection::start_filters_probe()
                } else if login_probe {
                    Connection::start_login_probe()
                } else if window_probe {
                    Connection::start_window_probe()
                } else if probing {
                    Connection::start_probe(bytes, reload_settings)
                } else {
                    let mut screen = Screen::new(80, 24).map_err(str::to_owned)?;
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
    let mut message = use_signal(|| "尚未連線 — 離線顯示預覽".to_owned());
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
                    onclick: move |_| { if let Err(error) = connect.connect() { message.set(error); } }, "連線" }
                button { disabled: fatal() || props.probe.is_some() || !matches!(phase(), Phase::Connecting | Phase::Connected),
                    onclick: move |_| { if let Err(error) = disconnect.disconnect() { message.set(error); } }, "斷線" }
                button { id: "zoom-out", "aria-label": "縮小", "－" }
                span { id: "zoom-level", "100%" }
                button { id: "zoom-in", "aria-label": "放大", "＋" }
                button { id: "zoom-fit", "符合視窗" }
                button { id: "select-all", "整頁選取" }
                button { id: "export-png", "匯出 PNG" }
                button { id: "export-rows", disabled: true, title: "先拖曳選取要截圖的行", "選取行 PNG" }
                button { id: "font-settings", "字型" }
                button { id: "filters-open", "一般黑名單" }
                button { id: "filters-reveal", hidden: true, "暫時顯示推文" }
                output { id: "filters-status", role: "status" }
                label { title: "滑過圖片連結會載入小預覽",
                    "圖片預覽 "
                    select { id: "hover-position", "aria-label": "滑過圖片連結時的預覽位置",
                        option { value: "top_right", "右上" }
                        option { value: "follow_cursor", "跟隨游標" }
                        option { value: "off", "關閉" }
                    }
                }
                button { id: "vim-toggle", "aria-pressed": "false", "Vim：關" }
                button { id: "vim-once", hidden: true, "原生一次 (f)" }
                output { id: "vim-status", role: "status" }
                  button { id: "keep-alive", r#type: "button", "aria-pressed": "true", "防閒置：開" }
                button { id: "login-open", disabled: !cfg!(target_os="macos"), "登入" }
                button { id: "login-stop", hidden: true, "取消登入" }
                div { id: "browse-controls", hidden: true,
                    button { id: "browse-back", disabled: true, "返回" }
                    button { id: "browse-previous", disabled: true, "上頁" }
                    button { id: "browse-next", disabled: true, "下頁" }
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
                                message.set(format!("{error}；請關閉並重新開啟視窗"));
                                let window = dioxus::desktop::window();
                                window.set_close_behavior(dioxus::desktop::WindowCloseBehaviour::WindowCloses);
                                window.set_visible(true);
                            }
                        });
                    },
                    canvas { id: "screen", "aria-label": "PTT 畫面" }
                    div { id: "selection", "aria-hidden": "true" }
                    div { id: "navigation-marker", "aria-hidden": "true", hidden: true }
                    output { id: "surface-note", role: "status" }
                    textarea { id: "keyboard", "aria-label": "PTT 鍵盤輸入", autocomplete: "off", autocapitalize: "off", "autocorrect": "off", wrap: "off", spellcheck: "false" }
                }
            }
            dialog { id: "filters-dialog", class: "paste-preview font-dialog", "aria-labelledby": "filters-title",
                h2 { id: "filters-title", "一般黑名單" }
                p { "遮黑已辨識的標準推文內容，保留作者與時間。ID 完整比對，忽略英文字母大小寫。" }
                p { "截斷、換行但沒有完整作者或時間的推文會保留原樣；關閉視窗不保存未送出的編輯。" }
                label { "已儲存帳號" select { id: "filters-list", size: "5", "aria-label": "已儲存帳號" } }
                label { "帳號 ID" input { id: "filters-id", maxlength: "12", autocomplete: "off", autocapitalize: "off", spellcheck: "false" } }
                label { "備註" input { id: "filters-note", maxlength: "128", autocomplete: "off" } }
                output { id: "filters-note-status", role: "status" }
                if filters_probe {
                    label { "測試剪貼簿讀回" textarea { id: "filters-copy-check", "aria-label": "測試剪貼簿讀回" } }
                }
                div { class: "dialog-actions",
                    button { id: "filters-remove", "移除並儲存" }
                    button { id: "filters-save", "新增／更新並儲存" }
                    button { id: "filters-close", "關閉" }
                }
            }
            dialog { id: "login-dialog", class: "paste-preview font-dialog", "aria-labelledby": "login-title",
                h2 { id: "login-title", "登入 PTT" }
                p { "按登入才送出帳密；只在 PTT 帳號／密碼提示操作。啟動與重連不自動登入。" }
                label { "帳號" input { id: "login-account", maxlength: "12", autocomplete: "off", autocapitalize: "off", spellcheck: "false" } }
                label { "密碼" input { id: "login-password", r#type: "password", maxlength: "72", autocomplete: "off" } }
                label { input { id: "login-remember", r#type: "checkbox" } "將此帳號與密碼存入 macOS 鑰匙圈" }
                p { id: "login-note", role: "status" }
                div { class: "dialog-actions",
                    button { id: "login-forget", "移除已存帳密" }
                    button { id: "login-cancel", "取消" }
                    button { id: "login-submit", "登入" }
                }
            }
            dialog { id: "paste-dialog", class: "paste-preview",
                "data-active": if pending_paste.read().is_some() { "true" } else { "false" },
                "aria-labelledby": "paste-title",
                h2 { id: "paste-title", "貼上 {paste_lines} 行文字" }
                pre { "{paste_text}" }
                div { class: "dialog-actions",
                    button { id: "paste-cancel", "取消" }
                    button { id: "paste-send", "送出文字" }
                }
            }
            dialog { id: "media-dialog", class: "media-dialog", "aria-labelledby": "media-title",
                div { class: "media-toolbar",
                    h2 { id: "media-title", "媒體預覽" }
                    button { id: "media-minus", "aria-label": "縮小媒體", "－" }
                    button { id: "media-plus", "aria-label": "放大媒體", "＋" }
                    button { id: "media-fit", "符合" }
                    button { id: "media-original", "原尺寸" }
                    button { id: "media-motion", hidden: true, "播放動圖" }
                    button { id: "media-retry", "重試" }
                    button { id: "media-external", "瀏覽器開啟" }
                    button { id: "media-close", "關閉" }
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
                h2 { id: "font-title", "字型與顯示" }
                label { "字型組合"
                    select { id: "font-preset",
                        option { value: "platform", "依平台初始值" }
                        option { value: "custom", "自訂" }
                        option { value: "bundled", "內建跨平台（隨 App 附帶）" }
                        option { value: "windows", "Windows" }
                        option { value: "macos", "macOS 黑體" }
                        option { value: "linux", "Linux" }
                        option { value: "macos_serif", "macOS 宋體" }
                    }
                }
                label { "英數字型" input { id: "font-ascii", r#type: "text", maxlength: "128" } }
                label { "中文字型" input { id: "font-cjk", r#type: "text", maxlength: "128" } }
                label { "字高" input { id: "font-size", r#type: "number", min: "8", max: "24", step: "1" } }
                label { "基線位置" input { id: "font-baseline", r#type: "number", min: "0", max: "24", step: "1" } }
                p { "內建跨平台組合可直接載入，無須安裝字型。其他組合使用本機字型；Windows 正黑體須另有合法安裝。" }
                details {
                    summary { "內建字型來源與授權（SIL OFL 1.1）" }
                    pre { class: "font-license", {include_str!("../../assets/fonts/NOTICE.txt")} }
                    pre { class: "font-license", {include_str!("../../assets/fonts/Cascadia-OFL.txt")} }
                    pre { class: "font-license", {include_str!("../../assets/fonts/NotoSansCJK-OFL.txt")} }
                }
                label { "設定檔名稱" input { id: "font-profile-name", r#type: "text", maxlength: "128", value: "我的顯示設定" } }
                div { class: "dialog-actions",
                    button { id: "profile-import", "載入設定檔至預覽" }
                    button { id: "profile-export", "匯出目前生效設定" }
                }
                canvas { id: "font-preview", width: "576", height: "192", "aria-label": "中英混排與雙色字預覽" }
                output { id: "font-note", role: "status" }
                output { id: "font-availability", role: "status" }
                div { class: "dialog-actions",
                    button { id: "font-defaults", "恢復預設" }
                    button { id: "font-cancel", "取消" }
                    button { id: "font-save", "套用並儲存" }
                }
            }
        }
    }
}
