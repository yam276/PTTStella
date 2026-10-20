fn main() -> std::io::Result<()> {
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
