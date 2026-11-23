//! Animation clock shared by modal and article media owners.
//! Owns one decoder or a verified bounded cycle; callers decide visibility.
use pttstella::media::{
    loader::{Error, Image},
    native_image::{Decoder, Frame},
};
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::Arc,
    time::Duration,
};
use tokio::time::Instant;

pub struct Playback {
    source: Arc<Image>,
    decoder: Option<Decoder>,
    pixels: SharedPixelBuffer<Rgba8Pixel>,
    due: Option<Instant>,
    enabled: bool,
    finished: bool,
    cycles: u32,
    delay: Duration,
    frame: u64,
    playing: bool,
    looping: image::metadata::LoopCount,
    cache: Option<CycleCache>,
    cache_overflowed: bool,
}

// At most two article players, or one exclusive modal player, exist on the
// media worker. This bounds their additional resident frame pixels to 32 MiB.
const CYCLE_CACHE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone)]
struct CachedFrame {
    pixels: SharedPixelBuffer<Rgba8Pixel>,
    delay: Duration,
}

struct CycleCache {
    frames: Vec<CachedFrame>,
    // Hashes only narrow the search. Full pixel equality resolves collisions.
    unique: HashMap<u64, Vec<usize>>,
    bytes: usize,
    maximum: usize,
    position: usize,
    complete: bool,
}

impl CycleCache {
    fn new(source: &Image, looping: image::metadata::LoopCount) -> Option<Self> {
        let area = source.width.checked_mul(source.height)?;
        let bytes = area.checked_mul(4)?;
        let maximum = source.animation_pixels.checked_div(area)?;
        if source.mime != "image/gif"
            || !source.animated_format
            || bytes == 0
            || bytes > CYCLE_CACHE_BYTES
            || maximum == 0
            || maximum > 512
            || !source.animation_pixels.is_multiple_of(area)
            || matches!(looping, image::metadata::LoopCount::Finite(n) if n.get() == 1)
        {
            return None;
        }
        Some(Self {
            frames: Vec::new(),
            unique: HashMap::new(),
            bytes: 0,
            maximum,
            position: 0,
            complete: false,
        })
    }

    fn push(&mut self, pixels: &SharedPixelBuffer<Rgba8Pixel>, delay: Duration) -> bool {
        if self.complete || self.frames.len() >= self.maximum {
            return false;
        }
        let mut hash = DefaultHasher::new();
        (pixels.width(), pixels.height()).hash(&mut hash);
        pixels.as_bytes().hash(&mut hash);
        let hash = hash.finish();
        let prior = self.unique.get(&hash).and_then(|indices| {
            indices.iter().find_map(|&index| {
                let frame = &self.frames[index];
                (frame.pixels.width() == pixels.width()
                    && frame.pixels.height() == pixels.height()
                    && frame.pixels.as_bytes() == pixels.as_bytes())
                .then(|| frame.pixels.clone())
            })
        });
        let pixels = if let Some(prior) = prior {
            prior
        } else {
            let bytes = pixels.as_bytes().len();
            if bytes > CYCLE_CACHE_BYTES.saturating_sub(self.bytes) {
                return false;
            }
            self.bytes += bytes;
            self.unique.entry(hash).or_default().push(self.frames.len());
            pixels.clone()
        };
        self.frames.push(CachedFrame { pixels, delay });
        self.position = self.frames.len() - 1;
        true
    }
}
fn pixels(frame: Frame) -> SharedPixelBuffer<Rgba8Pixel> {
    SharedPixelBuffer::clone_from_slice(
        frame.pixels.as_raw(),
        frame.pixels.width(),
        frame.pixels.height(),
    )
}
impl Playback {
    pub fn new(source: Arc<Image>) -> Result<Self, Error> {
        let mut decoder = Decoder::new(source.clone())?;
        let frame = decoder.next_frame()?.ok_or(Error::Format)?;
        let delay = frame.delay;
        let looping = decoder.looping;
        let cache = CycleCache::new(&source, looping);
        let mut player = Self {
            enabled: source.mime == "image/gif",
            source,
            decoder: Some(decoder),
            pixels: pixels(frame),
            due: None,
            finished: false,
            cycles: 0,
            delay,
            frame: 1,
            playing: false,
            looping,
            cache,
            cache_overflowed: false,
        };
        player.cache_current();
        Ok(player)
    }
    pub fn pixels(&self) -> SharedPixelBuffer<Rgba8Pixel> {
        self.pixels.clone()
    }
    pub fn frame(&self) -> u64 {
        self.frame
    }
    pub fn playing(&self) -> bool {
        self.playing
    }
    pub fn stop(&mut self) {
        self.enabled = false;
        self.playing = false;
        self.due = None;
        self.decoder = None;
        self.cache = None;
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.due
    }
    fn first(&mut self) -> Result<(), Error> {
        // Release the previous compositing canvas before opening another one.
        self.decoder = None;
        self.cache = None;
        let mut decoder = Decoder::new(self.source.clone())?;
        let frame = decoder.next_frame()?.ok_or(Error::Format)?;
        self.delay = frame.delay;
        self.pixels = pixels(frame);
        self.frame = self.frame.saturating_add(1);
        self.looping = decoder.looping;
        self.cache = (!self.cache_overflowed)
            .then(|| CycleCache::new(&self.source, self.looping))
            .flatten();
        self.cache_current();
        self.decoder = Some(decoder);
        Ok(())
    }

    fn cache_current(&mut self) {
        if let Some(cache) = &mut self.cache {
            if cache.push(&self.pixels, self.delay) {
                // Share a deduplicated composited image, retaining this frame's delay.
                self.pixels = cache.frames[cache.position].pixels.clone();
            } else {
                // Continue the same incremental decoder. Never retain a partial
                // cycle or repeatedly refill an image that exceeded the budget.
                self.cache = None;
                self.cache_overflowed = true;
            }
        }
    }

    fn repeat(&mut self) -> bool {
        self.cycles = self.cycles.saturating_add(1);
        match self.looping {
            image::metadata::LoopCount::Infinite => true,
            image::metadata::LoopCount::Finite(n) => self.cycles < n.get(),
        }
    }

    fn show_cached(&mut self, position: usize) -> bool {
        let Some(cache) = &mut self.cache else {
            return false;
        };
        cache.position = position;
        let frame = &cache.frames[position];
        self.pixels = frame.pixels.clone();
        self.delay = frame.delay;
        self.frame = self.frame.saturating_add(1);
        self.due = Some(Instant::now() + self.delay);
        true
    }
    /// Pauses at the first frame; natural finite completion holds the last.
    /// Refocus alone never restarts a naturally completed animation.
    pub fn permit(&mut self, permit: bool) -> Result<bool, Error> {
        let playing = self.enabled && !self.finished && permit && self.source.animated_format;
        if playing == self.playing {
            return Ok(false);
        }
        self.playing = playing;
        if playing {
            self.due = Some(Instant::now() + self.delay);
        } else {
            self.due = None;
            if !self.finished {
                self.cycles = 0;
                self.first()?;
            }
        }
        Ok(true)
    }
    pub fn motion(&mut self, enabled: bool) -> Result<bool, Error> {
        self.enabled = enabled;
        if enabled && self.finished {
            self.finished = false;
            self.cycles = 0;
            self.first()?;
            return Ok(true);
        }
        Ok(false)
    }
    /// Advance at most one frame. Scheduling starts from delivery, so suspended
    /// windows never accumulate a catch-up queue.
    pub fn advance(&mut self) -> Result<bool, Error> {
        if self.due.is_none() {
            return Ok(false);
        }
        self.due = None;
        if let Some(cache) = &self.cache
            && cache.complete
        {
            let next = cache.position + 1;
            if next < cache.frames.len() {
                return Ok(self.show_cached(next));
            }
            if self.repeat() {
                return Ok(self.show_cached(0));
            }
            self.finished = true;
            self.cache = None;
            return Ok(false);
        }
        let Some(decoder) = &mut self.decoder else {
            return Ok(false);
        };
        let next = match decoder.next_frame()? {
            Some(frame) => Some(frame),
            None => {
                self.decoder = None;
                if self.repeat() {
                    if let Some(cache) = &mut self.cache
                        && cache.frames.len() == cache.maximum
                    {
                        cache.complete = true;
                        return Ok(self.show_cached(0));
                    }
                    self.cache = None;
                    let mut decoder = Decoder::new(self.source.clone())?;
                    let frame = decoder.next_frame()?;
                    self.decoder = Some(decoder);
                    frame
                } else {
                    self.finished = true;
                    self.cache = None;
                    None
                }
            }
        };
        if let Some(frame) = next {
            self.delay = frame.delay;
            self.pixels = pixels(frame);
            self.cache_current();
            self.frame = self.frame.saturating_add(1);
            self.due = Some(Instant::now() + self.delay);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::metadata::LoopCount;

    fn source(bytes: &[u8]) -> Arc<Image> {
        Arc::new(Image {
            bytes: bytes.to_vec(),
            mime: "image/gif",
            width: 64,
            height: 32,
            animated_format: true,
            animation_pixels: 64 * 32 * 2,
        })
    }

    fn solid(width: u32, height: u32, value: u8) -> SharedPixelBuffer<Rgba8Pixel> {
        let mut pixels = SharedPixelBuffer::new(width, height);
        pixels
            .make_mut_slice()
            .fill(Rgba8Pixel::new(value, 0, 0, 255));
        pixels
    }

    #[test]
    fn a_complete_cycle_reuses_composited_buffers_and_preserves_every_delay() {
        let mut player = Playback::new(source(include_bytes!(
            "../../../tests/fixtures/media/motion.gif"
        )))
        .unwrap();
        let first = player.pixels();
        assert_eq!(&first.as_bytes()[..4], &[255, 0, 0, 255]);
        player.permit(true).unwrap();
        for step in 0..8 {
            assert!(player.advance().unwrap());
            let expected = if step % 2 == 0 {
                [0, 0, 255, 255]
            } else {
                [255, 0, 0, 255]
            };
            assert_eq!(&player.pixels().as_bytes()[..4], &expected);
            assert_eq!(player.delay, Duration::from_millis(300));
            assert_eq!(player.frame(), step + 2);
            if step % 2 == 1 {
                assert_eq!(
                    player.pixels().as_bytes().as_ptr(),
                    first.as_bytes().as_ptr()
                );
                assert!(player.decoder.is_none());
            }
        }
        let cache = player.cache.as_ref().unwrap();
        assert!(cache.complete);
        assert_eq!(cache.frames.len(), 2);
        assert_eq!(cache.bytes, 64 * 32 * 4 * 2);
        player.permit(false).unwrap();
        assert_eq!(&player.pixels().as_bytes()[..4], &[255, 0, 0, 255]);
        assert!(player.deadline().is_none());
        assert!(!player.cache.as_ref().unwrap().complete);
        assert_eq!(player.cache.as_ref().unwrap().bytes, 64 * 32 * 4);
        player.stop();
        assert!(player.cache.is_none() && player.decoder.is_none());
    }

    #[test]
    fn finite_cached_and_incremental_paths_stop_at_the_same_last_frame_and_explicitly_replay() {
        let image = source(include_bytes!("../../../tests/fixtures/media/twice.gif"));
        let mut cached = Playback::new(image.clone()).unwrap();
        let mut incremental = Playback::new(image).unwrap();
        incremental.cache = None;
        cached.permit(true).unwrap();
        incremental.permit(true).unwrap();
        for expected in [true, true, true, false, false] {
            assert_eq!(cached.advance().unwrap(), expected);
            assert_eq!(incremental.advance().unwrap(), expected);
            assert_eq!(cached.pixels().as_bytes(), incremental.pixels().as_bytes());
            assert_eq!(cached.delay, Duration::from_millis(30));
            assert_eq!(cached.frame(), incremental.frame());
        }
        assert_eq!(&cached.pixels().as_bytes()[..4], &[0, 0, 255, 255]);
        assert!(cached.finished && cached.cache.is_none() && cached.decoder.is_none());
        cached.permit(true).unwrap();
        assert!(!cached.playing());
        assert!(cached.motion(true).unwrap());
        assert_eq!(&cached.pixels().as_bytes()[..4], &[255, 0, 0, 255]);
        assert_eq!(cached.cycles, 0);
        assert!(!cached.cache.as_ref().unwrap().complete);
        assert!(
            Playback::new(source(include_bytes!(
                "../../../tests/fixtures/media/once.gif"
            )))
            .unwrap()
            .cache
            .is_none()
        );
    }

    #[test]
    fn unique_pixel_budget_is_bounded_while_duplicate_frames_keep_their_own_delay() {
        let mut image = Arc::try_unwrap(source(&[])).unwrap();
        image.width = 1024;
        image.height = 1024;
        image.animation_pixels = 1024 * 1024 * 8;
        let mut cache = CycleCache::new(&image, LoopCount::Infinite).unwrap();
        let first = solid(1024, 1024, 1);
        assert!(cache.push(&first, Duration::from_millis(100)));
        let duplicate = solid(1024, 1024, 1);
        assert_ne!(first.as_bytes().as_ptr(), duplicate.as_bytes().as_ptr());
        assert!(cache.push(&duplicate, Duration::from_millis(250)));
        assert_eq!(cache.bytes, 4 * 1024 * 1024);
        assert_eq!(cache.frames[1].delay, Duration::from_millis(250));
        assert_eq!(
            cache.frames[0].pixels.as_bytes().as_ptr(),
            cache.frames[1].pixels.as_bytes().as_ptr()
        );
        for color in 2..=4 {
            assert!(cache.push(&solid(1024, 1024, color), Duration::from_millis(100)));
        }
        assert_eq!(cache.bytes, CYCLE_CACHE_BYTES);
        assert!(!cache.push(&solid(1024, 1024, 5), Duration::from_millis(100)));
        assert_eq!(cache.frames.len(), 5);
        image.width = usize::MAX;
        assert!(CycleCache::new(&image, LoopCount::Infinite).is_none());
    }

    #[test]
    fn hash_collisions_never_substitute_different_composited_pixels() {
        let mut image = Arc::try_unwrap(source(&[])).unwrap();
        image.width = 2;
        image.height = 1;
        image.animation_pixels = 4;
        let mut cache = CycleCache::new(&image, LoopCount::Infinite).unwrap();
        let first = solid(2, 1, 1);
        let second = solid(2, 1, 2);
        assert!(cache.push(&first, Duration::from_millis(100)));
        let mut collision = DefaultHasher::new();
        (second.width(), second.height()).hash(&mut collision);
        second.as_bytes().hash(&mut collision);
        cache.unique.insert(collision.finish(), vec![0]);
        assert!(cache.push(&second, Duration::from_millis(100)));
        assert_eq!(
            cache.frames[1].pixels.as_bytes(),
            &[2, 0, 0, 255, 2, 0, 0, 255]
        );
        assert_eq!(cache.bytes, 16);
    }

    #[test]
    fn exhausting_the_cache_continues_the_same_decoder_without_losing_frames() {
        let mut player = Playback::new(source(include_bytes!(
            "../../../tests/fixtures/media/motion.gif"
        )))
        .unwrap();
        player.cache.as_mut().unwrap().bytes = CYCLE_CACHE_BYTES;
        player.permit(true).unwrap();
        for step in 0..6 {
            assert!(player.advance().unwrap());
            let expected = if step % 2 == 0 {
                [0, 0, 255, 255]
            } else {
                [255, 0, 0, 255]
            };
            assert_eq!(&player.pixels().as_bytes()[..4], &expected);
            assert!(player.cache.is_none() && player.decoder.is_some());
        }
        assert!(player.cache_overflowed);
        player.permit(false).unwrap();
        assert!(player.cache.is_none());
    }

    #[test]
    fn cached_cycles_preserve_transparency_previous_disposal_and_nonuniform_delays() {
        // CC0, self-authored 2x1 GIF. Each LZW pixel is preceded by clear=4;
        // the second frame restores the previous canvas before frame three.
        let bytes = [
            b"GIF89a\x02\x00\x01\x00\x81\x00\x00".as_slice(),
            &[255, 0, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255],
            b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00",
            b"\x21\xf9\x04\x04\x05\x00\x00\x00\x2c\x00\x00\x00\x00\x02\x00\x01\x00\x00\x02\x02\x04\x51\x00",
            b"\x21\xf9\x04\x0d\x07\x00\x02\x00\x2c\x00\x00\x00\x00\x02\x00\x01\x00\x00\x02\x02\x14\x53\x00",
            b"\x21\xf9\x04\x04\x09\x00\x00\x00\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x4c\x01\x00\x3b",
        ].concat();
        let image = Arc::new(Image {
            bytes,
            mime: "image/gif",
            width: 2,
            height: 1,
            animated_format: true,
            animation_pixels: 6,
        });
        let mut cached = Playback::new(image.clone()).unwrap();
        let mut incremental = Playback::new(image).unwrap();
        incremental.cache = None;
        cached.permit(true).unwrap();
        incremental.permit(true).unwrap();
        let red = [255, 0, 0, 255];
        let blue = [0, 0, 255, 255];
        let expected = [[red, red], [red, blue], [blue, red]];
        for frame in 0..12 {
            assert_eq!(cached.pixels().as_bytes(), expected[frame % 3].concat());
            assert_eq!(cached.pixels().as_bytes(), incremental.pixels().as_bytes());
            assert_eq!(cached.delay, Duration::from_millis([50, 70, 90][frame % 3]));
            assert!(cached.advance().unwrap());
            assert!(incremental.advance().unwrap());
        }
        assert!(cached.cache.as_ref().unwrap().complete && cached.decoder.is_none());
    }
}
