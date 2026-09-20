//! Native file dialogs and the system clipboard, on every platform the editor targets.
//!
//! Filters, default names and button labels follow the Electron handlers in
//! `src/main/ipc/handlers.ts` so that the two builds present the same choices.
use std::path::PathBuf;

/// Opens a workflow, matching the Electron filter list.
pub fn open_workflow() -> Result<Option<PathBuf>, String> {
    Ok(rfd::FileDialog::new()
        .add_filter("Bite Workflow", &["bite"])
        .add_filter("All Files", &["*"])
        .pick_file())
}

/// Saves a file with a default name and one extension, as every save dialog does.
pub fn save_file(
    title: &str,
    filter_name: &str,
    extension: &str,
    default_name: &str,
) -> Result<Option<PathBuf>, String> {
    Ok(rfd::FileDialog::new()
        .set_title(title)
        .add_filter(filter_name, &[extension])
        .set_file_name(default_name)
        .save_file())
}

/// Chooses a folder. The Electron scan dialog labels its accept button `Select Folder`.
pub fn select_folder() -> Result<Option<PathBuf>, String> {
    Ok(rfd::FileDialog::new().pick_folder())
}

/// Chooses one or more images, using the shared supported-format list.
pub fn select_images() -> Result<Vec<PathBuf>, String> {
    Ok(rfd::FileDialog::new()
        .add_filter("All Supported Images", IMAGE_EXTENSIONS)
        .add_filter("All Files", &["*"])
        .pick_files()
        .unwrap_or_default())
}

/// The image extensions the Electron build accepts, from `src/shared/constants.ts`.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "avif", "svg", "svgz", "ico", "bmp", "tif", "tiff",
    "heic", "heif", "jp2", "j2k", "jpf", "jpx", "jxl", "psd", "psb", "exr", "hdr", "dpx", "cin",
    "cr2", "cr3", "nef", "nrw", "arw", "dng", "orf", "raf", "rw2", "pef", "srw", "x3f", "3fr",
    "kdc", "mrw", "erf", "rwl", "tga", "pcx", "ppm", "pbm", "pgm", "pnm", "sgi", "rgb", "rgba",
    "miff", "mng", "jng", "xbm", "xpm", "xwd", "sun", "iff", "lbm", "wbmp", "pict", "pct", "dds",
    "fits", "fts",
];

/// True when a path looks like an image the editor can import.
pub fn is_image_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .is_some_and(|extension| IMAGE_EXTENSIONS.contains(&extension.as_str()))
}

/// A confirmation with a message and a native title, used for the ImageMagick warning.
pub fn message(title: &str, description: &str) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(description)
        .set_level(rfd::MessageLevel::Warning)
        .show();
}

/// Shows a warning with two buttons and reports whether the first was chosen.
pub fn confirm(title: &str, description: &str) -> bool {
    matches!(
        rfd::MessageDialog::new()
            .set_title(title)
            .set_description(description)
            .set_level(rfd::MessageLevel::Warning)
            .set_buttons(rfd::MessageButtons::OkCancel)
            .show(),
        rfd::MessageDialogResult::Ok
    )
}

pub fn write_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text.to_string()))
        .map_err(|error| error.to_string())
}

pub fn read_clipboard() -> Result<Option<String>, String> {
    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
        Ok(text) => Ok(Some(text)),
        // An empty or non-text clipboard is a normal state, not a failure.
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// Opens a path in the platform file manager, for the summary dialog's folder button.
pub fn open_path(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(path).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(path).spawn();
    result.map(|_| ()).map_err(|error| error.to_string())
}

/// Opens a web address in the default browser, for the documentation and issue links.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    result.map(|_| ()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn image_paths_are_recognised_regardless_of_case() {
        assert!(is_image_path(Path::new("a/b/photo.PNG")));
        assert!(is_image_path(Path::new("shot.jpeg")));
        assert!(is_image_path(Path::new("raw.cr3")));
        assert!(!is_image_path(Path::new("workflow.bite")));
        assert!(!is_image_path(Path::new("notes.txt")));
        assert!(!is_image_path(Path::new("noextension")));
    }
}
