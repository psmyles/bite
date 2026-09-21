//! Everything that names a graphics API.
//!
//! The shell owns the device and the swapchain; sokol_gfx is handed the first at `sg::setup` and
//! a render target of the second per frame, and never touches a window. So the split here is by
//! lifetime and by platform: [`d3d11`] is the Windows device and swapchain and [`metal`] is the
//! macOS one, [`imgui`] is the renderer Dear ImGui draws through, [`textures`] is the editor's
//! own image store, and [`readback`] is how `--capture` gets pixels back out of a render target.
//!
//! The two platform modules are twins, not a trait: the set of targets is closed and known, so
//! the alias below costs nothing at runtime and keeps the shell free of `cfg`. Anything added to
//! one has to be added to the other - the contract is `Device` (`create`, `fill_environment`),
//! `Swapchain` (`new`, `size`, `resize`, `set_scale_factor`, `acquire`, `present`) and
//! `SWAPCHAIN_FORMAT`, which is the one constant the rest of the renderer agrees with them on.
//!
//! Above this, nothing in the editor names a graphics API at all.

#[cfg(windows)]
pub mod d3d11;
pub mod imgui;
#[cfg(target_os = "macos")]
pub mod metal;
pub mod readback;
pub mod textures;

#[cfg(windows)]
pub use d3d11::{Device, SWAPCHAIN_FORMAT, Swapchain};
#[cfg(target_os = "macos")]
pub use metal::{Device, SWAPCHAIN_FORMAT, Swapchain};
pub use textures::Textures;
