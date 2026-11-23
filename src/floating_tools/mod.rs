//! Window-independent tool chrome geometry and bounded drag/coast reducer.
//! Frontends own focus, timers and action admission; this model sends no input.
pub mod geometry;
mod state;
pub use state::State;
#[cfg(test)]
mod tests;
