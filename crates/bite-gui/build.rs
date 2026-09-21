//! Build steps for the editor executable:
//!
//!   1. Compile `simgui/simgui.c` - sokol_imgui.h, Dear ImGui's sokol_gfx renderer - as C,
//!      against the vendored sokol headers and the *same* `cimgui.h` `bite-imgui-sys` builds.
//!      The Rust side calls it through a handful of `extern "C"` declarations in `render::imgui`.
//!   2. Read the canonical product metadata from `product.json` (repo root) and, on Windows,
//!      embed it into the executable together with `build/icon.ico`: the icon Explorer and the
//!      taskbar draw, the name Task Manager lists, and the file-properties version tab.
//!   3. Re-export the same strings as `BITE_*` compile-time environment variables, so the editor
//!      reads them through `env!` instead of carrying its own copy. `product.json` is the one
//!      source of truth - the packaging script reads it too, and syncs the workspace version.

use std::{env, path::Path, path::PathBuf};

/// The product fields the build consumes.
struct Product {
    name: String,
    version: String,
    publisher: String,
    description: String,
    copyright: String,
    homepage: String,
}

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    compile_simgui(&target_os);
    let product = read_product();
    if target_os == "windows" {
        embed_resources(&product);
    }
    export_env(&product);
}

/// The defines `simgui.c` is compiled with.
///
/// **This list must match `bite-imgui-sys/build.rs`'s `DEFINES` exactly**, and that file's module
/// comment explains why: `cimgui.h` is a generated header, and the defines it was generated with
/// decide which struct *fields* exist. `simgui.c` and the Dear ImGui library see the same header
/// through the same include path, so a define on one side and not the other is two translation
/// units disagreeing about `ImGuiIO`'s layout - which nothing would report.
///
/// `CIMGUI_DEFINE_ENUMS_AND_STRUCTS` is only on this side: it asks the header for the C view of
/// the types, which is what a C translation unit needs and what the C++ one must not have.
const IMGUI_DEFINES: &[&str] = &[
    "CIMGUI_DEFINE_ENUMS_AND_STRUCTS",
    "CIMGUI_NO_EXPORT",
    "IMGUI_DISABLE_OBSOLETE_FUNCTIONS",
];

/// Compiles sokol_imgui.h (see `simgui/simgui.c`).
///
/// The sokol backend define must match the one the vendored `sokol` crate's build picks - the
/// same `SOKOL_BACKEND` override, the same per-OS default - because the two translation units
/// share sokol_gfx's structs, exactly as the ImGui defines above are shared with the sys crate.
fn compile_simgui(target_os: &str) {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let simgui = manifest.join("simgui");
    let sokol_c = manifest.join("../../vendor/sokol-rust/src/sokol/c");
    // One `cimgui.h` in the tree, not a copy beside `simgui.c`: the sys crate's, which is the one
    // its bindings were generated from.
    let cimgui = manifest.join("../bite-imgui-sys/vendor/cimgui");
    for file in ["simgui.c", "sokol_imgui.h"] {
        println!("cargo:rerun-if-changed={}", simgui.join(file).display());
    }
    println!("cargo:rerun-if-changed={}", cimgui.join("cimgui.h").display());
    println!("cargo:rerun-if-env-changed=SOKOL_BACKEND");

    let backend = match env::var("SOKOL_BACKEND").as_deref() {
        Ok("D3D11") => "SOKOL_D3D11",
        Ok("METAL") => "SOKOL_METAL",
        Ok("GL") => "SOKOL_GLCORE",
        Ok("GLES3") => "SOKOL_GLES3",
        Ok("WGPU") => "SOKOL_WGPU",
        _ => match target_os {
            "windows" => "SOKOL_D3D11",
            "macos" => "SOKOL_METAL",
            _ => "SOKOL_GLCORE",
        },
    };

    let mut build = cc::Build::new();
    build
        .file(simgui.join("simgui.c"))
        .include(&simgui)
        .include(&sokol_c)
        .include(&cimgui)
        .define(backend, None)
        // Renderer only: the window and its input are winit's (see render::imgui).
        .define("SOKOL_IMGUI_NO_SOKOL_APP", None)
        .warnings(false);
    for define in IMGUI_DEFINES {
        build.define(define, None);
    }
    if env::var("PROFILE").as_deref() == Ok("release") {
        build.define("NDEBUG", None);
        build.opt_level(2);
    }
    build.compile("bite_simgui");
}

/// Parses `../../product.json`. A missing file or field is a build error, not a fallback: every
/// name the installer and the About dialog show comes from here.
fn read_product() -> Product {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = Path::new(&manifest).join("../../product.json");
    println!("cargo:rerun-if-changed={}", path.display());

    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    let json: serde_json::Value = serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("{} is not valid JSON: {error}", path.display()));
    let field = |key: &str| -> String {
        json.get(key)
            .and_then(|value| value.as_str())
            .unwrap_or_else(|| panic!("product.json is missing the string field \"{key}\""))
            .to_string()
    };

    Product {
        name: field("productName"),
        version: field("version"),
        publisher: field("publisher"),
        description: field("description"),
        copyright: field("copyright"),
        homepage: field("homepage"),
    }
}

#[cfg(not(windows))]
fn embed_resources(_product: &Product) {}

/// Gated on `cfg(windows)` - the host - because `winresource` is a `cfg(windows)`
/// build-dependency and so is only in scope there. The caller keeps the `target_os` check, so
/// the pair reads as "Windows target, built on Windows"; a cross-build embeds nothing, which is
/// the honest outcome when the resource compiler is absent.
#[cfg(windows)]
fn embed_resources(product: &Product) {
    let manifest = env::var("CARGO_MANIFEST_DIR").unwrap();
    let icon = Path::new(&manifest).join("../../build/icon.ico");
    println!("cargo:rerun-if-changed={}", icon.display());

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(icon.to_str().expect("icon path is valid UTF-8"));
    resource.set("ProductName", &product.name);
    // FileDescription is the friendly name Task Manager and the taskbar tooltip show.
    resource.set("FileDescription", &product.name);
    resource.set("CompanyName", &product.publisher);
    resource.set("LegalCopyright", &product.copyright);
    resource.set("Comments", &product.description);
    resource.set("OriginalFilename", "bite-gui.exe");
    resource.set("InternalName", "bite-gui");
    // Override winresource's Cargo-derived strings and the numeric VS_FIXEDFILEINFO, so the
    // version Windows reports is product.json's even if the crate version drifts.
    resource.set("FileVersion", &product.version);
    resource.set("ProductVersion", &product.version);
    let packed = packed_version(&product.version);
    resource.set_version_info(winresource::VersionInfo::FILEVERSION, packed);
    resource.set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);
    resource
        .compile()
        .expect("failed to embed Windows resources");
}

/// Packs "major.minor.patch\[.build\]" into the `u64` VS_FIXEDFILEINFO layout. Each field is 16
/// bits, so a component above 65535 is clamped rather than bleeding into its neighbour.
#[cfg(windows)]
fn packed_version(version: &str) -> u64 {
    let mut fields = [0u64; 4];
    for (field, part) in fields.iter_mut().zip(version.split('.')) {
        *field = part.parse::<u64>().unwrap_or(0).min(0xFFFF);
    }
    (fields[0] << 48) | (fields[1] << 32) | (fields[2] << 16) | fields[3]
}

/// Re-exports the product strings for `env!`, so no user-facing string is hardcoded twice.
fn export_env(product: &Product) {
    println!("cargo:rustc-env=BITE_PRODUCT_NAME={}", product.name);
    println!("cargo:rustc-env=BITE_VERSION={}", product.version);
    println!("cargo:rustc-env=BITE_PUBLISHER={}", product.publisher);
    println!("cargo:rustc-env=BITE_DESCRIPTION={}", product.description);
    println!("cargo:rustc-env=BITE_COPYRIGHT={}", product.copyright);
    println!("cargo:rustc-env=BITE_HOMEPAGE={}", product.homepage);
}
