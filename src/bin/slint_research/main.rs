mod article;
mod chrome;
mod clipboard;
mod context_menu;
mod editor;
mod graphics;
mod ime_probe;
mod inline_engine;
mod keep_alive;
mod login;
mod loopback;
mod media;
mod media_fixture;
mod media_playback;
mod media_worker;
mod native_input;
mod output;
mod paste;
mod pointer;
mod profile;
mod renderer;
mod session;
mod station_mouse;
mod stations;
mod tools;
mod verification;

use pttstella::{
    connection::Connection,
    input, replay,
    screen::{Frame, Screen},
    settings::AppSettings,
};
use renderer::{Renderer, Size};
use slint::{ComponentHandle, Image, Model, Rgba8Pixel, SharedPixelBuffer, Timer};
use std::{
    cell::{Cell, RefCell},
    error::Error,
    fs::OpenOptions,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

slint::include_modules!();

fn update_grid(
    ui: &TerminalWindow,
    renderer: &Renderer,
    previous: &Cell<Option<Size>>,
    article: Option<&article::session::ArticleSession>,
) -> Result<(), &'static str> {
    if ui.get_article_active() {
        previous.set(None);
        return Ok(());
    }
    let dpi = ui.window().scale_factor();
    let Some(size) = Size::fit(
        ui.get_view_width(),
        ui.get_view_height(),
        dpi,
        ui.get_zoom(),
    ) else {
        previous.set(None);
        return Ok(());
    };
    if previous.get() != Some(size) {
        let start = Instant::now();
        let cursor = !ui.get_terminal_input() || ui.get_live_session();
        let pixels = if let Some(article) = article {
            article.paint_terminal_find(renderer, size, cursor)?
        } else {
            renderer.paint(size, cursor)?
        };
        ui.set_terminal_image(Image::from_rgba8_premultiplied(pixels));
        previous.set(Some(size));
        if std::env::var_os("PTTSTELLA_SLINT_TIMINGS").is_some() {
            eprintln!(
                "grid {}x{} dpi {dpi} raster_us {}",
                size.width,
                size.height,
                start.elapsed().as_micros()
            );
        }
    }
    ui.set_raster_width(size.width as f32 / dpi);
    ui.set_raster_height(size.height as f32 / dpi);
    if ui.get_terminal_input() {
        let caret = renderer.cursor_rect(size);
        ui.set_caret_left(caret.left / dpi);
        ui.set_caret_top(caret.top / dpi);
        ui.set_caret_height(caret.height() / dpi);
    }
    Ok(())
}

fn save_png(pixels: &SharedPixelBuffer<Rgba8Pixel>, output: &Path) -> Result<(), Box<dyn Error>> {
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut png = png::Encoder::new(file, pixels.width(), pixels.height());
    png.set_color(png::ColorType::Rgba);
    png.write_header()?.write_image_data(pixels.as_bytes())?;
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Probe {
    Grid,
    ProfileInfo,
    Article,
    Ime,
    Session(session::Probe),
    Connect,
}

struct Options {
    zoom: f32,
    capture: Option<PathBuf>,
    source_png: Option<PathBuf>,
    verify_context: Option<PathBuf>,
    verify_tools: Option<PathBuf>,
    verify_article: Option<PathBuf>,
    verify_article_session: Option<PathBuf>,
    verify_media: Option<PathBuf>,
    verify_drafts: Option<PathBuf>,
    probe: Option<Probe>,
}

fn options() -> Result<Options, &'static str> {
    const USAGE: &str = "usage: pttstella-slint [--connect|--show-profile|--probe-grid|--probe-drafts|--probe-media|--probe-inline|--probe-article|--probe-full-load|--probe-article-input|--probe-article-output|--probe-article-keepalive|--probe-article-paste|--probe-article-find|--probe-terminal-find|--probe-long-load|--probe-stalled-article|--probe-terminal-pages|--probe-ime|--probe-session|--probe-input|--probe-clipboard|--probe-keepalive|--probe-login|--probe-login-2fa|--probe-selection|--probe-vim|--probe-pointer|--probe-tools|--probe-tools-station|--probe-context-menu|--probe-context-menu-station|--probe-station-mouse|--probe-station-mouse-drag|--probe-station-mouse-motion|--probe-station-mouse-wheel|--probe-station-mouse-round-trip] [--zoom 100|125|150] [--capture NEW.png] [--source-png NEW.png] [--verify-drafts NEW_DIR] [--verify-media NEW_DIR] [--verify-context-menu NEW_DIR] [--verify-tools NEW_DIR] [--verify-article NEW_DIR] [--verify-article-session NEW_DIR]";
    let mut zoom = None;
    let mut capture = None;
    let mut probe = None;
    let mut source_png = None;
    let mut verify_context = None;
    let mut verify_tools = None;
    let mut verify_article = None;
    let mut verify_article_session = None;
    let mut verify_media = None;
    let mut verify_drafts = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--verify-drafts") if verify_drafts.is_none() => {
                verify_drafts = Some(args.next().ok_or(USAGE)?.into());
            }
            Some("--verify-media") if verify_media.is_none() => {
                verify_media = Some(args.next().ok_or(USAGE)?.into());
            }
            Some("--probe-inline") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticleInline))
            }
            Some("--probe-drafts") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Drafts))
            }
            Some("--probe-media") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Media))
            }
            Some("--connect") if probe.is_none() => probe = Some(Probe::Connect),
            Some("--show-profile") if probe.is_none() => probe = Some(Probe::ProfileInfo),
            Some("--probe-grid") if probe.is_none() => probe = Some(Probe::Grid),
            Some("--probe-full-load") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticleFull))
            }
            Some("--probe-article-input") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticleInput))
            }
            Some("--probe-article-keepalive") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticleKeepAlive))
            }
            Some("--probe-article-find") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticleFind))
            }
            Some("--probe-terminal-find") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticleFindTerminal))
            }
            Some("--probe-article-paste") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticlePaste))
            }
            Some("--probe-article-output") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticleOutput))
            }
            Some("--probe-long-load") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticleLong))
            }
            Some("--probe-stalled-article") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticleStall))
            }
            Some("--probe-terminal-pages") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ArticleTerminal))
            }
            Some("--probe-article") if probe.is_none() => probe = Some(Probe::Article),
            Some("--probe-ime") if probe.is_none() => probe = Some(Probe::Ime),
            Some("--probe-session") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Reconnect));
            }
            Some("--probe-input") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Input));
            }
            Some("--probe-clipboard") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Clipboard));
            }
            Some("--probe-keepalive") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::KeepAlive));
            }
            Some("--probe-login") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Login));
            }
            Some("--probe-login-2fa") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Login2fa));
            }
            Some("--probe-selection") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Selection));
            }
            Some("--probe-vim") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Vim));
            }
            Some("--probe-pointer") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Pointer));
            }
            Some("--probe-tools") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Tools));
            }
            Some("--probe-tools-station") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ToolsStation));
            }
            Some("--probe-context-menu") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ContextMenu));
            }
            Some("--probe-context-menu-station") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::ContextMenuStation));
            }
            Some("--probe-station-mouse") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::Mouse))
            }
            Some("--probe-station-mouse-drag") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::MouseDrag))
            }
            Some("--probe-station-mouse-motion") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::MouseMotion))
            }
            Some("--probe-station-mouse-wheel") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::MouseWheel))
            }
            Some("--probe-station-mouse-round-trip") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::MouseRoundTrip))
            }
            Some("--zoom") if zoom.is_none() => {
                zoom = Some(match args.next().as_deref().and_then(|s| s.to_str()) {
                    Some("100") => 1.,
                    Some("125") => 1.25,
                    Some("150") => 1.5,
                    _ => return Err(USAGE),
                });
            }
            Some("--capture") if capture.is_none() => {
                capture = Some(args.next().ok_or(USAGE)?.into())
            }
            Some("--verify-article-session") if verify_article_session.is_none() => {
                verify_article_session = Some(args.next().ok_or(USAGE)?.into());
            }
            Some("--verify-article") if verify_article.is_none() => {
                verify_article = Some(args.next().ok_or(USAGE)?.into());
            }
            Some("--verify-tools") if verify_tools.is_none() => {
                verify_tools = Some(args.next().ok_or(USAGE)?.into());
            }
            Some("--verify-context-menu") if verify_context.is_none() => {
                verify_context = Some(args.next().ok_or(USAGE)?.into());
            }
            Some("--source-png") if source_png.is_none() => {
                source_png = Some(args.next().ok_or(USAGE)?.into())
            }
            _ => return Err(USAGE),
        }
    }
    if (matches!(
        probe,
        Some(Probe::Ime | Probe::Session(_) | Probe::Connect | Probe::ProfileInfo)
    ) && capture.is_some())
        || (source_png.is_some()
            && (probe != Some(Probe::Grid) || capture.is_none() || source_png == capture))
    {
        return Err(USAGE);
    }
    if verify_context.is_some()
        && (!matches!(
            probe,
            Some(Probe::Session(
                session::Probe::ContextMenu | session::Probe::ContextMenuStation
            ))
        ) || capture.is_some())
    {
        return Err(USAGE);
    }
    if verify_tools.is_some()
        && (!matches!(
            probe,
            Some(Probe::Session(
                session::Probe::Tools | session::Probe::ToolsStation
            ))
        ) || capture.is_some()
            || verify_context.is_some())
    {
        return Err(USAGE);
    }
    if verify_article.is_some()
        && (probe != Some(Probe::Article)
            || capture.is_some()
            || verify_context.is_some()
            || verify_tools.is_some())
    {
        return Err(USAGE);
    }
    if verify_article_session.is_some()
        && (!matches!(
            probe,
            Some(Probe::Session(
                session::Probe::ArticleFull
                    | session::Probe::ArticleInput
                    | session::Probe::ArticleOutput
                    | session::Probe::ArticleKeepAlive
                    | session::Probe::ArticlePaste
                    | session::Probe::ArticleFind
                    | session::Probe::ArticleFindTerminal
                    | session::Probe::ArticleLong
                    | session::Probe::ArticleStall
                    | session::Probe::ArticleTerminal
            ))
        ) || capture.is_some()
            || verify_context.is_some()
            || verify_tools.is_some()
            || verify_article.is_some())
    {
        return Err(USAGE);
    }
    if verify_media.is_some()
        && (!matches!(
            probe,
            Some(Probe::Session(
                session::Probe::Media | session::Probe::ArticleInline
            ))
        ) || capture.is_some()
            || verify_article_session.is_some()
            || verify_article.is_some()
            || verify_tools.is_some()
            || verify_context.is_some())
    {
        return Err(USAGE);
    }
    if verify_drafts.is_some()
        && (probe != Some(Probe::Session(session::Probe::Drafts))
            || capture.is_some()
            || verify_context.is_some()
            || verify_tools.is_some()
            || verify_article.is_some()
            || verify_article_session.is_some()
            || verify_media.is_some())
    {
        return Err(USAGE);
    }
    Ok(Options {
        verify_drafts,
        verify_media,
        verify_article_session,
        verify_article,
        verify_tools,
        verify_context,
        zoom: zoom.unwrap_or(1.),
        capture,
        source_png,
        probe,
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let options = options()?;
    for directory in [
        &options.verify_context,
        &options.verify_tools,
        &options.verify_article,
        &options.verify_article_session,
        &options.verify_media,
        &options.verify_drafts,
    ]
    .into_iter()
    .flatten()
    {
        // Reserve evidence output before starting any connection or UI task.
        std::fs::create_dir(directory)?;
    }
    if options.probe == Some(Probe::ProfileInfo) {
        println!("{}", profile::Profile::system()?.describe()?);
        return Ok(());
    }
    let bytes = replay::parity_grid()?;
    let mut screen = Screen::new(80, 24)?;
    match options.probe {
        Some(Probe::Article) => {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()?;
            let _context = runtime.enter();
            return run(
                options,
                screen.frame(),
                None,
                AppSettings {
                    display: replay::parity_display_profile(),
                    ..Default::default()
                },
            );
        }
        Some(Probe::Grid) => {
            return loopback::with_grid(bytes, |frame| {
                run(
                    options,
                    frame,
                    None,
                    AppSettings {
                        display: replay::parity_display_profile(),
                        ..Default::default()
                    },
                )
            });
        }
        Some(Probe::Session(probe)) => {
            let mut settings = AppSettings {
                display: replay::parity_display_profile(),
                ..Default::default()
            };
            if matches!(probe, session::Probe::Tools | session::Probe::ToolsStation) {
                settings.tools_opacity = pttstella::settings::ToolOpacity::try_from(20)?;
            }
            return session::with_probe(probe, bytes, |connection| {
                run(
                    options,
                    screen.frame(),
                    Some((session::Mode::Probe(probe), connection)),
                    settings,
                )
            });
        }
        Some(Probe::Connect) => {
            let profile = profile::Profile::system()?;
            let (settings, warning) = profile.load_settings();
            pttstella::i18n::initialize(settings.language)?;
            let mut stations = stations::Stations::load(&profile);
            if let Some(warning) = warning {
                stations.warnings.insert(0, warning);
            }
            let frame = screen.frame();
            return session::with_live(screen, |connection| {
                run(
                    options,
                    frame,
                    Some((session::Mode::Live(stations), connection)),
                    settings,
                )
            });
        }
        _ => {}
    }
    screen.feed(&bytes);
    replay::verify_parity_grid(&screen.frame())?;
    if options.probe == Some(Probe::Ime) {
        // A blank row with a real Screen cursor gives the native field an exact grid anchor.
        screen.feed(b"\x1b[22;5H\x1b[?25h");
    }
    run(options, screen.frame(), None, AppSettings::default())
}

fn run(
    options: Options,
    frame: Frame,
    connection: Option<(session::Mode, Rc<Connection>)>,
    settings: AppSettings,
) -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "native-video")]
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("skia".into())
        .require_opengl_with_version(3, 3)
        .select()?;
    // The macOS fixture used less physical memory with Skia's CPU compositor.
    #[cfg(all(target_os = "macos", not(feature = "native-video")))]
    if std::env::var_os("SLINT_BACKEND").is_none() {
        slint::BackendSelector::new()
            .backend_name("winit-skia-software".into())
            .select()?;
    }
    let ui = TerminalWindow::new()?;
    // Probe settings live only in this owned temporary namespace; retain it
    // until output workers have drained at both normal and capture shutdown.
    let media_probe_settings = if matches!(
        options.probe,
        Some(Probe::Session(
            session::Probe::Media | session::Probe::ArticleInline
        ))
    ) {
        Some(
            tempfile::Builder::new()
                .prefix("pttstella-slint-media-settings-")
                .tempdir()?,
        )
    } else {
        None
    };
    ui.on_link_activated({
        let weak = ui.as_weak();
        let live = options.probe == Some(Probe::Connect);
        move |url| {
            if let Some(ui) = weak.upgrade() {
                if live {
                    if let Err(error) = pttstella::links::validate(&url).and_then(|url| {
                        webbrowser::open(url.as_str())
                            .map_err(|e| pttstella::msg!("link-open-failed", e = e.to_string()))
                    }) {
                        ui.set_input_notice(error.into());
                    }
                } else {
                    ui.set_input_notice(url);
                }
            }
        }
    });
    if options.probe == Some(Probe::Ime) {
        ime_probe::mount(&ui);
    }
    let display = settings.display.resolve()?;
    let renderer = Rc::new(RefCell::new(Renderer::new(frame, &display)?));
    let article_owner = if options.probe == Some(Probe::Article) {
        let (cache, source) = article::fixture(&display)?;
        ui.set_result("文章視窗探針 · Home/End、PageUp/PageDown、j/k 與滾輪".into());
        let article_renderer = renderer.borrow().fork(source.frame.clone());
        Some(article::ArticleView::mount(
            &ui,
            cache,
            source,
            article_renderer,
        )?)
    } else {
        None
    };

    let frame_gate = Rc::new(RefCell::new(session::FrameGate::default()));
    let presented = Rc::new(Cell::new(None));
    let article_bridge: Rc<RefCell<Option<Rc<article::session::ArticleSession>>>> =
        Rc::new(RefCell::new(None));
    let previous = Rc::new(Cell::new(None));
    let pending = Rc::new(Cell::new(false));
    ui.on_geometry_changed({
        let (weak, renderer, previous) = (ui.as_weak(), renderer.clone(), previous.clone());
        let (frame_gate, presented) = (frame_gate.clone(), presented.clone());
        let article_bridge = article_bridge.clone();
        move || {
            if pending.replace(true) {
                return;
            }
            if let Some(ui) = weak.upgrade() {
                ui.invoke_output_geometry_changed();
                ui.set_frame_paint_pending(true);
                ui.invoke_clear_selection();
                ui.invoke_keep_alive_changed();
            }
            let (weak, renderer, previous, pending) = (
                weak.clone(),
                renderer.clone(),
                previous.clone(),
                pending.clone(),
            );
            let (frame_gate, presented) = (frame_gate.clone(), presented.clone());
            let article_bridge = article_bridge.clone();
            Timer::single_shot(Duration::ZERO, move || {
                pending.set(false);
                if let Some(ui) = weak.upgrade() {
                    match update_grid(
                        &ui,
                        &renderer.borrow(),
                        &previous,
                        article_bridge.borrow().as_deref(),
                    ) {
                        Err(error) => {
                            presented.set(None);
                            ui.set_result(error.into());
                        }
                        Ok(()) if previous.get().is_some() => {
                            if let Some(id) = frame_gate.borrow().staged()
                                && id.revision == renderer.borrow().frame().revision
                                && article_bridge
                                    .borrow()
                                    .as_ref()
                                    .is_none_or(|a| a.terminal_matches(id))
                            {
                                if presented.replace(Some(id)) != Some(id) {
                                    eprintln!("session frame {}:{}", id.generation, id.revision);
                                }
                            } else {
                                presented.set(None);
                            }
                        }
                        Ok(()) => presented.set(None),
                    }
                    ui.set_frame_paint_pending(false);
                    if let Some(owner) = article_bridge.borrow().as_ref() {
                        owner.terminal_rendered();
                    }
                    ui.invoke_session_updated();
                    ui.invoke_login_updated();
                    ui.invoke_output_updated();
                    ui.invoke_station_mouse_updated();
                    ui.invoke_context_updated();
                    ui.invoke_tools_updated();
                    ui.invoke_keyboard_updated();
                    ui.invoke_keep_alive_changed();
                }
            });
        }
    });
    ui.on_toggle_fullscreen({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade() {
                let window = ui.window();
                window.set_fullscreen(!window.is_fullscreen());
            }
        }
    });
    ui.on_leave_fullscreen({
        let weak = ui.as_weak();
        move || {
            if let Some(ui) = weak.upgrade()
                && ui.window().is_fullscreen()
            {
                ui.window().set_fullscreen(false);
                return true;
            }
            false
        }
    });
    ui.on_submit({
        let weak = ui.as_weak();
        move |text| {
            if let Some(ui) = weak.upgrade() {
                let result = match input::prepare_paste(&text) {
                    Ok(paste) => format!("離線編碼成功 · {} bytes", paste.bytes.len()),
                    Err(error) => format!("輸入未通過 · {error}"),
                };
                ui.set_result(result.into());
            }
        }
    });
    ui.set_zoom(options.zoom);
    // Search changes only display styles; do not revoke geometry/output leases.
    let find_paint = Rc::new(Timer::default());
    ui.on_find_redraw({
        let (weak, renderer, previous, bridge, timer) = (
            ui.as_weak(),
            renderer.clone(),
            previous.clone(),
            article_bridge.clone(),
            find_paint.clone(),
        );
        move || {
            let (weak, renderer, previous, bridge) = (
                weak.clone(),
                renderer.clone(),
                previous.clone(),
                bridge.clone(),
            );
            timer.start(slint::TimerMode::SingleShot, Duration::ZERO, move || {
                if let Some(ui) = weak.upgrade() {
                    previous.set(None);
                    if let Err(error) = update_grid(
                        &ui,
                        &renderer.borrow(),
                        &previous,
                        bridge.borrow().as_deref(),
                    ) {
                        ui.set_result(error.into());
                    }
                }
            });
        }
    });
    let mut article_session_owner = None;
    let mut login_owner = None;
    let mut paste_owner = None;
    let mut output_owner = None;
    let mut context_owner = None;
    let mut tools_owner = None;
    let mut station_mouse_owner = None;
    let mut media_owner = None;
    let mut editor_owner = None;
    let drafts_probe_directory = if options.probe == Some(Probe::Session(session::Probe::Drafts))
        && std::env::var_os("PTTSTELLA_SLINT_DRAFTS_PROBE_DIR").is_none()
    {
        Some(
            tempfile::Builder::new()
                .prefix("PTTStella-Slint-Drafts-")
                .tempdir()?,
        )
    } else {
        None
    };
    let clipboard = clipboard::Clipboard::default();
    let verification_connection = if options.verify_context.is_some()
        || options.verify_tools.is_some()
        || options.verify_article_session.is_some()
    {
        connection
            .as_ref()
            .map(|(_, connection)| connection.clone())
    } else {
        None
    };
    let watcher = connection
        .map(|(mode, connection)| {
            if matches!(
                &mode,
                session::Mode::Live(_)
                    | session::Mode::Probe(
                        session::Probe::ArticleInline
                            | session::Probe::ArticleFull
                            | session::Probe::ArticleInput
                            | session::Probe::ArticleOutput
                            | session::Probe::ArticleKeepAlive
                            | session::Probe::ArticlePaste
                            | session::Probe::ArticleFind
                            | session::Probe::ArticleFindTerminal
                            | session::Probe::ArticleLong
                            | session::Probe::ArticleStall
                            | session::Probe::ArticleTerminal
                    )
            ) {
                article_session_owner = Some(article::session::ArticleSession::mount(
                    &ui,
                    connection.clone(),
                    renderer.clone(),
                    previous.clone(),
                    presented.clone(),
                    display.clone(),
                    if matches!(&mode, session::Mode::Live(_)) {
                        settings.article_mode
                    } else if matches!(
                        &mode,
                        session::Mode::Probe(
                            session::Probe::ArticleTerminal | session::Probe::ArticleFindTerminal
                        )
                    ) {
                        pttstella::settings::ArticleMode::Terminal
                    } else {
                        pttstella::settings::ArticleMode::Web
                    },
                )?);
                article_bridge.replace(article_session_owner.clone());
            }
            if matches!(&mode, session::Mode::Live(_)) {
                article_session_owner
                    .as_ref()
                    .ok_or("live projection owner missing")?
                    .replace_rules(profile::Profile::system()?.filters().load()?);
            }
            if matches!(
                &mode,
                session::Mode::Probe(
                    session::Probe::ArticleOutput
                        | session::Probe::ArticleFind
                        | session::Probe::ArticleFindTerminal
                )
            ) {
                article_session_owner
                    .as_ref()
                    .ok_or("article rules owner missing")?
                    .replace_rules(pttstella::filters::Rules::default().edit_policy(
                        "Blocked",
                        "",
                        pttstella::filters::Policy::MaskRow,
                        false,
                    )?);
            }
            station_mouse_owner = Some(station_mouse::StationMouse::mount(
                &ui,
                connection.clone(),
                previous.clone(),
                presented.clone(),
            ));
            if matches!(
                &mode,
                session::Mode::Live(_)
                    | session::Mode::Probe(
                        session::Probe::ArticleInline
                            | session::Probe::ArticleFull
                            | session::Probe::ArticleInput
                            | session::Probe::ArticleOutput
                            | session::Probe::ArticleKeepAlive
                            | session::Probe::ArticlePaste
                            | session::Probe::ArticleFind
                            | session::Probe::ArticleFindTerminal
                            | session::Probe::ArticleLong
                            | session::Probe::ArticleStall
                    )
            ) {
                paste_owner = Some(native_input::mount_article(
                    &ui,
                    connection.clone(),
                    presented.clone(),
                    clipboard.clone(),
                    article_session_owner
                        .as_ref()
                        .ok_or("article input owner missing")?
                        .clone(),
                ));
            }
            if matches!(
                &mode,
                session::Mode::Probe(
                    session::Probe::Drafts
                        | session::Probe::Media
                        | session::Probe::Input
                        | session::Probe::Clipboard
                        | session::Probe::KeepAlive
                        | session::Probe::Login
                        | session::Probe::Login2fa
                        | session::Probe::Selection
                        | session::Probe::ArticleTerminal
                        | session::Probe::Vim
                        | session::Probe::Pointer
                        | session::Probe::ContextMenu
                        | session::Probe::ContextMenuStation
                        | session::Probe::Tools
                        | session::Probe::ToolsStation
                        | session::Probe::Mouse
                        | session::Probe::MouseDrag
                        | session::Probe::MouseMotion
                        | session::Probe::MouseWheel
                        | session::Probe::MouseRoundTrip
                )
            ) {
                paste_owner = Some(native_input::mount(
                    &ui,
                    connection.clone(),
                    presented.clone(),
                    clipboard.clone(),
                ));
            }
            if matches!(
                &mode,
                session::Mode::Live(_)
                    | session::Mode::Probe(
                        session::Probe::KeepAlive
                            | session::Probe::ArticleKeepAlive
                            | session::Probe::Login
                            | session::Probe::Login2fa
                    )
            ) {
                keep_alive::mount(
                    &ui,
                    connection.clone(),
                    presented.clone(),
                    article_session_owner.clone(),
                );
            }
            if matches!(
                &mode,
                session::Mode::Live(_)
                    | session::Mode::Probe(session::Probe::Login | session::Probe::Login2fa)
            ) {
                login_owner = Some(login::Login::mount(
                    &ui,
                    connection.clone(),
                    presented.clone(),
                    matches!(&mode, session::Mode::Probe(_)),
                ));
            }
            if matches!(
                &mode,
                session::Mode::Live(_)
                    | session::Mode::Probe(
                        session::Probe::Media
                            | session::Probe::ArticleInline
                            | session::Probe::ArticleFull
                            | session::Probe::ArticleInput
                            | session::Probe::ArticleOutput
                            | session::Probe::ArticleKeepAlive
                            | session::Probe::ArticlePaste
                            | session::Probe::ArticleFind
                            | session::Probe::ArticleFindTerminal
                            | session::Probe::ArticleLong
                            | session::Probe::ArticleStall
                            | session::Probe::Selection
                            | session::Probe::ArticleTerminal
                            | session::Probe::Pointer
                            | session::Probe::ContextMenu
                            | session::Probe::ContextMenuStation
                            | session::Probe::Tools
                            | session::Probe::ToolsStation
                            | session::Probe::Mouse
                            | session::Probe::MouseDrag
                            | session::Probe::MouseMotion
                            | session::Probe::MouseWheel
                            | session::Probe::MouseRoundTrip
                    )
            ) {
                let output = output::Output::mount(
                    &ui,
                    connection.clone(),
                    renderer.clone(),
                    previous.clone(),
                    presented.clone(),
                    display.clone(),
                    clipboard.clone(),
                    article_session_owner.clone(),
                );
                context_owner = Some(context_menu::ContextMenu::mount(
                    &ui,
                    connection.clone(),
                    renderer.clone(),
                    output.clone(),
                ));
                tools_owner = Some(tools::Tools::mount(
                    &ui,
                    connection.clone(),
                    output.clone(),
                    settings.tools_opacity,
                    settings.tools_access,
                ));
                ui.on_media_request({
                    let weak = ui.as_weak();
                    let output = output.clone();
                    move |index| {
                        if let Some(ui) = weak.upgrade()
                            && let Some(url) = usize::try_from(index)
                                .ok()
                                .and_then(|i| output.media_url(&ui, i))
                        {
                            ui.invoke_link_activated(url.into());
                        }
                    }
                });
                if matches!(
                    &mode,
                    session::Mode::Live(_)
                        | session::Mode::Probe(
                            session::Probe::Media | session::Probe::ArticleInline
                        )
                ) {
                    media_owner = Some(media::Media::mount(
                        &ui,
                        connection.clone(),
                        output.clone(),
                        matches!(
                            &mode,
                            session::Mode::Probe(
                                session::Probe::Media | session::Probe::ArticleInline
                            )
                        ),
                        settings.clone(),
                        if let Some(directory) = &media_probe_settings {
                            pttstella::settings::store::Store::at(
                                directory.path().join("settings.json"),
                            )
                        } else {
                            profile::Profile::system()?.settings()
                        },
                    ));
                }
                if let (Some(media), Some(article)) = (&media_owner, &article_session_owner) {
                    media.mount_inline(&ui, article);
                    article.attach_media(media);
                    if let Some(context) = &context_owner {
                        context.attach_media(media);
                    }
                }
                output_owner = Some(output);
            }
            if matches!(
                &mode,
                session::Mode::Live(_) | session::Mode::Probe(session::Probe::Drafts)
            ) {
                let store = if matches!(&mode, session::Mode::Probe(session::Probe::Drafts)) {
                    let directory = std::env::var_os("PTTSTELLA_SLINT_DRAFTS_PROBE_DIR")
                        .map(PathBuf::from)
                        .or_else(|| drafts_probe_directory.as_ref().map(|d| d.path().to_owned()))
                        .ok_or("isolated drafts directory missing")?;
                    Ok(pttstella::drafts::store::Store::at(
                        directory.join("probe-drafts.json"),
                    ))
                } else {
                    profile::Profile::system().map(|p| p.drafts())
                };
                editor_owner = Some(editor::Editor::mount(
                    &ui,
                    connection.clone(),
                    presented.clone(),
                    clipboard.clone(),
                    &renderer.borrow(),
                    store,
                )?);
            }
            let (renderer, previous) = (renderer.clone(), previous.clone());
            session::mount(&ui, connection, mode, frame_gate, move |ui, frame| {
                // The shared owner rasterizes its filtered projection. The
                // session watcher only stages source identity and status.
                if article_bridge.borrow().is_none() {
                    renderer.borrow_mut().set_frame(frame);
                    previous.set(None);
                }
                ui.invoke_geometry_changed();
            })
        })
        .transpose()?;
    let verification = if let Some(directory) = options.verify_drafts {
        Some(editor::probe::start(
            &ui,
            editor_owner
                .as_ref()
                .cloned()
                .ok_or("editor owner missing")?,
            directory,
        )?)
    } else if options.probe == Some(Probe::Session(session::Probe::ArticleInline))
        && options.verify_media.is_some()
    {
        Some(article::inline::probe::start(
            &ui,
            media_owner
                .as_ref()
                .and_then(|m| m.inline_owner())
                .ok_or("inline owner missing")?,
            article_session_owner
                .as_ref()
                .cloned()
                .ok_or("inline article missing")?,
            options
                .verify_media
                .clone()
                .ok_or("inline verification directory missing")?,
            media_probe_settings
                .as_ref()
                .ok_or("isolated media settings missing")?
                .path()
                .join("settings.json"),
        )?)
    } else if let Some(directory) = options.verify_media {
        let start = media::probe::start;
        #[cfg(feature = "native-video")]
        let start = if std::env::var_os("PTTSTELLA_VIDEO_FIXTURE").is_some() {
            media::video::probe::start
        } else {
            start
        };
        Some(start(
            &ui,
            media_owner
                .as_ref()
                .cloned()
                .ok_or("media probe owner missing")?,
            output_owner
                .as_ref()
                .cloned()
                .ok_or("media probe output missing")?,
            directory,
        )?)
    } else if let Some(directory) = options.verify_article_session {
        Some(article::session_probe::start(
            &ui,
            article_session_owner
                .as_ref()
                .cloned()
                .ok_or("article session owner missing")?,
            verification_connection
                .clone()
                .ok_or("article probe connection missing")?,
            output_owner.as_ref().cloned(),
            paste_owner.as_ref().cloned(),
            directory,
            match options.probe {
                Some(Probe::Session(case)) => case,
                _ => return Err("article probe mode missing".into()),
            },
        )?)
    } else if let Some(directory) = options.verify_article {
        Some(article::probe::start(
            &ui,
            article_owner
                .as_ref()
                .cloned()
                .ok_or("article probe owner missing")?,
            directory,
        )?)
    } else if let Some(directory) = options.verify_context {
        Some(context_menu::probe::start(
            &ui,
            verification_connection
                .clone()
                .ok_or("context probe connection missing")?,
            output_owner
                .as_ref()
                .cloned()
                .ok_or("context probe output owner missing")?,
            directory,
            options.probe == Some(Probe::Session(session::Probe::ContextMenuStation)),
        )?)
    } else if let Some(directory) = options.verify_tools {
        Some(tools::probe::start(
            &ui,
            verification_connection.ok_or("tools probe connection missing")?,
            output_owner
                .as_ref()
                .cloned()
                .ok_or("tools probe output owner missing")?,
            tools_owner
                .as_ref()
                .cloned()
                .ok_or("tools probe owner missing")?,
            directory,
            options.probe == Some(Probe::Session(session::Probe::ToolsStation)),
        )?)
    } else {
        None
    };
    if let Some(path) = options.capture {
        ui.show()?;
        ui.window().set_size(slint::LogicalSize::new(1000., 690.));
        let outcome = Rc::new(RefCell::new(None));
        Timer::single_shot(Duration::from_millis(250), {
            let (weak, outcome) = (ui.as_weak(), outcome.clone());
            move || {
                let result = (|| -> Result<(), Box<dyn Error>> {
                    let ui = weak.upgrade().ok_or("snapshot window closed")?;
                    let renderer = renderer.borrow();
                    if ui.get_article_active() {
                        if ui.get_article_tiles().row_count() == 0 {
                            return Err("snapshot article has no visible rows".into());
                        }
                    } else {
                        update_grid(&ui, &renderer, &previous, None)?;
                        previous.get().ok_or("snapshot terminal has no layout")?;
                    }
                    save_png(&ui.window().take_snapshot()?, &path)?;
                    if let Some(output) = options.source_png {
                        let size = Size {
                            width: pttstella::export::WIDTH,
                            height: pttstella::export::HEIGHT,
                        };
                        save_png(&renderer.paint(size, false)?, &output)?;
                    }
                    Ok(())
                })();
                outcome.replace(Some(result));
                if let Err(error) = slint::quit_event_loop() {
                    outcome.replace(Some(Err(error.into())));
                }
            }
        });
        slint::run_event_loop()?;
        ui.hide()?;
        if let Some(editor) = &editor_owner {
            editor.finish()?;
        }
        if let Some(media) = &media_owner {
            media.finish()?;
        }
        if let Some(paste) = &paste_owner {
            paste.finish()?;
        }
        if let Some(article) = &article_owner {
            article.finish();
        }
        outcome.take().ok_or("snapshot callback did not run")??;
    } else {
        let result = ui.run();
        let editor_cleanup = editor_owner.map(|editor| editor.finish()).transpose();
        let media_cleanup = media_owner.map(|media| media.finish()).transpose();
        if let Some(article) = &article_session_owner {
            article.finish();
        }
        if let Some(article) = &article_owner {
            article.finish();
        }
        if let Some(tools) = tools_owner {
            tools.finish();
        }
        if let Some(context) = context_owner {
            context.finish();
        }
        if let Some(mouse) = station_mouse_owner {
            mouse.finish();
        }
        if let Some(login) = login_owner {
            login.finish();
        }
        let output_cleanup = output_owner.map(|output| output.finish()).transpose();
        let paste_cleanup = paste_owner.map(|paste| paste.finish()).transpose();
        if let Some(watcher) = watcher {
            watcher.abort();
        }
        let verification_result = verification
            .map(|verification| verification.finish())
            .transpose();
        result?;
        editor_cleanup?;
        media_cleanup?;
        output_cleanup?;
        paste_cleanup?;
        verification_result?;
    }
    Ok(())
}
