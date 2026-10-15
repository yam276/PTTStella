use base64::Engine;
use dioxus::prelude::*;
use pttstella::{
    connection::{Connection, Phase, View},
    encoding,
    input::{self, PreparedPaste},
    screen::{Frame, Screen},
    selection::{self, Selection},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::watch;

#[derive(Clone, PartialEq, Props)]
pub struct AppProps {
    pub probe: Option<PathBuf>,
}

#[derive(Clone, Serialize)]
struct Paint {
    frame: Frame,
    generation: u64,
    probe: bool,
}
#[derive(Clone)]
struct PendingPaste {
    generation: u64,
    prepared: PreparedPaste,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SurfaceEvent {
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
        bytes: Vec<u8>,
        generation: u64,
        trusted: bool,
    },
    Text {
        text: String,
        generation: u64,
        trusted: bool,
        #[serde(default)]
        composition_input: bool,
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

fn preview(probe: bool) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let text = "PTTStella\r\n\r\n尚未連線，歡迎使用 PTTStella。";
    bytes.extend_from_slice(&encoding::encode(text).map_err(|e| e.to_string())?);
    bytes.extend_from_slice(b"\x1b[4;1H\x1b[31;44m\xa4\x1b[32;45m\xa4\x1b[0m  ");
    bytes.extend_from_slice(&encoding::encode("雙色字顯示預覽").map_err(|e| e.to_string())?);
    bytes.extend_from_slice(b"\x1b[7;1H");
    bytes.extend_from_slice(&encoding::encode("正在連線 PTT…\r\n\r\n連線後，點一下畫面即可輸入帳號密碼。\r\n滑鼠移至上緣可顯示連線與等比例縮放工具。").map_err(|e| e.to_string())?);
    if probe {
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
            bytes.extend_from_slice(format!("\x1b[0;{fg};{bg}mMM").as_bytes());
        }
        // Also exercise intensity-on, intensity-off and full reset independently.
        bytes.extend_from_slice(b"\x1b[17;1H\x1b[0;31mMM\x1b[1mMM\x1b[22mMM\x1b[0mMM\x1b[19;1H");
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
    let owner = use_hook(move || {
        preview(probing)
            .and_then(|bytes| {
                if probing {
                    Connection::start_probe(bytes)
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
            }
            }
            section { class: "workspace",
                div { class: "surface", id: "surface",
                    onmounted: move |_| {
                        if started() { return; }
                        started.set(true);
                        let connection = connection.clone(); let probe = probe.clone(); let closing = closing.1.clone();
                        spawn(async move {
                            let result = surface(connection.clone(), closing, probe, message, phase, pending_paste).await;
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
                    output { id: "surface-note", role: "status" }
                    textarea { id: "keyboard", "aria-label": "PTT 鍵盤輸入", autocomplete: "off", spellcheck: "false" }
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
) -> Result<(), String> {
    let mut receiver = connection.updates.clone();
    let mut eval = document::eval(include_str!("../assets/surface.js"));
    let first = receiver.borrow_and_update().clone();
    let mut last_status = first.message.clone();
    let mut painting = paint(&first, false);
    let mut displayed: Option<Paint> = None;
    eval.send(&painting).map_err(|e| e.to_string())?;
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
    let mut enter_count = 0;
    let mut closed_checked = false;
    let mut probe_started = false;
    let mut copy_checked = false;
    let mut composition_checked = false;
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
        if pending_report.is_some()
            && arrow_checked
            && enter_count == 2
            && input_bytes == expected_input
            && complete
            && closed_checked
            && copy_checked
            && composition_checked
            && paste_single
            && paste_confirm
            && paste_cancel
            && paste_rejected == 3
            && !stopping
        {
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
            painting = paint(&view, run_probe);
            eval.send(&painting).map_err(|e| e.to_string())?;
            in_flight = Some((view.generation, view.frame.revision));
            dirty = false;
        }
        tokio::select! {
            event = eval.recv::<SurfaceEvent>() => match event.map_err(|e| e.to_string())? {
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
                    eval.send(serde_json::json!({"selection": {
                        "generation":generation,"revision":revision,"request":request,"value":selected
                    }})).map_err(|e| e.to_string())?;
                },
                SurfaceEvent::Copied { generation, revision, request, trusted } => {
                    copy_checked |= trusted && copied_selection == Some((generation, revision, request));
                },
                SurfaceEvent::Key { bytes, generation, trusted } => {
                    if stopping { continue; }
                    if probe.is_some() {
                        let current = receiver.borrow().generation;
                        arrow_checked |= trusted && generation == current && bytes == [27,91,65];
                        if trusted && generation == current && bytes == [13] { enter_count += 1; }
                        closed_checked |= trusted && generation == current && bytes == [27,91,67]
                            && receiver.borrow().phase == Phase::Disconnected;
                    }
                    message.set(match connection.input(generation, bytes) {
                        Ok(()) => receiver.borrow().message.clone(), Err(error) => error,
                    });
                },
                SurfaceEvent::Text { text, generation, trusted, composition_input } => {
                    if stopping { continue; }
                    if probe.is_some() && (!(trusted || composition_input) || generation != receiver.borrow().generation || input_bytes.len() >= expected_input.len()) {
                        return Err("unexpected probe input".into());
                    }
                    composition_checked |= composition_input && text == "中";
                    let encoded = input::encode(&text).map_err(|e| e.to_string());
                    let result = encoded.and_then(|bytes| {
                        if probe.is_some() { input_bytes.push(bytes.clone()); }
                        connection.input(generation, bytes)
                    });
                    message.set(match result {
                        Ok(()) => receiver.borrow().message.clone(),
                        Err(error) => {
                            eval.send(serde_json::json!({"notice":error})).map_err(|e| e.to_string())?;
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
                    std::fs::write(path.join("screen.png"), pixels).map_err(|e| e.to_string())?;
                    let report = serde_json::json!({"status":if valid {"passed"} else {"failed"},
                        "scope":"mounted WebView canvas and input bridge; synthetic data only",
                        "red_pixels":red_pixels,"green_pixels":green_pixels,"left_bg":left_bg,"right_bg":right_bg,
                        "fonts":fonts,
                        "dpr":dpr,"pixel_size":pixel_size,"font_stack":font_stack,"grid":[80,24],
                        "fills_body":fills_body,"ansi_colours":ansi_colours,"colour_samples":colour_samples,
                        "art_seam_pixels":art_seam_pixels,
                        "art_zoom_seams":art_zoom_seams,"art_geometry":art_geometry,
                        "hover_controls":hover_controls,"proportional_zoom":proportional_zoom,"zoom_samples":zoom_samples,
                        "font_fallback_per_glyph":"not measured"});
                    pending_report = Some(report);
                    close_deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(25));
                },
                SurfaceEvent::Stopped { clean } => {
                    connection.shutdown().await?;
                    if let Some(mut report) = pending_report.take() {
                        report["listeners_released"] = clean.into();
                        report["connection_worker_joined"] = true.into();
                        report["arrow_input"] = arrow_checked.into(); report["enter_input"] = (enter_count == 2).into();
                        report["text_input"] = (input_bytes == expected_input).into();
                        report["trusted_webview_input"] = (arrow_checked && enter_count == 2 && input_bytes == expected_input).into();
                        report["loopback_input_roundtrip"] = complete.into();
                        report["closed_after_roundtrip"] = closed_checked.into();
                        report["selection_copy"] = copy_checked.into();
                        report["ime_composition"] = composition_checked.into();
                        report["paste_single"] = paste_single.into();report["paste_confirm"] = paste_confirm.into();
                        report["paste_cancel"] = paste_cancel.into();report["paste_rejected"] = (paste_rejected == 3).into();
                        if !clean || !arrow_checked || enter_count != 2 || input_bytes != expected_input || !complete || !closed_checked || !copy_checked || !composition_checked || !paste_single || !paste_confirm || !paste_cancel || paste_rejected != 3 { report["status"] = "failed".into(); }
                        let path = probe.as_ref().ok_or("missing probe directory")?;
                        std::fs::write(path.join("probe.json"), report.to_string()).map_err(|e| e.to_string())?;
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

fn paint(view: &View, probe: bool) -> Paint {
    Paint {
        frame: view.frame.clone(),
        generation: view.generation,
        probe,
    }
}
