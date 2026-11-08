//! Self-authored local image endpoints; the BBS peer rejects every input byte.
use pttstella::media::probe_server::Server;
pub struct Fixtures {
    _files: tempfile::TempDir,
    png: Server,
    gif: Server,
    once: Server,
    video: Option<Server>,
}
impl Fixtures {
    pub fn start(menu_probe: Option<&str>) -> Result<Self, String> {
        let files = tempfile::Builder::new()
            .prefix("pttstella-native-images-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        let png = files.path().join("owned.png");
        let gif = files.path().join("owned.gif");
        let once = files.path().join("once.gif");
        std::fs::write(
            &once,
            if menu_probe == Some("error") {
                b"invalid-owned-gif".as_slice()
            } else {
                include_bytes!("../../../tests/fixtures/media/once.gif").as_slice()
            },
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            &png,
            include_bytes!("../../../tests/fixtures/media/colours.png"),
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            &gif,
            include_bytes!("../../../tests/fixtures/media/motion.gif"),
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            png: Server::start_cached(&png)?,
            gif: Server::start(&gif, false)?,
            once: Server::start(&once, menu_probe == Some("stall"))?,
            video: std::env::var_os("PTTSTELLA_VIDEO_FIXTURE")
                .map(|path| {
                    Server::start(
                        std::path::Path::new(&path),
                        std::env::var_os("PTTSTELLA_VIDEO_STALL").is_some(),
                    )
                })
                .transpose()?,
            _files: files,
        })
    }
    pub fn page(&self) -> Vec<u8> {
        let video = self
            .video
            .as_ref()
            .map(|s| format!("Video {}\r\n\r\n", s.url()))
            .unwrap_or_default();
        format!("\x1b[2J\x1b[HNative image preview / owned loopback fixtures\r\n\r\nPNG {}\r\n\r\nGIF {}\r\n\r\nOnce {}\r\n\r\n{video}Close preview before returning to BBS. No station keys are expected.",self.png.url(),self.gif.url(),self.once.url()).into_bytes()
    }
    pub fn article_page(&self, page: usize) -> Result<Vec<u8>, String> {
        if !(1..=2).contains(&page) {
            return Err("inline fixture page out of range".into());
        }
        let first = if page == 1 { 1 } else { 23 };
        let mut text = String::from("\x1b[0m\x1b[2J\x1b[H");
        for line in first..first + 23 {
            let content = match line {
                2 => self.png.url(),
                4 => self.gif.url(),
                6 => self.once.url(),
                8 => self.png.url(),
                _ => format!("Native inline synthetic line {line:04}"),
            };
            text.push_str(&content);
            text.push_str("\r\n");
        }
        text.push_str(&format!("\x1b[24;1H\x1b[30;47m\x1b[2K瀏覽 第 {page}/2 頁 ({}%) 目前顯示: 第 {first}~{} 行 (y)回應(X)推文(h)說明\x1b[0m\x1b[24;80H",if page==1{50}else{100},first+22));
        pttstella::encoding::encode(&text).map_err(|e| e.to_string())
    }
    pub fn finish(&mut self) -> Result<(), String> {
        let png = self.png.finish()?;
        let gif = self.gif.finish()?;
        let once = self.once.finish()?;
        let video = self.video.as_mut().map(Server::finish).transpose()?;
        eprintln!(
            "{}",
            serde_json::json!({"native_media_http":{"png":png,"gif":gif,"once":once,"video":video}})
        );
        Ok(())
    }
}
// Read back only immediately after an explicit copy in the owned media probe.
// Normal preview operations never inspect clipboard contents after writing.
pub fn verify_picture(clipboard: &crate::clipboard::Clipboard, mime: &str) -> Result<(), String> {
    let pixels = clipboard.read_image()?;
    if (pixels.width, pixels.height) != (64, 32) || pixels.bytes.len() != 64 * 32 * 4 {
        return Err("native picture clipboard dimensions differ from fixture".into());
    }
    for (i, pixel) in pixels.bytes.chunks_exact(4).enumerate() {
        let color = if mime == "image/gif" || i % 64 < 32 {
            [255, 0, 0, 255]
        } else {
            [0, 0, 255, 255]
        };
        if pixel != color {
            return Err("native picture clipboard differs from independent fixture colors".into());
        }
    }
    eprintln!(
        "{}",
        serde_json::json!({"native_picture_clipboard":{"mime":mime,"width":64,"height":32,"expected_pixels":true}})
    );
    Ok(())
}
pub fn verify_url(clipboard: &crate::clipboard::Clipboard, expected: &str) -> Result<(), String> {
    if clipboard.read_text()? != expected {
        return Err("native media URL clipboard mismatch".into());
    }
    eprintln!(
        "{}",
        serde_json::json!({"native_media_url_clipboard":"exact_fixture_url"})
    );
    Ok(())
}
