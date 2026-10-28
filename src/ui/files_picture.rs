//! Explicit original-image output; no network work or renderer-supplied pixels.
use super::{
    Clipboard, Completion, Error, ExportDestination, Permit, blocking_result, timestamp_stem,
    write_clipboard_image,
};
use crate::ui::surface_state::OutputLease;
use pttstella::{media::loader::Image, storage};
use std::{path::PathBuf, sync::Arc};

pub(in crate::ui) struct Request {
    pub image: Arc<Image>,
    pub lease: OutputLease,
    pub source: tokio::sync::watch::Receiver<pttstella::connection::View>,
    pub destination: ExportDestination,
    pub probe: Option<PathBuf>,
}
fn format(image: &Image) -> Result<(image::ImageFormat, &'static str), String> {
    if image.bytes.len() > pttstella::media::loader::MAX_BYTES
        || image.width == 0
        || image.height == 0
        || image.width > 8192
        || image.height > 8192
        || image.width.saturating_mul(image.height) > 16 * 1024 * 1024
    {
        return Err(pttstella::msg!("image-dimensions"));
    }
    Ok(match image.mime {
        "image/png" => (image::ImageFormat::Png, "png"),
        "image/jpeg" => (image::ImageFormat::Jpeg, "jpg"),
        "image/gif" => (image::ImageFormat::Gif, "gif"),
        "image/webp" => (image::ImageFormat::WebP, "webp"),
        _ => return Err(pttstella::msg!("image-invalid")),
    })
}
fn pixels(image: &Image) -> Result<arboard::ImageData<'static>, String> {
    let (format, _) = format(image)?;
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(&image.bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|_| pttstella::msg!("image-decode-failed"))?;
    if (decoded.width() as usize, decoded.height() as usize) != (image.width, image.height) {
        return Err(pttstella::msg!("image-size-mismatch"));
    }
    Ok(arboard::ImageData {
        width: image.width,
        height: image.height,
        bytes: decoded.to_rgba8().into_raw().into(),
    })
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
        let (_, extension) = format(&image).map_err(Error::Failed)?;
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
                if !path.extension().is_some_and(|value| {
                    value.eq_ignore_ascii_case(extension)
                        || extension == "jpg" && value.eq_ignore_ascii_case("jpeg")
                }) {
                    return Err(Error::Failed(pttstella::msg!("picture-extension")));
                }
                storage::replace(&path, &image.bytes).map_err(Error::Failed)?;
            } else {
                let pixels = pixels(&image).map_err(Error::Failed)?;
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
            animation_pixels: 0,
        }
    }
    #[test]
    fn original_png_and_gif_first_frame_have_independent_expected_pixels() {
        let png = source(
            "image/png",
            include_bytes!("../../tests/fixtures/media/colours.png"),
        );
        let pixels = pixels(&png).unwrap();
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
        let first = super::pixels(&gif).unwrap();
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
        source.bytes = vec![0; pttstella::media::loader::MAX_BYTES + 1];
        assert!(pixels(&source).is_err());
    }
}
