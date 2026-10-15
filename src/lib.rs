//! PTT-specific screen state. This is not a shell, PTY or general terminal emulator.
#[cfg(feature = "desktop")]
pub mod connection;
pub mod encoding;
pub mod screen;
pub mod selection;
pub mod telnet;
