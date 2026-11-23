//! Article batch and at most two visible animation clocks on the media thread.
use crate::media_playback::Playback;
use pttstella::{
    media::{
        inline::{Entry, InlineImages, Projection, Target, Update},
        loader::Image,
        native_image::Decoder,
    },
    surface_state::Identity,
};
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::{collections::BTreeMap, sync::Arc};
use tokio::time::Instant;
const ANIMATION_PIXELS: usize = 134_217_728;
#[derive(Clone, Default)]
pub struct Resource {
    pub original: Option<Arc<Image>>,
    pub first: Option<SharedPixelBuffer<Rgba8Pixel>>,
    pub pixels: Option<SharedPixelBuffer<Rgba8Pixel>>,
    pub playing: bool,
    pub frame: u64,
    pub error: Option<String>,
}
#[derive(Clone, Default)]
pub struct State {
    pub epoch: u64,
    pub identity: Option<Identity>,
    pub entries: Vec<Entry>,
    pub resources: BTreeMap<usize, Resource>,
    pub limited: bool,
    pub error: Option<String>,
    pub automatic: bool,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Controls {
    pub epoch: u64,
    pub permit: bool,
    /// Already ranked by distance to viewport center; only these two may play.
    pub visible: [Option<usize>; 2],
}
pub struct Engine {
    pub batch: InlineImages,
    pub state: State,
    players: BTreeMap<usize, Playback>,
}
impl Engine {
    pub fn new(epoch: u64) -> Self {
        Self {
            batch: InlineImages::default(),
            state: State {
                epoch,
                ..Default::default()
            },
            players: BTreeMap::new(),
        }
    }
    pub fn reset(&mut self, epoch: u64) {
        self.batch.close();
        self.players.clear();
        self.state = State {
            epoch,
            ..Default::default()
        };
    }
    pub fn project(
        &mut self,
        projection: Projection<'static>,
        target: Option<Target>,
        automatic: bool,
    ) {
        self.automatic(automatic);
        self.state.identity = Some(projection.identity());
        let result = match target {
            Some(target) => self.batch.open_projection(&projection, target),
            None => self.batch.follow_projection(&projection),
        };
        match result {
            Ok(Some(open)) => {
                self.state.entries = open.entries;
                self.state.limited = open.limited;
                self.state.error = None;
            }
            Ok(None) => {}
            Err(e) => self.state.error = Some(e),
        }
        self.state.automatic = self.batch.automatic_batch();
    }
    pub fn automatic(&mut self, enabled: bool) -> bool {
        if !self.batch.set_auto_expand(enabled) {
            return false;
        }
        self.players.clear();
        self.state.resources.clear();
        self.state.entries.clear();
        self.state.limited = false;
        self.state.error = None;
        self.state.automatic = false;
        true
    }
    pub fn loaded(&mut self, update: Update) {
        if self.batch.batch() != Some(update.batch) {
            return;
        }
        let resource = match update.result {
            Ok(loaded) => {
                let original = loaded.image.clone();
                match Decoder::new(original.clone()).and_then(|mut decoder| {
                    decoder
                        .next_frame()?
                        .ok_or(pttstella::media::loader::Error::Format)
                }) {
                    Ok(frame) => {
                        self.batch.decoded(update.batch, update.index, &loaded);
                        let pixels = SharedPixelBuffer::clone_from_slice(
                            frame.pixels.as_raw(),
                            frame.pixels.width(),
                            frame.pixels.height(),
                        );
                        Resource {
                            original: Some(original),
                            first: Some(pixels.clone()),
                            pixels: Some(pixels),
                            frame: 1,
                            ..Default::default()
                        }
                    }
                    Err(e) => Resource {
                        error: Some(e.to_string()),
                        ..Default::default()
                    },
                }
            }
            Err(e) => Resource {
                error: Some(e),
                ..Default::default()
            },
        };
        self.state.resources.insert(update.index, resource);
    }
    /// Frames and compositing canvases are released when a source leaves the
    /// chosen set. The original bytes and bounded first frame remain shared.
    pub fn reconcile(&mut self, controls: Controls, modal: bool) -> bool {
        let mut desired = Vec::with_capacity(2);
        let mut cost = 0usize;
        if controls.epoch == self.state.epoch && controls.permit && !modal {
            for key in controls.visible.into_iter().flatten() {
                if desired.contains(&key) {
                    continue;
                }
                let Some(image) = self
                    .state
                    .resources
                    .get(&key)
                    .filter(|r| r.error.is_none())
                    .and_then(|r| r.original.as_ref())
                else {
                    continue;
                };
                if image.mime != "image/gif" || !image.animated_format {
                    continue;
                }
                let next = if image.animation_pixels == 0 {
                    ANIMATION_PIXELS
                } else {
                    image.animation_pixels
                };
                if next <= ANIMATION_PIXELS - cost {
                    cost += next;
                    desired.push(key);
                }
            }
        }
        let mut changed = false;
        self.players.retain(|key, _| {
            if desired.contains(key) {
                return true;
            }
            if let Some(r) = self.state.resources.get_mut(key) {
                r.pixels = r.first.clone();
                r.playing = false;
                r.frame = 1;
                changed = true;
            }
            false
        });
        for key in desired {
            if !self.players.contains_key(&key) {
                let Some(r) = self.state.resources.get_mut(&key) else {
                    continue;
                };
                let Some(image) = r.original.clone() else {
                    continue;
                };
                match Playback::new(image).and_then(|mut player| {
                    player.permit(true)?;
                    Ok(player)
                }) {
                    Ok(player) => {
                        r.pixels = Some(player.pixels());
                        r.frame = player.frame();
                        r.playing = player.playing();
                        self.players.insert(key, player);
                    }
                    Err(e) => r.error = Some(e.to_string()),
                }
                changed = true;
            } else if let Some(player) = self.players.get_mut(&key) {
                match player.permit(true) {
                    Ok(true) => {
                        if let Some(r) = self.state.resources.get_mut(&key) {
                            r.pixels = Some(player.pixels());
                            r.frame = player.frame();
                            r.playing = player.playing();
                        }
                        changed = true;
                    }
                    Ok(false) => {}
                    Err(e) => {
                        player.stop();
                        if let Some(r) = self.state.resources.get_mut(&key) {
                            r.error = Some(e.to_string());
                            r.playing = false;
                        }
                        changed = true;
                    }
                }
            }
        }
        changed
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.players.values().filter_map(Playback::deadline).min()
    }
    pub fn advance(&mut self) {
        let now = Instant::now();
        for (key, player) in &mut self.players {
            if player.deadline().is_none_or(|at| at > now) {
                continue;
            }
            let result = player.advance();
            if let Some(r) = self.state.resources.get_mut(key) {
                r.pixels = Some(player.pixels());
                r.frame = player.frame();
                r.playing = player.playing();
                if let Err(e) = result {
                    player.stop();
                    r.playing = false;
                    r.error = Some(e.to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn aggregate_animation_cost_and_modal_gate_release_decoder_set() {
        let mut engine = Engine::new(1);
        // A valid high-frame-count fixture crosses only the aggregate budget.
        // Its decoder needs one small canvas, never 306 retained frame buffers.
        let mut bytes = Vec::new();
        {
            let canvas = image::RgbaImage::from_pixel(512, 512, image::Rgba([255, 0, 0, 255]));
            let mut encoder = image::codecs::gif::GifEncoder::new_with_speed(&mut bytes, 30);
            encoder
                .set_repeat(image::codecs::gif::Repeat::Infinite)
                .unwrap();
            encoder
                .encode_frames((0..306).map(|_| image::Frame::new(canvas.clone())))
                .unwrap();
        }
        let file = tempfile::Builder::new().suffix(".gif").tempfile().unwrap();
        std::fs::write(file.path(), bytes).unwrap();
        let mut server = pttstella::media::probe_server::Server::start(file.path(), false).unwrap();
        let original = pttstella::media::loader::Loader::default()
            .load(&server.url())
            .await
            .unwrap()
            .image;
        assert_eq!(
            (original.width, original.height, original.animation_pixels),
            (512, 512, 80_216_064)
        );
        for key in 0..2 {
            let first = Playback::new(original.clone()).unwrap().pixels();
            engine.state.resources.insert(
                key,
                Resource {
                    original: Some(original.clone()),
                    first: Some(first.clone()),
                    pixels: Some(first),
                    frame: 1,
                    ..Default::default()
                },
            );
        }
        let controls = Controls {
            epoch: 1,
            permit: true,
            visible: [Some(0), Some(1)],
        };
        assert!(engine.reconcile(controls, false));
        assert_eq!(engine.players.len(), 1);
        assert!(engine.state.resources[&0].playing && !engine.state.resources[&1].playing);
        assert!(engine.reconcile(controls, true));
        assert!(engine.players.is_empty() && engine.deadline().is_none());
        assert!(engine.state.resources.values().all(|r| !r.playing
            && r.frame == 1
            && r.pixels.as_ref().unwrap().as_bytes()[..4] == [255, 0, 0, 255]));
        engine.reset(2);
        assert!(engine.state.resources.is_empty() && engine.players.is_empty());
        assert!(server.finish().unwrap().joined);
    }
}
