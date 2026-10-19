//! Desktop UI boundaries: view, bridge, presentation, preferences and event coordination.
mod app;
mod bookmarks;
mod bridge;
mod commands;
mod files;
mod files_recent;
mod lifecycle;
mod outbound;
mod preferences;
mod presentation;
mod probe;
mod protocol;
mod recent;
mod sites;
mod surface;
mod surface_state;

pub use app::{AppProps, app};

#[cfg(test)]
mod outbound_tests;
