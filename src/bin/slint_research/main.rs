#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_exit;
mod article;
mod benchmark;
mod bookmarks_manager;
mod chrome;
mod clipboard;
mod context_menu;
mod destination;
mod editor;
mod file_job;
mod filters_manager;
mod graphics;
mod ime_probe;
mod inline_engine;
mod keep_alive;
mod locale;
mod login;
mod loopback;
mod media;
mod media_fixture;
mod media_playback;
mod media_worker;
mod native_input;
#[cfg(target_os = "macos")]
mod native_menu;
mod native_text;
mod notice;
mod notice_probe;
mod output;
mod palette;
mod paste;
mod pointer;
mod preferences;
mod profile;
mod renderer;
mod session;
mod shell_probe;
mod sites_manager;
mod station_color;
mod station_mouse;
mod stations;
mod system_motion;
mod system_motion_probe;
mod tools;
mod ui_fonts;
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
        let cursor = !ui.get_terminal_input() || !ui.get_prototype_chrome();
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

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
enum RenderBackend {
    #[default]
    Auto,
    Software,
    OpenGl,
    Metal,
}
impl RenderBackend {
    fn software_video(self, backend_override: Option<&str>) -> bool {
        match self {
            Self::Software | Self::Metal => true,
            Self::OpenGl => false,
            Self::Auto => backend_override
                .map_or(cfg!(target_os = "macos"), |name| name.ends_with("software")),
        }
    }
}

struct Options {
    benchmark: Option<PathBuf>,
    store_profile: Option<profile::Profile>,
    media_origin: Option<String>,
    verify_shell: bool,
    verify_motion: bool,
    renderer: RenderBackend,
    zoom: f32,
    capture: Option<PathBuf>,
    source_png: Option<PathBuf>,
    verify_context: Option<PathBuf>,
    verify_tools: Option<PathBuf>,
    verify_article: Option<PathBuf>,
    verify_article_session: Option<PathBuf>,
    verify_media: Option<PathBuf>,
    verify_color: Option<PathBuf>,
    verify_drafts: Option<PathBuf>,
    probe: Option<Probe>,
}

fn options() -> Result<Options, Box<dyn Error>> {
    options_from(std::env::args_os().skip(1))
}

fn options_from(
    mut args: impl Iterator<Item = std::ffi::OsString>,
) -> Result<Options, Box<dyn Error>> {
    const USAGE: &str = "usage: pttstella-slint [--probe-store NEW_DIR MARKED_STORE_ROOT|--verify-motion NEW_DIR|--verify-shell NEW_DIR|--probe-benchmark NEW_DIR|--probe-editor-color|--connect|--show-profile|--probe-grid|--probe-drafts|--probe-media|--probe-inline|--probe-article|--probe-full-load|--probe-article-input|--probe-article-output|--probe-article-keepalive|--probe-article-paste|--probe-article-find|--probe-terminal-find|--probe-long-load|--probe-stalled-article|--probe-terminal-pages|--probe-ime|--probe-session|--probe-input|--probe-clipboard|--probe-keepalive|--probe-login|--probe-login-2fa|--probe-selection|--probe-vim|--probe-pointer|--probe-tools|--probe-tools-station|--probe-context-menu|--probe-context-menu-station|--probe-station-mouse|--probe-station-mouse-drag|--probe-station-mouse-motion|--probe-station-mouse-wheel|--probe-station-mouse-round-trip] [--store-scene grid|drafts|gifs|long] [--media-origin LOOPBACK_HTTP_ORIGIN] [--software-renderer|--opengl-renderer|--metal-renderer] [--zoom 100|125|150] [--capture NEW.png] [--source-png NEW.png] [--verify-editor-color NEW_DIR] [--verify-drafts NEW_DIR] [--verify-media NEW_DIR] [--verify-context-menu NEW_DIR] [--verify-tools NEW_DIR] [--verify-article NEW_DIR] [--verify-article-session NEW_DIR]";
    let mut zoom = None;
    let mut benchmark = None;
    let mut store_root = None;
    let mut store_scene = None;
    let mut media_origin = None;
    let mut verify_shell = false;
    let mut verify_motion = false;
    let mut renderer = None;
    let mut capture = None;
    let mut probe = None;
    let mut source_png = None;
    let mut verify_context = None;
    let mut verify_tools = None;
    let mut verify_article = None;
    let mut verify_article_session = None;
    let mut verify_media = None;
    let mut verify_drafts = None;
    let mut verify_color = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--media-origin") if media_origin.is_none() => {
                let value = args.next().ok_or(USAGE)?;
                media_origin = Some(replay::media_benchmark::origin(
                    value.to_str().ok_or(USAGE)?,
                )?);
            }
            Some("--store-scene") if store_scene.is_none() => {
                store_scene = Some(match args.next().as_deref().and_then(|s| s.to_str()) {
                    Some("grid") => session::Probe::Benchmark,
                    Some("drafts") => session::Probe::Drafts,
                    Some("gifs") => session::Probe::DualGifs,
                    Some("long") => session::Probe::ArticleLong,
                    _ => return Err(USAGE.into()),
                });
            }
            Some("--probe-store") if probe.is_none() => {
                benchmark = Some(args.next().ok_or(USAGE)?.into());
                store_root = Some(PathBuf::from(args.next().ok_or(USAGE)?));
                probe = Some(Probe::Session(session::Probe::Benchmark));
            }
            Some("--verify-motion") if probe.is_none() => {
                benchmark = Some(args.next().ok_or(USAGE)?.into());
                verify_motion = true;
                probe = Some(Probe::Session(session::Probe::Benchmark));
            }
            Some("--verify-shell") if probe.is_none() => {
                benchmark = Some(args.next().ok_or(USAGE)?.into());
                verify_shell = true;
                probe = Some(Probe::Session(session::Probe::Benchmark));
            }
            Some("--probe-benchmark") if probe.is_none() => {
                benchmark = Some(args.next().ok_or(USAGE)?.into());
                probe = Some(Probe::Session(session::Probe::Benchmark));
            }
            Some("--software-renderer") if renderer.is_none() => {
                renderer = Some(RenderBackend::Software);
            }
            Some("--opengl-renderer") if renderer.is_none() => {
                renderer = Some(RenderBackend::OpenGl);
            }
            Some("--metal-renderer") if renderer.is_none() => {
                renderer = Some(RenderBackend::Metal);
            }
            Some("--verify-editor-color") if verify_color.is_none() => {
                verify_color = Some(args.next().ok_or(USAGE)?.into());
            }
            Some("--probe-editor-color") if probe.is_none() => {
                probe = Some(Probe::Session(session::Probe::EditorColor));
            }
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
                    _ => return Err(USAGE.into()),
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
            _ => return Err(USAGE.into()),
        }
    }
    if (matches!(
        probe,
        Some(Probe::Ime | Probe::Session(_) | Probe::Connect | Probe::ProfileInfo)
    ) && capture.is_some())
        || (source_png.is_some()
            && (probe != Some(Probe::Grid) || capture.is_none() || source_png == capture))
    {
        return Err(USAGE.into());
    }
    if verify_context.is_some()
        && (!matches!(
            probe,
            Some(Probe::Session(
                session::Probe::ContextMenu | session::Probe::ContextMenuStation
            ))
        ) || capture.is_some())
    {
        return Err(USAGE.into());
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
        return Err(USAGE.into());
    }
    if verify_article.is_some()
        && (probe != Some(Probe::Article)
            || capture.is_some()
            || verify_context.is_some()
            || verify_tools.is_some())
    {
        return Err(USAGE.into());
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
        return Err(USAGE.into());
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
        return Err(USAGE.into());
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
        return Err(USAGE.into());
    }
    if verify_color.is_some()
        && (probe != Some(Probe::Session(session::Probe::EditorColor))
            || capture.is_some()
            || verify_drafts.is_some()
            || verify_media.is_some()
            || verify_context.is_some()
            || verify_tools.is_some()
            || verify_article.is_some()
            || verify_article_session.is_some())
    {
        return Err(USAGE.into());
    }
    // Select only after rejecting mixed probe/verifier options. The scene changes
    // the existing strict loopback peer, never the marked store paths or schema.
    if let Some(scene) = store_scene {
        if store_root.is_none() {
            return Err(USAGE.into());
        }
        probe = Some(Probe::Session(scene));
    }
    if (probe == Some(Probe::Session(session::Probe::DualGifs))) != media_origin.is_some() {
        return Err(USAGE.into());
    }
    // Ordinary app launches enter the saved/default connection flow. A capture
    // without an explicit probe retains the offline grid export behavior.
    let probe = Some(probe.unwrap_or(if capture.is_some() {
        Probe::Grid
    } else {
        Probe::Connect
    }));
    Ok(Options {
        benchmark,
        media_origin,
        store_profile: store_root.map(profile::Profile::fixture).transpose()?,
        verify_shell,
        verify_motion,
        renderer: renderer.unwrap_or_default(),
        verify_color,
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
        &options.benchmark,
        &options.verify_context,
        &options.verify_tools,
        &options.verify_article,
        &options.verify_article_session,
        &options.verify_media,
        &options.verify_drafts,
        &options.verify_color,
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
    let bytes = match options.media_origin.as_deref() {
        Some(origin) => replay::media_benchmark::page(origin)?,
        None => replay::parity_grid()?,
    };
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
                None,
                None,
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
                    None,
                    None,
                )
            });
        }
        Some(Probe::Session(probe)) => {
            let mut settings = AppSettings {
                display: replay::parity_display_profile(),
                ..Default::default()
            };
            if probe == session::Probe::ArticleFull
                && std::env::var_os("PTTSTELLA_SLINT_STARTUP_BOUNDARY_PROBE").is_some()
            {
                settings.article_boundary_navigation = true;
            }
            if matches!(probe, session::Probe::Tools | session::Probe::ToolsStation) {
                settings.tools_opacity = pttstella::settings::ToolOpacity::try_from(20)?;
            }
            let preferences = if let Some(profile) = &options.store_profile {
                let preferences = profile.load_preferences()?;
                settings = preferences.app().clone();
                pttstella::i18n::initialize(settings.language)?;
                Some(preferences)
            } else if let Some(directory) = &options.benchmark {
                let store = pttstella::settings::store::Store::at(directory.join("settings.json"));
                store.save(settings.clone())?;
                Some(pttstella::settings::preferences::Preferences::loaded(
                    Ok(store),
                    settings.clone(),
                    None,
                )?)
            } else {
                None
            };
            return session::with_probe(
                probe,
                bytes,
                options.verify_media.clone(),
                |connection, site_peers| {
                    run(
                        options,
                        screen.frame(),
                        Some((session::Mode::Probe(probe), connection)),
                        settings,
                        preferences,
                        site_peers,
                    )
                },
            );
        }
        Some(Probe::Connect) => {
            let profile = profile::Profile::system()?;
            let preferences = profile.load_preferences()?;
            let settings = preferences.app().clone();
            let warning = preferences.warning().map(str::to_owned);
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
                    Some(preferences),
                    None,
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
    run(
        options,
        screen.frame(),
        None,
        AppSettings::default(),
        None,
        None,
    )
}

fn run(
    options: Options,
    frame: Frame,
    connection: Option<(session::Mode, Rc<Connection>)>,
    settings: AppSettings,
    mut loaded_preferences: Option<pttstella::settings::preferences::Preferences>,
    site_peers: Option<Rc<pttstella::connection::site_probe::Peers>>,
) -> Result<(), Box<dyn Error>> {
    // Metal draws the UI on the GPU, but doesn't expose an OpenGL context to
    // libmpv. Select its existing software video bridge before mounting it;
    // a supported rendering notifier need not deliver a GL setup callback.
    let software_video = options
        .renderer
        .software_video(std::env::var("SLINT_BACKEND").ok().as_deref());
    if options.renderer == RenderBackend::Software {
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("skia-software".into())
            .select()?;
    } else if options.renderer == RenderBackend::OpenGl {
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("skia".into())
            .require_opengl_with_version(3, 3)
            .select()?;
    } else if options.renderer == RenderBackend::Metal {
        #[cfg(target_os = "macos")]
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("skia".into())
            .require_metal()
            .select()?;
        #[cfg(not(target_os = "macos"))]
        return Err("Metal rendering requires macOS".into());
    } else if std::env::var_os("SLINT_BACKEND").is_some() {
        #[cfg(feature = "native-video")]
        if !software_video {
            slint::BackendSelector::new()
                .require_opengl_with_version(3, 3)
                .select()?;
        } else {
            slint::BackendSelector::new().select()?;
        }
        #[cfg(not(feature = "native-video"))]
        slint::BackendSelector::new().select()?;
    } else {
        #[cfg(all(feature = "native-video", not(target_os = "macos")))]
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("skia".into())
            .require_opengl_with_version(3, 3)
            .select()?;
        // The native-video feature must not force a persistent GL context on
        // macOS. The software compositor keeps the existing CPU video bridge;
        // explicit GL/Metal choices remain available for measured workloads.
        #[cfg(target_os = "macos")]
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("skia-software".into())
            .select()?;
    }
    let ui = TerminalWindow::new()?;
    // The platform exists now; register process-wide fallbacks before showing
    // this root or creating any settings/editor/manager windows.
    ui_fonts::register()?;
    ui.set_macos_shortcuts(cfg!(target_os = "macos"));
    let notice_owner = notice::Owner::mount(&ui)?;
    let system_motion_owner = system_motion::Owner::mount(
        &ui,
        options.verify_motion || matches!(options.probe, Some(Probe::Connect | Probe::Ime)),
    )?;
    let motion_audit_directory = options.benchmark.clone().filter(|_| options.verify_motion);
    let notice_audit_directory = options.benchmark.clone().filter(|_| options.verify_shell);
    ui.set_media_video_software(software_video);
    locale::refresh(&ui);
    let quit_requested = Rc::new(Cell::new(false));
    #[cfg(target_os = "macos")]
    let _native_menu = native_menu::Menu::mount(&ui)?;

    // Probe settings live only in this owned temporary namespace; retain it
    // until output workers have drained at both normal and capture shutdown.
    let settings_probe = options.probe == Some(Probe::Session(session::Probe::Media))
        && std::env::var_os("PTTSTELLA_SLINT_SETTINGS_PROBE").is_some();
    let settings_search_probe = options.probe == Some(Probe::Session(session::Probe::Media))
        && std::env::var("PTTSTELLA_SLINT_SETTINGS_PROBE").as_deref() == Ok("search");
    let destination_probe = options.probe == Some(Probe::Session(session::Probe::Media))
        && std::env::var_os("PTTSTELLA_SLINT_DESTINATION_PROBE").is_some();
    let bookmarks_probe = options.probe == Some(Probe::Session(session::Probe::Media))
        && std::env::var_os("PTTSTELLA_SLINT_BOOKMARKS_PROBE").is_some();
    let capture_probe = options.probe == Some(Probe::Session(session::Probe::Media))
        && std::env::var_os("PTTSTELLA_SLINT_CAPTURE_PROBE").is_some();
    let updates_probe = options.probe == Some(Probe::Session(session::Probe::Media))
        && std::env::var_os("PTTSTELLA_SLINT_UPDATES_PROBE").is_some();
    let replies_probe = options.probe == Some(Probe::Session(session::Probe::Media))
        && std::env::var_os("PTTSTELLA_SLINT_REPLIES_PROBE").is_some();
    let filters_probe = options.probe == Some(Probe::Session(session::Probe::Media))
        && std::env::var_os("PTTSTELLA_SLINT_FILTERS_PROBE").is_some();
    let filters_probe_store = if settings_search_probe || filters_probe {
        Some(
            tempfile::Builder::new()
                .prefix("pttstella-slint-filters-")
                .tempdir()?,
        )
    } else {
        None
    };
    let sites_probe_store = if options.probe == Some(Probe::Session(session::Probe::Media))
        && (std::env::var_os("PTTSTELLA_SLINT_SITES_PROBE").is_some()
            || site_peers.is_some()
            || settings_search_probe)
    {
        let directory = tempfile::Builder::new()
            .prefix("pttstella-slint-sites-")
            .tempdir()?;
        let path = directory.path().join("sites.json");
        let store = pttstella::sites::store::Store::at(path);
        let mut seed = pttstella::sites::Sites::default();
        seed.favourites.push(pttstella::sites::Favourite {
            name: "Legacy synthetic".into(),
            target: pttstella::sites::Target::new(
                "telnet://legacy.example:2323".into(),
                pttstella::encoding::TextEncoding::Gbk,
            )?,
        });
        if let Some(peers) = &site_peers {
            for (name, target) in [
                ("FIRST UAO peer", &peers.first),
                ("GBK peer", &peers.gbk),
                ("UAO peer", &peers.uao),
                ("Held dial peer", &peers.hanging),
            ] {
                seed.favourites.push(pttstella::sites::Favourite {
                    name: name.into(),
                    target: target.clone(),
                });
            }
        }
        store.save(seed)?;
        Some((directory, store))
    } else {
        None
    };
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
                        crate::notice::error(&ui, error);
                    }
                } else {
                    crate::notice::success(&ui, url);
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
                            crate::notice::error(&ui, error);
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
                        crate::notice::error(&ui, error);
                    }
                }
            });
        }
    });
    let mut article_session_owner = None;
    let mut login_owner = None;
    let mut keep_alive_mounted = false;
    let mut paste_owner = None;
    let mut output_owner = None;
    let mut sites_owner = None;
    let mut filters_owner = None;
    let mut bookmarks_owner = None;
    let mut destination_owner = None;
    let mut context_owner = None;
    let mut tools_owner = None;
    let mut station_mouse_owner = None;
    let mut media_owner = None;
    let mut editor_owner = None;
    let mut station_color_owner = None;
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
    let verification_connection = if bookmarks_probe
        || options.verify_shell
        || options.verify_motion
        || replies_probe
        || updates_probe
        || options.verify_context.is_some()
        || options.verify_tools.is_some()
        || options.verify_article_session.is_some()
        || options.probe == Some(Probe::Session(session::Probe::Media))
            && std::env::var_os("PTTSTELLA_SLINT_ACCOUNT_ACTIONS").is_some()
    {
        connection
            .as_ref()
            .map(|(_, connection)| connection.clone())
    } else {
        None
    };
    let long_store_scene = options.store_profile.is_some()
        && options.probe == Some(Probe::Session(session::Probe::ArticleLong));
    let benchmark_connection = if options.media_origin.is_some() || long_store_scene {
        connection
            .as_ref()
            .map(|(_, connection)| connection.clone())
    } else {
        None
    };
    let preparation_fixture = if options.store_profile.is_some()
        && options.probe == Some(Probe::Session(session::Probe::Drafts))
    {
        benchmark::Fixture::DraftBoard(
            connection
                .as_ref()
                .map(|(_, c)| c.clone())
                .ok_or("marked drafts source connection missing")?,
        )
    } else {
        benchmark::Fixture::ParityGrid
    };
    let watcher = connection
        .map(|(mode, connection)| {
            let full_services = mode.full_services() || options.store_profile.is_some();
            ui.set_prototype_chrome(!full_services);
            if full_services {
                ui.set_result("".into());
            }
            if full_services
                || matches!(
                    &mode,
                    session::Mode::Probe(
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
                )
                || settings_search_probe
                || destination_probe
                || bookmarks_probe
                || capture_probe
                || replies_probe
                || updates_probe
                || filters_probe
            {
                let mut article_preferences = settings.clone();
                if !full_services {
                    article_preferences.article_mode = if matches!(
                        &mode,
                        session::Mode::Probe(
                            session::Probe::ArticleTerminal | session::Probe::ArticleFindTerminal
                        )
                    ) || settings_search_probe
                        || destination_probe
                        || bookmarks_probe
                        || filters_probe
                            && !std::env::var("PTTSTELLA_SLINT_FILTERS_PROBE")
                                .is_ok_and(|v| v == "web")
                    {
                        pttstella::settings::ArticleMode::Terminal
                    } else {
                        pttstella::settings::ArticleMode::Web
                    };
                }
                article_session_owner = Some(article::session::ArticleSession::mount(
                    &ui,
                    connection.clone(),
                    renderer.clone(),
                    previous.clone(),
                    presented.clone(),
                    &article_preferences,
                )?);
                article_bridge.replace(article_session_owner.clone());
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
            if full_services
                || matches!(
                    &mode,
                    session::Mode::Probe(
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
                )
                || capture_probe
                || replies_probe
                || updates_probe
            {
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
            if !full_services
                && matches!(
                    &mode,
                    session::Mode::Probe(
                        session::Probe::Drafts
                            | session::Probe::EditorColor
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
                )
                && !capture_probe
                && !replies_probe
                && !updates_probe
            {
                paste_owner = Some(native_input::mount(
                    &ui,
                    connection.clone(),
                    presented.clone(),
                    clipboard.clone(),
                ));
            }
            if full_services
                || matches!(
                    &mode,
                    session::Mode::Probe(
                        session::Probe::KeepAlive
                            | session::Probe::ArticleKeepAlive
                            | session::Probe::Login
                            | session::Probe::Login2fa
                    )
                )
                || settings_probe
            {
                keep_alive::mount(
                    &ui,
                    connection.clone(),
                    presented.clone(),
                    article_session_owner.clone(),
                );
                keep_alive_mounted = true;
            }
            if full_services
                || matches!(
                    &mode,
                    session::Mode::Probe(session::Probe::Login | session::Probe::Login2fa)
                )
                || settings_probe
            {
                login_owner = Some(login::Login::mount(
                    &ui,
                    connection.clone(),
                    presented.clone(),
                    matches!(&mode, session::Mode::Probe(_)),
                ));
            }
            if full_services
                || matches!(
                    &mode,
                    session::Mode::Probe(
                        session::Probe::EditorColor
                            | session::Probe::Media
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
                )
            {
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
                station_color_owner = Some(station_color::StationColor::mount(
                    &ui,
                    connection.clone(),
                    output.clone(),
                ));
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
                if full_services
                    || matches!(
                        &mode,
                        session::Mode::Probe(session::Probe::Media | session::Probe::ArticleInline)
                    )
                {
                    let preferences = match loaded_preferences.take() {
                        Some(preferences) => preferences,
                        None => {
                            let directory = media_probe_settings
                                .as_ref()
                                .ok_or("isolated settings seed missing")?;
                            let store = pttstella::settings::store::Store::at(
                                directory.path().join("settings.json"),
                            );
                            // The shared Store merges against its actual document. A
                            // synthetic in-memory baseline must have the same seed,
                            // rather than treating the product defaults as external edits.
                            store.save(settings.clone())?;
                            pttstella::settings::preferences::Preferences::loaded(
                                Ok(store),
                                settings.clone(),
                                None,
                            )?
                        }
                    };
                    media_owner = Some(media::Media::mount(
                        &ui,
                        connection.clone(),
                        output.clone(),
                        matches!(
                            &mode,
                            session::Mode::Probe(
                                session::Probe::Media
                                    | session::Probe::ArticleInline
                                    | session::Probe::DualGifs
                            )
                        ),
                        preferences,
                        matches!(&mode, session::Mode::Live(_)),
                    ));
                }
                if let (Some(media), Some(article)) = (&media_owner, &article_session_owner) {
                    media.mount_inline(&ui, article);
                    if let Some(context) = &context_owner {
                        context.attach_media(media);
                    }
                }
                let sites_state = if let Some(profile) = &options.store_profile {
                    Some(Rc::new(RefCell::new(
                        pttstella::sites::session::State::load(Ok(profile.sites()), true),
                    )))
                } else {
                    match &mode {
                        session::Mode::Probe(session::Probe::Benchmark) => {
                            options.benchmark.as_ref().map(|directory| {
                                Rc::new(RefCell::new(pttstella::sites::session::State::load(
                                    Ok(pttstella::sites::store::Store::at(
                                        directory.join("sites.json"),
                                    )),
                                    true,
                                )))
                            })
                        }
                        session::Mode::Live(stations) => Some(stations.state.clone()),
                        session::Mode::Probe(session::Probe::Media) => {
                            sites_probe_store.as_ref().map(|(_, store)| {
                                Rc::new(RefCell::new(pttstella::sites::session::State::load(
                                    Ok(store.clone()),
                                    true,
                                )))
                            })
                        }
                        _ => None,
                    }
                };
                if let Some(state) = sites_state {
                    sites_owner = Some(sites_manager::Manager::mount(
                        &ui,
                        connection.clone(),
                        output.clone(),
                        state,
                        matches!(&mode, session::Mode::Probe(_)),
                    ));
                }
                let filter_store = if let Some(directory) = &options.benchmark {
                    Some(Ok(options.store_profile.as_ref().map_or_else(
                        || pttstella::filters::store::Store::at(directory.join("filters.json")),
                        profile::Profile::filters,
                    )))
                } else if matches!(&mode, session::Mode::Live(_)) {
                    Some(profile::Profile::system().map(|p| p.filters()))
                } else {
                    filters_probe_store.as_ref().map(|d| {
                        Ok(pttstella::filters::store::Store::at(
                            d.path().join("filters.json"),
                        ))
                    })
                };
                if let (Some(store), Some(article)) = (filter_store, article_session_owner.as_ref())
                {
                    filters_owner = Some(filters_manager::Manager::mount(
                        &ui,
                        output.clone(),
                        article.clone(),
                        store,
                    ));
                }
                if let (Some(context), Some(filters)) = (&context_owner, &filters_owner) {
                    context.attach_filters(filters);
                }
                let bookmark_store = if let Some(directory) = &options.benchmark {
                    Some(Ok(options.store_profile.as_ref().map_or_else(
                        || pttstella::bookmarks::store::Store::at(directory.join("bookmarks.json")),
                        profile::Profile::bookmarks,
                    )))
                } else if matches!(&mode, session::Mode::Live(_)) {
                    Some(profile::Profile::system().map(|p| p.bookmarks()))
                } else if options.probe == Some(Probe::Session(session::Probe::Media))
                    && (std::env::var_os("PTTSTELLA_SLINT_BOOKMARKS_PROBE").is_some()
                        || capture_probe
                        || destination_probe)
                {
                    Some(
                        options
                            .verify_media
                            .as_ref()
                            .map(|d| {
                                // Explicit loopback probes may reopen the same
                                // data file while keeping each evidence run new.
                                // Live mode above always uses its normal profile.
                                let file = std::env::var_os("PTTSTELLA_SLINT_BOOKMARKS_PROBE_FILE")
                                    .map(PathBuf::from)
                                    .unwrap_or_else(|| d.join("bookmarks.json"));
                                pttstella::bookmarks::store::Store::at(file)
                            })
                            .ok_or_else(|| "bookmark verification directory missing".to_owned()),
                    )
                } else {
                    None
                };
                if let (Some(store), Some(article)) =
                    (bookmark_store, article_session_owner.as_ref())
                {
                    if bookmarks_probe {
                        bookmarks_manager::probe::prepare_native(&store)?;
                    }
                    bookmarks_owner = Some(bookmarks_manager::Manager::mount(
                        &ui,
                        output.clone(),
                        article.clone(),
                        connection.clone(),
                        store,
                    ));
                }
                if let (Some(context), Some(bookmarks)) = (&context_owner, &bookmarks_owner) {
                    context.attach_bookmarks(bookmarks);
                }
                let recent_store = if let Some(directory) = &options.benchmark {
                    Some(Ok(options.store_profile.as_ref().map_or_else(
                        || {
                            pttstella::recent_boards::store::Store::at(
                                directory.join("recent-boards.json"),
                            )
                        },
                        profile::Profile::recent,
                    )))
                } else if matches!(&mode, session::Mode::Live(_)) {
                    Some(profile::Profile::system().map(|p| p.recent()))
                } else if destination_probe {
                    Some(
                        options
                            .verify_media
                            .as_ref()
                            .map(|d| {
                                pttstella::recent_boards::store::Store::at(
                                    d.join("recent-boards.json"),
                                )
                            })
                            .ok_or_else(|| "destination verification directory missing".to_owned()),
                    )
                } else {
                    None
                };
                if let (Some(store), Some(article)) = (recent_store, article_session_owner.as_ref())
                {
                    destination_owner = Some(destination::Manager::mount(
                        &ui,
                        connection.clone(),
                        article.clone(),
                        output.clone(),
                        store,
                    ));
                }
                output_owner = Some(output);
            }
            if full_services || matches!(&mode, session::Mode::Probe(session::Probe::Drafts)) {
                let store = if let Some(directory) = &options.benchmark {
                    Ok(options.store_profile.as_ref().map_or_else(
                        || pttstella::drafts::store::Store::at(directory.join("drafts.json")),
                        profile::Profile::drafts,
                    ))
                } else if matches!(&mode, session::Mode::Probe(session::Probe::Drafts)) {
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
            if let Some(media) = &media_owner {
                media.preferences.subscribe({
                    let renderer = renderer.clone();
                    let previous = previous.clone();
                    let output = output_owner.as_ref().map(Rc::downgrade);
                    let editor = editor_owner.as_ref().map(Rc::downgrade);
                    let context = context_owner.as_ref().map(Rc::downgrade);
                    let sites = sites_owner.as_ref().map(Rc::downgrade);
                    let filters = filters_owner.as_ref().map(Rc::downgrade);
                    let bookmarks = bookmarks_owner.as_ref().map(Rc::downgrade);
                    let destination = destination_owner.as_ref().map(Rc::downgrade);
                    let article = article_session_owner.as_ref().map(Rc::downgrade);
                    let media = Rc::downgrade(media);
                    let weak = ui.as_weak();
                    let applied = RefCell::new(settings.clone());
                    move |settings| {
                        let changed = applied.borrow().clone();
                        if changed.display != settings.display {
                            let display = settings.display.resolve()?;
                            renderer
                                .borrow_mut()
                                .configure(&display)
                                .map_err(str::to_owned)?;
                            previous.set(None);
                            if let Some(output) = output.as_ref().and_then(|o| o.upgrade()) {
                                output.set_display(display);
                            }
                            if let Some(editor) = editor.as_ref().and_then(|o| o.upgrade()) {
                                editor.copy_display(&renderer.borrow());
                            }
                        }
                        if (changed.display != settings.display
                            || changed.article_mode != settings.article_mode
                            || changed.article_boundary_navigation
                                != settings.article_boundary_navigation)
                            && let Some(article) = article.as_ref().and_then(|o| o.upgrade())
                        {
                            article.preferences_changed(settings)?;
                        }
                        pttstella::i18n::application().select(settings.language);
                        if let Some(ui) = weak.upgrade() {
                            if changed.language != settings.language {
                                locale::refresh(&ui);
                                locale::refresh_statuses(&ui, changed.language);
                                if let Some(sites) = sites.as_ref().and_then(|o| o.upgrade()) {
                                    sites.refresh_labels();
                                }
                                if let Some(filters) = filters.as_ref().and_then(|o| o.upgrade()) {
                                    filters.refresh_labels();
                                }
                                if let Some(destination) =
                                    destination.as_ref().and_then(|o| o.upgrade())
                                {
                                    destination.refresh_labels();
                                }
                                if let Some(bookmarks) =
                                    bookmarks.as_ref().and_then(|o| o.upgrade())
                                {
                                    bookmarks.refresh_labels();
                                }
                                if let Some(context) = context.as_ref().and_then(|o| o.upgrade()) {
                                    context.refresh_labels();
                                }
                                if let Some(article) = article.as_ref().and_then(|o| o.upgrade()) {
                                    article.refresh_language();
                                }
                                if let Some(editor) = editor.as_ref().and_then(|o| o.upgrade()) {
                                    editor.refresh_language();
                                }
                            }
                            ui.set_tools_opacity(settings.tools_opacity.percent() as f32 / 100.);
                            ui.set_tools_visible(settings.tools_access.show_floating_button);
                            ui.set_tools_right_click(settings.tools_access.right_click_menu);
                            ui.invoke_tools_updated();
                            ui.invoke_native_menu_refresh();
                        }
                        applied.replace(settings.clone());
                        if let Some(media) = media.upgrade() {
                            media.preferences.refresh_labels();
                        }
                        Ok(())
                    }
                });
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
    let app_exit = app_exit::Exit::mount(
        &ui,
        media_owner.as_ref(),
        article_session_owner.as_ref(),
        &system_motion_owner,
        app_exit::Managers {
            sites: sites_owner.as_ref(),
            filters: filters_owner.as_ref(),
            bookmarks: bookmarks_owner.as_ref(),
            destination: destination_owner.as_ref(),
        },
        quit_requested.clone(),
    );
    let release_probe_directory = if options.probe == Some(Probe::Session(session::Probe::Media))
        && std::env::var("PTTSTELLA_SLINT_SETTINGS_PROBE").as_deref() == Ok("releases")
    {
        options.verify_media.clone()
    } else {
        None
    };
    let settings_shutdown_directory = if options.probe
        == Some(Probe::Session(session::Probe::Media))
        && std::env::var("PTTSTELLA_SLINT_SETTINGS_PROBE").is_ok_and(|v| v == "shutdown")
    {
        options.verify_media.clone()
    } else {
        None
    };
    let native_quit_directory = if options.probe == Some(Probe::Session(session::Probe::Media))
        && std::env::var("PTTSTELLA_SLINT_SETTINGS_PROBE").is_ok_and(|v| v == "native-quit")
    {
        options.verify_media.clone()
    } else {
        None
    };
    let destination_verification_directory =
        options.verify_media.clone().filter(|_| destination_probe);
    let bookmarks_verification_directory = options
        .verify_media
        .clone()
        .filter(|_| bookmarks_probe || capture_probe);
    let filters_verification_directory = options.verify_media.clone().filter(|_| filters_probe);
    let sites_verification_directory = options
        .verify_media
        .clone()
        .filter(|_| sites_owner.is_some());
    let draft_verification_directory = options.verify_drafts.clone();
    #[cfg(feature = "native-video")]
    let video_exit_directory = options.verify_media.clone().filter(|_| {
        options.probe == Some(Probe::Session(session::Probe::Media))
            && std::env::var_os("PTTSTELLA_VIDEO_PROBE_EXIT").is_some()
    });
    let verification = if options.verify_motion {
        Some(system_motion_probe::start(
            &ui,
            system_motion_owner.clone(),
            verification_connection
                .as_ref()
                .cloned()
                .ok_or("motion connection owner missing")?,
            options
                .benchmark
                .clone()
                .ok_or("motion directory missing")?,
        )?)
    } else if options.verify_shell {
        Some(shell_probe::start(
            &ui,
            notice_owner.clone(),
            verification_connection
                .as_ref()
                .cloned()
                .ok_or("shell connection owner missing")?,
            options
                .benchmark
                .clone()
                .ok_or("shell output directory missing")?,
        )?)
    } else if native_quit_directory.is_some()
        || destination_probe
            && std::env::var("PTTSTELLA_SLINT_DESTINATION_PROBE").as_deref() == Ok("native")
        || bookmarks_probe
            && std::env::var("PTTSTELLA_SLINT_BOOKMARKS_PROBE").as_deref() == Ok("native")
        || capture_probe
            && matches!(
                std::env::var("PTTSTELLA_SLINT_CAPTURE_PROBE").as_deref(),
                Ok("native" | "native-list")
            )
        || updates_probe
            && std::env::var("PTTSTELLA_SLINT_UPDATES_PROBE").as_deref() == Ok("native")
        || replies_probe
            && std::env::var("PTTSTELLA_SLINT_REPLIES_PROBE").as_deref() == Ok("native")
        || std::env::var("PTTSTELLA_SLINT_FILTERS_PROBE").is_ok_and(|v| v == "native")
    {
        None
    } else if updates_probe {
        Some(article::inline::updates_probe::start(
            &ui,
            media_owner
                .as_ref()
                .and_then(|m| m.inline_owner())
                .ok_or("updates inline owner missing")?,
            article_session_owner
                .as_ref()
                .cloned()
                .ok_or("updates article owner missing")?,
            verification_connection
                .as_ref()
                .cloned()
                .ok_or("updates connection missing")?,
            options
                .verify_media
                .clone()
                .ok_or("updates directory missing")?,
        )?)
    } else if replies_probe {
        Some(article::replies_probe::start(
            &ui,
            article_session_owner
                .as_ref()
                .cloned()
                .ok_or("reply navigation owner missing")?,
            verification_connection
                .as_ref()
                .cloned()
                .ok_or("reply navigation connection missing")?,
            options
                .verify_media
                .clone()
                .ok_or("reply navigation directory missing")?,
        )?)
    } else if capture_probe {
        Some(bookmarks_manager::capture_probe::start(
            &ui,
            bookmarks_owner
                .as_ref()
                .cloned()
                .ok_or("capture verifier owner missing")?,
            options
                .verify_media
                .clone()
                .ok_or("capture verification directory missing")?,
        )?)
    } else if destination_probe {
        Some(destination::probe::start(
            &ui,
            destination_owner
                .as_ref()
                .cloned()
                .ok_or("destination verifier owner missing")?,
            bookmarks_owner
                .as_ref()
                .cloned()
                .ok_or("destination bookmark handoff owner missing")?,
            options
                .verify_media
                .clone()
                .ok_or("destination verification directory missing")?,
        )?)
    } else if bookmarks_probe {
        Some(bookmarks_manager::probe::start(
            &ui,
            bookmarks_owner
                .as_ref()
                .cloned()
                .ok_or("bookmark verification owner missing")?,
            verification_connection
                .as_ref()
                .cloned()
                .ok_or("bookmark verification connection missing")?,
            options
                .verify_media
                .clone()
                .ok_or("bookmark verification directory missing")?,
        )?)
    } else if filters_probe && std::env::var_os("PTTSTELLA_SLINT_ACCOUNT_ACTIONS").is_some() {
        Some(filters_manager::account_probe::start(
            &ui,
            filters_owner
                .as_ref()
                .cloned()
                .ok_or("account filter owner missing")?,
            verification_connection
                .clone()
                .ok_or("account connection missing")?,
            media_owner
                .as_ref()
                .cloned()
                .ok_or("account media missing")?,
            options
                .verify_media
                .clone()
                .ok_or("account output directory missing")?,
            filters_probe_store
                .as_ref()
                .ok_or("account store missing")?
                .path()
                .join("filters.json"),
        )?)
    } else if filters_probe {
        Some(filters_manager::probe::start(
            &ui,
            filters_owner
                .as_ref()
                .cloned()
                .ok_or("filter manager missing")?,
            options
                .verify_media
                .clone()
                .ok_or("filter verifier directory missing")?,
            filters_probe_store
                .as_ref()
                .ok_or("filter probe store missing")?
                .path()
                .join("filters.json"),
        )?)
    } else if let Some((directory, owner)) = options
        .verify_media
        .as_ref()
        .filter(|_| sites_probe_store.is_some() && !settings_search_probe)
        .zip(sites_owner.as_ref())
    {
        if let Some(peers) = &site_peers {
            Some(sites_manager::switch_probe::start(
                &ui,
                owner.clone(),
                peers.clone(),
                directory.clone(),
            )?)
        } else {
            Some(sites_manager::probe::start(
                &ui,
                owner.clone(),
                directory.clone(),
                sites_probe_store
                    .as_ref()
                    .ok_or("site probe store missing")?
                    .0
                    .path()
                    .join("sites.json"),
            )?)
        }
    } else if let Some(directory) = options.verify_color {
        Some(station_color::probe::start(
            &ui,
            station_color_owner
                .as_ref()
                .cloned()
                .ok_or("station color owner missing")?,
            directory,
        )?)
    } else if let Some(directory) = options.verify_drafts {
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
            if std::env::var_os("PTTSTELLA_VIDEO_PROBE_EXIT").is_some() {
                media::video::exit_probe::start
            } else {
                media::video::probe::start
            }
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
            media_probe_settings
                .as_ref()
                .ok_or("isolated media settings missing")?
                .path()
                .join("settings.json"),
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
    let preparation_fixture = if let Some(origin) = &options.media_origin {
        benchmark::Fixture::DualGifs {
            origin: origin.clone(),
            connection: benchmark_connection.ok_or("dual GIF source connection missing")?,
            article: article_session_owner
                .clone()
                .ok_or("dual GIF article owner missing")?,
        }
    } else if long_store_scene {
        benchmark::Fixture::LongArticle {
            connection: benchmark_connection.ok_or("long article source connection missing")?,
            article: article_session_owner
                .clone()
                .ok_or("long article owner missing")?,
        }
    } else {
        preparation_fixture
    };
    let benchmark_preparation = options.benchmark.clone().map(|directory| {
        benchmark::Preparation::start(&ui, renderer.clone(), previous.clone(), presented.clone(), directory,
            serde_json::json!({
                "article":article_session_owner.is_some(), "input":paste_owner.is_some(),
                "output":output_owner.is_some(), "login":login_owner.is_some(),
                "keep_alive":keep_alive_mounted, "connection_watch":watcher.is_some(),
                "sites":sites_owner.is_some(), "filters":filters_owner.is_some(),
                "bookmarks":bookmarks_owner.is_some(), "destination":destination_owner.is_some(),
                "context":context_owner.is_some(), "tools":tools_owner.is_some(),
                "station_mouse":station_mouse_owner.is_some(), "media_settings":media_owner.is_some(),
                "editor":editor_owner.is_some(), "station_color":station_color_owner.is_some(),
            }), preparation_fixture)
    });
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
                if let Some(ui) = weak.upgrade() {
                    ui.invoke_app_quit();
                }
            }
        });
        app_exit.run(ui)?;
        notice_owner.stop();
        let motion_cleanup = system_motion_owner.finish();
        let exit_cleanup = app_exit.finish();
        if let Some(color) = &station_color_owner {
            color.finish();
        }
        if let Some(editor) = &editor_owner {
            editor.finish()?;
        }
        if let Some(destination) = &destination_owner {
            destination.finish()?;
        }
        if let Some(bookmarks) = &bookmarks_owner {
            bookmarks.finish()?;
        }
        if let Some(filters) = &filters_owner {
            filters.finish()?;
        }
        if let Some(sites) = &sites_owner {
            sites.finish()?;
        }
        if let Some(media) = &media_owner {
            media.preferences.finish()?;
            media.finish()?;
        }
        if let Some(paste) = &paste_owner {
            paste.finish()?;
        }
        if let Some(article) = &article_owner {
            article.finish();
        }
        exit_cleanup?;
        motion_cleanup?;
        outcome.take().ok_or("snapshot callback did not run")??;
    } else {
        let result = app_exit.run(ui);
        notice_owner.stop();
        let motion_cleanup = system_motion_owner.finish();
        let motion_audit = motion_audit_directory
            .as_ref()
            .map(|dir| system_motion_owner.audit_closed(dir))
            .transpose();
        if !notice_owner.clean() {
            return Err("notification timer did not stop".into());
        }
        if let Some(dir) = &notice_audit_directory {
            notice_owner.audit_closed(dir)?;
        }
        let exit_cleanup = app_exit.finish();
        let editor_cleanup = editor_owner
            .as_ref()
            .map(|editor| editor.finish())
            .transpose();
        let editor_close_audit = match (&editor_owner, &draft_verification_directory) {
            (Some(editor), Some(dir)) => {
                editor::probe::audit_closed(editor, dir, editor_cleanup.is_ok())
            }
            _ => Ok(()),
        };
        let destination_cleanup = destination_owner.as_ref().map(|o| o.finish()).transpose();
        let destination_close_audit =
            match (&destination_owner, &destination_verification_directory) {
                (Some(owner), Some(dir)) => destination::probe::audit_closed(owner, dir),
                _ => Ok(()),
            };
        let bookmarks_cleanup = bookmarks_owner.as_ref().map(|s| s.finish()).transpose();
        let bookmarks_close_audit = match (&bookmarks_owner, &bookmarks_verification_directory) {
            (Some(owner), Some(dir)) if capture_probe => {
                bookmarks_manager::capture_probe::audit_closed(owner, dir)
            }
            (Some(owner), Some(dir)) => bookmarks_manager::probe::audit_closed(owner, dir),
            _ => Ok(()),
        };
        let filters_cleanup = filters_owner.as_ref().map(|s| s.finish()).transpose();
        let filters_close_audit = match (
            &filters_owner,
            &filters_verification_directory,
            &filters_probe_store,
        ) {
            (Some(owner), Some(dir), Some(store)) => {
                filters_manager::probe::audit_closed(owner, dir, store.path().join("filters.json"))
            }
            _ => Ok(()),
        };
        let sites_cleanup = sites_owner.as_ref().map(|s| s.finish()).transpose();
        let sites_close_audit = match (&sites_owner, &sites_verification_directory) {
            (Some(owner), Some(dir)) => sites_manager::probe::audit_closed(owner, dir),
            _ => Ok(()),
        };
        let settings_cleanup = media_owner
            .as_ref()
            .map(|media| {
                media.preferences.finish()?;
                if let Some(dir) = &release_probe_directory {
                    media.preferences.audit_release_probe(dir)?;
                }
                if let Some(dir) = &settings_shutdown_directory {
                    media.preferences.audit_shutdown(dir)?;
                }
                if let Some(dir) = &native_quit_directory {
                    media
                        .preferences
                        .audit_native_quit(dir, quit_requested.get())?;
                }

                Ok::<_, String>(())
            })
            .transpose();
        let media_cleanup = media_owner.as_ref().map(|media| media.finish()).transpose();
        #[cfg(feature = "native-video")]
        let video_exit_audit = video_exit_directory
            .as_ref()
            .map(|dir| {
                app_exit.finish()?;
                media::video::exit_probe::audit_closed(
                    media_owner.as_ref().ok_or("exit media owner missing")?,
                    dir,
                )
            })
            .transpose();
        if let Some(article) = &article_session_owner {
            article.finish();
        }
        if let Some(article) = &article_owner {
            article.finish();
        }
        if let Some(tools) = tools_owner {
            tools.finish();
        }
        if let Some(color) = station_color_owner {
            color.finish();
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
        let benchmark_result = benchmark_preparation.map(|p| p.finish()).transpose();
        #[cfg(target_os = "macos")]
        {
            _native_menu.stop();
            if let Some(dir) = &native_quit_directory {
                if !_native_menu.stopped() {
                    return Err("native menu cleanup incomplete".into());
                }
                std::fs::write(dir.join("native-menu-closed.json"),br#"{"stopped":true,"timer_running":false,"actions_retained":false,"weak_targets_retained":false}"#)?;
            }
        }
        result?;
        exit_cleanup?;
        motion_cleanup?;
        motion_audit?;
        editor_cleanup?;
        editor_close_audit?;
        sites_cleanup?;
        sites_close_audit?;
        destination_cleanup?;
        destination_close_audit?;
        bookmarks_cleanup?;
        bookmarks_close_audit?;
        filters_cleanup?;
        filters_close_audit?;
        settings_cleanup?;
        media_cleanup?;
        #[cfg(feature = "native-video")]
        video_exit_audit?;
        output_cleanup?;
        paste_cleanup?;
        verification_result?;
        benchmark_result?;
    }
    Ok(())
}

#[cfg(test)]
mod startup_tests {
    use super::*;

    fn parse(args: &[&str]) -> Options {
        options_from(args.iter().map(std::ffi::OsString::from)).unwrap()
    }

    #[test]
    fn normal_launch_uses_the_live_owner_with_or_without_renderer_or_zoom_options() {
        for args in [
            &[][..],
            &["--software-renderer"][..],
            &["--opengl-renderer"][..],
            &["--metal-renderer"][..],
            &["--zoom", "150"][..],
            &["--connect"][..],
        ] {
            assert!(parse(args).probe == Some(Probe::Connect));
        }
    }

    #[test]
    fn renderer_overrides_are_exclusive_and_preserve_the_selected_probe() {
        assert_eq!(parse(&[]).renderer, RenderBackend::Auto);
        let choices = [
            ("--software-renderer", RenderBackend::Software),
            ("--opengl-renderer", RenderBackend::OpenGl),
            ("--metal-renderer", RenderBackend::Metal),
        ];
        for (flag, selected) in choices {
            let options = parse(&[flag, "--probe-grid"]);
            assert_eq!(options.renderer, selected);
            assert!(options.probe == Some(Probe::Grid));
            for (other, _) in choices {
                assert!(options_from([flag, other].map(Into::into).into_iter()).is_err());
            }
        }
    }

    #[test]
    fn video_bridge_matches_the_requested_api_and_preserves_environment_overrides() {
        for backend in [None, Some("winit-skia"), Some("winit-skia-software")] {
            assert!(RenderBackend::Software.software_video(backend));
            assert!(RenderBackend::Metal.software_video(backend));
            assert!(!RenderBackend::OpenGl.software_video(backend));
        }
        assert_eq!(
            RenderBackend::Auto.software_video(None),
            cfg!(target_os = "macos")
        );
        assert!(RenderBackend::Auto.software_video(Some("winit-skia-software")));
        assert!(!RenderBackend::Auto.software_video(Some("winit-skia")));
    }

    #[test]
    fn explicit_probes_and_capture_never_fall_into_live_startup() {
        for args in [
            &["--probe-grid"][..],
            &["--probe-ime"][..],
            &["--probe-media"][..],
            &["--probe-benchmark", "new-benchmark"][..],
            &["--show-profile"][..],
            &["--capture", "new.png"][..],
            &["--software-renderer", "--capture", "new.png"][..],
        ] {
            assert!(parse(args).probe != Some(Probe::Connect));
        }
        assert!(parse(&["--capture", "new.png"]).probe == Some(Probe::Grid));
        assert!(
            options_from(
                ["--connect", "--capture", "new.png"]
                    .map(Into::into)
                    .into_iter()
            )
            .is_err()
        );
    }

    #[test]
    fn benchmark_requires_its_own_directory_and_cannot_mix_with_other_probes() {
        let parsed = parse(&["--probe-benchmark", "new-benchmark"]);
        assert_eq!(parsed.benchmark, Some(PathBuf::from("new-benchmark")));
        assert!(parsed.probe == Some(Probe::Session(session::Probe::Benchmark)));
        assert!(parse(&["--verify-shell", "new-shell"]).verify_shell);
        for args in [
            &["--probe-benchmark"][..],
            &["--probe-benchmark", "a", "--connect"][..],
            &["--probe-grid", "--probe-benchmark", "a"][..],
            &["--probe-benchmark", "a", "--capture", "image.png"][..],
            &["--probe-benchmark", "a", "--verify-media", "b"][..],
        ] {
            assert!(options_from(args.iter().map(std::ffi::OsString::from)).is_err());
        }
    }

    #[test]
    fn store_probe_requires_marked_data_and_remains_an_explicit_full_service_loopback() {
        let root = tempfile::tempdir().unwrap();
        let args = [
            std::ffi::OsString::from("--probe-store"),
            root.path().join("new-evidence").into_os_string(),
            root.path().as_os_str().to_owned(),
        ];
        assert!(options_from(args.clone().into_iter()).is_err());
        std::fs::write(
            root.path().join(".store-compatibility.json"),
            br#"{"format":"PTTStella.StoreCompatibility","version":1,"synthetic":true}"#,
        )
        .unwrap();
        let parsed = options_from(args.clone().into_iter()).unwrap();
        assert!(parsed.probe == Some(Probe::Session(session::Probe::Benchmark)));
        assert!(parsed.store_profile.is_some());
        assert!(!parsed.verify_shell && !parsed.verify_motion);
        assert!(!root.path().join("new-evidence").exists());
        assert!(!root.path().join("probe-settings.json").exists());
        for extra in ["--connect", "--probe-grid", "--verify-shell"] {
            let mut invalid = args.to_vec();
            invalid.push(extra.into());
            assert!(options_from(invalid.into_iter()).is_err());
        }
        assert!(options_from(["--probe-store", "new"].map(Into::into).into_iter()).is_err());
    }

    #[test]
    fn store_scenes_require_the_marked_store_entry_and_do_not_seed_either_store() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(".store-compatibility.json"),
            br#"{"format":"PTTStella.StoreCompatibility","version":1,"synthetic":true}"#,
        )
        .unwrap();
        let path = root.path().join("probe-drafts.json");
        let original = br#"{"version":1,"entries":[]}"#;
        std::fs::write(&path, original).unwrap();
        let base = [
            std::ffi::OsString::from("--probe-store"),
            root.path().join("new-evidence").into_os_string(),
            root.path().as_os_str().to_owned(),
        ];
        for (name, scene) in [
            ("grid", session::Probe::Benchmark),
            ("drafts", session::Probe::Drafts),
            ("long", session::Probe::ArticleLong),
        ] {
            let mut args = base.to_vec();
            args.extend(["--store-scene", name].map(Into::into));
            let parsed = options_from(args.into_iter()).unwrap();
            assert!(parsed.probe == Some(Probe::Session(scene)));
            assert!(parsed.store_profile.is_some());
            assert!(!parsed.verify_shell && !parsed.verify_motion);
        }
        let mut gif_args = base.to_vec();
        gif_args.extend(
            [
                "--store-scene",
                "gifs",
                "--media-origin",
                "http://127.0.0.1:9876",
            ]
            .map(Into::into),
        );
        let gifs = options_from(gif_args.into_iter()).unwrap();
        assert!(gifs.probe == Some(Probe::Session(session::Probe::DualGifs)));
        assert_eq!(gifs.media_origin.as_deref(), Some("http://127.0.0.1:9876"));
        for extra in [
            vec!["--store-scene", "gifs"],
            vec!["--media-origin", "http://127.0.0.1:9876"],
            vec![
                "--store-scene",
                "gifs",
                "--media-origin",
                "http://example.com:9876",
            ],
            vec![
                "--store-scene",
                "grid",
                "--media-origin",
                "http://127.0.0.1:9876",
            ],
            vec!["--store-scene", "unknown"],
            vec!["--store-scene"],
            vec!["--store-scene", "drafts", "--store-scene", "grid"],
            vec!["--store-scene", "drafts", "--verify-drafts", "other"],
            vec!["--store-scene", "drafts", "--probe-grid"],
        ] {
            let mut args = base.to_vec();
            args.extend(extra.into_iter().map(Into::into));
            assert!(options_from(args.into_iter()).is_err());
        }
        for args in [
            vec!["--store-scene", "drafts"],
            vec!["--probe-drafts", "--store-scene", "drafts"],
            vec!["--probe-benchmark", "other", "--store-scene", "grid"],
            vec!["--connect", "--store-scene", "drafts"],
        ] {
            assert!(options_from(args.into_iter().map(Into::into)).is_err());
        }
        assert_eq!(std::fs::read(path).unwrap(), original);
        assert!(!root.path().join("new-evidence").exists());
        assert!(!root.path().join("drafts.json").exists());
        assert!(!root.path().join("probe-settings.json").exists());
    }
}
