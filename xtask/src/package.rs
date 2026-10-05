//! `cargo xtask package-windows`: the whole Windows distribution build.
//!
//! 1. Reads `product.json`, the one source of the product's name, version and publisher.
//! 2. Syncs `[workspace.package] version` in Cargo.toml to it and refreshes Cargo.lock, so neither
//!    the crate version (`bite --version`) nor the exes' version resources drift from the
//!    installer's. Nothing in a `cargo build` reads product.json into a manifest, so this does.
//! 3. Redraws the icons if the artwork changed (`icons`): both `build.rs` files embed
//!    `build/icon.ico`, so it has to be current before the build.
//! 4. Builds `bite-gui.exe` and `bite.exe` in release.
//! 5. Checks the payload: the bundled ImageMagick (the real binary, not its LFS pointer, and
//!    without the legacy utilities), the node and format definitions, the licence notices.
//! 6. Draws Setup's wizard images into `target/package/wizard` and compiles `packaging/bite.iss`
//!    with Inno Setup into `dist/Bite-Windows-<version>-Setup.exe`.
//!
//! With `--no-build`, steps 3 and 4 are skipped: `target/release` and the icons are used as they
//! are. The only tracked files this edits are Cargo.toml and Cargo.lock (step 2) and the icons
//! (step 3).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{icons, util};

/// The product fields the installer needs, from product.json.
struct Product {
    name: String,
    version: String,
    publisher: String,
    copyright: String,
    homepage: String,
    exe: String,
    cli: String,
    extension: String,
    prog_id: String,
    type_name: String,
}

/// Reads product.json; a missing or empty field is an error, not a fallback.
fn product() -> Result<Product, String> {
    let path = util::root().join("product.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let field = |pointer: &str| -> Result<String, String> {
        v.pointer(pointer)
            .and_then(|x| x.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("product.json is missing the string field {pointer}"))
    };
    Ok(Product {
        name: field("/productName")?,
        version: field("/version")?,
        publisher: field("/publisher")?,
        copyright: field("/copyright")?,
        homepage: field("/homepage")?,
        exe: field("/exeName")?,
        cli: field("/cliName")?,
        extension: field("/fileAssociation/extension")?,
        prog_id: field("/fileAssociation/progId")?,
        type_name: field("/fileAssociation/typeName")?,
    })
}

/// `manifest` with its `[workspace.package]` version set to `version`, and the version it had;
/// None when the section or its version line is missing.
fn with_workspace_version(manifest: &str, version: &str) -> Option<(String, String)> {
    let mut in_section = false;
    let mut old = None;
    let mut out = String::with_capacity(manifest.len());
    for line in manifest.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == "[workspace.package]";
        }
        let value = in_section
            .then(|| trimmed.strip_prefix("version"))
            .flatten()
            .and_then(|rest| rest.trim_start().strip_prefix('='))
            .map(|rest| rest.trim().trim_matches('"'));
        match value {
            Some(current) if old.is_none() => {
                old = Some(current.to_string());
                let eol = &line[line.trim_end().len()..];
                out.push_str(&format!("version = \"{version}\"{eol}"));
            }
            _ => out.push_str(line),
        }
    }
    old.map(|old| (out, old))
}

/// Step 2; true when Cargo.toml changed.
fn sync_version(version: &str) -> Result<bool, String> {
    let path = util::root().join("Cargo.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (synced, old) = with_workspace_version(&text, version)
        .ok_or_else(|| format!("no [workspace.package] version in {}", path.display()))?;
    let changed = old != version;
    if changed {
        std::fs::write(&path, synced).map_err(|e| format!("{}: {e}", path.display()))?;
        eprintln!("  Cargo.toml: {old} -> {version}");
    } else {
        eprintln!("  Cargo.toml is at {version}");
    }
    // Unconditionally: the manifest can be in sync while the committed lock is not. `--workspace`
    // re-resolves the members only, so every registry dependency stays pinned as it was.
    util::run(util::cargo().args(["update", "--workspace", "--quiet"]))?;
    Ok(changed)
}

/// Step 5: what an install needs to work, checked before Inno Setup packs it.
fn check_payload(p: &Product) -> Result<(), String> {
    let root = util::root();
    let release = root.join("target").join("release");
    for exe in [&p.exe, &p.cli] {
        let path = release.join(format!("{exe}.exe"));
        if !path.is_file() {
            return Err(format!("{} is missing; drop --no-build", path.display()));
        }
    }

    // Both binaries resolve <exe dir>\magick\magick.exe first, so the bundle is what makes an
    // install work without ImageMagick on PATH.
    let magick_dir = root.join("resources/win/magick");
    let magick = magick_dir.join("magick.exe");
    let size = std::fs::metadata(&magick)
        .map_err(|_| {
            format!(
                "{} is missing; it is tracked with Git LFS: git lfs pull",
                magick.display()
            )
        })?
        .len();
    // An LFS pointer is a few hundred bytes. Shipping it makes an installer whose every run
    // fails, so the size is checked, not just the presence.
    if size < 1024 * 1024 {
        return Err(format!(
            "{} is {size} bytes: a Git LFS pointer, not the binary; git lfs pull",
            magick.display()
        ));
    }
    // Upstream's portable drop carries the legacy utilities beside magick.exe, each a full 30 MB
    // static copy of the same library that nothing here runs (every call is `magick <tool>`).
    // Re-vendoring is the one way they come back, and a diff doesn't show them.
    let stray: Vec<String> = std::fs::read_dir(&magick_dir)
        .map_err(|e| format!("{}: {e}", magick_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.to_ascii_lowercase().ends_with(".exe") && !n.eq_ignore_ascii_case("magick.exe")
        })
        .collect();
    if !stray.is_empty() {
        return Err(format!(
            "{} holds legacy utilities that must not ship: {}; magick.exe runs them all as \
             subcommands, so delete them",
            magick_dir.display(),
            stray.join(", ")
        ));
    }
    eprintln!("  ImageMagick: {:.1} MB", size as f64 / 1024.0 / 1024.0);

    for dir in ["node-definitions", "format-definitions"] {
        let count = std::fs::read_dir(root.join(dir))
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                    .count()
            })
            .unwrap_or(0);
        if count == 0 {
            return Err(format!(
                "no definitions in {dir}: the install would have none"
            ));
        }
        eprintln!("  {dir}: {count}");
    }

    // The ImageMagick licence requires its notice to travel with the binary: a missing one is a
    // compliance bug, not a cosmetic one.
    for notice in ["LICENSE", "THIRD_PARTY_LICENSES"] {
        if !root.join(notice).is_file() {
            return Err(format!(
                "{notice} is missing, and the installer must ship it"
            ));
        }
    }
    Ok(())
}

fn fresh_dir(dir: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))
}

/// Inno Setup's compiler: where its installer puts it, or on PATH.
fn iscc() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("ProgramFiles(x86)")
            .map(|p| PathBuf::from(p).join("Inno Setup 6/ISCC.exe")),
        std::env::var_os("ProgramFiles").map(|p| PathBuf::from(p).join("Inno Setup 6/ISCC.exe")),
        std::env::var_os("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("Programs/Inno Setup 6/ISCC.exe")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.is_file())
        .or_else(|| {
            Command::new("ISCC")
                .arg("/?")
                .output()
                .ok()
                .map(|_| PathBuf::from("ISCC"))
        })
}

pub fn run_windows(args: &[String]) -> Result<(), String> {
    if !cfg!(windows) {
        return Err("package-windows runs on Windows (Inno Setup)".into());
    }
    let iscc = iscc().ok_or(
        "Inno Setup 6.5.2 or later was not found (ISCC.exe); install it from jrsoftware.org \
         or with `winget install JRSoftware.InnoSetup`",
    )?;
    let p = product()?;
    eprintln!("{} {} - {}", p.name, p.version, p.publisher);
    let build = !util::flag(args, "--no-build");

    util::step("version");
    if sync_version(&p.version)? && !build {
        eprintln!("  --no-build, so target/release may still hold the previous version");
    }

    let art = icons::Artwork::load()?;
    if build {
        // The build embeds the .ico, so it has to be current first.
        util::step("icons");
        let redrawn = icons::refresh(&art)?;
        if redrawn.is_empty() {
            eprintln!("  {} and {} are current", icons::ICO, icons::WINDOW_ICON);
        }
        for path in redrawn {
            eprintln!("  redrew {path} from the artwork; commit it");
        }
        util::step("release build");
        util::run(util::cargo().args(["build", "--release", "-p", "bite-gui", "-p", "bite-cli"]))?;
    }

    util::step("payload");
    check_payload(&p)?;

    util::step("wizard images");
    let wizard = util::root().join("target/package/wizard");
    fresh_dir(&wizard)?;
    let (large, small) = icons::write_wizard_images(&art, &wizard)?;

    util::step("installer");
    let root = util::root();
    let out = root.join("dist");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    util::run(
        Command::new(&iscc)
            .arg("/Qp")
            .arg(format!("/DMyAppName={}", p.name))
            .arg(format!("/DMyAppVersion={}", p.version))
            .arg(format!("/DMyAppPublisher={}", p.publisher))
            .arg(format!("/DMyAppCopyright={}", p.copyright))
            .arg(format!("/DMyAppExe={}.exe", p.exe))
            .arg(format!("/DMyAppCliExe={}.exe", p.cli))
            .arg(format!("/DMyAppProgId={}", p.prog_id))
            .arg(format!("/DMyAppTypeName={}", p.type_name))
            .arg(format!("/DMyAppExtension={}", p.extension))
            .arg(format!("/DMyAppUrl={}", p.homepage))
            .arg(format!("/DSourceIcon={}", root.join(icons::ICO).display()))
            .arg(format!("/DWizardImages={large}"))
            .arg(format!("/DWizardSmallImages={small}"))
            .arg(format!("/DOutputDir={}", out.display()))
            .arg(root.join("packaging/bite.iss")),
    )?;
    let setup = out.join(format!("{}-Windows-{}-Setup.exe", p.name, p.version));
    let size = std::fs::metadata(&setup).map(|m| m.len()).unwrap_or(0);
    println!(
        "{} ({:.2} MB)",
        setup.display(),
        size as f64 / 1024.0 / 1024.0
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_workspace_version_is_synced() {
        let manifest = "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"0.5.1\"\r\n\
                        edition = \"2021\"\n\n[workspace.dependencies]\nversion = \"9\"\n";
        let (synced, old) = with_workspace_version(manifest, "0.6.0").unwrap();
        assert_eq!(old, "0.5.1");
        assert_eq!(synced, manifest.replace("0.5.1", "0.6.0"));
        assert!(with_workspace_version("[package]\nversion = \"1\"\n", "2").is_none());
    }

    #[test]
    fn product_json_has_every_field() {
        product().unwrap();
    }
}
