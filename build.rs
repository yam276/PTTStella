fn main() -> std::io::Result<()> {
    #[cfg(feature = "native-video")]
    {
        println!("cargo:rerun-if-env-changed=MPV_LIB_DIR");
        let directory = if let Some(path) = std::env::var_os("MPV_LIB_DIR") {
            std::path::PathBuf::from(path)
        } else {
            let host = std::env::var("HOST").map_err(|error| {
                std::io::Error::other(format!("native-video requires Cargo HOST: {error}"))
            })?;
            let target = std::env::var("TARGET").map_err(|error| {
                std::io::Error::other(format!("native-video requires Cargo TARGET: {error}"))
            })?;
            // pkg-config here describes the build machine. Never let a cross
            // build silently select that machine's library search directory.
            if host != target {
                return Err(std::io::Error::other(format!(
                    "Cross-compiling native-video ({host} -> {target}) requires MPV_LIB_DIR \
                     for the target libmpv; host pkg-config is not used"
                )));
            }
            let result = std::process::Command::new("pkg-config")
                .args(["--variable=libdir", "mpv"])
                .output()?;
            if !result.status.success() {
                return Err(std::io::Error::other(
                    "native-video requires libmpv and pkg-config, or MPV_LIB_DIR",
                ));
            }
            let value = String::from_utf8(result.stdout).map_err(std::io::Error::other)?;
            std::path::PathBuf::from(value.trim())
        };
        if !directory.is_dir() {
            return Err(std::io::Error::other(
                "MPV library directory is unavailable",
            ));
        }
        println!("cargo:rustc-link-search=native={}", directory.display());
    }

    #[cfg(feature = "slint-research")]
    slint_build::compile("src/bin/slint_research/terminal.slint").map_err(std::io::Error::other)?;
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=PTTSTELLA_NOTICES_FILE");
    let notices = match std::env::var_os("PTTSTELLA_NOTICES_FILE") {
        Some(path) => {
            let path = std::path::PathBuf::from(path);
            println!("cargo:rerun-if-changed={}", path.display());
            let text = std::fs::read_to_string(path)?;
            if text.is_empty() || text.len() > 32 * 1024 * 1024 {
                return Err(std::io::Error::other("invalid third-party notices size"));
            }
            text
        }
        None => String::new(),
    };
    let output = std::env::var_os("OUT_DIR")
        .ok_or_else(|| std::io::Error::other("Cargo did not provide OUT_DIR"))?;
    std::fs::write(
        std::path::Path::new(&output).join("third-party-notices.txt"),
        notices,
    )?;
    #[cfg(feature = "desktop")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=assets/icons/pttstella.ico");
        winresource::WindowsResource::new()
            .set_icon("assets/icons/pttstella.ico")
            .set("ProductName", "PTTStella")
            .set("FileDescription", "PTTStella BBS desktop client")
            .set("OriginalFilename", "pttstella.exe")
            .set("CompanyName", "yam276")
            .compile()?;
    }
    Ok(())
}
