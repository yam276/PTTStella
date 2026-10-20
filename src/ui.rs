//! Desktop UI boundaries: view, bridge, presentation, preferences and event coordination.
mod app;
mod article_cache;
mod bookmarks;
mod bridge;
mod commands;
mod files;
mod files_recent;
mod filter_import;
mod inline_images;
mod lifecycle;
mod outbound;
mod preferences;
mod presentation;
mod probe;
mod protocol;
mod recent;
mod settings_finish;
mod sites;
mod surface;
mod surface_state;

pub use app::{AppProps, app};

#[cfg(test)]
mod outbound_tests;
