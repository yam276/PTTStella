#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod ui;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let flag = args.next();
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
        Some(flag) if flag == "--probe" => match args.next() {
            Some(path) => Some(std::path::PathBuf::from(path)),
            None => {
                eprintln!("--probe requires an output directory");
                std::process::exit(2);
            }
        },
        Some(_) => {
            eprintln!(
                "usage: pttstella [--probe OUTPUT_DIRECTORY DEBUG_PORT | --probe-live OUTPUT_DIRECTORY]"
            );
            std::process::exit(2);
        }
    };
    let debug_port = if probe.is_some() {
        args.next()
            .and_then(|p| p.to_str().and_then(|s| s.parse::<u16>().ok()))
            .filter(|p| *p != 0)
    } else {
        None
    };
    if probe.is_some() && debug_port.is_none() {
        eprintln!("--probe requires an output directory and loopback debug port");
        std::process::exit(2);
    }
    let reload_settings = match args.next() {
        None => false,
        Some(flag) if probe.is_some() && flag == "--reload-settings" => true,
        Some(_) => {
            eprintln!("unexpected arguments");
            std::process::exit(2);
        }
    };
    if args.next().is_some() {
        eprintln!("unexpected arguments");
        std::process::exit(2);
    }
    let mut config = dioxus::desktop::Config::new();
    if let (Some(path), Some(port)) = (&probe, debug_port) {
        config = config
            .with_data_directory(path.join(if reload_settings {
                "webview-reload"
            } else {
                "webview"
            }))
            .with_windows_browser_args(format!(
                "--remote-debugging-address=127.0.0.1 --remote-debugging-port={port}"
            ));
    }
    let props = ui::AppProps {
        probe,
        reload_settings,
    };
    let window = dioxus::desktop::WindowBuilder::new()
        .with_title("PTTStella")
        .with_inner_size(dioxus::desktop::LogicalSize::new(1040.0, 740.0));
    dioxus::desktop::launch::launch_virtual_dom(
        dioxus::prelude::VirtualDom::new_with_props(ui::app, props),
        config
            .with_window(window)
            .with_close_behaviour(dioxus::desktop::WindowCloseBehaviour::WindowHides)
            .with_menu(None),
    );
}
