//! PTT-specific screen state. This is not a shell, PTY or general terminal emulator.
pub mod article_target;
pub mod board_navigation;
#[cfg(feature = "desktop")]
pub mod connection;
pub mod encoding;
pub mod export;
pub mod filters;
pub mod i18n;
pub mod input;
pub mod links;
pub mod login;
pub mod media;
pub mod navigation;
pub mod recent_boards;
pub mod screen;
pub mod selection;
pub mod settings;
#[cfg(feature = "desktop")]
pub mod storage;
pub mod telnet;
pub mod text;
#[cfg(feature = "desktop")]
mod tls;
pub mod vim;
