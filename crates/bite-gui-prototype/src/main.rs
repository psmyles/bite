//! The editor executable.
use bite_gui_prototype::smoke;
use std::path::{Path, PathBuf};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let Some(position) = arguments.iter().position(|value| value == "--capture") {
        if let Err(error) = capture(&arguments[position + 1..]) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    // A bare first argument is a workflow to open, as the file association passes one.
    let initial = arguments
        .iter()
        .find(|argument| !argument.starts_with("--"))
        .map(PathBuf::from);
    if let Err(error) = bite_gui_prototype::app::run(initial) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

/// Renders the comparison scenes without opening a window.
///
/// `--capture <directory> [workflow] [scale]` writes one image per scene.
fn capture(arguments: &[String]) -> Result<(), String> {
    let directory = arguments
        .first()
        .ok_or("--capture needs an output directory")?;
    let workflow = arguments
        .get(1)
        .filter(|value| value.ends_with(".bite"))
        .map(PathBuf::from);
    let scale = arguments
        .iter()
        .skip(1)
        .find_map(|value| value.parse::<f32>().ok())
        .unwrap_or(1.0)
        .clamp(1.0, 4.0);
    let written = smoke::capture_all(
        Path::new(directory),
        workflow.as_deref(),
        (1600, 1000),
        scale,
    )?;
    println!("Captured {} scenes", written.len());
    Ok(())
}
