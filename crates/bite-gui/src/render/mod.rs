//! Everything that names a graphics API.
//!
//! The shell owns the device and the swapchain; sokol_gfx is handed the first at `sg::setup` and
//! a render-target view of the second per frame, and never touches a window. So the split here is
//! by lifetime and by platform: [`d3d11`] is the Windows device and swapchain (the macOS twin
//! will be a `CAMetalLayer` beside it), [`imgui`] is the renderer Dear ImGui draws through,
//! [`textures`] is the editor's own image store, and [`readback`] is how `--capture` gets pixels
//! back out of a render target.
//!
//! Above this, nothing in the editor names a graphics API at all.

#[cfg(windows)]
pub mod d3d11;
pub mod imgui;
#[cfg(windows)]
pub mod readback;
pub mod textures;

#[cfg(windows)]
pub use d3d11::{Device, SWAPCHAIN_FORMAT, Swapchain};
pub use textures::Textures;
