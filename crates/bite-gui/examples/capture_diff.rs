//! Compares two `--capture` directories, image by image.
//!
//! The sokol migration changes the ImGui version in one phase and the renderer in another, each
//! checked against the capture taken before it (`docs/sokol-migration-plan.md`). "Checked" has to
//! mean something more precise than looking at two folders, because the differences that matter
//! are a pixel of glyph hinting here and one least-significant bit of blend rounding there - and
//! the two must not be confused.
//!
//! So this reports, per scene: how many pixels differ at all, the largest per-channel difference,
//! and where the worst one is. A run where every scene is identical is the strong result; a run
//! where every difference is 1 LSB is the acceptable one; anything else names the scene to open.
//!
//! A report is not always enough to judge a difference, so a second form magnifies one region of
//! one scene into a before-and-after strip that can be looked at:
//!
//! ```text
//! cargo run --release -p bite-gui --example capture_diff -- <before-dir> <after-dir>
//! cargo run --release -p bite-gui --example capture_diff -- <before-dir> <after-dir> \
//!     <scene.png> <x> <y> <w> <h> <zoom> <out.png>
//! ```

use std::path::{Path, PathBuf};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let (Some(before), Some(after)) = (arguments.first(), arguments.get(1)) else {
        eprintln!("usage: capture_diff <before-dir> <after-dir> [<scene.png> <x> <y> <w> <h> <zoom> <out.png>]");
        std::process::exit(2);
    };
    if arguments.len() >= 9 {
        let number = |index: usize| -> u32 { arguments[index].parse().unwrap_or(0) };
        if let Err(error) = compare_region(
            &Path::new(before).join(&arguments[2]),
            &Path::new(after).join(&arguments[2]),
            (number(3), number(4), number(5), number(6)),
            number(7).max(1),
            Path::new(&arguments[8]),
        ) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    match run(Path::new(before), Path::new(after)) {
        Ok(worst) => {
            if worst == 0 {
                println!("\nidentical");
            } else {
                println!("\nlargest per-channel difference anywhere: {worst}");
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

/// Compares every PNG in `before` against its namesake in `after`, and answers with the largest
/// per-channel difference seen anywhere - 0 when the two directories are identical.
fn run(before: &Path, after: &Path) -> Result<u8, String> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(before)
        .map_err(|error| format!("cannot read {}: {error}", before.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("png"))
        .collect();
    names.sort();
    if names.is_empty() {
        return Err(format!("{} holds no PNGs", before.display()));
    }

    let mut worst_anywhere = 0u8;
    let mut identical = 0usize;
    let mut missing = 0usize;
    for path in &names {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let other = after.join(path.file_name().unwrap_or_default());
        if !other.exists() {
            println!("{name:<28} MISSING in {}", after.display());
            missing += 1;
            continue;
        }
        // A byte-identical file is identical without decoding either side, which is the common
        // case and the one worth being fast.
        if std::fs::read(path).ok() == std::fs::read(&other).ok() {
            identical += 1;
            continue;
        }
        let a = decode(path)?;
        let b = decode(&other)?;
        if a.1 != b.1 || a.2 != b.2 {
            println!("{name:<28} SIZE {}x{} vs {}x{}", a.1, a.2, b.1, b.2);
            worst_anywhere = u8::MAX;
            continue;
        }
        let mut differing = 0usize;
        let mut worst = 0u8;
        let mut worst_at = (0u32, 0u32);
        let (left_pixels, _) = a.0.as_chunks::<4>();
        let (right_pixels, _) = b.0.as_chunks::<4>();
        for (index, (left, right)) in left_pixels.iter().zip(right_pixels).enumerate() {
            if left == right {
                continue;
            }
            differing += 1;
            let delta = left
                .iter()
                .zip(right)
                .map(|(l, r)| l.abs_diff(*r))
                .max()
                .unwrap_or(0);
            if delta > worst {
                worst = delta;
                worst_at = ((index as u32) % a.1, (index as u32) / a.1);
            }
        }
        if differing == 0 {
            // Different bytes on disk, same pixels: a re-encode, which is not a difference.
            identical += 1;
            continue;
        }
        let total = (a.1 as usize) * (a.2 as usize);
        println!(
            "{name:<28} {differing:>8} px ({:>6.3}%)  worst delta {worst:>3} at {},{}",
            differing as f64 * 100.0 / total as f64,
            worst_at.0,
            worst_at.1,
        );
        worst_anywhere = worst_anywhere.max(worst);
    }
    println!(
        "\n{} scenes: {identical} identical, {} differing, {missing} missing",
        names.len(),
        names.len() - identical - missing
    );
    Ok(worst_anywhere)
}

/// Decodes a PNG to RGBA8, with its width and height.
fn decode(path: &Path) -> Result<(Vec<u8>, u32, u32), String> {
    let file = std::fs::File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::ALPHA | png::Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let mut buffer = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    buffer.truncate(info.buffer_size());
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "{}: expected RGBA8 after expansion, got {:?}/{:?}",
            path.display(),
            info.color_type,
            info.bit_depth
        ));
    }
    Ok((buffer, info.width, info.height))
}

/// Writes a magnified before-and-after strip of one region of one scene, side by side with a gap
/// between them, so a difference the report only counts can be looked at.
fn compare_region(
    before: &Path,
    after: &Path,
    region: (u32, u32, u32, u32),
    zoom: u32,
    out: &Path,
) -> Result<(), String> {
    let a = decode(before)?;
    let b = decode(after)?;
    let (x, y, w, h) = region;
    let gap = 8u32;
    let width = w * zoom * 2 + gap;
    let height = h * zoom;
    let mut canvas = vec![0u8; (width * height * 4) as usize];
    for (panel, source) in [&a, &b].into_iter().enumerate() {
        let offset = panel as u32 * (w * zoom + gap);
        for row in 0..h * zoom {
            for column in 0..w * zoom {
                let sx = x + column / zoom;
                let sy = y + row / zoom;
                if sx >= source.1 || sy >= source.2 {
                    continue;
                }
                let from = ((sy * source.1 + sx) * 4) as usize;
                let to = (((row * width) + column + offset) * 4) as usize;
                canvas[to..to + 4].copy_from_slice(&source.0[from..from + 4]);
            }
        }
    }
    let file = std::fs::File::create(out).map_err(|error| format!("{}: {error}", out.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(&canvas))
        .map_err(|error| format!("{}: {error}", out.display()))?;
    println!("wrote {} ({width}x{height}, before | after)", out.display());
    Ok(())
}
