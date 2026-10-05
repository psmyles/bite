//! `cargo xtask icons`: the Windows icons, drawn from `build/icons/icon.png` (the artwork: square,
//! full bleed, on transparency where the bite is taken out of it).
//!
//! * `build/icon.ico`, committed because every build reads it: both `build.rs` files embed it in
//!   their exe (Explorer, the taskbar and `.bite` files show it) and Inno Setup gives it to Setup.
//!   The artwork as it is, at each size Windows asks for.
//! * `crates/bite-gui/assets/icon-256.png`, the window icon `bite-gui`'s `build.rs` decodes and
//!   the editor hands winit (`icon.rs`). Committed for the same reason.
//! * **Setup's wizard:** the small image (top right) and the large one (the last page), drawn by
//!   `package-windows` into `target/package/wizard`.
//!
//! A test keeps the committed two current with the artwork, and `package-windows` redraws them
//! when they aren't. Scaling is Lanczos on premultiplied colour, so edges don't pick up the black
//! that the artwork's clear pixels hold.

use std::path::Path;

use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::{self, FilterType};
use image::{ExtendedColorType, ImageBuffer, ImageEncoder, ImageFormat, Rgba, RgbaImage};

use crate::util;

/// The artwork, relative to the workspace root.
const ARTWORK: &str = "build/icons/icon.png";
/// The Windows icon, relative to the workspace root.
pub const ICO: &str = "build/icon.ico";
/// The window icon, relative to the workspace root; `bite-gui`'s `build.rs` requires its edge.
pub const WINDOW_ICON: &str = "crates/bite-gui/assets/icon-256.png";
const WINDOW_ICON_EDGE: u32 = 256;
/// The .ico's sizes: the 16 px small icon at 100 % to 400 % display scale (which covers the 32 px
/// large icon to 200 %), and 256 px for Explorer's big views.
const ICO_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];
/// The panel behind the artwork on Setup's last page (sRGB): the dark fill of the Icon Composer
/// document (`build/icons/bite.icon`). `bite.iss`'s `WizardImageBackColor` is the same colour.
pub const BACKDROP: [u8; 3] = [0x21, 0x21, 0x21];

/// The artwork, premultiplied, in 0..1.
pub struct Artwork(ImageBuffer<Rgba<f32>, Vec<f32>>);

impl Artwork {
    pub fn load() -> Result<Artwork, String> {
        let path = util::root().join(ARTWORK);
        let img = image::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Artwork::new(&img.to_rgba8()))
    }

    fn new(img: &RgbaImage) -> Artwork {
        let mut out = ImageBuffer::new(img.width(), img.height());
        for (o, p) in out.pixels_mut().zip(img.pixels()) {
            let a = f32::from(p[3]) / 255.0;
            let c = |v: u8| f32::from(v) / 255.0 * a;
            *o = Rgba([c(p[0]), c(p[1]), c(p[2]), a]);
        }
        Artwork(out)
    }

    /// A `width` x `height` image with the artwork fitted into a `fit` px square at its centre,
    /// over `back` (transparent without one).
    pub fn draw(&self, width: u32, height: u32, fit: u32, back: Option<[u8; 3]>) -> RgbaImage {
        let (aw, ah) = self.0.dimensions();
        let scale = fit as f32 / aw.max(ah) as f32;
        let sw = ((aw as f32 * scale).round() as u32).max(1);
        let sh = ((ah as f32 * scale).round() as u32).max(1);
        let art = imageops::resize(&self.0, sw, sh, FilterType::Lanczos3);
        let ox = width.saturating_sub(sw) / 2;
        let oy = height.saturating_sub(sh) / 2;
        let back = back.map(|b| b.map(|v| f32::from(v) / 255.0));
        RgbaImage::from_fn(width, height, |x, y| {
            let src = if x >= ox && y >= oy && x - ox < sw && y - oy < sh {
                let p = art.get_pixel(x - ox, y - oy);
                // Lanczos rings past the range; keep each colour within its alpha.
                let a = p[3].clamp(0.0, 1.0);
                [
                    p[0].clamp(0.0, a),
                    p[1].clamp(0.0, a),
                    p[2].clamp(0.0, a),
                    a,
                ]
            } else {
                [0.0; 4]
            };
            let [r, g, b, a] = match back {
                Some([br, bg, bb]) => {
                    let k = 1.0 - src[3];
                    [src[0] + br * k, src[1] + bg * k, src[2] + bb * k, 1.0]
                }
                None => src,
            };
            let byte = |v: f32| (v * 255.0).round() as u8;
            if a <= 0.0 {
                Rgba([0, 0, 0, 0])
            } else {
                Rgba([byte(r / a), byte(g / a), byte(b / a), byte(a)])
            }
        })
    }

    /// The artwork alone, edge to edge (it carries its own margin).
    fn square(&self, size: u32) -> RgbaImage {
        self.draw(size, size, size, None)
    }
}

pub fn run_icons(args: &[String]) -> Result<(), String> {
    let art = Artwork::load()?;
    let redrawn = if util::flag(args, "--force") {
        write_all(&art)?;
        vec![ICO, WINDOW_ICON]
    } else {
        refresh(&art)?
    };
    for path in [ICO, WINDOW_ICON] {
        let state = if redrawn.contains(&path) {
            "redrawn"
        } else {
            "is current"
        };
        println!("{path} {state}");
    }
    if let Some(dir) = util::value(args, "--preview") {
        let dir = Path::new(dir).join("wizard");
        write_wizard_images(&art, &dir)?;
        println!("Setup's wizard images are in {}", dir.display());
    }
    Ok(())
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

fn create_dir(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))
}

fn write_all(art: &Artwork) -> Result<(), String> {
    let root = util::root();
    write(&root.join(ICO), &ico(art))?;
    write(&root.join(WINDOW_ICON), &png(&window_icon(art)))
}

/// Redraw whichever of [`ICO`] and [`WINDOW_ICON`] no longer match the artwork; returns those.
pub fn refresh(art: &Artwork) -> Result<Vec<&'static str>, String> {
    let root = util::root();
    let mut redrawn = Vec::new();
    if !ico_is_current(art) {
        write(&root.join(ICO), &ico(art))?;
        redrawn.push(ICO);
    }
    if !window_icon_is_current(art) {
        write(&root.join(WINDOW_ICON), &png(&window_icon(art)))?;
        redrawn.push(WINDOW_ICON);
    }
    Ok(redrawn)
}

/// Whether the committed .ico holds what the artwork draws, give or take rounding: scaling isn't
/// bit-exact across platforms' maths libraries, so the bytes may differ where the pixels don't.
fn ico_is_current(art: &Artwork) -> bool {
    let Some(have) = std::fs::read(util::root().join(ICO))
        .ok()
        .and_then(|b| read_ico(&b))
    else {
        return false;
    };
    let want: Vec<RgbaImage> = ICO_SIZES.iter().map(|&s| art.square(s)).collect();
    have.len() == want.len() && have.iter().zip(&want).all(|(h, w)| alike(h, w))
}

fn window_icon(art: &Artwork) -> RgbaImage {
    art.square(WINDOW_ICON_EDGE)
}

fn window_icon_is_current(art: &Artwork) -> bool {
    image::open(util::root().join(WINDOW_ICON))
        .map(|have| alike(&have.to_rgba8(), &window_icon(art)))
        .unwrap_or(false)
}

/// Two images of one size whose premultiplied pixels are within 2 of each other (straight colour
/// under a nearly clear pixel can differ by anything).
fn alike(a: &RgbaImage, b: &RgbaImage) -> bool {
    let pre = |p: &Rgba<u8>| {
        let alpha = u32::from(p[3]);
        [0, 1, 2].map(|i| (u32::from(p[i]) * alpha / 255) as u8)
    };
    a.dimensions() == b.dimensions()
        && a.pixels().zip(b.pixels()).all(|(p, q)| {
            p[3].abs_diff(q[3]) <= 2 && pre(p).iter().zip(pre(q)).all(|(x, y)| x.abs_diff(y) <= 2)
        })
}

/// The Windows icon file.
fn ico(art: &Artwork) -> Vec<u8> {
    let images: Vec<RgbaImage> = ICO_SIZES.iter().map(|&s| art.square(s)).collect();
    ico_file(&images)
}

/// An .ico holding `images`: 256 px as PNG, as Windows stores it, smaller ones as 32-bit DIBs,
/// which every reader takes.
fn ico_file(images: &[RgbaImage]) -> Vec<u8> {
    let blobs: Vec<Vec<u8>> = images
        .iter()
        .map(|i| if i.width() >= 256 { png(i) } else { dib(i) })
        .collect();
    let count = u16::try_from(images.len()).expect("an .ico holds at most 65535 images");
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // 1: icons
    out.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * images.len();
    for (img, blob) in images.iter().zip(&blobs) {
        // 256 is written as 0.
        out.push((img.width() % 256) as u8);
        out.push((img.height() % 256) as u8);
        out.push(0); // no palette
        out.push(0); // reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += blob.len();
    }
    for blob in blobs {
        out.extend_from_slice(&blob);
    }
    out
}

/// A 32-bit DIB as an .ico stores it: a BITMAPINFOHEADER whose height counts the 1-bit AND mask
/// too, the BGRA rows bottom-up, then the mask (set where the pixel is clear).
fn dib(img: &RgbaImage) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let mask_row = w.div_ceil(32) as usize * 4;
    let size = (w * h * 4) as usize + mask_row * h as usize;
    let mut out = Vec::with_capacity(40 + size);
    out.extend_from_slice(&40u32.to_le_bytes()); // header size
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(2 * h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]); // resolution, palette
    for y in (0..h).rev() {
        for x in 0..w {
            let [r, g, b, a] = img.get_pixel(x, y).0;
            out.extend_from_slice(&[b, g, r, a]);
        }
    }
    for y in (0..h).rev() {
        let mut row = vec![0u8; mask_row];
        for x in 0..w {
            if img.get_pixel(x, y)[3] == 0 {
                row[x as usize / 8] |= 0x80 >> (x % 8);
            }
        }
        out.extend_from_slice(&row);
    }
    out
}

/// The images in an .ico that [`ico_file`] wrote, or None for anything else.
fn read_ico(bytes: &[u8]) -> Option<Vec<RgbaImage>> {
    let u16_at = |o: usize| Some(u16::from_le_bytes(bytes.get(o..o + 2)?.try_into().ok()?));
    let u32_at = |o: usize| Some(u32::from_le_bytes(bytes.get(o..o + 4)?.try_into().ok()?));
    if u16_at(0)? != 0 || u16_at(2)? != 1 {
        return None;
    }
    (0..usize::from(u16_at(4)?))
        .map(|i| {
            let entry = 6 + 16 * i;
            let len = u32_at(entry + 8)? as usize;
            let offset = u32_at(entry + 12)? as usize;
            let blob = bytes.get(offset..offset + len)?;
            if blob.starts_with(b"\x89PNG") {
                let img = image::load_from_memory_with_format(blob, ImageFormat::Png).ok()?;
                return Some(img.to_rgba8());
            }
            let w = u32::from_le_bytes(blob.get(4..8)?.try_into().ok()?);
            let h = u32::from_le_bytes(blob.get(8..12)?.try_into().ok()?) / 2;
            let px = blob.get(40..40 + (w * h * 4) as usize)?;
            Some(RgbaImage::from_fn(w, h, |x, y| {
                let o = (((h - 1 - y) * w + x) * 4) as usize;
                Rgba([px[o + 2], px[o + 1], px[o], px[o + 3]])
            }))
        })
        .collect()
}

pub fn png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Best, PngFilter::Adaptive)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ExtendedColorType::Rgba8,
        )
        .expect("a PNG encodes into memory");
    out
}

/// Setup's wizard images, drawn into `dir`, returned as the comma-separated lists Inno Setup takes
/// (it picks the file that suits the display scale): the large image (the last page) the artwork
/// on the [`BACKDROP`] panel, the small one (top right of the other pages) the artwork alone.
pub fn write_wizard_images(art: &Artwork, dir: &Path) -> Result<(String, String), String> {
    // Inno Setup's sizes for 100 % to 250 % display scale.
    const LARGE: [(u32, u32); 7] = [
        (164, 314),
        (192, 386),
        (246, 459),
        (273, 556),
        (328, 604),
        (355, 700),
        (410, 797),
    ];
    const SMALL: [(u32, u32); 7] = [
        (55, 55),
        (64, 68),
        (83, 80),
        (92, 97),
        (110, 106),
        (119, 123),
        (138, 140),
    ];
    create_dir(dir)?;
    let mut lists = [Vec::new(), Vec::new()];
    for (list, (name, sizes, back, fit)) in lists.iter_mut().zip([
        ("large", LARGE, Some(BACKDROP), 0.7),
        ("small", SMALL, None, 1.0),
    ]) {
        for (w, h) in sizes {
            let path = dir.join(format!("{name}-{w}x{h}.png"));
            let fit = (w.min(h) as f32 * fit).round() as u32;
            write(&path, &png(&art.draw(w, h, fit, back)))?;
            list.push(path.display().to_string());
        }
    }
    let [large, small] = lists;
    Ok((large.join(","), small.join(",")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 40 px artwork: an opaque white disc on clear black.
    fn disc() -> Artwork {
        Artwork::new(&RgbaImage::from_fn(40, 40, |x, y| {
            let (dx, dy) = (x as f32 - 19.5, y as f32 - 19.5);
            if dx * dx + dy * dy < 15.0 * 15.0 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        }))
    }

    #[test]
    fn the_committed_icons_are_current() {
        let art = Artwork::load().unwrap();
        assert!(
            ico_is_current(&art),
            "{ICO} doesn't match {ARTWORK}; run `cargo xtask icons`"
        );
        assert!(
            window_icon_is_current(&art),
            "{WINDOW_ICON} doesn't match {ARTWORK}; run `cargo xtask icons`"
        );
    }

    #[test]
    fn an_ico_reads_back_as_written() {
        let art = disc();
        let images = vec![art.square(16), art.square(20), art.square(256)];
        let bytes = ico_file(&images);
        // 256 is stored as 0, and as a PNG.
        assert_eq!(bytes[6 + 32], 0);
        let back = read_ico(&bytes).unwrap();
        assert_eq!(back, images);
    }

    #[test]
    fn edges_take_no_colour_from_clear_pixels() {
        for p in disc().square(16).pixels().filter(|p| p[3] > 0) {
            assert!(p[0] >= 250 && p[1] >= 250 && p[2] >= 250, "{p:?}");
        }
    }

    #[test]
    fn the_large_wizard_image_is_opaque_on_the_backdrop() {
        let img = disc().draw(164, 314, 115, Some(BACKDROP));
        assert!(img.pixels().all(|p| p[3] == 255));
        let back = Rgba([BACKDROP[0], BACKDROP[1], BACKDROP[2], 255]);
        assert_eq!(*img.get_pixel(0, 0), back);
        assert_eq!(*img.get_pixel(82, 157), Rgba([255, 255, 255, 255]));
    }
}
