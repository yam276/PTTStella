//! A hover lease can survive unrelated repainting, never new input or source identity.
use crate::{links::Link, navigation::Stamp};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct Lease {
    pub sequence: u64,
    pub stamp: Stamp,
    pub index: usize,
    pub link: Link,
}
impl Lease {
    pub fn renew(&mut self, stamp: Stamp, complete: bool, links: &[Link]) -> bool {
        if self.stamp.generation != stamp.generation
            || self.stamp.input_epoch != stamp.input_epoch
            || !complete
        {
            return false;
        }
        if let Some(index) = links.iter().position(|link| link == &self.link) {
            self.index = index;
            self.stamp = stamp;
            true
        } else {
            false
        }
    }
}
