//! Putting the bundled command line tool on the PATH.
//!
//! The .dmg is drag-to-install and runs nothing, so unlike the Windows installer's PATH task this
//! is the app's own job: Bite > Install Command Line Tool links `/usr/local/bin/bite` to the CLI
//! inside the bundle, asking for an administrator password when the folder needs one. That
//! folder is on the default PATH of every macOS shell, and on Apple Silicon it is not Homebrew's,
//! so a link there collides with nothing.
//!
//! The link points into the bundle rather than at a copy, so the CLI keeps finding its
//! definitions and ImageMagick in `Contents/Resources` - which is why `bite-cli` and
//! `Magick::discover` resolve their own path before looking beside it - and it updates with the
//! app. It also breaks if the app is moved, which the startup check then reports.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the link goes.
pub const LINK: &str = "/usr/local/bin/bite";

/// What the CLI is called, on the PATH and inside the bundle.
const CLI_NAME: &str = env!("BITE_CLI_NAME");

/// The CLI inside the running bundle, or `None` when this is not a bundled build - a
/// development build has nothing worth linking.
pub fn bundled_cli() -> Option<PathBuf> {
    let executable = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .ok()?;
    let directory = executable.parent()?;
    let in_bundle = directory.ends_with("Contents/MacOS")
        && directory
            .parent()
            .and_then(Path::parent)
            .and_then(Path::extension)
            .is_some_and(|extension| extension == "app");
    let cli = directory.join(CLI_NAME);
    (in_bundle && cli.is_file()).then_some(cli)
}

/// Whether a shell would find a `bite` command.
///
/// An app started from Finder inherits launchd's PATH rather than the shell's, so the PATH it
/// sees misses Homebrew's folder; that one is checked by name. `is_file` follows links, so a link
/// left dangling by a moved or deleted app counts as missing.
pub fn on_path() -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain(["/usr/local/bin", "/opt/homebrew/bin"].map(PathBuf::from))
        .any(|directory| directory.join(CLI_NAME).is_file())
}

/// The status bar's startup warning, when the bundled CLI is not reachable from a shell.
pub fn startup_notice() -> Option<String> {
    (bundled_cli().is_some() && !on_path()).then(|| {
        format!(
            "The {CLI_NAME} command is not on your PATH - Bite > Install Command Line Tool adds it"
        )
    })
}

/// Links [`LINK`] to the bundled CLI, asking for an administrator password if the folder is not
/// writable. Returns the CLI the link now points at.
pub fn install() -> Result<PathBuf, String> {
    let cli = bundled_cli().ok_or(
        "This is not a packaged build of Bite, so there is no bundled command to install.",
    )?;
    // A quarantined app run from Downloads is started from a randomised read-only copy, and one
    // run from the disk image lives on a volume that goes away. A link to either breaks.
    let location = cli.to_string_lossy();
    if location.contains("/AppTranslocation/") || location.starts_with("/Volumes/") {
        return Err(
            "Move Bite to your Applications folder and open it from there first: \
             a link to this copy would stop working."
                .into(),
        );
    }
    let link = Path::new(LINK);
    // Replacing an earlier link is what reinstalling is; replacing a real file would be
    // overwriting some other program.
    if let Ok(metadata) = link.symlink_metadata() {
        if !metadata.file_type().is_symlink() {
            return Err(format!(
                "{LINK} already exists and is not a link to Bite. Remove it and try again."
            ));
        }
    }

    if link_directly(&cli, link).is_err() {
        link_as_administrator(&cli)?;
    }
    match std::fs::canonicalize(link) {
        Ok(target) if target == cli => Ok(cli),
        _ => Err(format!("{LINK} was not created.")),
    }
}

fn link_directly(cli: &Path, link: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(link.parent().unwrap_or(Path::new("/")))?;
    let _ = std::fs::remove_file(link);
    std::os::unix::fs::symlink(cli, link)
}

/// The same link through `osascript`, which shows the system's own password prompt. The path is
/// handed to the shell by AppleScript's `quoted form of`, so the only quoting done here is the
/// AppleScript string literal's.
fn link_as_administrator(cli: &Path) -> Result<(), String> {
    let directory = Path::new(LINK).parent().unwrap_or(Path::new("/"));
    let script = format!(
        "do shell script \"/bin/mkdir -p {} && /bin/ln -sfn \" & quoted form of \"{}\" & \" {}\" \
         with prompt \"Bite wants to install the {CLI_NAME} command in {}.\" \
         with administrator privileges",
        directory.display(),
        applescript_string(&cli.to_string_lossy()),
        LINK,
        directory.display(),
    );
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .output()
        .map_err(|error| format!("Could not ask for an administrator password: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let error = String::from_utf8_lossy(&output.stderr);
    // -128 is "User canceled", which needs no explanation back.
    if error.contains("(-128)") {
        Err("Cancelled.".into())
    } else {
        Err(error.trim().to_string())
    }
}

/// `text` as the inside of an AppleScript string literal.
fn applescript_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applescript_strings_escape_quotes_and_backslashes() {
        assert_eq!(
            applescript_string(r#"/Apps/My "Bite"\x.app"#),
            r#"/Apps/My \"Bite\"\\x.app"#
        );
    }

    #[test]
    fn a_development_build_has_nothing_to_install() {
        // The test binary lives in target/, not in a bundle.
        assert!(bundled_cli().is_none());
        assert!(startup_notice().is_none());
    }
}
