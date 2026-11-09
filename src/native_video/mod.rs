//! Native video backend shared by the preview and its portable probe.
mod control;
pub mod gl_state;
pub mod mpv;
pub mod session;
pub use control::{Command, Status};

pub mod software;
pub mod surface;
