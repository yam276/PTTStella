//! Explicit original-image output; no network work or renderer-supplied pixels.
use super::{
    Clipboard, Completion, Error, ExportDestination, Permit, blocking_result, timestamp_stem,
    write_clipboard_image,
};
use crate::ui::surface_state::OutputLease;
use pttstella::media::{loader::Image, picture};
use std::{path::PathBuf, sync::Arc};

pub(in crate::ui) struct Request {
    pub image: Arc<Image>,
    pub lease: OutputLease,
    pub source: tokio::sync::watch::Receiver<pttstella::connection::View>,
    pub destination: ExportDestination,
    pub probe: Option<PathBuf>,
}
async fn pick(extension: &'static str, probe: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(directory) = probe {
        return Some(directory.join(format!("picture.{extension}")));
    }
    let window = dioxus::desktop::window();
    window.window.set_focus();
    rfd::AsyncFileDialog::new()
        .set_parent(window.window.as_ref())
        .set_title(pttstella::msg!("picture-save-title"))
        .add_filter("Image", &[extension])
        .set_file_name(format!("{}-image.{extension}", timestamp_stem()))
        .save_file()
        .await
        .map(|file| file.path().to_path_buf())
}
pub(in crate::ui) async fn output(
    permit: Permit,
    request: Request,
    clipboard: Clipboard,
) -> Completion {
    let Request {
        image,
        lease,
        source,
        destination,
        probe,
    } = request;
    let result = async {
        permit.check()?;
        lease.check(&source.borrow()).map_err(|_| Error::Stale)?;
        let extension = picture::extension(&image).map_err(Error::Failed)?;
        let path = if destination == ExportDestination::File {
            pick(extension, probe.clone()).await
        } else {
            None
        };
        permit.check()?;
        if destination == ExportDestination::File && path.is_none() {
            return Ok(false);
        }
        blocking_result(permit, move || {
            lease.check(&source.borrow()).map_err(|_| Error::Stale)?;
            if let Some(path) = path {
                picture::save(&path, &image, || lease.check(&source.borrow()))
                    .map_err(Error::Failed)?;
            } else {
                let pixels = picture::pixels(image).map_err(Error::Failed)?;
                lease.check(&source.borrow()).map_err(|_| Error::Stale)?;
                write_clipboard_image(
                    clipboard,
                    pixels,
                    probe.map(|directory| directory.join("picture-clipboard.png")),
                )
                .map_err(Error::Failed)?;
            }
            Ok(true)
        })
        .await
    }
    .await;
    Completion::Picture {
        destination,
        result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source(mime: &'static str, bytes: &[u8]) -> Image {
        Image {
            bytes: bytes.to_vec(),
            mime,
            width: 64,
            height: 32,
            animated_format: false,
            animation_pixels: 64 * 32 * if mime == "image/gif" { 2 } else { 1 },
        }
    }
    fn pixels(source: &Image) -> Result<arboard::ImageData<'static>, String> {
        picture::pixels(Arc::new(Image {
            bytes: source.bytes.clone(),
            mime: source.mime,
            width: source.width,
            height: source.height,
            animated_format: source.animated_format,
            animation_pixels: source.animation_pixels,
        }))
    }
    #[test]
    fn original_png_and_gif_first_frame_have_independent_expected_pixels() {
        let png = source(
            "image/png",
            include_bytes!("../../tests/fixtures/media/colours.png"),
        );
        let pixels = picture::pixels(Arc::new(png)).unwrap();
        assert_eq!((pixels.width, pixels.height), (64, 32));
        for y in 0..32 {
            for x in 0..64 {
                assert_eq!(
                    &pixels.bytes[(y * 64 + x) * 4..(y * 64 + x + 1) * 4],
                    if x < 32 {
                        &[255, 0, 0, 255]
                    } else {
                        &[0, 0, 255, 255]
                    }
                );
            }
        }
        let gif = source(
            "image/gif",
            include_bytes!("../../tests/fixtures/media/motion.gif"),
        );
        let first = picture::pixels(Arc::new(gif)).unwrap();
        assert!(
            first
                .bytes
                .chunks_exact(4)
                .all(|pixel| pixel == [255, 0, 0, 255])
        );
    }
    #[test]
    fn corrupt_unknown_oversized_and_mismatched_sources_do_not_produce_clipboard_pixels() {
        let mut source = source(
            "image/png",
            include_bytes!("../../tests/fixtures/media/colours.png"),
        );
        source.width = 63;
        assert!(pixels(&source).is_err());
        source.width = 8193;
        assert!(pixels(&source).is_err());
        source.width = 64;
        source.mime = "image/svg+xml";
        assert!(pixels(&source).is_err());
        source.mime = "image/png";
        source.bytes.truncate(33);
        assert!(pixels(&source).is_err());
        source.bytes = vec![0; pttstella::media::loader::max_image_bytes(source.mime) + 1];
        assert!(pixels(&source).is_err());
    }
}
