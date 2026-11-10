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
//! - **The thumbnails are WebP at quality 85** (FD12), encoded by libwebp with ImageMagick's own
//!   settings for `-quality 85`, and with the source's ICC profile embedded as ImageMagick's
//!   `-thumbnail` keeps it. The preview chain that runs over them reads what it always has, and the
//!   cache stays the size it was: lossless files were three to seven times larger.
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

    // An opaque picture is written without alpha. ImageMagick's lossy WebP dropped an alpha
    // plane that held nothing too, so the preview chain sees the channels it always saw. The
    // thumbnail itself is what is scanned: fire-decode's `alpha_opaque` only describes a source
    // that has an alpha channel, and is false for one with none.
    let (pixels, _) = image.pixels.as_chunks::<4>();
    let mut encoded = if pixels.iter().all(|pixel| pixel[3] == u8::MAX) {
        let rgb: Vec<u8> = pixels.iter().flat_map(|&[r, g, b, _]| [r, g, b]).collect();
        encode_lossy(&rgb, image.width, image.height, false)?
    } else {
        encode_lossy(&image.pixels, image.width, image.height, true)?
    };
    if let Some(icc) = image.icc.filter(|icc| !icc.is_empty()) {
        encoded = with_icc(&encoded, &icc, image.width, image.height)?;
    }
    write_whole(out, &encoded)?;
    Ok((width, height, name))
}

/// The quality bite's thumbnails have always been written at (`-quality 85`).
const QUALITY: f32 = 85.0;

/// Encodes RGB (`alpha` false) or RGBA pixels as a lossy WebP at [`QUALITY`], through libwebp's
/// simple API: its default preset, which is what ImageMagick's WebP writer starts from too.
fn encode_lossy(pixels: &[u8], width: u32, height: u32, alpha: bool) -> Result<Vec<u8>, String> {
    let channels: usize = if alpha { 4 } else { 3 };
    let too_large = || "the thumbnail is too large to encode".to_string();
    let stride = (width as usize)
        .checked_mul(channels)
        .filter(|stride| stride.checked_mul(height as usize) == Some(pixels.len()))
        .ok_or_else(|| "the thumbnail's buffer does not match its size".to_string())?;
    let (width, height, stride) = (
        i32::try_from(width).map_err(|_| too_large())?,
        i32::try_from(height).map_err(|_| too_large())?,
        i32::try_from(stride).map_err(|_| too_large())?,
    );
    let mut output: *mut u8 = std::ptr::null_mut();
    // SAFETY: `pixels` holds exactly `height` rows of `stride` bytes (checked above) and outlives
    // the call, which only reads it. libwebp allocates the output itself and stores it in
    // `output`; it is copied out below and freed with libwebp's own `WebPFree`.
    let size = unsafe {
        if alpha {
            libwebp_sys::WebPEncodeRGBA(
                pixels.as_ptr(),
                width,
                height,
                stride,
                QUALITY,
                &mut output,
            )
        } else {
            libwebp_sys::WebPEncodeRGB(pixels.as_ptr(), width, height, stride, QUALITY, &mut output)
        }
    };
    if output.is_null() {
        return Err("libwebp could not encode the thumbnail".into());
    }
    // SAFETY: libwebp returned `output` with `size` bytes written to it, and it is freed only
    // after they are copied.
    let encoded = unsafe { std::slice::from_raw_parts(output, size) }.to_vec();
    // SAFETY: `output` came from libwebp's allocator and is freed exactly once.
    unsafe { libwebp_sys::WebPFree(output.cast()) };
    if size == 0 {
        return Err("libwebp could not encode the thumbnail".into());
    }
    Ok(encoded)
}

/// Adds an ICC profile to a WebP from libwebp's simple encoder, which writes none: the extended
/// format's `VP8X` header, then the `ICCP` chunk, then the image's own chunks, in the order the
/// WebP container specification gives. libwebp writes a `VP8X` of its own when there is alpha,
/// and its flags are kept.
fn with_icc(webp: &[u8], icc: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    const ICC_FLAG: u8 = 0x20;
    let malformed = || "libwebp wrote a WebP this cannot add a profile to".to_string();
    if webp.len() < 12 || &webp[..4] != b"RIFF" || &webp[8..12] != b"WEBP" {
        return Err(malformed());
    }
    let mut chunks = &webp[12..];
    let mut flags = ICC_FLAG;
    if chunks.starts_with(b"VP8X") {
        // A 10-byte payload: the flags, three reserved bytes and the canvas size.
        flags |= *chunks.get(8).ok_or_else(malformed)?;
        chunks = chunks.get(18..).ok_or_else(malformed)?;
    }
    let chunk = |fourcc: &[u8], payload: &[u8], out: &mut Vec<u8>| -> Result<(), String> {
        let size = u32::try_from(payload.len()).map_err(|_| malformed())?;
        out.extend_from_slice(fourcc);
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            out.push(0);
        }
        Ok(())
    };
    let mut vp8x = vec![flags, 0, 0, 0];
    vp8x.extend_from_slice(&(width - 1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&(height - 1).to_le_bytes()[..3]);
    let mut body = b"WEBP".to_vec();
    chunk(b"VP8X", &vp8x, &mut body)?;
    chunk(b"ICCP", icc, &mut body)?;
    body.extend_from_slice(chunks);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(
        &u32::try_from(body.len())
            .map_err(|_| malformed())?
            .to_le_bytes(),
    );
    out.extend_from_slice(&body);
    Ok(out)
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
    fn a_thumbnail_is_a_lossy_webp_at_most_size_on_its_longer_side() {
        let root = scratch("thumb");
        let source = root.join("wide.png");
        fs::write(&source, png_rgb(400, 100, |x, y| [x as u8, y as u8, 7])).unwrap();
        let out = root.join("thumb.webp");
        let measured = thumbnail(&source, 64, &out, &AtomicBool::new(false), None).unwrap();
        assert_eq!(measured, (400, 100, "PNG"));
        let mut decoder =
            image_webp::WebPDecoder::new(io::Cursor::new(fs::read(&out).unwrap())).unwrap();
        assert_eq!(decoder.dimensions(), (64, 16));
        assert!(decoder.is_lossy());
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

    /// With alpha, libwebp writes a `VP8X` header of its own; the profile joins it rather than
    /// replacing its alpha flag.
    #[test]
    fn a_transparent_tagged_source_keeps_both_its_alpha_and_its_profile() {
        let root = scratch("alpha-icc");
        let source = root.join("tagged-cut-out.png");
        let icc = b"an odd number of bytes, padded in the chunk".to_vec();
        assert_eq!(icc.len() % 2, 1);
        let mut encoded = Vec::new();
        let mut info = ::png::Info::with_size(8, 8);
        info.color_type = ::png::ColorType::Rgba;
        info.bit_depth = ::png::BitDepth::Eight;
        info.icc_profile = Some(icc.clone().into());
        let encoder = ::png::Encoder::with_info(&mut encoded, info).unwrap();
        let mut writer = encoder.write_header().unwrap();
        let pixels: Vec<u8> = (0..64u8).flat_map(|i| [i * 3, 90, 200, i * 4]).collect();
        writer.write_image_data(&pixels).unwrap();
        writer.finish().unwrap();
        fs::write(&source, encoded).unwrap();
        let out = root.join("thumb.webp");
        thumbnail(&source, 8, &out, &AtomicBool::new(false), None).unwrap();
        let mut decoder =
            image_webp::WebPDecoder::new(io::Cursor::new(fs::read(&out).unwrap())).unwrap();
        assert!(decoder.has_alpha());
        assert_eq!(decoder.icc_profile().unwrap(), Some(icc));
        let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
        decoder.read_image(&mut pixels).unwrap();
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
