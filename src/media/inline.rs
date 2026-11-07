//! Article-scoped expansion; page projections use only acknowledged filtered links.
use crate::{
    media::{
        self,
        loader::{Loaded, Loader},
    },
    navigation::Context,
};
use crate::{presentation::Paint, surface_state::Identity};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

const MAX_IMAGES: usize = 8;
const MAX_ARTICLE_IMAGES: usize = 32;
const MAX_PIXELS: usize = 16 * 1024 * 1024;
const MAX_BYTES: usize = 32 * 1024 * 1024;
#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    #[default]
    All,
    Link {
        index: usize,
    },
}
#[derive(Clone, Serialize)]
pub struct Entry {
    pub index: usize,
    pub row: usize,
    pub key: usize,
}
#[derive(Serialize)]
pub struct Open {
    pub batch: u64,
    pub presentation: u64,
    pub generation: u64,
    pub revision: u64,
    pub first_line: usize,
    pub article_id: Option<u64>,
    pub entries: Vec<Entry>,
    pub limited: bool,
}
pub struct Update {
    pub batch: u64,
    pub index: usize,
    pub result: Result<Loaded, String>,
}
/// Immutable link projection without copying the article's glyph/cell frame.
/// Constructed by the frontend only from its acknowledged filtered Paint.
#[derive(Clone)]
enum ProjectedLinks<'a> {
    Borrowed(&'a [crate::links::Link]),
    Owned(Arc<[crate::links::Link]>),
}
impl std::ops::Deref for ProjectedLinks<'_> {
    type Target = [crate::links::Link];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Borrowed(links) => links,
            Self::Owned(links) => links,
        }
    }
}
#[derive(Clone)]
pub struct Projection<'a> {
    identity: Identity,
    article_id: Option<u64>,
    first_line: usize,
    eligible: bool,
    links: ProjectedLinks<'a>,
}
impl<'a> Projection<'a> {
    pub fn of(paint: &'a Paint) -> Self {
        Self {
            identity: Identity {
                presentation: paint.presentation,
                generation: paint.generation,
                revision: paint.frame.revision,
            },
            article_id: paint.article.as_ref().map(|a| a.id),
            first_line: paint.article.as_ref().map_or(1, |a| a.first_line),
            eligible: matches!(paint.navigation, Context::ArticleReading { .. })
                && paint.article_mode != crate::settings::ArticleMode::Terminal
                && paint.connected
                && !paint.input_pending
                && !paint.output_pending,
            links: ProjectedLinks::Borrowed(&paint.links),
        }
    }
    /// Native worker handoff copies links once, never the article's raster or
    /// glyph/cell frame. Existing synchronous callers keep a borrowed slice.
    pub fn into_owned(self) -> Projection<'static> {
        Projection {
            identity: self.identity,
            article_id: self.article_id,
            first_line: self.first_line,
            eligible: self.eligible,
            links: match self.links {
                ProjectedLinks::Borrowed(links) => ProjectedLinks::Owned(Arc::from(links)),
                ProjectedLinks::Owned(links) => ProjectedLinks::Owned(links),
            },
        }
    }
    pub fn links(&self) -> &[crate::links::Link] {
        &self.links
    }
    /// The same deduplicated targets and joined-fragment placement feed both
    /// the resource batch and native expansion controls.
    pub fn images(&self) -> Vec<(usize, usize, String)> {
        let mut targets: Vec<(usize, usize, String)> = Vec::new();
        for (index, link) in self.links.iter().enumerate() {
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
        targets
    }
    pub fn identity(&self) -> Identity {
        self.identity
    }
    pub fn eligible(&self) -> bool {
        self.eligible
    }
}
type Job = BoxFuture<'static, Result<Loaded, media::loader::Error>>;
#[derive(Default)]
pub struct InlineImages {
    sequence: u64,
    identity: Option<Identity>,
    article_id: Option<u64>,
    turning: bool,
    expand_all: bool,
    auto_expand: bool,
    auto_batch: bool,
    targets: Vec<String>,
    images: BTreeMap<usize, Arc<media::loader::Image>>,
    queue: VecDeque<(usize, String)>,
    job: Option<(usize, Job)>,
    loader: Loader,
    bytes: usize,
    pixels: usize,
}
impl InlineImages {
    pub fn close(&mut self) {
        self.identity = None;
        self.article_id = None;
        self.turning = false;
        self.expand_all = false;
        self.auto_batch = false;
        self.targets.clear();
        self.images.clear();
        self.queue.clear();
        self.job = None;
        self.bytes = 0;
        self.pixels = 0;
    }
    pub fn batch(&self) -> Option<u64> {
        self.identity.map(|_| self.sequence)
    }
    pub fn active(&self) -> bool {
        self.identity.is_some()
    }
    /// Presentation adapters may discard a late automatic result after opt-out.
    pub fn automatic_batch(&self) -> bool {
        self.auto_batch
    }
    pub fn set_auto_expand(&mut self, enabled: bool) -> bool {
        self.auto_expand = enabled;
        if !enabled && self.auto_batch {
            self.close();
            return true;
        }
        false
    }
    pub fn navigation(&mut self, context: &Context, intent: crate::navigation::Intent) -> bool {
        crate::navigation::plan(context, intent).is_ok_and(|bytes| self.input(context, &bytes))
    }
    /// Only actual reading movements preserve the current article scope.
    pub fn input(&mut self, context: &Context, bytes: &[u8]) -> bool {
        if !self.active() || bytes.is_empty() {
            return false;
        }
        if crate::article_cache::reading_input(context, bytes) {
            self.turning = true;
            false
        } else {
            self.close();
            true
        }
    }
    pub fn observe(&mut self, view: &crate::connection::View) -> bool {
        let Some(identity) = self.identity else {
            return false;
        };
        let invalid = identity.generation != view.generation
            || view.phase != crate::connection::Phase::Connected
            || !view.site.ptt_features()
            || view.mouse.route != crate::mouse::input::Route::Local;
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
    pub fn follow(&mut self, paint: &Paint) -> Result<Option<Open>, String> {
        self.follow_projection(&Projection::of(paint))
    }
    pub fn follow_projection(&mut self, paint: &Projection<'_>) -> Result<Option<Open>, String> {
        let automatic = self.auto_expand && paint.article_id.is_some();
        if !self.active() && (!automatic || paint.links.is_empty()) {
            return Ok(None);
        }
        self.project(paint, None, automatic)
    }
    pub fn open(&mut self, paint: &Paint, target: Target) -> Result<Option<Open>, String> {
        self.open_projection(&Projection::of(paint), target)
    }
    pub fn open_projection(
        &mut self,
        paint: &Projection<'_>,
        target: Target,
    ) -> Result<Option<Open>, String> {
        self.project(paint, Some(target), false)
    }
    fn project(
        &mut self,
        paint: &Projection<'_>,
        request: Option<Target>,
        automatic: bool,
    ) -> Result<Option<Open>, String> {
        let identity = paint.identity;
        let article_id = paint.article_id;
        if self.identity.is_some() && self.article_id != article_id {
            self.close();
        }
        if self.identity == Some(identity)
            && (request.is_none() && (!automatic || self.auto_batch)
                || matches!(request, Some(Target::All)) && self.expand_all)
        {
            return Ok(None);
        }
        if !paint.eligible {
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
        let targets = paint.images();
        if targets.is_empty() && !self.active() {
            return Ok(None);
        }
        if !self.active() {
            self.sequence = self
                .sequence
                .checked_add(1)
                .ok_or_else(|| crate::msg!("media-sequence-exhausted"))?;
        }
        let sequence = self.sequence;
        let previous_identity = self.identity;
        let previous_count = self.targets.len();
        if matches!(request, Some(Target::All)) {
            self.expand_all = true;
            self.auto_batch = false;
        } else if automatic && !self.expand_all {
            self.auto_batch = true;
        }
        self.turning = false;
        self.identity = Some(identity);
        self.article_id = article_id;
        let admit_all = automatic
            || matches!(request, Some(Target::All))
            || request.is_none() && self.expand_all;
        let max_added = if automatic {
            MAX_ARTICLE_IMAGES
        } else {
            MAX_IMAGES
        };
        let mut limited = false;
        let mut added = 0;
        let mut entries = Vec::new();
        for (index, row, url) in targets {
            if !self.targets.contains(&url) && !admit_all && selected.as_ref() != Some(&url) {
                continue;
            }
            let key = match self.targets.iter().position(|known| known == &url) {
                Some(key) => key,
                None if self.targets.len() < MAX_ARTICLE_IMAGES && added < max_added => {
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
            first_line: paint.first_line,
            entries,
            limited,
        }))
    }
    pub fn image(&self, paint: &Paint, index: usize) -> Option<Arc<media::loader::Image>> {
        self.image_projection(&Projection::of(paint), index)
    }
    pub fn image_projection(
        &self,
        paint: &Projection<'_>,
        index: usize,
    ) -> Option<Arc<media::loader::Image>> {
        if self.identity != Some(paint.identity)
            || self.article_id != paint.article_id
            || !paint.eligible
        {
            return None;
        }
        let target = media::classify(&paint.links.get(index)?.url).ok()??;
        let key = self.targets.iter().position(|url| *url == target.url)?;
        self.images.get(&key).cloned()
    }
    /// Retain only a successfully decoded original from this still-active batch.
    /// Uses the existing loader TTL/no-store/LRU limits; no second byte cache.
    pub fn decoded(&mut self, batch: u64, index: usize, loaded: &Loaded) {
        if self.batch() == Some(batch)
            && self
                .images
                .get(&index)
                .is_some_and(|image| Arc::ptr_eq(image, &loaded.image))
        {
            self.loader.retain(loaded);
        }
    }
    fn start_next(&mut self) {
        self.job = self
            .queue
            .pop_front()
            .map(|(index, url)| (index, self.loader.load(&url)));
    }
    fn admit(&mut self, pixels: usize, bytes: usize) -> Result<(), String> {
        if pixels > MAX_PIXELS - self.pixels || bytes > MAX_BYTES - self.bytes {
            return Err(crate::msg!("inline-image-budget"));
        }
        self.pixels += pixels;
        self.bytes += bytes;
        Ok(())
    }
    pub async fn update(&mut self) -> Update {
        let (index, result) = match self.job.as_mut() {
            Some((index, job)) => (*index, job.await),
            None => std::future::pending().await,
        };
        let result = result.map_err(|e| e.to_string()).and_then(|loaded| {
            let pixels = loaded.image.width.saturating_mul(loaded.image.height);
            let bytes = loaded.image.bytes.len();
            self.admit(pixels, bytes)?;
            self.images.insert(index, loaded.image.clone());
            Ok(loaded)
        });
        self.start_next();
        // Retained originals share the existing article payload budget and are
        // released with this scope; explicit copy/save never redownloads.
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
        let frame = crate::screen::Screen::new(80, 24).unwrap().frame();
        let mut p = crate::presentation::sample_packet(&frame, &Default::default());
        p.connected = true;
        p.navigation = Context::ArticleReading {
            first_line: 1,
            last_line: 23,
            percent: 50,
        };
        p.links = (0..10)
            .map(|n| crate::links::Link {
                row: n,
                start: 0,
                end: 30,
                url: format!("http://127.0.0.1:9/{n}.png"),
            })
            .collect();
        p
    }
    #[test]
    fn native_handoff_owns_only_the_acknowledged_link_projection() {
        let mut paint = page();
        let borrowed = Projection::of(&paint);
        assert!(matches!(borrowed.links, ProjectedLinks::Borrowed(_)));
        assert_eq!(borrowed.links.as_ptr(), paint.links.as_ptr());
        let owned = borrowed.into_owned();
        assert!(matches!(owned.links, ProjectedLinks::Owned(_)));
        let cloned = owned.clone();
        assert_eq!(
            owned.links.as_ptr(),
            cloned.links.as_ptr(),
            "native projection clones share immutable links"
        );
        let identity = owned.identity();
        paint.links[4].url = "https://example.test/changed.png".into();
        paint.links[4].row = 20;
        paint.frame.revision += 1;
        paint.connected = false;
        let mut owner = InlineImages::default();
        let opened = owner
            .open_projection(&owned, Target::Link { index: 4 })
            .unwrap()
            .unwrap();
        assert_eq!(
            (opened.presentation, opened.generation, opened.revision),
            (
                identity.presentation,
                identity.generation,
                identity.revision
            )
        );
        assert_eq!((opened.entries[0].index, opened.entries[0].row), (4, 4));
        assert_eq!(owner.targets, ["http://127.0.0.1:9/4.png"]);
        assert!(!Projection::of(&paint).eligible());
        owner.close();
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
    #[test]
    fn automatic_origin_tracks_opt_out_and_explicit_all_promotion() {
        let paint = page();
        let mut projection = Projection::of(&paint);
        projection.article_id = Some(1);
        let mut owner = InlineImages::default();
        owner.set_auto_expand(true);
        assert!(owner.follow_projection(&projection).unwrap().is_some());
        assert!(owner.automatic_batch());
        assert!(owner.set_auto_expand(false));
        assert!(!owner.automatic_batch() && !owner.active());
        owner.set_auto_expand(true);
        owner.follow_projection(&projection).unwrap();
        owner.open_projection(&projection, Target::All).unwrap();
        assert!(!owner.automatic_batch());
        assert!(!owner.set_auto_expand(false));
        assert!(owner.active());
        owner.close();
        assert!(!owner.automatic_batch());
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
    #[tokio::test]
    async fn only_current_decode_ack_retains_the_original_across_scope_close() {
        use tokio::{
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 2048];
                let count = socket.read(&mut request).await.unwrap();
                assert!(
                    String::from_utf8_lossy(&request[..count])
                        .starts_with("GET /fixture.png HTTP/1.1")
                );
                let bytes = include_bytes!("../../tests/fixtures/media/colours.png");
                socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nCache-Control: max-age=60\r\nConnection: close\r\n\r\n",bytes.len()).as_bytes()).await.unwrap();
                socket.write_all(bytes).await.unwrap();
            }
            assert!(
                tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                    .await
                    .is_err()
            );
        });
        let mut paint = page();
        paint.links.truncate(1);
        paint.links[0].url = format!("http://{address}/fixture.png");
        let mut owner = InlineImages::default();
        let first = owner.open(&paint, Target::All).unwrap().unwrap();
        let first_image = owner.update().await.result.unwrap();
        owner.close(); // No decoder ACK: must download again.
        owner.open(&paint, Target::All).unwrap();
        let second = owner.update().await.result.unwrap();
        assert!(!second.cached && !Arc::ptr_eq(&first_image.image, &second.image));
        owner.decoded(first.batch, 0, &first_image); // Old batch cannot admit bytes.
        owner.close();
        let third = owner.open(&paint, Target::All).unwrap().unwrap();
        let third_image = owner.update().await.result.unwrap();
        assert!(!third_image.cached);
        owner.decoded(third.batch, 0, &third_image);
        owner.close();
        owner.open(&paint, Target::All).unwrap();
        let cached = owner.update().await.result.unwrap();
        assert!(cached.cached && Arc::ptr_eq(&third_image.image, &cached.image));
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
        p.article_mode = crate::settings::ArticleMode::Terminal;
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

#[cfg(test)]
mod output_tests {
    use super::*;
    #[test]
    fn original_output_resolves_only_current_owned_links_and_clears_with_the_article() {
        let frame = crate::screen::Screen::new(80, 24).unwrap().frame();
        let mut paint = crate::presentation::sample_packet(&frame, &Default::default());
        paint.connected = true;
        paint.navigation = Context::ArticleReading {
            first_line: 1,
            last_line: 23,
            percent: 100,
        };
        paint.links = vec![crate::links::Link {
            row: 2,
            start: 0,
            end: 20,
            url: "https://example.com/a.png".into(),
        }];
        let mut owner = InlineImages::default();
        owner.open(&paint, Target::Link { index: 0 }).unwrap();
        assert!(owner.image(&paint, 0).is_none());
        let image = Arc::new(media::loader::Image {
            bytes: vec![1],
            mime: "image/png",
            width: 1,
            height: 1,
            animated_format: false,
            animation_pixels: 0,
        });
        owner.images.insert(0, image.clone());
        assert!(Arc::ptr_eq(&owner.image(&paint, 0).unwrap(), &image));
        assert!(owner.image(&paint, 1).is_none());
        for change in [
            "generation",
            "presentation",
            "revision",
            "pending",
            "terminal",
            "url",
        ] {
            let mut stale = paint.clone();
            match change {
                "generation" => stale.generation += 1,
                "presentation" => stale.presentation += 1,
                "revision" => stale.frame.revision += 1,
                "pending" => stale.input_pending = true,
                "terminal" => stale.article_mode = crate::settings::ArticleMode::Terminal,
                _ => stale.links[0].url = "https://example.com/b.png".into(),
            }
            assert!(owner.image(&stale, 0).is_none(), "{change}");
        }
        owner.close();
        assert!(owner.images.is_empty());
        assert!(owner.image(&paint, 0).is_none());
    }
}
