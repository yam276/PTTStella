//! Desktop UI boundaries: view, bridge, presentation, preferences and event coordination.
mod app;
mod commands;
mod lifecycle;
mod preferences;
mod presentation;
mod probe;
mod protocol;
mod surface;

pub use app::{AppProps, app};
