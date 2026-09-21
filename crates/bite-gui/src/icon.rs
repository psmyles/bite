//! The window and taskbar icon.
//!
//! `assets/icon-256.png` is generated from the one master, `build/icons/icon.png`, by
//! `scripts/generate-icons.ps1`. The executable's own icon - what Explorer draws - is the
//! multi-resolution `build/icon.ico` embedded by `build.rs`; this is the runtime icon winit
//! hands the window manager, which wants raw RGBA.
//!
//! The PNG is decoded at build time (`build.rs::decode_icon`) and embedded already unpacked, so
//! the launch path carries no decode and a malformed asset is a build error rather than a window
//! that quietly falls back to the system default.

/// The icon artwork, decoded to straight-alpha RGBA by `build.rs`. Edge kept in sync with its
/// `ICON_EDGE`.
const ICON_EDGE: u32 = 256;
static ICON_RGBA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon.rgba"));

/// The icon for winit.
///
/// `None` only if winit rejects the buffer, which it cannot: the build asserted the dimensions
/// and the format. The window would keep the system default, which is not worth aborting a
/// launch over.
pub fn window_icon() -> Option<winit::window::Icon> {
    winit::window::Icon::from_rgba(ICON_RGBA.to_vec(), ICON_EDGE, ICON_EDGE).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The build decoded something of the right shape, and winit accepts it.
    #[test]
    fn the_icon_is_square_rgba_that_winit_takes() {
        assert_eq!(ICON_RGBA.len(), (ICON_EDGE * ICON_EDGE * 4) as usize);
        assert!(window_icon().is_some());
        // Straight alpha, not premultiplied, and an icon that is not entirely transparent.
        assert!(ICON_RGBA.as_chunks::<4>().0.iter().any(|pixel| pixel[3] > 0));
    }
}
