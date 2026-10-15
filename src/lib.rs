//! PTT-specific screen state. This is not a shell, PTY or general terminal emulator.
#[cfg(feature = "desktop")]
pub mod connection;
pub mod encoding;
pub mod export;
pub mod input;
pub mod links;
pub mod screen;
pub mod selection;
#[cfg(feature = "desktop")]
pub mod storage;
pub mod telnet;
