//! Incremental animation clock shared by modal and article media owners.
//! Owns one decoder; callers decide visibility and aggregate canvas admission.
use pttstella::media::{
    loader::{Error, Image},
    native_image::{Decoder, Frame},
};
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::{sync::Arc, time::Duration};
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
        Ok(Self {
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
        })
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
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.due
    }
    fn first(&mut self) -> Result<(), Error> {
        // Release the previous compositing canvas before opening another one.
        self.decoder = None;
        let mut decoder = Decoder::new(self.source.clone())?;
        let frame = decoder.next_frame()?.ok_or(Error::Format)?;
        self.delay = frame.delay;
        self.pixels = pixels(frame);
        self.frame = self.frame.saturating_add(1);
        self.decoder = Some(decoder);
        Ok(())
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
        let Some(decoder) = &mut self.decoder else {
            return Ok(false);
        };
        let next = match decoder.next_frame()? {
            Some(frame) => Some(frame),
            None => {
                self.cycles = self.cycles.saturating_add(1);
                let repeat = match decoder.looping {
                    image::metadata::LoopCount::Infinite => true,
                    image::metadata::LoopCount::Finite(n) => self.cycles < n.get(),
                };
                self.decoder = None;
                if repeat {
                    let mut decoder = Decoder::new(self.source.clone())?;
                    let frame = decoder.next_frame()?;
                    self.decoder = Some(decoder);
                    frame
                } else {
                    self.finished = true;
                    None
                }
            }
        };
        if let Some(frame) = next {
            self.delay = frame.delay;
            self.pixels = pixels(frame);
            self.frame = self.frame.saturating_add(1);
            self.due = Some(Instant::now() + self.delay);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
