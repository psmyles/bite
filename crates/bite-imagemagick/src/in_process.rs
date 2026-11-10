//! What bite reads without starting ImageMagick, through fire-decode (fire-decode plan, step 5):
//! every file's size and format from its header, and the thumbnails of the formats routed to it.
//!
//! - **Routing is by extension** (FD9): PNG, TGA, PSD, EXR, HEIC, HEIF and AVIF are thumbnailed
//!   here; everything else stays on ImageMagick. JPEG stays because ImageMagick decodes it at a
//!   reduced DCT scale (`jpeg:size`), which fire-decode does not. Camera raw stays because
//!   ImageMagick develops it, where fire-decode would read the camera's embedded preview.
//!   `.psb` (which fire-decode does not read) and Radiance `.hdr` stay too, though the format chips
//!   group them with PSD and EXR.
//! - **A file fire-decode cannot thumbnail goes to ImageMagick** (FD10), whatever the error,
//!   `TooLarge` included. ImageMagick runs in a process of its own, so a file it cannot handle
//!   cannot take bite down, and it has no size limits.
//! - **The thumbnails are lossless WebP** (FD12), with the source's ICC profile embedded as
//!   ImageMagick's `-thumbnail` keeps it, so the preview chain that runs over them reads what it
//!   always has.
//!
//! The pixels match ImageMagick's own thumbnails where it matters for parity: no ICC conversion,
//! no EXIF rotation, HEIF's `irot` / `imir` applied, a float source clamped without a curve, and
//! an area average of stored values (fire-decode's `thumbnail()` setup).
use std::{
    fs, io,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use fire_decode::{DecodeCtl, DecodeOptions, Format};

/// The extensions thumbnailed in process (FD9), lower case.
const ROUTED: &[&str] = &["png", "tga", "psd", "exr", "heic", "heif", "avif"];

/// Whether `path` is thumbnailed here rather than by ImageMagick.
pub fn is_routed(path: &Path) -> bool {
    extension(path).is_some_and(|ext| ROUTED.contains(&ext.as_str()))
}

/// A file's size and format from its header, without decoding it: what `identify -format
/// "%w %h %m"` would print. `None` for a file fire-decode does not read, which ImageMagick
/// measures instead (FD19).
///
/// The size is the stored one, as `identify` reports it: no EXIF rotation, but a HEIF's own
/// `irot` applied, since that is part of the image.
pub fn probe(path: &Path) -> Option<(u32, u32, &'static str)> {
    let ext = extension(path);
    let file = fs::File::open(path).ok()?;
    let probe = fire_decode::probe(io::BufReader::new(file), ext.as_deref()).ok()?;
    let name = magick_name(probe.format, ext.as_deref())?;
    (probe.width > 0 && probe.height > 0).then_some((probe.width, probe.height, name))
}

/// Writes `source`'s thumbnail, at most `size` on its longer side, to `out` as a lossless WebP.
/// Returns the source's size and format, as [`probe`] does. `cancelled` stops a long decode
/// (a large HEIC) part way through.
///
/// `threads` caps the decode, the calling thread included (`None`: as many as fire-decode's
/// budget has free). Several decodes at once must each be capped to their share: fire-decode
/// hands a decoder whatever threads are free when it starts, for the whole decode, so the first
/// of three HEICs took nearly every core and the other two ran on one each, three times slower
/// than decoding them one after another.
pub fn thumbnail(
    source: &Path,
    size: u32,
    out: &Path,
    cancelled: &AtomicBool,
    threads: Option<usize>,
) -> Result<(u32, u32, &'static str), String> {
    let bytes = fs::read(source).map_err(|error| error.to_string())?;
    let ext = extension(source);
    let cancel = || cancelled.load(Ordering::Relaxed);
    let ctl = DecodeCtl {
        progress: &|_| {},
        cancel: &cancel,
    };
    let mut options = DecodeOptions::thumbnail(size);
    options.max_threads = threads;
    let image = fire_decode::decode_with_progress(&bytes, ext.as_deref(), &options, &ctl)
        .map_err(|error| error.to_string())?;
    let (width, height) = image.downscaled_from.unwrap_or((image.width, image.height));
    let name = magick_name(image.source.format, ext.as_deref()).unwrap_or("UNKNOWN");

    let mut encoded = Vec::new();
    let mut encoder = image_webp::WebPEncoder::new(&mut encoded);
    if let Some(icc) = image.icc {
        encoder.set_icc_profile(icc);
    }
    // An opaque picture is written without alpha. ImageMagick's lossy WebP dropped an alpha
    // plane that held nothing too, so the preview chain sees the channels it always saw. The
    // thumbnail itself is what is scanned: fire-decode's `alpha_opaque` only describes a source
    // that has an alpha channel, and is false for one with none.
    let (pixels, _) = image.pixels.as_chunks::<4>();
    let written = if pixels.iter().all(|pixel| pixel[3] == u8::MAX) {
        let rgb: Vec<u8> = pixels.iter().flat_map(|&[r, g, b, _]| [r, g, b]).collect();
        encoder.encode(&rgb, image.width, image.height, image_webp::ColorType::Rgb8)
    } else {
        encoder.encode(
            &image.pixels,
            image.width,
            image.height,
            image_webp::ColorType::Rgba8,
        )
    };
    written.map_err(|error| error.to_string())?;
    write_whole(out, &encoded)?;
    Ok((width, height, name))
}

/// Writes `bytes` to `path` all at once: into a file of its own beside it, then renamed over it.
/// A thumbnail half written by a cancelled import would otherwise read as a valid cache entry,
/// and the preview worker may be writing the same thumbnail at the same moment.
fn write_whole(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let partial = path.with_extension(format!(
        "{}-{:?}.partial",
        std::process::id(),
        std::thread::current().id()
    ));
    let written = fs::write(&partial, bytes).and_then(|()| fs::rename(&partial, path));
    if written.is_err() {
        let _ = fs::remove_file(&partial);
    }
    written.map_err(|error| error.to_string())
}

/// The name `identify`'s `%m` gives a format, so a file reads the same in the file list whether
/// it was measured here or by ImageMagick. A `.jpg` is `JPEG`.
fn magick_name(format: Format, ext: Option<&str>) -> Option<&'static str> {
    Some(match format {
        Format::Png => "PNG",
        Format::Jpeg => "JPEG",
        Format::Gif => "GIF",
        Format::Bmp => "BMP",
        Format::Tiff => "TIFF",
        Format::Tga => "TGA",
        Format::Ico => "ICO",
        Format::Cur => "CUR",
        Format::WebP => "WEBP",
        Format::Qoi => "QOI",
        Format::Pnm => match ext {
            Some("pgm") => "PGM",
            Some("pbm") => "PBM",
            Some("pam") => "PAM",
            Some("pfm") => "PFM",
            Some("pnm") => "PNM",
            _ => "PPM",
        },
        Format::Farbfeld => "FF",
        Format::JpegXl => "JXL",
        Format::RadianceHdr => "HDR",
        Format::Exr => "EXR",
        Format::Dds => "DDS",
        Format::Psd => "PSD",
        Format::Heic => "HEIC",
        Format::Heif => "HEIF",
        Format::Avif => "AVIF",
        _ => return None,
    })
}

/// The extension, lower case.
fn extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("bite-in-process-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    /// An RGBA PNG of `width`×`height`.
    fn png(width: u32, height: u32, rgba: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                pixels.extend(rgba(x, y));
            }
        }
        let mut encoded = Vec::new();
        let mut encoder = ::png::Encoder::new(&mut encoded, width, height);
        encoder.set_color(::png::ColorType::Rgba);
        encoder.set_depth(::png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
        writer.finish().unwrap();
        encoded
    }

    /// An RGB PNG, with no alpha channel at all.
    fn png_rgb(width: u32, height: u32, rgb: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);
        for y in 0..height {
            for x in 0..width {
                pixels.extend(rgb(x, y));
            }
        }
        let mut encoded = Vec::new();
        let mut encoder = ::png::Encoder::new(&mut encoded, width, height);
        encoder.set_color(::png::ColorType::Rgb);
        encoder.set_depth(::png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&pixels).unwrap();
        writer.finish().unwrap();
        encoded
    }

    /// A tiny ICC payload: fire-decode hands the bytes back whether or not it applies them, and
    /// this build applies none.
    fn png_with_icc(icc: &[u8]) -> Vec<u8> {
        let mut encoded = Vec::new();
        let mut info = ::png::Info::with_size(2, 2);
        info.color_type = ::png::ColorType::Rgb;
        info.bit_depth = ::png::BitDepth::Eight;
        info.icc_profile = Some(icc.to_vec().into());
        let encoder = ::png::Encoder::with_info(&mut encoded, info).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[9; 12]).unwrap();
        writer.finish().unwrap();
        encoded
    }

    #[test]
    fn routing_is_by_extension_in_any_case() {
        for routed in [
            "a.png", "a.TGA", "a.psd", "a.exr", "a.HEIC", "a.heif", "a.avif",
        ] {
            assert!(is_routed(Path::new(routed)), "{routed}");
        }
        for magick in [
            "a.jpg", "a.jpeg", "a.psb", "a.hdr", "a.nef", "a.svg", "a.webp", "a",
        ] {
            assert!(!is_routed(Path::new(magick)), "{magick}");
        }
    }

    #[test]
    fn a_thumbnail_is_a_lossless_webp_at_most_size_on_its_longer_side() {
        let root = scratch("thumb");
        let source = root.join("wide.png");
        fs::write(&source, png_rgb(400, 100, |x, y| [x as u8, y as u8, 7])).unwrap();
        let out = root.join("thumb.webp");
        let measured = thumbnail(&source, 64, &out, &AtomicBool::new(false), None).unwrap();
        assert_eq!(measured, (400, 100, "PNG"));
        let mut decoder =
            image_webp::WebPDecoder::new(io::Cursor::new(fs::read(&out).unwrap())).unwrap();
        assert_eq!(decoder.dimensions(), (64, 16));
        assert!(!decoder.is_lossy());
        // No alpha channel in the source, so no alpha plane, as ImageMagick's had none.
        assert!(!decoder.has_alpha());
        let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
        decoder.read_image(&mut pixels).unwrap();
        // Every partial file is gone.
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    /// An alpha channel that holds nothing is dropped as well: an RGBA PNG screenshot.
    #[test]
    fn an_opaque_alpha_channel_is_not_written() {
        let root = scratch("opaque");
        let source = root.join("screenshot.png");
        fs::write(&source, png(32, 32, |x, y| [x as u8, y as u8, 9, 255])).unwrap();
        let out = root.join("thumb.webp");
        thumbnail(&source, 16, &out, &AtomicBool::new(false), None).unwrap();
        let decoder =
            image_webp::WebPDecoder::new(io::Cursor::new(fs::read(&out).unwrap())).unwrap();
        assert!(!decoder.has_alpha());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_transparent_source_keeps_its_alpha() {
        let root = scratch("alpha");
        let source = root.join("cut-out.png");
        fs::write(
            &source,
            png(32, 32, |x, _| [200, 10, 10, if x < 16 { 0 } else { 255 }]),
        )
        .unwrap();
        let out = root.join("thumb.webp");
        thumbnail(&source, 16, &out, &AtomicBool::new(false), None).unwrap();
        let decoder =
            image_webp::WebPDecoder::new(io::Cursor::new(fs::read(&out).unwrap())).unwrap();
        assert!(decoder.has_alpha());
        fs::remove_dir_all(root).unwrap();
    }

    /// ImageMagick's `-thumbnail` keeps the colour profile, and the preview chain runs over the
    /// thumbnail, so the profile has to be in it (FD12).
    #[test]
    fn the_source_profile_is_embedded() {
        let root = scratch("icc");
        let source = root.join("tagged.png");
        let icc = b"not really a profile, but carried as bytes".to_vec();
        fs::write(&source, png_with_icc(&icc)).unwrap();
        let out = root.join("thumb.webp");
        thumbnail(&source, 64, &out, &AtomicBool::new(false), None).unwrap();
        let mut decoder =
            image_webp::WebPDecoder::new(io::Cursor::new(fs::read(&out).unwrap())).unwrap();
        assert_eq!(decoder.icc_profile().unwrap(), Some(icc));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_cancelled_thumbnail_writes_nothing() {
        let root = scratch("cancel");
        let source = root.join("a.png");
        fs::write(&source, png(8, 8, |_, _| [1, 2, 3, 255])).unwrap();
        let out = root.join("thumb.webp");
        assert!(thumbnail(&source, 4, &out, &AtomicBool::new(true), None).is_err());
        assert!(!out.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_file_that_is_not_a_picture_is_left_to_imagemagick() {
        let root = scratch("broken");
        let source = root.join("broken.png");
        fs::write(&source, b"this is not a picture").unwrap();
        assert_eq!(probe(&source), None);
        assert!(thumbnail(
            &source,
            64,
            &root.join("t.webp"),
            &AtomicBool::new(false),
            None
        )
        .is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn probe_names_formats_the_way_identify_does() {
        assert_eq!(magick_name(Format::Jpeg, Some("jpg")), Some("JPEG"));
        assert_eq!(magick_name(Format::Heif, Some("heif")), Some("HEIF"));
        assert_eq!(magick_name(Format::Pnm, Some("pgm")), Some("PGM"));
        assert_eq!(magick_name(Format::Farbfeld, Some("ff")), Some("FF"));
        assert_eq!(magick_name(Format::Unknown, None), None);
    }
}
