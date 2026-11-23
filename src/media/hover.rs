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
    /// Correlate independently authorized hits for dwell/promotion. Repaints
    /// may change revision/index; input, session and link movement change the
    /// source. This does not itself admit an unconfirmed hit.
    pub fn same_source(&self, other: &Self) -> bool {
        self.stamp.generation == other.stamp.generation
            && self.stamp.input_epoch == other.stamp.input_epoch
            && self.link == other.link
    }
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
