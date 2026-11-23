//! Original-image output shared by native desktop frontends. No HTTP or UI.
use super::{loader::Image, native_image::Decoder};
use std::{path::Path, sync::Arc};

pub fn extension(image: &Image) -> Result<&'static str, String> {
    if image.bytes.len() > super::loader::max_image_bytes(image.mime)
        || image.width == 0
        || image.height == 0
        || image.width > 8192
        || image.height > 8192
        || image.width.saturating_mul(image.height) > 16 * 1024 * 1024
    {
        return Err(crate::msg!("image-dimensions"));
    }
    match image.mime {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/gif" => Ok("gif"),
        "image/webp" => Ok("webp"),
        _ => Err(crate::msg!("image-invalid")),
    }
}

/// Decode the original first frame, independent of preview zoom or playback.
pub fn pixels(image: Arc<Image>) -> Result<arboard::ImageData<'static>, String> {
    extension(&image)?;
    let frame = Decoder::new(image)
        .and_then(|mut decoder| decoder.next_frame())
        .map_err(|_| crate::msg!("image-decode-failed"))?
        .ok_or_else(|| crate::msg!("image-decode-failed"))?;
    Ok(arboard::ImageData {
        width: frame.pixels.width() as usize,
        height: frame.pixels.height() as usize,
        bytes: frame.pixels.into_raw().into(),
    })
}

/// Save the admitted original bytes, retaining GIF animation and metadata.
/// The caller revalidates its output lease immediately before the OS write.
pub fn save(
    path: &Path,
    image: &Image,
    verify: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let extension = extension(image)?;
    if !path.extension().is_some_and(|value| {
        value.eq_ignore_ascii_case(extension)
            || extension == "jpg" && value.eq_ignore_ascii_case("jpeg")
    }) {
        return Err(crate::msg!("picture-extension"));
    }
    verify()?;
    crate::storage::replace(path, &image.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn gif() -> Arc<Image> {
        Arc::new(Image {
            bytes: include_bytes!("../../tests/fixtures/media/motion.gif").to_vec(),
            mime: "image/gif",
            width: 64,
            height: 32,
            animated_format: true,
            animation_pixels: 64 * 32 * 2,
        })
    }
    #[test]
    fn original_gif_output_preserves_bytes_but_clipboard_uses_red_first_frame() {
        let image = gif();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("original.GIF");
        save(&path, &image, || Ok(())).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), image.bytes);
        let pixels = pixels(image).unwrap();
        assert_eq!((pixels.width, pixels.height), (64, 32));
        assert!(pixels.bytes.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
    }
    #[test]
    fn revoked_write_and_wrong_extension_preserve_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("existing.gif");
        std::fs::write(&path, b"previous").unwrap();
        assert!(save(&path, &gif(), || Err("revoked".into())).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"previous");
        let wrong = directory.path().join("wrong.png");
        assert!(save(&wrong, &gif(), || Ok(())).is_err());
        assert!(!wrong.exists());
    }
}
