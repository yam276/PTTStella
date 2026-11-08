//! A native consumer of the existing Connection watch and single writer.
use crate::{TerminalWindow, stations::Stations};
use pttstella::{
    connection::{CloseOutcome, Connection, Phase},
    screen::{Frame, Screen},
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FrameId {
    pub generation: u64,
    pub revision: u64,
}

#[derive(Default)]
pub struct FrameGate {
    observed: Option<FrameId>,
    staged: Option<FrameId>,
}

impl FrameGate {
    pub fn stage(&mut self, id: FrameId, pending: bool) -> bool {
        if self.observed.is_some_and(|latest| id < latest) {
            return false;
        }
        self.observed = Some(id);
        if pending || self.staged == Some(id) {
            return false;
        }
        self.staged = Some(id);
        true
    }

    pub fn staged(&self) -> Option<FrameId> {
        self.staged
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    EditorColor,
    Drafts,
    Media,
    Reconnect,
    ArticleInline,
    ArticleFull,
    ArticleInput,
    ArticleOutput,
    ArticleKeepAlive,
    ArticlePaste,
    ArticleFind,
    ArticleFindTerminal,
    ArticleLong,
    ArticleStall,
    ArticleTerminal,
    Input,
    Clipboard,
    KeepAlive,
    Login,
    Login2fa,
    Selection,
    Vim,
    Pointer,
    ContextMenu,
    ContextMenuStation,
    Tools,
    ToolsStation,
    Mouse,
    MouseDrag,
    MouseMotion,
    MouseWheel,
    MouseRoundTrip,
}

pub enum Mode {
    Probe(Probe),
    Live(Stations),
}

pub fn with_live(
    screen: Screen,
    show: impl FnOnce(Rc<Connection>) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    with_connection(|| Connection::start(screen), None, show)
}

pub fn with_probe(
    probe: Probe,
    bytes: Vec<u8>,
    show: impl FnOnce(Rc<Connection>) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let draft_failure = if probe == Probe::Drafts {
        draft_failure()?
    } else {
        None
    };
    let color_failure = if probe == Probe::EditorColor {
        match std::env::var("PTTSTELLA_STATION_COLOR_CASE")
            .ok()
            .as_deref()
        {
            None => None,
            Some("cancel") => Some(pttstella::connection::EditorColorFailure::Cancel),
            Some("unsupported") => Some(pttstella::connection::EditorColorFailure::Unsupported),
            Some("partial") => Some(pttstella::connection::EditorColorFailure::Partial),
            _ => return Err("unknown station colour fixture".into()),
        }
    } else {
        None
    };
    let slow_drafts = probe == Probe::Drafts
        && std::env::var_os("PTTSTELLA_SLINT_DRAFTS_SLOW").as_deref()
            == Some(std::ffi::OsStr::new("1"));
    let menu_probe = if probe == Probe::ArticleInline {
        std::env::var("PTTSTELLA_INLINE_MENU_PROBE").ok()
    } else {
        None
    };
    let mut media = if matches!(probe, Probe::Media | Probe::ArticleInline) {
        Some(crate::media_fixture::Fixtures::start(
            menu_probe.as_deref(),
        )?)
    } else {
        None
    };
    let bytes = if probe == Probe::EditorColor {
        pttstella::editor_color::fixture::Editor::new(
            &["P\x1b[32m中\x1b[0m文\x1b[34mQ", "", "第三行"],
            true,
            false,
            true,
        )
        .wire()
    } else {
        bytes
    };
    let bytes = match media.as_ref() {
        Some(f) if probe == Probe::ArticleInline => f.article_page(1)?,
        Some(f) => f.page(),
        None => bytes,
    };
    let fixture = match probe {
        Probe::EditorColor if color_failure.is_some() => "slint_editor_color_negative",
        Probe::EditorColor => "slint_editor_color",
        Probe::Drafts if draft_failure.is_some() => "slint_drafts_failure",
        Probe::Drafts if slow_drafts => "slint_drafts_slow",
        Probe::Drafts => "slint_drafts",
        Probe::Media => "slint_native_media",
        Probe::ArticleInline => "slint_native_inline",
        Probe::Reconnect => "slint_session",
        Probe::ArticleFull => "slint_full_load",
        Probe::ArticleInput => "slint_article_input",
        Probe::ArticleOutput => "slint_article_output",
        Probe::ArticleKeepAlive => "slint_article_keepalive",
        Probe::ArticlePaste => "slint_article_paste",
        Probe::ArticleFind | Probe::ArticleFindTerminal => "slint_article_find",
        Probe::ArticleLong => "slint_long_load",
        Probe::ArticleStall => "slint_stalled_article",
        Probe::ArticleTerminal => "slint_terminal_pages",
        Probe::Input => "slint_input",
        Probe::Clipboard => "slint_clipboard",
        Probe::KeepAlive => "slint_keepalive",
        Probe::Login => "slint_login",
        Probe::Login2fa => "slint_login_2fa",
        Probe::Selection => "slint_selection",
        Probe::Vim => "slint_vim",
        Probe::Pointer => "slint_pointer",
        Probe::ContextMenu => "slint_context_menu",
        Probe::ContextMenuStation => "slint_context_menu_station",
        Probe::Tools => "slint_tools",
        Probe::ToolsStation => "slint_tools_station",
        Probe::Mouse => "slint_mouse",
        Probe::MouseDrag => "slint_mouse_drag",
        Probe::MouseMotion => "slint_mouse_motion",
        Probe::MouseWheel => "slint_mouse_wheel",
        Probe::MouseRoundTrip => "slint_mouse_round_trip",
    };
    let result = with_connection(
        || match probe {
            Probe::EditorColor => match color_failure {
                Some(case) => Connection::start_editor_color_failure_probe(case),
                None => Connection::start_editor_color_probe(bytes),
            },
            Probe::Drafts => {
                if let Some(case) = draft_failure {
                    Connection::start_drafts_failure_probe(case)
                } else if slow_drafts {
                    Connection::start_drafts_slow_probe()
                } else {
                    Connection::start_drafts_probe()
                }
            }
            Probe::Media => Connection::start_ui_probe(bytes),
            Probe::ArticleInline => Connection::start_inline_pages_probe(
                bytes,
                media
                    .as_ref()
                    .ok_or("inline fixtures missing")?
                    .article_page(2)?,
            ),
            Probe::Reconnect => Connection::start_toast_reconnect_probe(bytes),
            Probe::ArticleFull | Probe::ArticleLong => {
                let long = probe == Probe::ArticleLong;
                let page = |n| {
                    if long {
                        pttstella::replay::long_load_page(n, Some("http://127.0.0.1:9"))
                    } else {
                        pttstella::replay::full_load_page(n, Some("http://127.0.0.1:9"))
                    }
                };
                {
                    let start = if std::env::var_os("PTTSTELLA_READING_PROBE_FAST").as_deref()
                        == Some(std::ffi::OsStr::new("1"))
                    {
                        Connection::start_fast_article_load_probe
                    } else {
                        Connection::start_article_load_probe
                    };
                    start(
                        page(1)?,
                        (2..=if long { 38 } else { 20 })
                            .map(page)
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                }
            }
            Probe::ArticleInput => Connection::start_article_input_probe(
                pttstella::replay::native_article_input_page(1)?,
                pttstella::replay::native_article_input_page(2)?,
            ),
            Probe::ArticleKeepAlive => Connection::start_article_keep_alive_probe(),
            Probe::ArticlePaste => Connection::start_article_paste_probe(),
            Probe::ArticleFind | Probe::ArticleFindTerminal => {
                Connection::start_article_find_probe()
            }
            Probe::ArticleOutput => Connection::start_article_load_probe(
                pttstella::replay::native_article_output_page(1)?,
                vec![pttstella::replay::native_article_output_page(2)?],
            ),
            Probe::ArticleStall => Connection::start_article_stall_probe(
                pttstella::replay::full_load_page(1, Some("http://127.0.0.1:9"))?,
            ),
            Probe::ArticleTerminal => Connection::start_inline_pages_probe(
                pttstella::replay::full_load_page(1, Some("http://127.0.0.1:9"))?,
                pttstella::replay::full_load_page(2, Some("http://127.0.0.1:9"))?,
            ),
            Probe::Input => Connection::start_native_input_probe(),
            Probe::Clipboard => Connection::start_editor_clipboard_probe(
                pttstella::replay::editor_clipboard().map_err(|error| error.to_string())?,
            ),
            Probe::KeepAlive => Connection::start_keep_alive_probe(),
            Probe::Login => Connection::start_login_probe(),
            Probe::Login2fa => Connection::start_verification_probe(),
            Probe::Selection => Connection::start_ui_probe(
                pttstella::replay::parity_grid().map_err(|error| error.to_string())?,
            ),
            Probe::Vim => Connection::start_window_probe(),
            Probe::Pointer => Connection::start_pointer_rows_probe(),
            Probe::ContextMenu => Connection::start_context_menu_probe(false),
            Probe::ContextMenuStation => Connection::start_context_menu_probe(true),
            Probe::Tools => Connection::start_context_menu_probe(false),
            Probe::ToolsStation => Connection::start_context_menu_probe(true),
            Probe::Mouse => Connection::start_mouse_probe(false),
            Probe::MouseDrag => {
                Connection::start_mouse_samples_probe(pttstella::connection::MouseSampleProbe::Drag)
            }
            Probe::MouseMotion => Connection::start_mouse_samples_probe(
                pttstella::connection::MouseSampleProbe::Motion,
            ),
            Probe::MouseWheel => Connection::start_mouse_samples_probe(
                pttstella::connection::MouseSampleProbe::Wheel,
            ),
            Probe::MouseRoundTrip => Connection::start_mouse_samples_probe(
                pttstella::connection::MouseSampleProbe::RoundTrip,
            ),
        },
        Some((
            fixture,
            if draft_failure == Some(pttstella::connection::DraftFailure::Disconnect) {
                CloseOutcome::NoConnection
            } else {
                CloseOutcome::Acknowledged
            },
        )),
        show,
    );
    if let Some(media) = &mut media {
        media.finish()?;
    }
    result
}

pub fn draft_failure() -> Result<Option<pttstella::connection::DraftFailure>, String> {
    use pttstella::connection::DraftFailure as F;
    match std::env::var("PTTSTELLA_DRAFTS_SOURCE_CASE")
        .ok()
        .as_deref()
    {
        None => Ok(None),
        Some("changed-template") => Ok(Some(F::ChangedTemplate)),
        Some("after-title") => Ok(Some(F::AfterTitle)),
        Some("after-delete") => Ok(Some(F::AfterDelete)),
        Some("after-write") => Ok(Some(F::AfterWrite)),
        Some("disconnect") => Ok(Some(F::Disconnect)),
        Some("close-capture") => Ok(Some(F::CloseCapture)),
        Some("close-transfer") => Ok(Some(F::CloseTransfer)),
        _ => Err("unknown draft failure fixture".into()),
    }
}

fn with_connection(
    start: impl FnOnce() -> Result<Connection, String>,
    fixture: Option<(&str, CloseOutcome)>,
    show: impl FnOnce(Rc<Connection>) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    let connection = Rc::new(runtime.block_on(async { start() })?);
    let result = {
        let _runtime_context = runtime.enter();
        show(connection.clone())
    };
    let close = runtime.block_on(connection.shutdown());
    match (result, close, fixture) {
        (Ok(()), Ok(outcome), Some((fixture, expected))) if outcome == expected => {
            eprintln!(
                "{}",
                serde_json::json!({
                    "status": "passed", "fixture": fixture, "close": outcome,
                    "expected_close":expected,
                })
            );
            Ok(())
        }
        (Ok(()), Ok(outcome), Some(_)) => {
            Err(format!("session probe ended without the expected Close: {outcome:?}").into())
        }
        (Ok(()), Ok(CloseOutcome::Unconfirmed), None) => Err("session Close unconfirmed".into()),
        (Ok(()), Ok(_), None) => Ok(()),
        (Ok(()), Err(error), _) => Err(error.into()),
        (Err(error), Ok(_), _) => Err(error),
        (Err(error), Err(cleanup), _) => Err(format!("{error}; cleanup: {cleanup}").into()),
    }
}

pub fn mount(
    ui: &TerminalWindow,
    connection: Rc<Connection>,
    mode: Mode,
    frame_gate: Rc<RefCell<FrameGate>>,
    mut stage: impl FnMut(&TerminalWindow, Frame) + 'static,
) -> Result<slint::JoinHandle<()>, Box<dyn Error>> {
    ui.set_session_active(true);
    let live = matches!(&mode, Mode::Live(_));
    let show_reconnect = live || matches!(&mode, Mode::Probe(Probe::Reconnect));
    ui.set_show_reconnect(show_reconnect);
    let connecting = Rc::new(Cell::new(None));
    ui.on_reconnect({
        let weak = ui.as_weak();
        let connection = connection.clone();
        let connecting = connecting.clone();
        move || {
            if let Some(ui) = weak.upgrade()
                && ui.get_can_reconnect()
            {
                request_connection(&ui, &connection, &connecting, 1, || connection.connect());
            }
        }
    });
    match mode {
        Mode::Live(stations) => {
            ui.set_live_session(true);
            ui.set_session_warning(stations.warnings.join(" · ").into());
            ui.set_stations(ModelRc::new(VecModel::from(
                stations
                    .entries
                    .iter()
                    .map(|(label, _)| label.clone())
                    .collect::<Vec<_>>(),
            )));
            ui.on_switch_station({
                let weak = ui.as_weak();
                let connection = connection.clone();
                let connecting = connecting.clone();
                move |index| {
                    if connecting.get().is_none()
                        && let Some(ui) = weak.upgrade()
                        && let Some((_, target)) = usize::try_from(index)
                            .ok()
                            .and_then(|i| stations.entries.get(i))
                    {
                        request_connection(&ui, &connection, &connecting, 2, || {
                            connection.connect_to(target.clone())
                        });
                    }
                }
            });
            ui.invoke_switch_station(0);
        }
        Mode::Probe(_) => connection.connect()?,
    }
    let weak = ui.as_weak();
    let mut updates = connection.updates.clone();
    Ok(slint::spawn_local(async move {
        while let Some(ui) = weak.upgrade() {
            let (message, can_reconnect, frame) = {
                let view = updates.borrow_and_update();
                let id = FrameId {
                    generation: view.generation,
                    revision: view.frame.revision,
                };
                // Switching increments once for retiring the old transport and once for dialing.
                // The first watch event can still contain the old frame with its old phase.
                if connecting
                    .get()
                    .is_some_and(|expected| expected <= view.generation)
                {
                    connecting.set(None);
                }
                let frame = frame_gate
                    .borrow_mut()
                    .stage(id, view.output_pending || connecting.get().is_some())
                    .then(|| view.frame.clone());
                let can_reconnect = show_reconnect
                    && connecting.get().is_none()
                    && (live || view.generation == 1)
                    && matches!(view.phase, Phase::Disconnected | Phase::Error);
                (view.message.clone(), can_reconnect, frame)
            };
            if connecting.get().is_none() {
                ui.set_result(message.into());
            }
            ui.set_can_reconnect(can_reconnect);
            ui.set_switching_station(connecting.get().is_some());
            if let Some(frame) = frame {
                stage(&ui, frame);
            }
            ui.invoke_session_updated();
            ui.invoke_login_updated();
            ui.invoke_output_updated();
            ui.invoke_station_mouse_updated();
            ui.invoke_context_updated();
            ui.invoke_station_color_updated();
            ui.invoke_tools_updated();
            ui.invoke_keyboard_updated();
            ui.invoke_keep_alive_changed();
            drop(ui);
            if updates.changed().await.is_err() {
                break;
            }
            // Even if the producer has another update ready, yield to native events.
            tokio::task::yield_now().await;
        }
    })?)
}

fn request_connection(
    ui: &TerminalWindow,
    connection: &Connection,
    connecting: &Cell<Option<u64>>,
    advances: u64,
    enqueue: impl FnOnce() -> Result<(), String>,
) {
    let generation = connection.updates.borrow().generation;
    match enqueue() {
        Ok(()) => {
            connecting.set(Some(generation.saturating_add(advances)));
            ui.set_result(pttstella::msg!("status-connecting").into());
            ui.set_switching_station(true);
            ui.set_can_reconnect(false);
            ui.invoke_keep_alive_changed();
            ui.invoke_focus_terminal();
        }
        Err(error) => ui.set_result(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pttstella::{connection::View, input, replay, vim};
    use std::time::Duration;

    async fn wait_for(connection: &Connection, matches: impl Fn(&View) -> bool) -> View {
        let mut updates = connection.updates.clone();
        tokio::time::timeout(Duration::from_secs(5), updates.wait_for(matches))
            .await
            .unwrap()
            .unwrap()
            .clone()
    }

    fn row(frame: &Frame, row: usize) -> String {
        frame
            .glyphs
            .iter()
            .filter(|glyph| glyph.row == row)
            .map(|glyph| glyph.text.as_str())
            .collect()
    }

    #[test]
    fn pending_new_generation_retires_older_frames_and_revision_restarts() {
        let id = |generation, revision| FrameId {
            generation,
            revision,
        };
        let mut gate = FrameGate::default();
        assert!(gate.stage(id(1, 42), false));
        assert!(!gate.stage(id(1, 42), false));
        assert!(!gate.stage(id(1, 41), false));
        assert!(!gate.stage(id(2, 2), true));
        assert!(!gate.stage(id(1, 43), false));
        assert_eq!(gate.staged(), Some(id(1, 42)));
        assert!(gate.stage(id(2, 2), false));
        assert!(!gate.stage(id(1, 99), false));
        assert_eq!(gate.staged(), Some(id(2, 2)));
        assert!(!gate.stage(id(2, 5), true));
        assert!(!gate.stage(id(2, 4), false));
        assert!(gate.stage(id(2, 5), false));
    }

    #[tokio::test]
    async fn strict_peer_checks_control_ascii_and_one_uao_commit() {
        let connection = Connection::start_native_input_probe().unwrap();
        connection.connect().unwrap();
        let view = wait_for(&connection, |view| {
            row(&view.frame, 0).starts_with("INPUT 0/5")
        })
        .await;
        let mut keyboard = vim::Keyboard::default();
        for letter in ['X', 'Z', 'A'] {
            let key = vim::Key {
                key: "注".into(),
                code: format!("Key{letter}"),
                ctrl: true,
                alt: false,
                shift: false,
                repeat: false,
            };
            let vim::Effect::Native(bytes) = keyboard.key(&key, &view.context()) else {
                panic!("physical Control key did not produce native bytes");
            };
            connection.input(view.generation, bytes).unwrap();
        }
        let third = wait_for(&connection, |view| {
            row(&view.frame, 0).starts_with("INPUT 3/5")
        })
        .await;
        let plain = input::prepare_paste_with("p", third.site.encoding).unwrap();
        assert!(!plain.requires_confirmation());
        connection
            .request_paste(third.stamp(), plain.bytes)
            .unwrap()
            .wait()
            .await
            .unwrap();
        connection
            .input(
                view.generation,
                input::encode_with("中", view.site.encoding).unwrap(),
            )
            .unwrap();
        wait_for(&connection, |view| {
            row(&view.frame, 0).starts_with("INPUT 5/5 VERIFIED")
        })
        .await;
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
    }

    #[tokio::test]
    async fn reconnect_keeps_frames_in_order_and_peer_rejects_replayed_input() {
        let connection =
            Connection::start_toast_reconnect_probe(replay::parity_grid().unwrap()).unwrap();
        connection.connect().unwrap();
        let first = wait_for(&connection, |view| {
            view.phase == Phase::Disconnected && row(&view.frame, 2).starts_with("SESSION 1/2")
        })
        .await;
        connection.connect().unwrap();
        let second = wait_for(&connection, |view| {
            row(&view.frame, 2).starts_with("SESSION 2/2")
        })
        .await;
        assert!(first.generation < second.generation);
        let mut gate = FrameGate::default();
        let id = |view: &View| FrameId {
            generation: view.generation,
            revision: view.frame.revision,
        };
        assert!(gate.stage(id(&first), first.output_pending));
        assert!(gate.stage(id(&second), second.output_pending));
        assert!(!gate.stage(id(&first), first.output_pending));
        connection
            .input(first.generation, b"never replay".to_vec())
            .unwrap();
        assert_eq!(
            connection
                .request_paste(first.stamp(), b"stale paste".to_vec())
                .unwrap()
                .wait()
                .await,
            Err(pttstella::msg!("paste-stale"))
        );
        assert_eq!(
            connection.shutdown().await.unwrap(),
            CloseOutcome::Acknowledged
        );
    }
}
