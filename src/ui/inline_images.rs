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
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const MAX_IMAGES: usize = 8;
const MAX_ARTICLE_IMAGES: usize = 32;
const MAX_PIXELS: usize = 16 * 1024 * 1024;
const MAX_BYTES: usize = 24 * 1024 * 1024;
#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Target {
    #[default]
    All,
    Link {
        index: usize,
    },
}
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
    pub first_line: usize,
    pub article_id: Option<u64>,
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
    article_id: Option<u64>,
    turning: bool,
    expand_all: bool,
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
        self.article_id = None;
        self.turning = false;
        self.expand_all = false;
        self.targets.clear();
        self.queue.clear();
        self.job = None;
        self.bytes = 0;
        self.pixels = 0;
    }
    pub(super) fn batch(&self) -> Option<u64> {
        self.identity.map(|_| self.sequence)
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
        if super::article_cache::reading_input(context, bytes) {
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
        self.project(paint, None)
    }
    pub(super) fn open(&mut self, paint: &Paint, target: Target) -> Result<Option<Open>, String> {
        self.project(paint, Some(target))
    }
    fn project(&mut self, paint: &Paint, request: Option<Target>) -> Result<Option<Open>, String> {
        let identity = Identity {
            presentation: paint.presentation,
            generation: paint.generation,
            revision: paint.frame.revision,
        };
        let article_id = paint.article.as_ref().map(|article| article.id);
        if self.identity.is_some() && self.article_id != article_id {
            self.close();
        }
        if self.identity == Some(identity)
            && (request.is_none() || matches!(request, Some(Target::All)) && self.expand_all)
        {
            return Ok(None);
        }
        if !matches!(paint.navigation, Context::ArticleReading { .. })
            || paint.article_mode == pttstella::settings::ArticleMode::Terminal
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
        // Resolve only a closed Rust link index, never a renderer-supplied URL.
        let selected = match request {
            Some(Target::Link { index }) => {
                let Some(link) = paint.links.get(index) else {
                    return Ok(None);
                };
                let Ok(Some(target)) = media::classify(&link.url) else {
                    return Ok(None);
                };
                if target.kind != media::Kind::Image {
                    return Ok(None);
                }
                Some(target.url)
            }
            _ => None,
        };
        let mut targets: Vec<(usize, usize, String)> = Vec::new();
        for (index, link) in paint.links.iter().enumerate() {
            if let Ok(Some(target)) = media::classify(&link.url)
                && target.kind == media::Kind::Image
            {
                if let Some(existing) = targets.iter_mut().find(|(_, _, url)| url == &target.url) {
                    if link.row <= existing.1.saturating_add(1) {
                        existing.1 = existing.1.max(link.row);
                    }
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
        let previous_identity = self.identity;
        let previous_count = self.targets.len();
        if matches!(request, Some(Target::All)) {
            self.expand_all = true;
        }
        self.turning = false;
        self.identity = Some(identity);
        self.article_id = article_id;
        let admit_all =
            matches!(request, Some(Target::All)) || request.is_none() && self.expand_all;
        let mut limited = false;
        let mut added = 0;
        let mut entries = Vec::new();
        for (index, row, url) in targets {
            if !self.targets.contains(&url) && !admit_all && selected.as_ref() != Some(&url) {
                continue;
            }
            let key = match self.targets.iter().position(|known| known == &url) {
                Some(key) => key,
                None if self.targets.len() < MAX_ARTICLE_IMAGES && added < MAX_IMAGES => {
                    added += 1;
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
        if previous_identity == Some(identity) && previous_count == self.targets.len() && !limited {
            return Ok(None);
        }
        Ok(Some(Open {
            batch: sequence,
            article_id,
            presentation: identity.presentation,
            generation: identity.generation,
            revision: identity.revision,
            first_line: paint
                .article
                .as_ref()
                .map_or(1, |article| article.first_line),
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
        let open = owner.open(&p, Target::All).unwrap().unwrap();
        assert!(open.limited);
        assert_eq!(open.entries.len(), 8);
        assert_eq!(
            open.entries.iter().map(|e| e.index).collect::<Vec<_>>(),
            vec![0, 2, 3, 4, 5, 6, 7, 8]
        );
        assert!(owner.open(&p, Target::All).unwrap().is_none());
        assert_eq!(owner.queue.len(), 7);
        owner.close();
        assert!(owner.job.is_none());
        assert!(owner.queue.is_empty());
        assert_eq!(owner.open(&p, Target::All).unwrap().unwrap().batch, 2);
    }
    #[test]
    fn link_target_wire_format_is_closed_and_bounded_by_usize() {
        for value in [
            serde_json::json!({"kind":"link","index":-1}),
            serde_json::json!({"kind":"link","index":1.5}),
            serde_json::json!({"kind":"link","index":0,"url":"https://example.com/x.png"}),
            serde_json::json!({"kind":"url","url":"https://example.com/x.png"}),
        ] {
            assert!(serde_json::from_value::<Target>(value).is_err());
        }
        assert!(matches!(
            serde_json::from_value::<Target>(serde_json::json!({"kind":"link","index":0})).unwrap(),
            Target::Link { index: 0 }
        ));
    }
    #[test]
    fn single_requests_share_one_batch_without_expanding_other_urls_on_follow() {
        let mut p = page();
        let mut owner = InlineImages::default();
        let first = owner.open(&p, Target::Link { index: 4 }).unwrap().unwrap();
        assert_eq!(
            first
                .entries
                .iter()
                .map(|entry| (entry.index, entry.key))
                .collect::<Vec<_>>(),
            vec![(4, 0)]
        );
        assert_eq!(owner.targets, vec!["http://127.0.0.1:9/4.png"]);
        assert!(owner.queue.is_empty());
        assert!(owner.open(&p, Target::Link { index: 4 }).unwrap().is_none());
        let second = owner.open(&p, Target::Link { index: 2 }).unwrap().unwrap();
        assert_eq!(second.batch, first.batch);
        assert_eq!(
            second
                .entries
                .iter()
                .map(|entry| (entry.index, entry.key))
                .collect::<Vec<_>>(),
            vec![(2, 1), (4, 0)]
        );
        assert_eq!(owner.queue.len(), 1);
        p.frame.revision += 1;
        let followed = owner.follow(&p).unwrap().unwrap();
        assert_eq!(followed.entries.len(), 2);
        assert_eq!(owner.targets.len(), 2);
        assert_eq!(owner.queue.len(), 1);
        // Explicit all upgrades the same resource owner; known URLs are reused.
        let all = owner.open(&p, Target::All).unwrap().unwrap();
        assert_eq!(all.batch, first.batch);
        assert_eq!(owner.targets.len(), 10);
        assert_eq!(owner.queue.len(), 9);
        assert!(owner.open(&p, Target::All).unwrap().is_none());
    }
    #[test]
    fn single_target_rejects_missing_and_non_image_indices_without_starting_a_batch() {
        let mut p = page();
        p.links[0].url = "https://example.com/article".into();
        let mut owner = InlineImages::default();
        for index in [0, p.links.len(), usize::MAX] {
            assert!(owner.open(&p, Target::Link { index }).unwrap().is_none());
            assert!(!owner.active());
            assert!(owner.job.is_none());
            assert!(owner.queue.is_empty());
        }
        let first = owner.open(&p, Target::Link { index: 1 }).unwrap().unwrap();
        p.generation += 1;
        let next = owner.open(&p, Target::Link { index: 2 }).unwrap().unwrap();
        assert_eq!(next.batch, first.batch + 1);
        assert_eq!(owner.targets, vec!["http://127.0.0.1:9/2.png"]);
        assert!(owner.queue.is_empty());
    }
    #[tokio::test]
    async fn opening_one_image_never_polls_the_other_page_downloads() {
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
            assert!(String::from_utf8_lossy(&request[..count]).starts_with("GET /4.png HTTP/1.1"));
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
        let mut p = page();
        for (index, link) in p.links.iter_mut().enumerate() {
            link.url = format!("http://{address}/{index}.png");
        }
        let mut owner = InlineImages::default();
        owner.open(&p, Target::Link { index: 4 }).unwrap().unwrap();
        assert!(owner.update().await.result.is_ok());
        assert!(owner.job.is_none());
        assert!(owner.queue.is_empty());
        owner.close();
        server.await.unwrap();
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
        let open = owner.open(&paint, Target::All).unwrap().unwrap();
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
        assert!(owner.open(&p, Target::All).unwrap().is_none());
        p.input_pending = false;
        p.connected = false;
        assert!(owner.open(&p, Target::All).unwrap().is_none());
        p.connected = true;
        p.output_pending = true;
        assert!(owner.open(&p, Target::All).unwrap().is_none());
        p.output_pending = false;
        p.navigation = Context::ContinuePrompt;
        assert!(owner.open(&p, Target::All).unwrap().is_none());
        p = page();
        p.article_mode = pttstella::settings::ArticleMode::Terminal;
        assert!(owner.open(&p, Target::Link { index: 0 }).unwrap().is_none());
        p = page();
        p.links.clear();
        assert!(owner.open(&p, Target::All).unwrap().is_none());
        assert!(owner.job.is_none());
    }
    #[test]
    fn page_turns_reuse_resource_keys_and_only_queue_new_visible_urls() {
        let mut owner = InlineImages::default();
        let mut p = page();
        p.links.truncate(2);
        let first = owner.open(&p, Target::All).unwrap().unwrap();
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
        assert_eq!(
            owner.open(&page(), Target::All).unwrap().unwrap().batch,
            first.batch + 1
        );
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
            last = owner.open(&p, Target::All).unwrap();
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
        let first = owner.open(&p, Target::All).unwrap().unwrap();
        p.generation += 1;
        p.links.truncate(1);
        let next = owner.open(&p, Target::All).unwrap().unwrap();
        assert_eq!(next.batch, first.batch + 1);
        assert_eq!(owner.targets.len(), 1);
        assert_eq!(owner.queue.len(), 0);
        owner.close();
        assert!(!owner.active());
    }
}
