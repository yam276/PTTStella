fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
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
