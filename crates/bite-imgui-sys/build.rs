//! Compiles Dear ImGui and the cimgui C wrapper that `src/bindings.rs` is generated from.
//!
//! ## The define list is a contract, not a preference
//!
//! `vendor/cimgui/cimgui.h` is a *generated* header: cimgui's `generator.lua` ran the
//! preprocessor over `imgui.h` with a particular set of defines and wrote out whatever survived.
//! The tree vendored here is `dear-imgui-sys 0.17.0`'s, generated with
//! `IMGUI_DISABLE_OBSOLETE_FUNCTIONS` and without `IMGUI_USE_WCHAR32` - which is why the header
//! has no `ImFontAtlas_GetTexDataAsAlpha8`, no `ImGuiIO::FontGlobalScale`, no
//! `ImDrawData::CmdListsCount`, and an `ImWchar` that is 16 bits wide.
//!
//! Those are not only missing *functions*: they are missing struct **fields**. So the C++ here
//! has to be compiled with the same defines the header was generated with, or the layouts Rust
//! believes in - taken from that header - silently disagree with the ones the library was built
//! with, and every read of an `ImGuiIO` field past the first divergence is wrong.
//!
//! `DEFINES` below is that list, and it is shared: `bite-gui/build.rs` compiles `simgui.c` against
//! this same `cimgui.h`, so the two translation units must agree letter for letter. Change a
//! define here and it has to change there too; the comment at each end points at the other.

/// The defines the vendored `cimgui.h` was generated with, which every translation unit that
/// includes it must be compiled with. Mirrored by `bite-gui/build.rs` for the `simgui.c` compile.
///
/// `CIMGUI_NO_EXPORT` builds the wrapper statically rather than as a DLL.
/// `IMGUI_DISABLE_OBSOLETE_FUNCTIONS` is not a tidiness choice - see the module comment.
const DEFINES: &[&str] = &["CIMGUI_NO_EXPORT", "IMGUI_DISABLE_OBSOLETE_FUNCTIONS"];

fn main() {
    let imgui = "vendor/cimgui/imgui";
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include(imgui)
        .include("vendor/cimgui");
    for define in DEFINES {
        build.define(define, None);
    }
    for file in [
        "imgui.cpp",
        "imgui_draw.cpp",
        "imgui_tables.cpp",
        "imgui_widgets.cpp",
        // cimgui exports the demo entry points, so the translation unit must be linked.
        "imgui_demo.cpp",
    ] {
        build.file(format!("{imgui}/{file}"));
    }
    build
        .file("vendor/cimgui/cimgui.cpp")
        .warnings(false)
        .compile("bite_imgui");
    println!("cargo:rerun-if-changed=vendor/cimgui");
}
