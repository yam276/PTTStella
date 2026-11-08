//! Self-authored local image endpoints; the BBS peer rejects every input byte.
use pttstella::media::probe_server::Server;
pub struct Fixtures {
    _files: tempfile::TempDir,
    png: Server,
    gif: Server,
    once: Server,
}
impl Fixtures {
    pub fn start() -> Result<Self, String> {
        let files = tempfile::Builder::new()
            .prefix("pttstella-native-images-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        let png = files.path().join("owned.png");
        let gif = files.path().join("owned.gif");
        let once = files.path().join("once.gif");
        std::fs::write(
            &once,
            include_bytes!("../../../tests/fixtures/media/once.gif"),
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
            once: Server::start(&once, false)?,
            _files: files,
        })
    }
    pub fn page(&self) -> Vec<u8> {
        format!("\x1b[2J\x1b[HNative image preview / owned loopback fixtures\r\n\r\nPNG {}\r\n\r\nGIF {}\r\n\r\nOnce {}\r\n\r\nClose preview before returning to BBS. No station keys are expected.",self.png.url(),self.gif.url(),self.once.url()).into_bytes()
    }
    pub fn finish(&mut self) -> Result<(), String> {
        let png = self.png.finish()?;
        let gif = self.gif.finish()?;
        let once = self.once.finish()?;
        eprintln!(
            "{}",
            serde_json::json!({"native_media_http":{"png":png,"gif":gif,"once":once}})
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
