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
    Benchmark,
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

impl Mode {
    /// Explicit loopback benchmarks mount the same service owners as Live.
    /// Transport, data paths and credential scope remain synthetic.
    pub fn full_services(&self) -> bool {
        matches!(self, Self::Live(_) | Self::Probe(Probe::Benchmark))
    }
}

#[cfg(test)]
mod service_policy_tests {
    use super::*;

    #[test]
    fn full_service_benchmark_does_not_inherit_a_reduced_probe_owner_set() {
        assert!(Mode::Probe(Probe::Benchmark).full_services());
        assert!(!Mode::Probe(Probe::Selection).full_services());
        assert!(!Mode::Probe(Probe::Media).full_services());
    }
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
    directory: Option<std::path::PathBuf>,
    show: impl FnOnce(
        Rc<Connection>,
        Option<Rc<pttstella::connection::site_probe::Peers>>,
    ) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    use pttstella::connection::site_probe::{Case, Peers};
    let site_peers = if probe == Probe::Media {
        std::env::var("PTTSTELLA_SLINT_SITE_SWITCH")
            .ok()
            .map(|case| {
                let case = match case.as_str() {
                    "connected" => Case::Connected,
                    "disconnected" => Case::Disconnected,
                    "cancel" => Case::CancelDial,
                    "native" => Case::Native,
                    _ => return Err("unknown site-switch case".to_string()),
                };
                if case == Case::Native
                    && std::env::var("PTTSTELLA_SLINT_SETTINGS_PROBE").as_deref()
                        != Ok("native-quit")
                {
                    return Err("native site-switch requires passive native-quit mode".into());
                }
                Peers::start(
                    case,
                    directory
                        .as_deref()
                        .ok_or("site-switch probe needs a new verify-media directory")?,
                )
                .map(Rc::new)
            })
            .transpose()?
    } else {
        None
    };
    let draft_failure = if probe == Probe::Drafts {
        draft_failure()?
    } else {
        None
    };
    let storage_drafts = probe == Probe::Drafts && crate::editor::probe::uses_storage_case()?;
    if storage_drafts && draft_failure.is_some() {
        return Err("draft storage and station failure probes must be separate".into());
    }
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
        Some(f)
            if probe == Probe::Media
                && std::env::var_os("PTTSTELLA_SLINT_ACCOUNT_ACTIONS").is_some() =>
        {
            f.accounts_page()?
        }
        Some(f) if probe == Probe::ArticleInline => f.article_page(1)?,
        Some(f) => f.page(),
        None => bytes,
    };
    let fixture = match probe {
        Probe::Benchmark => "slint_full_parity_grid",
        Probe::EditorColor if color_failure.is_some() => "slint_editor_color_negative",
        Probe::EditorColor => "slint_editor_color",
        Probe::Drafts if storage_drafts => "slint_drafts_storage",
        Probe::Drafts if draft_failure.is_some() => "slint_drafts_failure",
        Probe::Drafts if slow_drafts => "slint_drafts_slow",
        Probe::Drafts => "slint_drafts",
        Probe::Media if std::env::var_os("PTTSTELLA_SLINT_ACCOUNT_ACTIONS").is_some() => {
            "slint_account_actions"
        }
        Probe::Media if std::env::var_os("PTTSTELLA_SLINT_DESTINATION_PROBE").is_some() => {
            "slint_native_destination"
        }
        Probe::Media if std::env::var_os("PTTSTELLA_SLINT_UPDATES_PROBE").is_some() => {
            "slint_native_updates"
        }
        Probe::Media if std::env::var_os("PTTSTELLA_SLINT_REPLIES_PROBE").is_some() => {
            "slint_native_replies"
        }
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
            Probe::Benchmark => Connection::start_ui_probe(bytes),
            Probe::EditorColor => match color_failure {
                Some(case) => Connection::start_editor_color_failure_probe(case),
                None => Connection::start_editor_color_probe(bytes),
            },
            Probe::Drafts => {
                if storage_drafts {
                    Connection::start_drafts_storage_probe()
                } else if let Some(case) = draft_failure {
                    Connection::start_drafts_failure_probe(case)
                } else if slow_drafts {
                    Connection::start_drafts_slow_probe()
                } else {
                    Connection::start_drafts_probe()
                }
            }
            Probe::Media => {
                if let Ok(case) = std::env::var("PTTSTELLA_SLINT_UPDATES_PROBE") {
                    if !matches!(
                        case.as_str(),
                        "append"
                            | "cancel"
                            | "blur"
                            | "native"
                            | "mixed-append"
                            | "mixed-cancel"
                            | "mixed-blur"
                    ) {
                        return Err("unknown native updates case".into());
                    }
                    let origin = std::env::var("PTTSTELLA_SLINT_UPDATES_ORIGIN").ok();
                    if let Some(origin) = origin.as_ref() {
                        let url = reqwest::Url::parse(origin).map_err(|e| e.to_string())?;
                        if url.scheme() != "http"
                            || url.host_str() != Some("127.0.0.1")
                            || url.port().is_none()
                            || url.path() != "/"
                            || url.query().is_some()
                            || url.fragment().is_some()
                            || !url.username().is_empty()
                            || url.password().is_some()
                        {
                            return Err(
                                "native updates image server must be loopback origin".into()
                            );
                        }
                    }
                    let mixed = case.starts_with("mixed-");
                    let case = case.strip_prefix("mixed-").unwrap_or(&case);
                    let start = if mixed {
                        Connection::start_live_updates_roles_probe
                    } else {
                        Connection::start_live_updates_probe
                    };
                    start(
                        origin.map(|s| s.trim_end_matches('/').to_owned()),
                        matches!(case, "cancel" | "blur"),
                    )
                } else if let Ok(case) = std::env::var("PTTSTELLA_SLINT_REPLIES_PROBE") {
                    match case.as_str() {
                        "1" | "native" => Connection::start_reply_navigation_probe(),
                        _ => Err("unknown reply navigation probe case".into()),
                    }
                } else if let Ok(case) = std::env::var("PTTSTELLA_SLINT_CAPTURE_PROBE") {
                    match case.as_str() {
                        "current" | "native" => Connection::start_bookmark_archive_probe(false),
                        "headerless" => Connection::start_bookmark_archive_probe(true),
                        "list" | "native-list" => Connection::start_bookmark_list_probe(),
                        "cancel" | "blur" => Connection::start_live_updates_probe(None, true),
                        _ => Err("unknown capture probe case".into()),
                    }
                } else if let Ok(case) = std::env::var("PTTSTELLA_SLINT_DESTINATION_PROBE") {
                    match case.as_str() {
                        "boards" | "native" => Connection::start_board_probe(),
                        "articles" => Connection::start_article_probe(),
                        _ => Err("unknown destination probe case".into()),
                    }
                } else if let Some(peers) = &site_peers {
                    let connection = Connection::start_local_station_probe(
                        Screen::new(80, 24).map_err(|e| e.to_string())?,
                    )?;
                    connection.connect_to(peers.first.clone())?;
                    Ok(connection)
                } else if std::env::var_os("PTTSTELLA_SLINT_ACCOUNT_ACTIONS").is_some() {
                    let fixture: serde_json::Value = serde_json::from_str(include_str!(
                        "../../../tests/fixtures/navigation.json"
                    ))
                    .map_err(|e| e.to_string())?;
                    let list = fixture["list"]
                        .as_str()
                        .ok_or("account list fixture missing")?;
                    Connection::start_inline_pages_probe(
                        bytes,
                        pttstella::encoding::encode(list).map_err(|e| e.to_string())?,
                    )
                } else {
                    Connection::start_ui_probe(
                        if std::env::var_os("PTTSTELLA_SLINT_FILTERS_PROBE").is_some() {
                            pttstella::replay::native_article_find_page()?
                        } else {
                            bytes
                        },
                    )
                }
            }
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
        |connection| show(connection, site_peers.clone()),
    );
    let peer_result = site_peers
        .as_ref()
        .map(|peers| peers.finish(result.is_err()))
        .transpose();
    if let Some(peers) = &site_peers
        && let Some(directory) = &directory
    {
        let report = match &peer_result {
            Ok(Some(audit)) => serde_json::to_value(audit)?,
            Err(error) => {
                serde_json::json!({"passed":false,"error":error,"audit":peers.audit().ok()})
            }
            _ => serde_json::json!({"passed":false}),
        };
        std::fs::write(
            directory.join("site-peer-final.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
    }
    if let Some(media) = &mut media {
        media.finish()?;
    }
    match (result, peer_result) {
        (Ok(()), Ok(_)) => Ok(()),
        (Err(e), Ok(_)) => Err(e),
        (Ok(()), Err(e)) => Err(e.into()),
        (Err(e), Err(cleanup)) => Err(format!("{e}; site peers: {cleanup}").into()),
    }
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
    if result.is_err()
        && let Some((fixture, expected)) = &fixture
    {
        eprintln!(
            "{}",
            serde_json::json!({
                "status":"failed", "fixture":fixture,
                "close":close.as_ref().ok(), "expected_close":expected,
                "cleanup_error":close.as_ref().err(),
            })
        );
    }
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
                && !ui.get_app_closing()
                && ui.get_can_reconnect()
                && !ui.get_manager_open()
                && !ui.get_settings_open()
            {
                request_connection(&ui, &connection, &connecting, 1, || connection.connect());
            }
        }
    });
    ui.on_sites_connect({
        let weak = ui.as_weak();
        let connection = connection.clone();
        let connecting = connecting.clone();
        move |address, encoding| {
            let Some(ui) = weak.upgrade() else {
                return false;
            };
            if ui.get_app_closing() || connecting.get().is_some() {
                return false;
            }
            let target = crate::sites_manager::encoding(encoding)
                .and_then(|e| pttstella::sites::Target::new(address.to_string(), e))
                .and_then(|t| {
                    if !live
                        && url::Url::parse(t.endpoint.as_str())
                            .ok()
                            .and_then(|u| u.host_str().map(str::to_owned))
                            .as_deref()
                            != Some("127.0.0.1")
                    {
                        return Err(pttstella::msg!("sites-probe-local"));
                    }
                    Ok(t)
                });
            match target {
                Ok(target) => request_connection(&ui, &connection, &connecting, 2, || {
                    connection.connect_to(target)
                }),
                Err(error) => {
                    ui.set_result(error.into());
                    false
                }
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
                    let entries =
                        crate::stations::Stations::choices(stations.state.borrow().sites().clone())
                            .0;
                    if connecting.get().is_none()
                        && let Some(ui) = weak.upgrade()
                        && !ui.get_app_closing()
                        && !ui.get_manager_open()
                        && !ui.get_settings_open()
                        && let Some((_, target)) =
                            usize::try_from(index).ok().and_then(|i| entries.get(i))
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
        // Connection messages commonly persist across hundreds of byte/frame
        // updates. Resolve their canonical locale once per message/language,
        // rather than formatting both catalogs for every screen update.
        let mut message_cache: Option<(pttstella::i18n::Language, String, String)> = None;
        while let Some(ui) = weak.upgrade() {
            let (message, can_reconnect, frame, attention) = {
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
                (
                    view.message.clone(),
                    can_reconnect,
                    frame,
                    view.phase != Phase::Connected,
                )
            };
            ui.set_connection_attention(attention || connecting.get().is_some());
            if connecting.get().is_none() {
                let language = pttstella::i18n::application().language();
                if message_cache
                    .as_ref()
                    .is_none_or(|(l, raw, _)| *l != language || raw != &message)
                {
                    let localized = crate::locale::connection_status(&message);
                    ui.set_connection_status(localized.as_str().into());
                    ui.set_result(localized.as_str().into());
                    message_cache = Some((language, message, localized));
                }
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
) -> bool {
    let generation = connection.updates.borrow().generation;
    match enqueue() {
        Ok(()) => {
            connecting.set(Some(generation.saturating_add(advances)));
            ui.set_result(pttstella::msg!("status-connecting").into());
            ui.set_connection_status(pttstella::msg!("status-connecting").into());
            ui.set_connection_attention(true);
            ui.set_switching_station(true);
            ui.set_can_reconnect(false);
            ui.invoke_keep_alive_changed();
            ui.invoke_focus_terminal();
            true
        }
        Err(error) => {
            ui.set_result(error.into());
            false
        }
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
