#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod fonts;
#[cfg(target_os = "macos")]
mod macos;
mod notices;
mod ui;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let flag = args.next();
    if flag.as_deref() == Some(std::ffi::OsStr::new("--export-licenses")) {
        let result = match (args.next(), args.next()) {
            (Some(path), None) => notices::export(std::path::Path::new(&path)),
            _ => Err(std::io::Error::other(
                "--export-licenses requires one new output file",
            )),
        };
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let window_probe = flag.as_deref() == Some(std::ffi::OsStr::new("--probe-window"));
    let filters_probe = flag.as_deref() == Some(std::ffi::OsStr::new("--probe-filters"));
    let login_probe = flag.as_deref() == Some(std::ffi::OsStr::new("--probe-login"));
    let surface_probe_flag = flag.as_deref() == Some(std::ffi::OsStr::new("--probe-surface"));
    let site_probe = flag.as_deref() == Some(std::ffi::OsStr::new("--probe-site"));
    if flag.as_deref() == Some(std::ffi::OsStr::new("--probe-live")) || site_probe {
        let result = (|| -> Result<(), String> {
            let directory = args
                .next()
                .map(std::path::PathBuf::from)
                .ok_or("missing output directory")?;
            let target = if site_probe {
                let address = args
                    .next()
                    .and_then(|value| value.into_string().ok())
                    .ok_or("missing station address")?;
                let encoding = match args
                    .next()
                    .and_then(|value| value.into_string().ok())
                    .as_deref()
                {
                    Some("uao") => pttstella::encoding::TextEncoding::Uao,
                    Some("big5") => pttstella::encoding::TextEncoding::Big5,
                    Some("gbk") => pttstella::encoding::TextEncoding::Gbk,
                    _ => return Err("encoding must be uao, big5 or gbk".into()),
                };
                Some(pttstella::sites::Target::new(address, encoding)?)
            } else {
                None
            };
            if args.next().is_some() {
                return Err("unexpected arguments".into());
            }
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            let report = runtime.block_on(async {
                match target {
                    Some(target) => pttstella::connection::probe_site(target).await,
                    None => pttstella::connection::probe_live().await,
                }
            })?;
            std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
            std::fs::write(directory.join("prelogin.json"), report.to_string())
                .map_err(|e| e.to_string())?;
            println!("{report}");
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let probe = match flag {
        None => None,
        Some(flag)
            if flag == "--probe"
                || window_probe
                || login_probe
                || filters_probe
                || surface_probe_flag =>
        {
            match args.next() {
                Some(path) => Some(std::path::PathBuf::from(path)),
                None => {
                    eprintln!("probe requires an output directory");
                    std::process::exit(2);
                }
            }
        }
        Some(_) => {
            eprintln!(
                "usage: pttstella [--export-licenses OUTPUT_FILE | --probe OUTPUT_DIRECTORY DEBUG_PORT | --probe-window OUTPUT_DIRECTORY | --probe-login OUTPUT_DIRECTORY | --probe-filters OUTPUT_DIRECTORY | --probe-surface OUTPUT_DIRECTORY CASE | --probe-live OUTPUT_DIRECTORY | --probe-site OUTPUT_DIRECTORY ADDRESS uao|big5|gbk]"
            );
            std::process::exit(2);
        }
    };
    let surface_probe = if surface_probe_flag {
        match args
            .next()
            .and_then(|value| value.into_string().ok())
            .as_deref()
        {
            Some(
                case @ ("fullscreen" | "decode" | "painted" | "send" | "renderer_error"
                | "untrusted" | "confirm" | "slow" | "dialogs" | "tools" | "sites"
                | "sites_restart" | "files" | "files_slow" | "fonts" | "sgr66"
                | "sgr66_legacy" | "login_2fa" | "osc8" | "filter_colors" | "inline"
                | "inline_pages" | "full_load" | "long_load" | "terminal_pages"
                | "short_overlap" | "short_reading" | "reading_rows" | "control_input"
                | "editor_clipboard" | "wide_last_row" | "diagnostics" | "inline_replies"
                | "short_links" | "picture_menu" | "context_menu" | "account_lists"
                | "merged_replies" | "account_actions" | "account_articles"
                | "pointer_rows" | "list_boundary" | "article_switch" | "boards"
                | "boards_close" | "recent" | "articles" | "articles_close" | "bookmarks"
                | "bookmarks_restart" | "bookmarks_empty" | "bookmarks_corrupt" | "mouse"
                | "mouse_boundary" | "mouse_drag" | "mouse_round_trip" | "mouse_motion"
                | "mouse_wheel"),
            ) => Some(case.to_owned()),
            _ => {
                eprintln!(
                    "--probe-surface requires fullscreen, decode, painted, send, renderer_error, untrusted, confirm, slow, dialogs, tools, sites, sites_restart, files, files_slow, fonts, sgr66, sgr66_legacy, login_2fa, osc8, inline, boards, boards_close, recent, articles, articles_close, bookmarks, bookmarks_restart, bookmarks_empty, bookmarks_corrupt, mouse, mouse_boundary, mouse_drag, mouse_round_trip, mouse_motion or mouse_wheel"
                );
                std::process::exit(2);
            }
        }
    } else {
        None
    };
    let debug_port = if probe.is_some()
        && !window_probe
        && !login_probe
        && !filters_probe
        && !surface_probe_flag
    {
        args.next()
            .and_then(|p| p.to_str().and_then(|s| s.parse::<u16>().ok()))
            .filter(|p| *p != 0)
    } else {
        None
    };
    if probe.is_some()
        && !window_probe
        && !login_probe
        && !filters_probe
        && !surface_probe_flag
        && debug_port.is_none()
    {
        eprintln!("--probe requires an output directory and loopback debug port");
        std::process::exit(2);
    }
    let mut reload_settings = false;
    let mut media_origin = None;
    while let Some(flag) = args.next() {
        if probe.is_some() && flag == "--reload-settings" && !reload_settings {
            reload_settings = true;
        } else if probe.is_some() && flag == "--media-origin" && media_origin.is_none() {
            let value = args.next().and_then(|value| value.into_string().ok());
            let parsed = value
                .as_deref()
                .and_then(|value| pttstella::media::validate(value).ok());
            if let Some(url) = parsed.filter(|url| {
                url.scheme() == "http"
                    && url.host_str() == Some("127.0.0.1")
                    && url.port().is_some()
                    && url.path() == "/"
                    && url.query().is_none()
                    && url.fragment().is_none()
            }) {
                media_origin = Some(url.as_str().trim_end_matches('/').to_owned());
            } else {
                eprintln!("media probe requires an explicit loopback HTTP origin");
                std::process::exit(2);
            }
        } else {
            eprintln!("unexpected arguments");
            std::process::exit(2);
        }
    }
    // This synchronous preflight runs before the native event loop exists.
    // The retained UI file owner still loads preferences and reports failures.
    let language = probe
        .as_ref()
        .map_or_else(pttstella::settings::store::Store::system, |path| {
            Ok(pttstella::settings::store::Store::at(
                path.join("probe-settings.json"),
            ))
        })
        .and_then(|store| store.load())
        .map(|settings| settings.language)
        .unwrap_or_default();
    if let Err(error) = pttstella::i18n::initialize(language) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    let mut config = dioxus::desktop::Config::new().with_disable_context_menu(true);
    #[cfg(target_os = "macos")]
    {
        let menu = match macos::menu() {
            Ok(menu) => menu,
            Err(error) => {
                eprintln!("cannot create macOS menu: {error}");
                std::process::exit(1);
            }
        };
        config = config.with_menu(menu);
    }
    #[cfg(not(target_os = "macos"))]
    {
        config = config.with_menu(None);
    }
    if let Some(path) = &probe {
        config = config.with_data_directory(path.join(if reload_settings {
            "webview-reload"
        } else {
            "webview"
        }));
        if let Some(port) = debug_port {
            config = config.with_windows_browser_args(format!(
                "--remote-debugging-address=127.0.0.1 --remote-debugging-port={port}"
            ));
        }
    }
    let test_window = ui::diagnostics::diagnostic_build() || probe.is_some();
    #[cfg(target_os = "macos")]
    if test_window {
        use dioxus::desktop::tao::{
            event_loop::EventLoopBuilder, platform::macos::EventLoopExtMacOS,
        };
        // Dioxus shows the WebView after initialization. Keep that show from
        // activating this test app over the user's currently active app.
        let mut event_loop = EventLoopBuilder::with_user_event().build();
        event_loop.set_activate_ignoring_other_apps(false);
        config = config.with_event_loop(event_loop);
    }
    let props = ui::AppProps {
        probe,
        window_probe,
        login_probe,
        filters_probe,
        surface_probe,
        reload_settings,
        media_origin,
    };
    let window = dioxus::desktop::WindowBuilder::new()
        .with_title(if ui::diagnostics::diagnostic_build() {
            "PTTStella（診斷版）"
        } else {
            "PTTStella"
        })
        .with_inner_size(dioxus::desktop::LogicalSize::new(1040.0, 740.0));
    let window = if test_window {
        window.with_always_on_top(true).with_focused(false)
    } else {
        window
    };
    #[cfg(target_os = "windows")]
    let window = {
        use dioxus::desktop::tao::{
            platform::windows::{IconExtWindows, WindowBuilderExtWindows},
            window::Icon,
        };
        let icon = match Icon::from_resource(1, None) {
            Ok(icon) => icon,
            Err(error) => {
                eprintln!("cannot load embedded Windows icon: {error}");
                std::process::exit(1);
            }
        };
        window
            .with_window_icon(Some(icon.clone()))
            .with_taskbar_icon(Some(icon))
    };
    dioxus::desktop::launch::launch_virtual_dom(
        dioxus::prelude::VirtualDom::new_with_props(ui::app, props),
        config
            .with_window(window)
            .with_close_behaviour(dioxus::desktop::WindowCloseBehaviour::WindowHides),
    );
}
