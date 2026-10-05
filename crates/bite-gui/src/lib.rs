//! The native BITE editor. Modules mirror the Electron renderer they reproduce.
pub mod app;
pub mod canvas;
pub mod color_picker;
pub mod commands;
pub mod controls;
pub mod create_menu;
pub mod dialogs;
pub mod icon;
pub mod log_window;
pub mod logging;
pub mod menu;
/// The macOS system menu bar, which is where Cmd+Q lives.
#[cfg(target_os = "macos")]
pub mod menubar;
pub mod modals;
/// Workflows handed over by Launch Services rather than on the command line.
#[cfg(target_os = "macos")]
pub mod openfiles;
pub mod panels;
pub mod persist;
pub mod platform;
pub mod render;
pub mod shell;
pub mod showcase;
pub mod smoke;
pub mod studio;
pub mod theme;
pub mod timing;
pub mod timings;
pub mod updates;
pub mod work;
