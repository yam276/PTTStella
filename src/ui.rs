//! Desktop UI boundaries: view, bridge, presentation, preferences and event coordination.
mod app;
mod bridge;
mod commands;
mod lifecycle;
mod outbound;
mod preferences;
mod presentation;
mod probe;
mod protocol;
mod surface;

pub use app::{AppProps, app};

#[cfg(test)]
mod outbound_tests;
