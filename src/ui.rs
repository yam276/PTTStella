use base64::Engine;
use dioxus::prelude::*;
use pttstella::{
    connection::{Connection, Phase, View},
    encoding, export,
    input::{self, PreparedPaste},
    links::{self, Link},
    media::{self, preview::Preview},
    navigation::{self, Context, Intent, Stamp},
    screen::{Frame, Screen},
    selection::{self, Selection},
    settings::{DisplaySettings, store::Store},
    storage,
    vim::{self, Effect},
};
use serde::{Deserialize, Serialize};
use std::{ops::Range, path::PathBuf, sync::Arc};
use tokio::sync::watch;

#[derive(Clone, PartialEq, Props)]
pub struct AppProps {
    pub probe: Option<PathBuf>,
    pub reload_settings: bool,
    pub media_origin: Option<String>,
}

#[derive(Clone, Serialize)]
struct Paint {
    keep_alive: bool,
    keep_alive_ready: bool,
    keep_alive_sent: u64,
    keep_alive_failure: Option<String>,
    input_epoch: u64,
    input_pending: bool,
    connected: bool,
    navigation: Context,
    frame: Frame,
    generation: u64,
    probe: bool,
    links: Vec<Link>,
    settings: DisplaySettings,
}
#[derive(Clone)]
struct PendingPaste {
    generation: u64,
    prepared: PreparedPaste,
}
struct SelectedRows {
    generation: u64,
    revision: u64,
    request: u64,
    rows: Range<usize>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum MediaAction {
    Close,
    Retry,
    External,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SurfaceEvent {
    KeyboardState {
        action: String,
        #[serde(default)]
        code: String,
        generation: u64,
        trusted: bool,
    },
    MediaDecoded {
        request: u64,
        success: bool,
    },
    Media {
        request: u64,
        action: MediaAction,
        trusted: bool,
    },
    KeepAlive {
        enabled: bool,
        trusted: bool,
    },
    KeepAliveGate {
        stamp: Stamp,
        ready: bool,
    },
    Navigate {
        stamp: Stamp,
        intent: Intent,
        trusted: bool,
    },
    SettingsOpen {
        trusted: bool,
    },
    SettingsPreview {
        request: u64,
        values: DisplaySettings,
    },
    SettingsSave {
        values: DisplaySettings,
        trusted: bool,
    },
    ExportPng {
        generation: u64,
        revision: u64,
        selection_request: Option<u64>,
        png: String,
        trusted: bool,
    },
    OpenLink {
        generation: u64,
        revision: u64,
        index: usize,
        trusted: bool,
    },
    Paste {
        text: String,
        generation: u64,
        trusted: bool,
    },
    PasteConfirm {
        trusted: bool,
    },
    PasteCancel {
        trusted: bool,
    },
    Select {
        generation: u64,
        revision: u64,
        request: u64,
        selection: Selection,
    },
    Copied {
        generation: u64,
        revision: u64,
        request: u64,
        trusted: bool,
    },
    Key {
        key: vim::Key,
        generation: u64,
        trusted: bool,
    },
    Text {
        text: String,
        generation: u64,
        trusted: bool,
        #[serde(default)]
        composition_input: bool,
        #[serde(default)]
        composed: bool,
    },
    Painted {
        revision: u64,
        generation: u64,
    },
    Probe(Box<ProbePixels>),
    Error {
        message: String,
    },
    Stopped {
        clean: bool,
    },
}

#[derive(Deserialize)]
struct ProbePixels {
    png: String,
    red_pixels: usize,
    green_pixels: usize,
    left_bg: Vec<u8>,
    right_bg: Vec<u8>,
    fonts: String,
    dpr: f64,
    pixel_size: [u32; 2],
    font_stack: String,
    fills_body: bool,
    colour_samples: Vec<Vec<u8>>,
    art_seam_pixels: usize,
    art_zoom_seams: Vec<usize>,
    art_geometry: bool,
    hover_controls: bool,
    proportional_zoom: bool,
    zoom_samples: Vec<[u32; 2]>,
}

fn preview(probe: bool, media_origin: Option<&str>) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let text = "PTTStella\r\n\r\n尚未連線，歡迎使用 PTTStella。";
    bytes.extend_from_slice(&encoding::encode(text).map_err(|e| e.to_string())?);
    bytes.extend_from_slice(b"\x1b[4;1H\x1b[31;44m\xa4\x1b[32;45m\xa4\x1b[0m  ");
    bytes.extend_from_slice(&encoding::encode("雙色字顯示預覽").map_err(|e| e.to_string())?);
    bytes.extend_from_slice(b"\x1b[7;1H");
    bytes.extend_from_slice(&encoding::encode("正在連線 PTT…\r\n\r\n連線後，點一下畫面即可輸入帳號密碼。\r\n滑鼠移至上緣可顯示連線與等比例縮放工具。").map_err(|e| e.to_string())?);
    if probe {
        if let Some(origin) = media_origin {
            for (row, path) in [
                (5, "colours.jpg"),
                (6, "colours.webp"),
                (8, "colours.png"),
                (9, "motion.gif"),
                (10, "movie.webm"),
                (11, "broken.png"),
                (14, "slow.png"),
            ] {
                bytes.extend_from_slice(format!("\x1b[{row};1H\x1b[2K{origin}/{path} ").as_bytes());
            }
            bytes.extend_from_slice(b"\x1b[18;1Hhttps://youtu.be/Abc_12-dEF0 ");
        }
        let solid = encoding::encode("█").map_err(|e| e.to_string())?;
        bytes.extend_from_slice(b"\x1b[12;1Hhttps://example.com/pttstella");
        bytes.extend_from_slice(b"\x1b[13;1H");
        bytes.extend_from_slice(&encoding::encode("●○◎⊙").map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[15;1H");
        bytes.extend_from_slice(&encoding::encode("一般色 / 亮色").map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[16;1H");
        for colour in 0..16 {
            let fg = if colour < 8 {
                30 + colour
            } else {
                90 + colour - 8
            };
            let bg = if colour == 0 { 47 } else { 40 };
            bytes.extend_from_slice(format!("\x1b[0;{fg};{bg}m").as_bytes());
            bytes.extend_from_slice(&solid);
        }
        // Also exercise intensity-on, intensity-off and full reset independently.
        bytes.extend_from_slice(b"\x1b[17;1H");
        for sgr in ["0;31", "1", "22", "0"] {
            bytes.extend_from_slice(format!("\x1b[{sgr}m").as_bytes());
            bytes.extend_from_slice(&solid);
        }
        bytes.extend_from_slice(b"\x1b[0;31;44m\x1b[20;1H");
        bytes.extend_from_slice(&encoding::encode(&"█".repeat(8)).map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[21;1H");
        bytes.extend_from_slice(&encoding::encode(&"█".repeat(8)).map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[22;1H");
        bytes.extend_from_slice(&encoding::encode("▄▌◢◣◤◥").map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[23;1H");
        bytes.extend_from_slice(&encoding::encode("▁▂▃▄▅▆▇█").map_err(|e| e.to_string())?);
        bytes.extend_from_slice(b"\x1b[0m\x1b[19;1H");
    }
    Ok(bytes)
}

pub fn app(props: AppProps) -> Element {
    let probing = props.probe.is_some();
    let reload_settings = props.reload_settings;
    let media_origin = props.media_origin.clone();
    let owner = use_hook(move || {
        preview(probing, media_origin.as_deref())
            .and_then(|bytes| {
                if probing {
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
        if matches!(
            event,
            dioxus::desktop::tao::event::Event::WindowEvent {
                event: dioxus::desktop::WindowEvent::CloseRequested,
                ..
            }
        ) {
            close_sender.send_replace(true);
        }
    });
    let probe = props.probe.clone();
    let media_origin = props.media_origin.clone();
    let connect = connection.clone();
    let disconnect = connection.clone();
    rsx! {
        style { {include_str!("../assets/app.css")} }
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
                button { id: "vim-toggle", "aria-pressed": "false", "Vim：關" }
                button { id: "vim-once", hidden: true, "原生一次 (f)" }
                output { id: "vim-status", role: "status" }
                  button { id: "keep-alive", r#type: "button", "aria-pressed": "true", "防閒置：開" }
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
                        spawn(async move {
                            let result = surface(connection.clone(), closing, probe, message, phase, pending_paste, (reload_settings, media_origin)).await;
                            if let Err(error) = result {
                                fatal.set(true);
                                message.set(format!("{error}；請關閉並重新開啟視窗"));
                                if let Err(cleanup) = connection.shutdown().await { message.set(cleanup); }
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
            dialog { id: "font-dialog", class: "paste-preview font-dialog", "aria-labelledby": "font-title",
                h2 { id: "font-title", "字型與顯示" }
                label { "字型組合"
                    select { id: "font-preset",
                        option { value: "custom", "自訂" }
                        option { value: "windows", "Windows" }
                        option { value: "macos", "macOS" }
                        option { value: "linux", "Linux" }
                    }
                }
                label { "英數字型" input { id: "font-ascii", r#type: "text", maxlength: "128" } }
                label { "中文字型" input { id: "font-cjk", r#type: "text", maxlength: "128" } }
                label { "字高" input { id: "font-size", r#type: "number", min: "8", max: "24", step: "1" } }
                label { "基線位置" input { id: "font-baseline", r#type: "number", min: "0", max: "24", step: "1" } }
                p { "保持固定格線；找不到指定字型時使用系統替代字型。" }
                canvas { id: "font-preview", width: "576", height: "192", "aria-label": "中英混排與雙色字預覽" }
                output { id: "font-note", role: "status" }
                div { class: "dialog-actions",
                    button { id: "font-defaults", "恢復預設" }
                    button { id: "font-cancel", "取消" }
                    button { id: "font-save", "套用並儲存" }
                }
            }
        }
    }
}

async fn surface(
    connection: Arc<Connection>,
    mut closing: watch::Receiver<bool>,
    probe: Option<PathBuf>,
    mut message: Signal<String>,
    mut phase: Signal<Phase>,
    mut pending_paste: Signal<Option<PendingPaste>>,
    options: (bool, Option<String>),
) -> Result<(), String> {
    let (reload_settings, media_origin) = options;
    let mut receiver = connection.updates.clone();
    let store = probe.as_ref().map_or_else(Store::system, |path| {
        Ok(Store::at(path.join("probe-settings.json")))
    });
    let (mut settings, mut settings_warning) =
        match store.as_ref().map_err(Clone::clone).and_then(Store::load) {
            Ok(settings) => (settings, None),
            Err(error) => (
                DisplaySettings::default(),
                Some(format!("{error}；目前使用預設顯示")),
            ),
        };
    let font_sample = font_sample()?;
    let script = [
        include_str!("../assets/media.js"),
        include_str!("../assets/surface.js"),
    ]
    .concat();
    let mut eval = document::eval(&script);
    let mut media_preview = Preview::default();
    let mut keyboard_policy = vim::Keyboard::default();
    let mut keyboard_generation = 0;
    let mut media_external = 0;
    let first = receiver.borrow_and_update().clone();
    let mut last_status = first.message.clone();
    let mut painting = paint(&first, false, &settings);
    let mut displayed: Option<Paint> = None;
    eval.send(&painting).map_err(|e| e.to_string())?;
    if let Some(warning) = &settings_warning {
        eval.send(serde_json::json!({"notice":warning}))
            .map_err(|e| e.to_string())?;
    }
    connection.connect()?;
    let mut in_flight = Some((first.generation, first.frame.revision));
    let mut dirty = false;
    let mut stopping = false;
    let mut close_deadline = None;
    let mut arrow_checked = false;
    let mut input_bytes: Vec<Vec<u8>> = Vec::new();
    let expected_input: Vec<Vec<u8>> = vec![
        vec![97],
        vec![98],
        vec![99],
        vec![0xa4, 0xa4],
        vec![0xa4, 0xa4],
        vec![110],
    ];
    let expected_enter_count = 3;
    let mut enter_count = 0;
    let mut closed_checked = false;
    let mut probe_started = false;
    let mut copy_checked = false;
    let mut composition_checked = false;
    let mut link_requests = 0;
    let mut export_checked = false;
    let mut rows_export_checked = false;
    let mut selected_rows: Option<SelectedRows> = None;
    let mut settings_saved = false;
    let mut navigation_requests = 0;
    let (mut paste_single, mut paste_confirm, mut paste_cancel, mut paste_rejected) =
        (false, false, false, 0);
    let mut copied_selection = None;
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
        let interactions_checked = arrow_checked
            && enter_count == expected_enter_count
            && input_bytes == expected_input
            && complete
            && closed_checked
            && copy_checked
            && composition_checked
            && link_requests == 1
            && export_checked
            && rows_export_checked
            && settings_saved
            && navigation_requests == 8
            && paste_single
            && paste_confirm
            && paste_cancel
            && paste_rejected == 3;
        let probe_checked = if reload_settings {
            probe_started && in_flight.is_none()
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
                && !probe_started
                && view.phase == Phase::Connected
                && view.received > 0;
            probe_started |= run_probe;
            painting = paint(&view, run_probe, &settings);
            eval.send(&painting).map_err(|e| e.to_string())?;
            in_flight = Some((view.generation, view.frame.revision));
            dirty = false;
        }
        tokio::select! {
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
                SurfaceEvent::KeyboardState {action,code,generation,trusted} => {
                    // Ending an owned edit only revokes permission; WebView CDP can
                    // report its compositionend as untrusted, including cancellation.
                    if stopping || (!trusted && action!="composition_end") || generation!=receiver.borrow().generation {continue;}
                    match action.as_str() {
                        "toggle"=>keyboard_policy.toggle(), "once"=>keyboard_policy.arm(),
                        "release"=>keyboard_policy.release(&code),
                        "composition_start"=>keyboard_policy.composition_start(),
                        "composition_end"=>keyboard_policy.composition_end(),
                        "independent_edit"=>keyboard_policy.independent_edit(),
                        "reset"=>keyboard_policy.reset(), _=>continue,
                    }
                    let context=navigation::analyze(&receiver.borrow().frame);
                    eval.send(serde_json::json!({"keyboard_state":keyboard_policy.status(&context)})).map_err(|e|e.to_string())?;
                },
                SurfaceEvent::MediaDecoded {request,success} => {
                    media_preview.decoded(request,success);
                },
                SurfaceEvent::KeepAlive {enabled,trusted} => {
                    if stopping || !trusted {continue;}
                    if let Err(error)=connection.keep_alive(enabled) {
                        eval.send(serde_json::json!({"notice":error})).map_err(|e|e.to_string())?;
                    }
                },
                SurfaceEvent::KeepAliveGate {stamp,ready} => {
                    if !stopping {connection.keep_alive_gate(stamp,ready && !keyboard_policy.status(&navigation::analyze(&receiver.borrow().frame)).pending);}
                },
                SurfaceEvent::Navigate {stamp,intent,trusted} => {
                    if stopping || !trusted { continue; }
                    let shown=displayed.as_ref().is_some_and(|p| p.generation==stamp.generation && p.frame.revision==stamp.revision && p.input_epoch==stamp.input_epoch);
                    let result=if !shown {Err(navigation::Error::Stale.to_string())}
                        else if pending_paste.read().is_some() {Err(navigation::Error::Unavailable.to_string())}
                        else {connection.navigate(stamp,intent).await};
                    if result.is_ok() && probe.is_some() {navigation_requests+=1;}
                    eval.send(match result {
                        Ok(())=>serde_json::json!({"navigation_result":"sent"}),
                        Err(error)=>serde_json::json!({"navigation_result":"rejected","notice":error}),
                    }).map_err(|e|e.to_string())?;
                },
                SurfaceEvent::SettingsOpen { trusted } => {
                    if stopping || !trusted { continue; }
                    eval.send(serde_json::json!({"settings_open": {
                        "values":settings,"defaults":DisplaySettings::default(),"presets":DisplaySettings::presets(),
                        "warning":settings_warning,
                        "sample":sample_packet(&font_sample,&settings),
                    }})).map_err(|e| e.to_string())?;
                },
                SurfaceEvent::SettingsPreview { request, values } => {
                    let reply = match values.validate() {
                        Ok(values) => serde_json::json!({"font_preview":{"request":request,"packet":sample_packet(&font_sample,&values)}}),
                        Err(error) => serde_json::json!({"font_preview":{"request":request,"error":error}}),
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                },
                SurfaceEvent::SettingsSave { values, trusted } => {
                    if stopping || !trusted { continue; }
                    let result = store.as_ref().map_err(Clone::clone).and_then(|store| store.save(values));
                    let reply = match result {
                        Ok(saved) => {
                            settings_saved |= probe.is_some() && saved.font_size==18.0 && saved.baseline==19.0;
                            settings=saved;settings_warning=None;dirty=true;
                            serde_json::json!({"settings_saved":true,"notice":"顯示設定已儲存"})
                        },
                        Err(error) => serde_json::json!({"settings_saved":false,"notice":error}),
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                },
                SurfaceEvent::ExportPng { generation, revision, selection_request, png, trusted } => {
                    if stopping || !trusted { continue; }
                    let result = (|| -> Result<Option<PathBuf>, String> {
                        let shown = displayed.as_ref().filter(|p| p.generation == generation && p.frame.revision == revision)
                            .ok_or("畫面已更新，請重新選取後匯出")?;
                        if (shown.frame.cols, shown.frame.rows) != (80, export::ROWS) { return Err("此畫面尺寸不支援 PNG 匯出".into()); }
                        let rows = match selection_request {
                            Some(request) => selected_rows.as_ref()
                                .filter(|s| s.generation == generation && s.revision == revision && s.request == request)
                                .map(|s| s.rows.clone()).ok_or("選取已變更，請重新選取後匯出")?,
                            None => 0..export::ROWS,
                        };
                        let encoded = png.strip_prefix("data:image/png;base64,").ok_or("PNG 格式不正確")?;
                        if encoded.len() > export::MAX_BASE64_BYTES { return Err("PNG 超過匯出大小上限".into()); }
                        let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|_| "PNG 編碼不正確")?;
                        export::check_export_header(&bytes, &rows).map_err(str::to_owned)?;
                        let path = if let Some(directory) = &probe {
                            Some(directory.join(if selection_request.is_some() {"export-rows.png"} else {"export.png"}))
                        } else {
                            // Parent-owned synchronous dialog: no detached native dialog task.
                            let window = dioxus::desktop::window();
                            rfd::FileDialog::new().set_parent(window.window.as_ref())
                                .set_title("匯出 PTT 畫面").add_filter("PNG", &["png"])
                                .set_file_name(if selection_request.is_some() {
                                    format!("PTTStella-lines-{}-{}.png", rows.start + 1, rows.end)
                                } else {"PTTStella.png".into()}).save_file()
                        };
                        if let Some(path) = &path {
                            if !path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("png")) {
                                return Err("請使用 .png 副檔名；尚未儲存".into());
                            }
                            storage::replace(path, &bytes)?;
                        }
                        Ok(path)
                    })();
                    let reply = match result {
                        Ok(Some(_)) => {
                            export_checked |= probe.is_some() && selection_request.is_none();
                            rows_export_checked |= probe.is_some() && selection_request.is_some();
                            serde_json::json!({"export_result":"saved","notice":"PNG 已儲存"})
                        },
                        Ok(None) => serde_json::json!({"export_result":"cancelled","notice":"未選擇儲存位置"}),
                        Err(error) => serde_json::json!({"export_result":"failed","notice":error}),
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                },
                SurfaceEvent::OpenLink { generation, revision, index, trusted } => {
                    if stopping || !trusted { continue; }
                    let link = displayed.as_ref()
                        .filter(|p| p.generation == generation && p.frame.revision == revision)
                        .and_then(|p| p.links.get(index)).cloned();
                    if let Some(link) = &link {
                        match media::classify(&link.url) {
                            Ok(Some(mut target)) => {
                                if probe.is_some() {
                                    let origin = media_origin.as_deref().ok_or("missing media probe origin")?;
                                    if target.kind == media::Kind::Youtube {
                                        target.image_url = Some(format!("{origin}/colours.png"));
                                    } else if !target.url.starts_with(&format!("{origin}/")) {
                                        return Err("unexpected media probe URL".into());
                                    }
                                }
                                let active = media_preview.open(generation,target)?;
                                eval.send(serde_json::json!({"media_open":{"request":active.request,"target":active.target}})).map_err(|e| e.to_string())?;
                                continue;
                            },
                            Err(error) => {
                                eval.send(serde_json::json!({"notice":error,"link_result":"failed"})).map_err(|e| e.to_string())?;
                                continue;
                            },
                            Ok(None) => {},
                        }
                    }
                    let result = match link {
                        Some(link) if probe.is_some() => {
                            if link.url != "https://example.com/pttstella" { return Err("unexpected probe URL".into()); }
                            link_requests += 1;Ok(())
                        },
                        Some(link) => webbrowser::open(&link.url).map_err(|e| format!("無法開啟網址：{e}")),
                        None => Err("畫面已更新，請重新選取網址".to_owned()),
                    };
                    let reply = match result {
                        Ok(()) => serde_json::json!({"link_result":"requested"}),
                        Err(error) => serde_json::json!({"link_result":"failed","notice":error}),
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                },
                SurfaceEvent::Media { request, action, trusted } => {
                    if stopping || !trusted { continue; }
                    let Some(preview) = media_preview.active() else { continue; };
                    if preview.request != request { continue; }
                    match action {
                        MediaAction::Close => media_preview.close(),
                        MediaAction::Retry => media_preview.retry(),
                        MediaAction::External => {
                            let result = if probe.is_some() {media_external+=1;Ok(())}
                                else {webbrowser::open(&preview.target.url).map_err(|e| format!("無法開啟網址：{e}"))};
                            eval.send(serde_json::json!({"media_external":{"request":request,"ok":result.is_ok(),"message":result.err()}})).map_err(|e| e.to_string())?;
                        },
                    }
                },
                SurfaceEvent::Paste { text, generation, trusted } => {
                    if stopping { continue; }
                    let result = input::prepare_paste(&text).map_err(|e| e.to_string()).and_then(|prepared| {
                        let current = receiver.borrow();
                        if current.phase != Phase::Connected || current.generation != generation {
                            return Err("連線已變更；貼上未送出".into());
                        }
                        if prepared.lines > 1 {
                            pending_paste.set(Some(PendingPaste { generation, prepared }));
                            Ok(false)
                        } else {
                            paste_single |= probe.is_some() && trusted && prepared.text == "中";
                            connection.input(generation, prepared.bytes).map(|()| true)
                        }
                    });
                    let reply = match result {
                        Ok(queued) => serde_json::json!({"paste_result":if queued {"queued"} else {"preview"}}),
                        Err(error) => {
                            if probe.is_some() && trusted { paste_rejected += 1; }
                            serde_json::json!({"notice":error,"paste_result":"rejected"})
                        },
                    };
                    eval.send(reply).map_err(|e| e.to_string())?;
                },
                SurfaceEvent::PasteCancel { trusted } => {
                    paste_cancel |= probe.is_some() && trusted && pending_paste.read().as_ref().is_some_and(|p| p.prepared.text == "A\nB");
                    pending_paste.set(None);
                    eval.send(serde_json::json!({"paste_result":"cancelled"})).map_err(|e| e.to_string())?;
                },
                SurfaceEvent::PasteConfirm { trusted } => {
                    let pending = pending_paste.write().take();
                    if let Some(pending) = pending {
                        let current = receiver.borrow();
                        let result = if current.phase == Phase::Connected && current.generation == pending.generation {
                            paste_confirm |= probe.is_some() && trusted && pending.prepared.text == "A\nB";
                            connection.input(pending.generation, pending.prepared.bytes)
                        } else { Err("連線已變更；貼上未送出".to_owned()) };
                        let reply = match result {
                            Ok(()) => serde_json::json!({"paste_result":"queued"}),
                            Err(error) => serde_json::json!({"notice":error,"paste_result":"rejected"}),
                        };
                        eval.send(reply).map_err(|e| e.to_string())?;
                    } else {
                        eval.send(serde_json::json!({"paste_result":"cancelled"})).map_err(|e| e.to_string())?;
                    }
                },
                SurfaceEvent::Select { generation, revision, request, selection } => {
                    let selected = displayed.as_ref()
                        .filter(|p| p.generation == generation && p.frame.revision == revision)
                        .and_then(|p| selection::extract(&p.frame, &selection).ok());
                    if probe.is_some() && selected.as_ref().is_some_and(|s| s.text == "中") {
                        copied_selection = Some((generation, revision, request));
                    }
                    selected_rows = selected.as_ref().map(|s| SelectedRows { generation, revision, request, rows: s.rows.clone() });
                    eval.send(serde_json::json!({"selection": {
                        "generation":generation,"revision":revision,"request":request,"value":selected
                    }})).map_err(|e| e.to_string())?;
                },
                SurfaceEvent::Copied { generation, revision, request, trusted } => {
                    copy_checked |= trusted && copied_selection == Some((generation, revision, request));
                },
                SurfaceEvent::Key { key, generation, trusted } => {
                    if stopping || !trusted || generation!=receiver.borrow().generation { continue; }
                    let view=receiver.borrow().clone();
                    let context=navigation::analyze(&view.frame);
                    let effect=keyboard_policy.key(&key,&context);
                    eval.send(serde_json::json!({"keyboard_state":keyboard_policy.status(&context)})).map_err(|e|e.to_string())?;
                    let bytes=match effect {
                        Effect::None=>continue,
                        Effect::Notice(notice)=>{eval.send(serde_json::json!({"notice":notice})).map_err(|e|e.to_string())?;continue;},
                        Effect::Browse(intent)=>{
                            let shown=displayed.as_ref().is_some_and(|p|p.generation==generation && p.navigation.kind()==context.kind());
                            let result=if !shown || pending_paste.read().is_some() {Err(navigation::Error::Stale.to_string())}
                                else {connection.browse_key(view.stamp(),intent).await};
                            if let Err(error)=result {eval.send(serde_json::json!({"notice":error})).map_err(|e|e.to_string())?;}
                            continue;
                        },
                        Effect::Native(bytes)=>bytes,
                    };
                    if probe.is_some() {
                        let current = receiver.borrow().generation;
                        arrow_checked |= trusted && generation == current && bytes == [27,91,65];
                        if trusted && generation == current && bytes == [13] { enter_count += 1; }
                        closed_checked |= trusted && generation == current && bytes == [27,91,67]
                            && receiver.borrow().phase == Phase::Disconnected;
                    }
                    message.set(match connection.input(generation, bytes) {
                        Ok(()) => receiver.borrow().message.clone(),
                        Err(error) => {
                            eval.send(serde_json::json!({"notice":error,"input_result":"rejected"})).map_err(|e| e.to_string())?;
                            error
                        },
                    });
                },
                SurfaceEvent::Text { text, generation, trusted, composition_input, composed } => {
                    if stopping { continue; }
                    if generation!=receiver.borrow().generation || !(trusted||composition_input) || !keyboard_policy.accept_text(composed) {continue;}
                    eval.send(serde_json::json!({"keyboard_state":keyboard_policy.status(&navigation::analyze(&receiver.borrow().frame))})).map_err(|e|e.to_string())?;
                    let vim_input=keyboard_policy.status(&navigation::analyze(&receiver.borrow().frame)).enabled;
                    if probe.is_some() && !vim_input && (!(trusted || composition_input) || generation != receiver.borrow().generation || input_bytes.len() >= expected_input.len()) {
                        return Err("unexpected probe input".into());
                    }
                    composition_checked |= composition_input && text == "中";
                    let encoded = input::encode(&text).map_err(|e| e.to_string());
                    let result = encoded.and_then(|bytes| {
                        if probe.is_some() && !vim_input { input_bytes.push(bytes.clone()); }
                        connection.input(generation, bytes)
                    });
                    message.set(match result {
                        Ok(()) => receiver.borrow().message.clone(),
                        Err(error) => {
                            eval.send(serde_json::json!({"notice":error,"input_result":"rejected"})).map_err(|e| e.to_string())?;
                            error
                        },
                    });
                },
                SurfaceEvent::Painted { revision, generation } => {
                    if in_flight != Some((generation, revision)) {
                        return Err("畫面回報未知版本".into());
                    }
                    in_flight = None;
                    painted = (generation,revision);
                    displayed = Some(painting.clone());
                },
                SurfaceEvent::Error { message } => return Err(message),
                SurfaceEvent::Probe(pixels) => {
                    let ProbePixels { png, red_pixels, green_pixels, left_bg, right_bg, fonts, dpr, pixel_size, font_stack, fills_body, colour_samples, art_seam_pixels, art_zoom_seams, art_geometry, hover_controls, proportional_zoom, zoom_samples } = *pixels;
                    let Some(path) = &probe else { return Err("unexpected probe response".into()); };
                    let expected: Vec<Vec<u8>> = [
                        [0,0,0], [128,0,0], [0,128,0], [128,128,0],
                        [0,0,128], [128,0,128], [0,128,128], [192,192,192],
                        [128,128,128], [255,0,0], [0,255,0], [255,255,0],
                        [0,0,255], [255,0,255], [0,255,255], [255,255,255],
                        [128,0,0], [255,0,0], [128,0,0], [192,192,192],
                    ].into_iter().map(Vec::from).collect();
                    let ansi_colours = colour_samples == expected;
                    let valid = red_pixels > 5 && green_pixels > 5
                        && left_bg == [0,0,128,255] && right_bg == [128,0,128,255] && fonts == "loaded"
                        && fills_body && ansi_colours && art_seam_pixels == 0
                        && art_zoom_seams == [0,0,0] && art_geometry && hover_controls && proportional_zoom;
                    std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
                    let encoded = png.strip_prefix("data:image/png;base64,").ok_or("invalid PNG response")?;
                    if encoded.len() > 4 * 1024 * 1024 { return Err("probe image exceeds limit".into()); }
                    let pixels = base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|e| e.to_string())?;
                    std::fs::write(path.join(if reload_settings {"reloaded-screen.png"} else {"screen.png"}), pixels).map_err(|e| e.to_string())?;
                    let report = serde_json::json!({"status":if valid {"passed"} else {"failed"},
                        "scope":"mounted WebView canvas and input bridge; synthetic data only",
                        "red_pixels":red_pixels,"green_pixels":green_pixels,"left_bg":left_bg,"right_bg":right_bg,
                        "fonts":fonts,
                        "dpr":dpr,"pixel_size":pixel_size,"font_stack":font_stack,"grid":[80,24],
                        "fills_body":fills_body,"ansi_colours":ansi_colours,"colour_samples":colour_samples,
                        "art_seam_pixels":art_seam_pixels,
                        "art_zoom_seams":art_zoom_seams,"art_geometry":art_geometry,
                        "hover_controls":hover_controls,"proportional_zoom":proportional_zoom,"zoom_samples":zoom_samples,
                        "settings_loaded":settings,
                        "font_fallback_per_glyph":"not measured"});
                    pending_report = Some(report);
                    close_deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(45));
                },
                SurfaceEvent::Stopped { clean } => {
                    media_preview.close();
                    connection.shutdown().await?;
                    if let Some(mut report) = pending_report.take() {
                        report["listeners_released"] = clean.into();
                        report["connection_worker_joined"] = true.into();
                        report["media_pending"] = media_preview.pending().into();
                        report["media_preview_open"] = media_preview.active().is_some().into();
                        report["media_external_requests"] = media_external.into();
                        report["arrow_input"] = arrow_checked.into(); report["enter_input"] = (enter_count == expected_enter_count).into();
                        report["text_input"] = (input_bytes == expected_input).into();
                        report["trusted_webview_input"] = (arrow_checked && enter_count == expected_enter_count && input_bytes == expected_input).into();
                        report["loopback_input_roundtrip"] = complete.into();
                        report["closed_after_roundtrip"] = closed_checked.into();
                        report["selection_copy"] = copy_checked.into();
                        report["ime_composition"] = composition_checked.into();
                        report["url_open"] = (link_requests == 1).into();
                        report["png_export"] = export_checked.into();
                        report["rows_png_export"] = rows_export_checked.into();
                        report["settings_saved"] = settings_saved.into();
                        report["mouse_navigation"] = (navigation_requests==8).into();
                        report["paste_single"] = paste_single.into();report["paste_confirm"] = paste_confirm.into();
                        report["paste_cancel"] = paste_cancel.into();report["paste_rejected"] = (paste_rejected == 3).into();
                        if !clean || !probe_checked { report["status"] = "failed".into(); }
                        if reload_settings { report["scope"]="settings reload in a second application process; synthetic data only".into(); }
                        let path = probe.as_ref().ok_or("missing probe directory")?;
                        std::fs::write(path.join(if reload_settings {"reload.json"} else {"probe.json"}), report.to_string()).map_err(|e| e.to_string())?;
                    }
                    let window = dioxus::desktop::window();
                    window.set_close_behavior(dioxus::desktop::WindowCloseBehaviour::WindowCloses);
                    window.close();
                    return Ok(());
                },
            },
            changed = receiver.changed() => {
                changed.map_err(|e| e.to_string())?;
                let view = receiver.borrow_and_update().clone();
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
                    media_preview.close();
                    connection.keep_alive_gate(receiver.borrow().stamp(),false);
                    eval.send(serde_json::json!({"stop":true})).map_err(|e| e.to_string())?;
                    stopping = true;
                    close_deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(5));
                }
            },
            _ = async { match close_deadline { Some(deadline) => tokio::time::sleep_until(deadline).await, None => std::future::pending().await } } => {
                return Err("畫面監聽器清理逾時".into());
            },
        }
    }
}

fn paint(view: &View, probe: bool, settings: &DisplaySettings) -> Paint {
    Paint {
        keep_alive: view.keep_alive,
        keep_alive_ready: view.keep_alive_ready,
        keep_alive_sent: view.keep_alive_sent,
        keep_alive_failure: view.keep_alive_failure.clone(),
        input_epoch: view.input_epoch,
        input_pending: view.input_pending,
        connected: view.phase == Phase::Connected,
        navigation: navigation::analyze(&view.frame),
        settings: settings.clone(),
        links: links::detect(&view.frame),
        frame: view.frame.clone(),
        generation: view.generation,
        probe,
    }
}

fn sample_packet(frame: &Frame, settings: &DisplaySettings) -> Paint {
    Paint {
        keep_alive: false,
        keep_alive_ready: false,
        keep_alive_sent: 0,
        keep_alive_failure: None,
        input_epoch: 0,
        input_pending: false,
        connected: false,
        navigation: navigation::analyze(frame),
        frame: frame.clone(),
        generation: 0,
        probe: false,
        links: Vec::new(),
        settings: settings.clone(),
    }
}

fn font_sample() -> Result<Frame, String> {
    let mut sample = Screen::new(24, 4).map_err(str::to_owned)?;
    sample.feed(b"ASCII xyz 0123456789\r\n\x1b[31;44m\xa4\x1b[32;45m\xa4\x1b[0m ");
    sample.feed(&encoding::encode("中文字型\r\n").map_err(|e| e.to_string())?);
    sample.feed(b"\x1b[31;44m");
    sample.feed(&encoding::encode("█▌◢◣◤◥ ●○◎⊙").map_err(|e| e.to_string())?);
    sample.feed(b"\x1b[0m\r\n");
    sample.feed(&encoding::encode("┌──┐ │  │ └──┘").map_err(|e| e.to_string())?);
    sample.feed(b"\x1b[0m\x1b[?25l");
    Ok(sample.frame())
}
