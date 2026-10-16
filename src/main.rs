#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod fonts;
#[cfg(target_os = "macos")]
mod macos;
mod ui;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let flag = args.next();
    let window_probe = flag.as_deref() == Some(std::ffi::OsStr::new("--probe-window"));
    let filters_probe = flag.as_deref() == Some(std::ffi::OsStr::new("--probe-filters"));
    let login_probe = flag.as_deref() == Some(std::ffi::OsStr::new("--probe-login"));
    let surface_probe_flag = flag.as_deref() == Some(std::ffi::OsStr::new("--probe-surface"));
    if flag.as_deref() == Some(std::ffi::OsStr::new("--probe-live")) {
        let result = (|| -> Result<(), String> {
            let directory = args
                .next()
                .map(std::path::PathBuf::from)
                .ok_or("missing output directory")?;
            if args.next().is_some() {
                return Err("unexpected arguments".into());
            }
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            let report = runtime.block_on(pttstella::connection::probe_live())?;
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
                "usage: pttstella [--probe OUTPUT_DIRECTORY DEBUG_PORT | --probe-window OUTPUT_DIRECTORY | --probe-login OUTPUT_DIRECTORY | --probe-filters OUTPUT_DIRECTORY | --probe-surface OUTPUT_DIRECTORY CASE | --probe-live OUTPUT_DIRECTORY]"
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
                case @ ("decode" | "painted" | "send" | "renderer_error" | "untrusted" | "confirm"
                | "slow" | "dialogs" | "files" | "files_slow"),
            ) => Some(case.to_owned()),
            _ => {
                eprintln!(
                    "--probe-surface requires decode, painted, send, renderer_error, untrusted, confirm, slow, dialogs, files or files_slow"
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
    let mut config = dioxus::desktop::Config::new();
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
        .with_title("PTTStella")
        .with_inner_size(dioxus::desktop::LogicalSize::new(1040.0, 740.0));
    dioxus::desktop::launch::launch_virtual_dom(
        dioxus::prelude::VirtualDom::new_with_props(ui::app, props),
        config
            .with_window(window)
            .with_close_behaviour(dioxus::desktop::WindowCloseBehaviour::WindowHides),
    );
}
