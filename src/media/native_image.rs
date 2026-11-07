//! Incremental native decoding of an already admitted media payload.
//! The caller owns the worker, scheduling and cancellation; no hidden threads.
use super::loader::{Error, Image};
use image::{AnimationDecoder, ImageDecoder, RgbaImage, metadata::LoopCount};
use std::{io::Cursor, sync::Arc, time::Duration};

#[derive(Clone)]
struct Bytes(Arc<Image>);
impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        &self.0.bytes
    }
}
type Input = Cursor<Bytes>;
enum Source {
    Still(Option<RgbaImage>),
    Animation(image::Frames<'static>),
}
pub struct Frame {
    pub pixels: RgbaImage,
    pub delay: Duration,
    pub number: usize,
}
pub struct Decoder {
    source: Source,
    dimensions: (u32, u32),
    maximum: usize,
    number: usize,
    pub looping: LoopCount,
}
fn input(source: &Arc<Image>) -> Input {
    Cursor::new(Bytes(source.clone()))
}
fn limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    // Includes decoder/compositing intermediates, not a resident-frame cache.
    limits.max_alloc = Some(192 * 1024 * 1024);
    limits
}
fn admit(decoder: &mut impl ImageDecoder, source: &Image) -> Result<(), Error> {
    let (w, h) = decoder.dimensions();
    if (w as usize, h as usize) != (source.width, source.height)
        || w == 0
        || h == 0
        || w > 8192
        || h > 8192
        || u64::from(w) * u64::from(h) > 16_777_216
    {
        return Err(Error::Dimensions);
    }
    decoder.set_limits(limits()).map_err(|_| Error::Dimensions)
}
fn still(mut decoder: impl ImageDecoder) -> Result<RgbaImage, Error> {
    let orientation = decoder.orientation().map_err(|_| Error::Format)?;
    let mut image = image::DynamicImage::from_decoder(decoder).map_err(|_| Error::Format)?;
    image.apply_orientation(orientation);
    Ok(image.into_rgba8())
}
impl Decoder {
    pub fn new(source: Arc<Image>) -> Result<Self, Error> {
        let area = source
            .width
            .checked_mul(source.height)
            .ok_or(Error::Dimensions)?;
        let maximum = source.animation_pixels.checked_div(area).unwrap_or(0);
        if area == 0
            || area > 16_777_216
            || source.width > 8192
            || source.height > 8192
            || maximum == 0
            || maximum > 512
            || source.bytes.len() > super::loader::max_image_bytes(source.mime)
        {
            return Err(Error::Dimensions);
        }
        let once = || LoopCount::Finite(std::num::NonZeroU32::MIN);
        let (decoded, looping) = match source.mime {
            "image/gif" => {
                let mut decoder = image::codecs::gif::GifDecoder::new(input(&source))
                    .map_err(|_| Error::Format)?;
                admit(&mut decoder, &source)?;
                let looping = decoder.loop_count();
                (Source::Animation(decoder.into_frames()), looping)
            }
            "image/png" => {
                let mut decoder = image::codecs::png::PngDecoder::new(input(&source))
                    .map_err(|_| Error::Format)?;
                admit(&mut decoder, &source)?;
                if decoder.is_apng().map_err(|_| Error::Format)? {
                    let decoder = decoder.apng().map_err(|_| Error::Format)?;
                    let looping = decoder.loop_count();
                    (Source::Animation(decoder.into_frames()), looping)
                } else {
                    let image = still(decoder)?;
                    (Source::Still(Some(image)), once())
                }
            }
            "image/webp" => {
                let mut decoder = image::codecs::webp::WebPDecoder::new(input(&source))
                    .map_err(|_| Error::Format)?;
                admit(&mut decoder, &source)?;
                if decoder.has_animation() {
                    let looping = decoder.loop_count();
                    (Source::Animation(decoder.into_frames()), looping)
                } else {
                    let image = still(decoder)?;
                    (Source::Still(Some(image)), once())
                }
            }
            "image/jpeg" => {
                let mut decoder = image::codecs::jpeg::JpegDecoder::new(input(&source))
                    .map_err(|_| Error::Format)?;
                admit(&mut decoder, &source)?;
                let image = still(decoder)?;
                (Source::Still(Some(image)), once())
            }
            _ => return Err(Error::Format),
        };
        let dimensions = match &decoded {
            Source::Still(Some(image)) => image.dimensions(),
            _ => (source.width as u32, source.height as u32),
        };
        Ok(Self {
            source: decoded,
            dimensions,
            maximum,
            number: 0,
            looping,
        })
    }
    pub fn next_frame(&mut self) -> Result<Option<Frame>, Error> {
        let (pixels, delay) = match &mut self.source {
            Source::Still(image) => match image.take() {
                Some(image) => (image, Duration::ZERO),
                None => return Ok(None),
            },
            Source::Animation(frames) => {
                let Some(frame) = frames.next() else {
                    return Ok(None);
                };
                let frame = frame.map_err(|_| Error::Format)?;
                let (numerator, denominator) = frame.delay().numer_denom_ms();
                if denominator == 0 {
                    return Err(Error::Format);
                }
                let micros = u64::from(numerator) * 1000 / u64::from(denominator);
                (
                    frame.into_buffer(),
                    Duration::from_micros(micros).max(Duration::from_millis(20)),
                )
            }
        };
        if pixels.dimensions() != self.dimensions || self.number >= self.maximum {
            return Err(Error::Dimensions);
        }
        self.number += 1;
        Ok(Some(Frame {
            pixels,
            delay,
            number: self.number,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload(bytes: &[u8], mime: &'static str, animated: bool) -> Arc<Image> {
        Arc::new(Image {
            bytes: bytes.to_vec(),
            mime,
            width: 64,
            height: 32,
            animated_format: animated,
            animation_pixels: 64 * 32 * if animated { 2 } else { 1 },
        })
    }
    #[test]
    fn png_pixels_are_independent_and_encoded_bytes_are_shared() {
        let source = payload(
            include_bytes!("../../tests/fixtures/media/colours.png"),
            "image/png",
            false,
        );
        let mut decoder = Decoder::new(source.clone()).unwrap();
        let frame = decoder.next_frame().unwrap().unwrap();
        assert_eq!(frame.pixels.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(frame.pixels.get_pixel(63, 31).0, [0, 0, 255, 255]);
        assert!(decoder.next_frame().unwrap().is_none());
        assert_eq!(
            source.bytes.as_ptr(),
            input(&source).get_ref().as_ref().as_ptr()
        );
    }
    #[test]
    fn gif_yields_only_requested_frames_and_retains_delays() {
        let source = payload(
            include_bytes!("../../tests/fixtures/media/motion.gif"),
            "image/gif",
            true,
        );
        let mut decoder = Decoder::new(source).unwrap();
        assert!(matches!(decoder.looping, LoopCount::Infinite));
        for color in [[255, 0, 0, 255], [0, 0, 255, 255]] {
            let frame = decoder.next_frame().unwrap().unwrap();
            assert_eq!(frame.pixels.get_pixel(32, 16).0, color);
            assert_eq!(frame.delay, Duration::from_millis(300));
        }
        assert!(decoder.next_frame().unwrap().is_none());
    }
    #[test]
    fn admitted_dimensions_are_rechecked_before_decoding() {
        let mut source = payload(
            include_bytes!("../../tests/fixtures/media/colours.png"),
            "image/png",
            false,
        );
        Arc::get_mut(&mut source).unwrap().width = 8192;
        assert!(matches!(Decoder::new(source), Err(Error::Dimensions)));
    }
    #[test]
    fn jpeg_exif_orientation_rotates_pixels_and_display_dimensions() {
        let rgb = (0..32)
            .flat_map(|_| (0..64).flat_map(|x| if x < 32 { [255, 0, 0] } else { [0, 0, 255] }))
            .collect::<Vec<u8>>();
        let mut encoded = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 100)
            .encode(&rgb, 64, 32, image::ExtendedColorType::Rgb8)
            .unwrap();
        // Independently authored EXIF/TIFF IFD: orientation SHORT=6, clockwise 90°.
        let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        let mut bytes = vec![0xff, 0xd8, 0xff, 0xe1];
        bytes.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        bytes.extend_from_slice(exif);
        bytes.extend_from_slice(&encoded[2..]);
        let mut decoder = Decoder::new(payload(&bytes, "image/jpeg", false)).unwrap();
        let frame = decoder.next_frame().unwrap().unwrap();
        assert_eq!(frame.pixels.dimensions(), (32, 64));
        for (point, color) in [((15, 8), [255u8, 0, 0, 255]), ((15, 56), [0, 0, 255, 255])] {
            assert!(
                frame
                    .pixels
                    .get_pixel(point.0, point.1)
                    .0
                    .iter()
                    .zip(color)
                    .all(|(a, b)| a.abs_diff(b) <= 30)
            );
        }
    }
    #[test]
    fn lossless_webp_retains_independent_rgba_pixels() {
        let rgba = (0..32)
            .flat_map(|_| {
                (0..64).flat_map(|x| {
                    if x < 32 {
                        [255, 0, 0, 255]
                    } else {
                        [0, 0, 255, 128]
                    }
                })
            })
            .collect::<Vec<u8>>();
        let mut encoded = Vec::new();
        image::codecs::webp::WebPEncoder::new_lossless(&mut encoded)
            .encode(&rgba, 64, 32, image::ExtendedColorType::Rgba8)
            .unwrap();
        let mut decoder = Decoder::new(payload(&encoded, "image/webp", false)).unwrap();
        let frame = decoder.next_frame().unwrap().unwrap();
        assert_eq!(frame.pixels.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(frame.pixels.get_pixel(63, 31).0, [0, 0, 255, 128]);
    }
}
