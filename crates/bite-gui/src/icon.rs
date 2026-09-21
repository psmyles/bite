//! The window and taskbar icon.
//!
//! `assets/icon-256.png` is generated from the one master, `build/icons/icon.png`, by
//! `scripts/generate-icons.ps1`. The executable's own icon - what Explorer draws - is the
//! multi-resolution `build/icon.ico` embedded by `build.rs`; this is the runtime icon winit
//! hands the window manager, which wants raw RGBA.

/// The icon artwork, decoded once when the window is created.
const ICON_PNG: &[u8] = include_bytes!("../assets/icon-256.png");

/// Decodes the embedded icon for winit. A failure is not worth aborting the launch over: the
/// window simply keeps the system default, so this returns `None` rather than an error.
pub fn window_icon() -> Option<winit::window::Icon> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(ICON_PNG));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buffer).ok()?;
    let (width, height, color) = (frame.width, frame.height, frame.color_type);
    let pixels = match color {
        png::ColorType::Rgba => buffer,
        // The master carries an alpha channel, so this is only a safety net for a re-exported
        // opaque PNG: pad each pixel out to RGBA.
        png::ColorType::Rgb => buffer
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 0xFF])
            .collect(),
        _ => return None,
    };
    winit::window::Icon::from_rgba(pixels, width, height).ok()
}
