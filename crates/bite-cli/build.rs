//! Embeds `build/icon.ico` and the `product.json` metadata into `bite.exe` on Windows, so the
//! CLI carries the same icon and version information as the editor. The editor's build script
//! (`crates/bite-gui/build.rs`) documents the arrangement; this one needs no
//! `BITE_*` exports, because the CLI prints its version from `CARGO_PKG_VERSION`, which the
//! packaging script keeps synced to `product.json`.

use std::{env, path::Path};

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = Path::new(&manifest).join("../../product.json");
    println!("cargo:rerun-if-changed={}", path.display());
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let json: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("{} is not valid JSON: {error}", path.display()));
    embed(&json);
}

#[cfg(not(windows))]
fn embed(_product: &serde_json::Value) {}

/// Gated on the host, as `winresource` is a `cfg(windows)` build-dependency.
#[cfg(windows)]
fn embed(product: &serde_json::Value) {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let icon = Path::new(&manifest).join("../../build/icon.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    let field = |key: &str| -> String {
        product
            .get(key)
            .and_then(|value| value.as_str())
            .unwrap_or_else(|| panic!("product.json is missing the string field \"{key}\""))
            .to_string()
    };
    let version = field("version");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(icon.to_str().expect("icon path is valid UTF-8"));
    resource.set("ProductName", &field("productName"));
    resource.set("FileDescription", &format!("{} CLI", field("productName")));
    resource.set("CompanyName", &field("publisher"));
    resource.set("LegalCopyright", &field("copyright"));
    resource.set("OriginalFilename", "bite.exe");
    resource.set("InternalName", "bite");
    resource.set("FileVersion", &version);
    resource.set("ProductVersion", &version);
    let mut fields = [0u64; 4];
    for (field, part) in fields.iter_mut().zip(version.split('.')) {
        *field = part.parse::<u64>().unwrap_or(0).min(0xFFFF);
    }
    let packed = (fields[0] << 48) | (fields[1] << 32) | (fields[2] << 16) | fields[3];
    resource.set_version_info(winresource::VersionInfo::FILEVERSION, packed);
    resource.set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);
    resource
        .compile()
        .expect("failed to embed Windows resources");
}
