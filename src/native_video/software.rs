//! Bounded CPU video fallback. The render API and conversion stay off the UI
//! thread. One coalesced wake and one latest frame replace per-frame queues.
use super::{mpv, session::RenderGate};
use serde::Serialize;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use tokio::sync::watch;

pub const MAX_WIDTH: u32 = 1280;
pub const MAX_HEIGHT: u32 = 720;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct View {
    pub width: u32,
    pub height: u32,
    pub active: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Size {
    width: u32,
    height: u32,
}
impl Size {
    fn fit(view: View) -> Option<Self> {
        if view.width == 0 || view.height == 0 {
            return None;
        }
        let factor = (f64::from(MAX_WIDTH) / f64::from(view.width))
            .min(f64::from(MAX_HEIGHT) / f64::from(view.height))
            .min(1.);
        Some(Self {
            width: (f64::from(view.width) * factor).floor().max(1.) as u32,
            height: (f64::from(view.height) * factor).floor().max(1.) as u32,
        })
    }
}

#[repr(C, align(64))]
#[derive(Clone, Copy)]
struct Block([u8; 64]);
const _: () = assert!(std::mem::size_of::<Block>() == 64);

pub(super) struct Buffer {
    size: Size,
    stride: usize,
    blocks: Vec<Block>,
}
impl Buffer {
    fn new(size: Size) -> Result<Self, String> {
        if size.width == 0 || size.height == 0 || size.width > MAX_WIDTH || size.height > MAX_HEIGHT
        {
            return Err("software video dimensions exceed 1280x720".into());
        }
        let stride = (size.width as usize * 4).div_ceil(64) * 64;
        let count = stride * size.height as usize / 64;
        let mut blocks = Vec::new();
        blocks.try_reserve_exact(count).map_err(|e| e.to_string())?;
        blocks.resize(count, Block([0; 64]));
        Ok(Self {
            size,
            stride,
            blocks,
        })
    }
    pub(super) fn target(&mut self) -> ([i32; 2], usize, &mut [u8]) {
        let length = self.blocks.len() * 64;
        // SAFETY: repr(C) Block has exactly 64 initialized bytes, size/alignment
        // 64, and Vec storage is contiguous. The slice borrows that same unique
        // allocation, including every stride's writable padding, for this call.
        let bytes =
            unsafe { std::slice::from_raw_parts_mut(self.blocks.as_mut_ptr().cast(), length) };
        (
            [self.size.width as i32, self.size.height as i32],
            self.stride,
            bytes,
        )
    }
    fn frame(&mut self, serial: u64) -> Result<Frame, String> {
        let width = self.size.width;
        let height = self.size.height;
        let (_, stride, bytes) = self.target();
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(width as usize * height as usize * 4)
            .map_err(|e| e.to_string())?;
        for row in bytes.chunks_exact(stride) {
            for pixel in row[..width as usize * 4].chunks_exact(4) {
                // libmpv's rgb0 alpha is unspecified; never expose it to Slint.
                pixels.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
        }
        Ok(Frame {
            size: [width, height],
            serial,
            pixels,
        })
    }
}
pub struct Frame {
    size: [u32; 2],
    serial: u64,
    pixels: Vec<u8>,
}
impl Frame {
    pub fn size(&self) -> [u32; 2] {
        self.size
    }
    pub fn serial(&self) -> u64 {
        self.serial
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub ready: bool,
    pub stopped: bool,
    pub frame: Option<Arc<Frame>>,
    pub error: Option<String>,
}
#[derive(Default, Serialize)]
pub struct Report {
    pub frames: u64,
    pub max_buffer_bytes: usize,
    pub renderer_retired: bool,
    pub joined: bool,
    pub error: Option<String>,
}
pub struct Software {
    view: Arc<Mutex<View>>,
    closed: Arc<AtomicBool>,
    wake: mpsc::SyncSender<()>,
    updates: watch::Receiver<Snapshot>,
    join: Option<std::thread::JoinHandle<Report>>,
}
impl Software {
    pub fn start(
        core: Arc<mpv::Client>,
        gate: RenderGate,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        let (wake, receive) = mpsc::sync_channel(1);
        let view = Arc::new(Mutex::new(View::default()));
        let closed = Arc::new(AtomicBool::new(false));
        let (output, updates) = watch::channel(Snapshot::default());
        let (owned_view, owned_closed, callback) = (view.clone(), closed.clone(), wake.clone());
        let join = std::thread::Builder::new()
            .name("slint-video-software".into())
            .spawn(move || {
                let worker = Worker {
                    view: owned_view,
                    closed: owned_closed,
                    callback,
                    receive,
                    output,
                    notify,
                };
                let mut report = Report::default();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(core, &gate, &worker, &mut report)
                }));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => report.error = Some(error),
                    Err(_) => report.error = Some("software video worker panicked".into()),
                }
                // run's renderer/core have dropped before retiring the render gate.
                gate.ready(false);
                report.renderer_retired = true;
                worker.output.send_replace(Snapshot {
                    stopped: true,
                    error: report.error.clone(),
                    ..Default::default()
                });
                (worker.notify)();
                report
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            view,
            closed,
            wake,
            updates,
            join: Some(join),
        })
    }
    pub fn updates(&self) -> watch::Receiver<Snapshot> {
        self.updates.clone()
    }
    pub fn set_view(&self, value: View) -> Result<(), String> {
        if self.closed.load(Ordering::Acquire) || self.updates.borrow().stopped {
            return Err("software renderer is closed".into());
        }
        let mut view = self
            .view
            .lock()
            .map_err(|_| "software video view poisoned")?;
        if *view != value {
            *view = value;
            let _ = self.wake.try_send(());
        }
        Ok(())
    }
    pub fn stop(&self) {
        self.closed.store(true, Ordering::Release);
        let _ = self.wake.try_send(());
    }
    pub fn finish(&mut self) -> Result<Report, String> {
        self.stop();
        if !self.updates.borrow().stopped {
            return Err("software renderer has not stopped".into());
        }
        let mut report = self
            .join
            .take()
            .ok_or("software renderer already joined")?
            .join()
            .map_err(|_| "software renderer thread panicked")?;
        report.joined = true;
        Ok(report)
    }
}

impl Drop for Software {
    fn drop(&mut self) {
        self.stop();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

struct Worker {
    view: Arc<Mutex<View>>,
    closed: Arc<AtomicBool>,
    callback: mpsc::SyncSender<()>,
    receive: mpsc::Receiver<()>,
    output: watch::Sender<Snapshot>,
    notify: Arc<dyn Fn() + Send + Sync>,
}
fn run(
    core: Arc<mpv::Client>,
    gate: &RenderGate,
    worker: &Worker,
    report: &mut Report,
) -> Result<(), String> {
    if worker.closed.load(Ordering::Acquire) {
        return Ok(());
    }
    let callback = worker.callback.clone();
    let mut renderer = mpv::Render::software(
        core,
        Box::new(move || {
            let _ = callback.try_send(());
        }),
    )?;
    gate.ready(true);
    worker.output.send_replace(Snapshot {
        ready: true,
        ..Default::default()
    });
    (worker.notify)();
    let mut buffer: Option<Buffer> = None;
    while worker.receive.recv().is_ok() {
        if worker.closed.load(Ordering::Acquire) {
            break;
        }
        renderer.software_update()?;
        let view = *worker
            .view
            .lock()
            .map_err(|_| "software video view poisoned")?;
        let Some(size) = Size::fit(view).filter(|_| view.active) else {
            continue;
        };
        let resized = buffer.as_ref().is_none_or(|b| b.size != size);
        if resized {
            buffer = Some(Buffer::new(size)?);
            if let Some(buffer) = &buffer {
                report.max_buffer_bytes = report.max_buffer_bytes.max(buffer.blocks.len() * 64);
            }
        }
        if let Some(buffer) = &mut buffer
            && renderer.draw_software(buffer, resized)?
            && !worker.closed.load(Ordering::Acquire)
        {
            report.frames = report.frames.saturating_add(1);
            let frame = Arc::new(buffer.frame(report.frames)?);
            worker.output.send_replace(Snapshot {
                ready: true,
                frame: Some(frame),
                ..Default::default()
            });
            (worker.notify)();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aligned_surface_is_bounded_and_does_not_publish_rgb0_padding() {
        assert!(Size::fit(View::default()).is_none());
        for invalid in [
            Size {
                width: 0,
                height: 1,
            },
            Size {
                width: 1,
                height: 0,
            },
            Size {
                width: MAX_WIDTH + 1,
                height: 1,
            },
            Size {
                width: 1,
                height: MAX_HEIGHT + 1,
            },
        ] {
            assert!(Buffer::new(invalid).is_err());
        }
        let size = Size::fit(View {
            width: u32::MAX,
            height: u32::MAX,
            active: true,
        })
        .unwrap();
        let mut buffer = Buffer::new(size).unwrap();
        let (_, stride, bytes) = buffer.target();
        assert_eq!((bytes.as_ptr() as usize) % 64, 0);
        assert_eq!(stride % 64, 0);
        assert!(bytes.len() <= MAX_WIDTH as usize * MAX_HEIGHT as usize * 4);
        let mut one = Buffer::new(Size {
            width: 1,
            height: 1,
        })
        .unwrap();
        one.target().2[..4].copy_from_slice(&[10, 20, 30, 7]);
        assert_eq!(one.frame(1).unwrap().pixels(), &[10, 20, 30, 255]);
    }
    #[tokio::test]
    async fn software_setup_never_fetches_and_cancel_retires_before_core_join() {
        for immediate in [true, false] {
            let file = tempfile::Builder::new().suffix(".mp4").tempfile().unwrap();
            std::fs::write(file.path(), b"owned fixture; never played").unwrap();
            let mut source = crate::media::probe_server::Server::start(file.path(), false).unwrap();
            let mut session = super::super::session::Session::start(
                source.url(),
                mpv::Layout::VideoOnly,
                Arc::new(|| {}),
            )
            .unwrap();
            let core = tokio::time::timeout(std::time::Duration::from_secs(5), session.wait_core())
                .await
                .unwrap()
                .unwrap();
            let mut software =
                Software::start(core, session.render_gate(), Arc::new(|| {})).unwrap();
            if !immediate {
                let mut updates = software.updates();
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    updates.wait_for(|s| s.ready || s.stopped),
                )
                .await
                .unwrap()
                .unwrap();
                assert!(updates.borrow().ready, "{:?}", updates.borrow().error);
                software
                    .set_view(View {
                        width: 2000,
                        height: 1200,
                        active: true,
                    })
                    .unwrap();
                tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            }
            assert_eq!(source.requests.load(Ordering::Acquire), 0);
            software.stop();
            let mut updates = software.updates();
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                updates.wait_for(|s| s.stopped),
            )
            .await
            .unwrap()
            .unwrap();
            let report = software.finish().unwrap();
            assert!(
                report.joined && report.renderer_retired,
                "{:?}",
                report.error
            );
            assert!(report.error.is_none());
            assert_eq!(report.frames, 0);
            assert!(software.set_view(View::default()).is_err());
            session.stop();
            let mut updates = session.updates();
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                updates.wait_for(|s| s.stage == super::super::session::Stage::Stopped),
            )
            .await
            .unwrap()
            .unwrap();
            let report = session.finish().unwrap();
            assert!(report.core_destroyed && report.control_joined == Some(true));
            assert!(report.stream.unwrap().stopped);
            assert!(source.finish().unwrap().joined);
        }
    }
}
