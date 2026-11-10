//! Native video backend shared by the preview and its portable probe.
mod control;
pub mod gl_state;
pub mod mpv;
pub mod session;
pub use control::{Command, Status};

pub mod software;
pub mod surface;

// Match both rendering paths: a backend choice must not silently reduce video
// resolution. Allocation is still bounded by the actual view and this budget.
pub(super) const MAX_RENDER_DIMENSION: u32 = 8192;
pub(super) const MAX_RENDER_PIXELS: u64 = 16_777_216;

pub(super) fn render_size_allowed(width: u32, height: u32) -> bool {
    width > 0
        && height > 0
        && width <= MAX_RENDER_DIMENSION
        && height <= MAX_RENDER_DIMENSION
        && u64::from(width) * u64::from(height) <= MAX_RENDER_PIXELS
}
