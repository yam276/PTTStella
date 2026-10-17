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
        self.active = None;
    }
    pub fn retry(&mut self) {
        self.pending = None;
        self.job = self
            .active
            .as_ref()
            .and_then(|active| active.target.image_url.as_ref())
            .map(|url| self.loader.load(url));
    }
    pub fn decoded(&mut self, request: u64, success: bool) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.request == request)
            && let Some(loaded) = self.pending.take()
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
