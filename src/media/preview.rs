//! Native preview lifetime: a request, its cancellable work and decode admission.
use super::{
    Target,
    loader::{Error, Image, Loaded, Loader},
};
use futures_util::future::BoxFuture;
use serde::Serialize;
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Presentation {
    Hover,
    Inspect,
}

#[derive(Clone)]
pub struct Active {
    pub request: u64,
    pub generation: u64,
    pub target: Target,
    pub presentation: Presentation,
}
pub struct Update {
    pub request: u64,
    pub result: Result<(Arc<Image>, bool), Error>,
}
#[derive(Default)]
pub struct Preview {
    sequence: u64,
    active: Option<Active>,
    job: Option<BoxFuture<'static, Result<Loaded, Error>>>,
    pending: Option<Loaded>,
    image: Option<Arc<Image>>,
    loader: Loader,
}
impl Preview {
    pub fn active(&self) -> Option<&Active> {
        self.active.as_ref()
    }
    pub fn pending(&self) -> bool {
        self.job.is_some() || self.pending.is_some()
    }
    pub fn open(&mut self, generation: u64, target: Target) -> Result<Active, String> {
        if let Some(active) = self.active.as_mut()
            && active.generation == generation
            && active.target == target
            && active.presentation == Presentation::Hover
        {
            active.presentation = Presentation::Inspect;
            return Ok(active.clone());
        }
        self.start(generation, target, Presentation::Inspect)
    }
    pub fn hover(&mut self, generation: u64, target: Target) -> Result<Active, String> {
        self.start(generation, target, Presentation::Hover)
    }
    fn start(
        &mut self,
        generation: u64,
        target: Target,
        presentation: Presentation,
    ) -> Result<Active, String> {
        let request = self
            .sequence
            .checked_add(1)
            .ok_or(crate::msg!("media-sequence-exhausted"))?;
        self.close();
        self.sequence = request;
        self.job = target.image_url.as_ref().map(|url| self.loader.load(url));
        let active = Active {
            request,
            generation,
            target,
            presentation,
        };
        self.active = Some(active.clone());
        Ok(active)
    }
    pub fn close(&mut self) {
        self.job = None;
        self.pending = None;
        self.image = None;
        self.active = None;
    }
    pub fn retry(&mut self) {
        self.pending = None;
        self.image = None;
        self.job = self
            .active
            .as_ref()
            .and_then(|active| active.target.image_url.as_ref())
            .map(|url| self.loader.load(url));
    }
    pub fn image(&self, request: u64, generation: u64) -> Option<Arc<Image>> {
        let active = self.active.as_ref()?;
        (active.request == request
            && active.generation == generation
            && active.presentation == Presentation::Inspect
            && self.pending.is_none())
        .then(|| self.image.clone())
        .flatten()
    }
    pub fn decoded(&mut self, request: u64, success: bool) {
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.request != request)
        {
            return;
        }
        if !success {
            self.image = None;
        }
        if let Some(loaded) = self.pending.take()
            && success
        {
            self.loader.retain(&loaded);
        }
    }
    pub async fn update(&mut self) -> Option<Update> {
        let result = match self.job.as_mut() {
            Some(job) => job.await,
            None => std::future::pending().await,
        };
        self.job = None;
        let active = self.active.as_ref()?;
        let result = match result {
            Ok(loaded) => {
                self.image = Some(loaded.image.clone());
                let result = (loaded.image.clone(), loaded.cached);
                self.pending = Some(loaded);
                Ok(result)
            }
            Err(error) => Err(error),
        };
        Some(Update {
            request: active.request,
            result,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        time::{Duration, timeout},
    };

    #[tokio::test]
    async fn originals_require_current_inspect_and_successful_decode_and_release_on_retry_or_close()
    {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let target = super::super::classify(&format!(
            "http://{}/image.png",
            listener.local_addr().unwrap()
        ))
        .unwrap()
        .unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") && request.len() < 4096 {
                request.push(socket.read_u8().await.unwrap());
            }
            assert!(request.starts_with(b"GET /image.png HTTP/1.1\r\n"));
            let bytes = include_bytes!("../../tests/fixtures/media/colours.png");
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nCache-Control: max-age=60\r\nConnection: close\r\n\r\n", bytes.len()).as_bytes()).await.unwrap();
            socket.write_all(bytes).await.unwrap();
        });
        let mut preview = Preview::default();
        let active = preview.hover(7, target.clone()).unwrap();
        assert!(preview.image(active.request, 7).is_none());
        let image = timeout(Duration::from_secs(3), preview.update())
            .await
            .unwrap()
            .unwrap()
            .result
            .unwrap()
            .0;
        preview.decoded(active.request + 1, true);
        assert!(preview.image(active.request, 7).is_none());
        preview.decoded(active.request, true);
        assert!(
            preview.image(active.request, 7).is_none(),
            "hover grants no original output"
        );
        assert_eq!(
            preview.open(7, target.clone()).unwrap().request,
            active.request
        );
        assert!(Arc::ptr_eq(
            &preview.image(active.request, 7).unwrap(),
            &image
        ));
        assert!(preview.image(active.request, 8).is_none());
        assert!(preview.image(active.request + 1, 7).is_none());
        preview.retry();
        assert!(preview.image(active.request, 7).is_none());
        assert!(
            timeout(Duration::from_secs(3), preview.update())
                .await
                .unwrap()
                .unwrap()
                .result
                .unwrap()
                .1,
            "retry reuses only the successfully decoded cache"
        );
        assert!(
            preview.image(active.request, 7).is_none(),
            "new decode ACK is required"
        );
        preview.decoded(active.request, false);
        assert!(preview.image(active.request, 7).is_none());
        let next = preview.open(8, target).unwrap();
        assert_ne!(active.request, next.request);
        preview.close();
        preview.decoded(next.request, true);
        assert!(preview.active().is_none());
        assert!(!preview.pending());
        assert!(preview.image(next.request, 8).is_none());
        timeout(Duration::from_secs(3), server)
            .await
            .unwrap()
            .unwrap();
    }
}
