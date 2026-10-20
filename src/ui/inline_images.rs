//! Article-scoped expansion; page projections use only acknowledged filtered links.
use super::{presentation::Paint, surface_state::Identity};
use futures_util::future::BoxFuture;
use pttstella::{
    media::{
        self,
        loader::{Loaded, Loader},
    },
    navigation::Context,
};
use serde::Serialize;
use std::collections::VecDeque;

const MAX_IMAGES: usize = 8;
const MAX_ARTICLE_IMAGES: usize = 32;
const MAX_PIXELS: usize = 16 * 1024 * 1024;
const MAX_BYTES: usize = 24 * 1024 * 1024;
#[derive(Clone, Serialize)]
pub(super) struct Entry {
    pub index: usize,
    pub row: usize,
    pub key: usize,
}
#[derive(Serialize)]
pub(super) struct Open {
    pub batch: u64,
    pub presentation: u64,
    pub generation: u64,
    pub revision: u64,
    pub entries: Vec<Entry>,
    pub limited: bool,
}
pub(super) struct Update {
    pub batch: u64,
    pub index: usize,
    pub result: Result<Loaded, String>,
}
type Job = BoxFuture<'static, Result<Loaded, media::loader::Error>>;
#[derive(Default)]
pub(super) struct InlineImages {
    sequence: u64,
    identity: Option<Identity>,
    turning: bool,
    targets: Vec<String>,
    queue: VecDeque<(usize, String)>,
    job: Option<(usize, Job)>,
    loader: Loader,
    bytes: usize,
    pixels: usize,
}
impl InlineImages {
    pub(super) fn close(&mut self) {
        self.identity = None;
        self.turning = false;
        self.targets.clear();
        self.queue.clear();
        self.job = None;
        self.bytes = 0;
        self.pixels = 0;
    }
    pub(super) fn active(&self) -> bool {
        self.identity.is_some()
    }
    pub(super) fn navigation(
        &mut self,
        context: &Context,
        intent: pttstella::navigation::Intent,
    ) -> bool {
        pttstella::navigation::plan(context, intent).is_ok_and(|bytes| self.input(context, &bytes))
    }
    /// Only actual reading movements preserve the current article scope.
    pub(super) fn input(&mut self, context: &Context, bytes: &[u8]) -> bool {
        if !self.active() || bytes.is_empty() {
            return false;
        }
        if matches!(context, Context::ArticleReading { .. })
            && matches!(
                bytes,
                b"\x1b[5~"
                    | b"\x1b[6~"
                    | b"\x1b[H"
                    | b"\x1b[F"
                    | b" "
                    | b"j"
                    | b"k"
                    | b"\x06"
                    | b"\x02"
            )
        {
            self.turning = true;
            false
        } else {
            self.close();
            true
        }
    }
    pub(super) fn observe(&mut self, view: &pttstella::connection::View) -> bool {
        let Some(identity) = self.identity else {
            return false;
        };
        let invalid = identity.generation != view.generation
            || view.phase != pttstella::connection::Phase::Connected
            || !view.site.ptt_features()
            || view.mouse.route != pttstella::mouse::input::Route::Local;
        // A page redraw may be fragmented. Keep resources hidden until a complete
        // reading frame is acknowledged, without treating a partial prompt as a new article.
        let left = !view.input_pending
            && !view.output_pending
            && view.frame.control_complete
            && match view.context() {
                Context::ArticleReading { .. } => false,
                Context::Unknown { .. } => !self.turning,
                _ => true,
            };
        if invalid || left {
            self.close();
            true
        } else {
            false
        }
    }
    pub(super) fn follow(&mut self, paint: &Paint) -> Result<Option<Open>, String> {
        if !self.active() {
            return Ok(None);
        }
        self.open(paint)
    }
    pub(super) fn open(&mut self, paint: &Paint) -> Result<Option<Open>, String> {
        let identity = Identity {
            presentation: paint.presentation,
            generation: paint.generation,
            revision: paint.frame.revision,
        };
        if self.identity == Some(identity) {
            return Ok(None);
        }
        if !matches!(paint.navigation, Context::ArticleReading { .. })
            || !paint.connected
            || paint.input_pending
            || paint.output_pending
        {
            return Ok(None);
        }
        if self
            .identity
            .is_some_and(|old| old.generation != identity.generation)
        {
            self.close();
        }
        let mut targets: Vec<(usize, usize, String)> = Vec::new();
        for (index, link) in paint.links.iter().enumerate() {
            if let Ok(Some(target)) = media::classify(&link.url)
                && target.kind == media::Kind::Image
            {
                if let Some(existing) = targets.iter_mut().find(|(_, _, url)| url == &target.url) {
                    existing.1 = existing.1.max(link.row);
                } else {
                    targets.push((index, link.row, target.url));
                }
            }
        }
        if targets.is_empty() && !self.active() {
            return Ok(None);
        }
        if !self.active() {
            self.sequence = self
                .sequence
                .checked_add(1)
                .ok_or_else(|| pttstella::msg!("media-sequence-exhausted"))?;
        }
        let sequence = self.sequence;
        self.turning = false;
        self.identity = Some(identity);
        let mut limited = targets.len() > MAX_IMAGES;
        let mut entries = Vec::new();
        for (index, row, url) in targets.into_iter().take(MAX_IMAGES) {
            let key = match self.targets.iter().position(|known| known == &url) {
                Some(key) => key,
                None if self.targets.len() < MAX_ARTICLE_IMAGES => {
                    let key = self.targets.len();
                    self.targets.push(url.clone());
                    self.queue.push_back((key, url));
                    key
                }
                None => {
                    limited = true;
                    continue;
                }
            };
            entries.push(Entry { index, row, key });
        }
        if self.job.is_none() {
            self.start_next();
        }
        Ok(Some(Open {
            batch: sequence,
            presentation: identity.presentation,
            generation: identity.generation,
            revision: identity.revision,
            entries,
            limited,
        }))
    }
    fn start_next(&mut self) {
        self.job = self
            .queue
            .pop_front()
            .map(|(index, url)| (index, self.loader.load(&url)));
    }
    fn admit(&mut self, pixels: usize, bytes: usize) -> Result<(), String> {
        if pixels > MAX_PIXELS - self.pixels || bytes > MAX_BYTES - self.bytes {
            return Err(pttstella::msg!("inline-image-budget"));
        }
        self.pixels += pixels;
        self.bytes += bytes;
        Ok(())
    }
    pub(super) async fn update(&mut self) -> Update {
        let (index, result) = match self.job.as_mut() {
            Some((index, job)) => (*index, job.await),
            None => std::future::pending().await,
        };
        let result = result.map_err(|e| e.to_string()).and_then(|loaded| {
            let pixels = loaded.image.width.saturating_mul(loaded.image.height);
            let bytes = loaded.image.bytes.len();
            self.admit(pixels, bytes)?;
            Ok(loaded)
        });
        self.start_next();
        // Inline owns only still canvases, not a second encoded-image cache.
        Update {
            batch: self.sequence,
            index,
            result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page() -> Paint {
        let frame = pttstella::screen::Screen::new(80, 24).unwrap().frame();
        let mut p = super::super::presentation::sample_packet(&frame, &Default::default());
        p.connected = true;
        p.navigation = Context::ArticleReading {
            first_line: 1,
            last_line: 23,
            percent: 50,
        };
        p.links = (0..10)
            .map(|n| pttstella::links::Link {
                row: n,
                start: 0,
                end: 30,
                url: format!("http://127.0.0.1:9/{n}.png"),
            })
            .collect();
        p
    }
    #[test]
    fn expansion_is_bounded_deduplicated_and_idempotent() {
        let mut p = page();
        p.links.insert(1, p.links[0].clone());
        let mut owner = InlineImages::default();
        let open = owner.open(&p).unwrap().unwrap();
        assert!(open.limited);
        assert_eq!(open.entries.len(), 8);
        assert_eq!(
            open.entries.iter().map(|e| e.index).collect::<Vec<_>>(),
            vec![0, 2, 3, 4, 5, 6, 7, 8]
        );
        assert!(owner.open(&p).unwrap().is_none());
        assert_eq!(owner.queue.len(), 7);
        owner.close();
        assert!(owner.job.is_none());
        assert!(owner.queue.is_empty());
        assert_eq!(owner.open(&p).unwrap().unwrap().batch, 2);
    }
    #[test]
    fn aggregate_budget_rejects_before_delivery_and_resets_with_page() {
        let mut owner = InlineImages::default();
        owner.admit(MAX_PIXELS - 1, MAX_BYTES - 1).unwrap();
        assert!(owner.admit(2, 1).is_err());
        assert!(owner.admit(1, 2).is_err());
        assert_eq!((owner.pixels, owner.bytes), (MAX_PIXELS - 1, MAX_BYTES - 1));
        owner.admit(1, 1).unwrap();
        assert!(owner.admit(1, 1).is_err());
        owner.close();
        owner.admit(MAX_PIXELS, MAX_BYTES).unwrap();
    }
    #[tokio::test]
    async fn sequential_loader_and_page_revocation_drop_unstarted_requests() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            let count = socket.read(&mut request).await.unwrap();
            assert!(String::from_utf8_lossy(&request[..count]).starts_with("GET /0.png HTTP/1.1"));
            let bytes = include_bytes!("../../tests/fixtures/media/colours.png");
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",bytes.len()).as_bytes()).await.unwrap();
            socket.write_all(bytes).await.unwrap();
            drop(socket);
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                    .await
                    .is_err()
            );
        });
        let mut paint = page();
        for (index, link) in paint.links.iter_mut().enumerate() {
            link.url = format!("http://{address}/{index}.png");
        }
        let mut owner = InlineImages::default();
        let open = owner.open(&paint).unwrap().unwrap();
        let update = owner.update().await;
        assert_eq!((update.batch, update.index), (open.batch, 0));
        let image = update.result.unwrap().image;
        assert_eq!((image.width, image.height), (64, 32));
        // The next future is owned but not polled: revocation cannot leave background traffic.
        assert!(owner.job.is_some());
        owner.close();
        assert!(owner.job.is_none());
        server.await.unwrap();
    }
    #[test]
    fn only_ready_reading_pages_can_start_downloads() {
        let mut owner = InlineImages::default();
        let mut p = page();
        p.input_pending = true;
        assert!(owner.open(&p).unwrap().is_none());
        p.input_pending = false;
        p.connected = false;
        assert!(owner.open(&p).unwrap().is_none());
        p.connected = true;
        p.output_pending = true;
        assert!(owner.open(&p).unwrap().is_none());
        p.output_pending = false;
        p.navigation = Context::ContinuePrompt;
        assert!(owner.open(&p).unwrap().is_none());
        p = page();
        p.links.clear();
        assert!(owner.open(&p).unwrap().is_none());
        assert!(owner.job.is_none());
    }
    #[test]
    fn page_turns_reuse_resource_keys_and_only_queue_new_visible_urls() {
        let mut owner = InlineImages::default();
        let mut p = page();
        p.links.truncate(2);
        let first = owner.open(&p).unwrap().unwrap();
        assert_eq!(owner.targets.len(), 2);
        assert!(!owner.input(&p.navigation, b"\x1b[6~"));
        p.frame.revision += 1;
        p.links.swap(0, 1);
        p.links[0].row = 8;
        let second = owner.follow(&p).unwrap().unwrap();
        assert_eq!(second.batch, first.batch);
        assert_eq!(second.entries[0].key, 1);
        assert_eq!(second.entries[0].index, 0);
        assert_eq!(second.entries[0].row, 8);
        assert_eq!(owner.targets.len(), 2);
        assert_eq!(owner.queue.len(), 1);
        p.frame.revision += 1;
        p.links.clear();
        assert!(owner.follow(&p).unwrap().unwrap().entries.is_empty());
        assert!(owner.active());
        assert!(owner.input(&p.navigation, b"n"));
        assert!(owner.follow(&page()).unwrap().is_none());
        assert!(owner.targets.is_empty());
        assert_eq!(owner.open(&page()).unwrap().unwrap().batch, first.batch + 1);
    }
    #[test]
    fn article_resource_count_is_bounded_across_many_pages_and_does_not_redownload_old_urls() {
        let mut owner = InlineImages::default();
        let mut p = page();
        let mut last = None;
        for page in 0..6 {
            p.frame.revision += 1;
            for (index, link) in p.links.iter_mut().enumerate() {
                link.url = format!("http://127.0.0.1:9/{page}-{index}.png");
            }
            last = owner.open(&p).unwrap();
        }
        assert!(last.unwrap().limited);
        assert_eq!(owner.targets.len(), 32);
        assert_eq!(owner.queue.len(), 31);
        owner.close();
        assert!(owner.targets.is_empty());
        assert!(owner.job.is_none());
    }
    #[test]
    fn a_new_connection_cannot_reuse_the_old_article_or_its_pending_download() {
        let mut owner = InlineImages::default();
        let mut p = page();
        let first = owner.open(&p).unwrap().unwrap();
        p.generation += 1;
        p.links.truncate(1);
        let next = owner.open(&p).unwrap().unwrap();
        assert_eq!(next.batch, first.batch + 1);
        assert_eq!(owner.targets.len(), 1);
        assert_eq!(owner.queue.len(), 0);
        owner.close();
        assert!(!owner.active());
    }
}
