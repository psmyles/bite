fn main() {
    let imgui = "vendor/cimgui/imgui";
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include(imgui)
        .include("vendor/cimgui")
        .define("CIMGUI_NO_EXPORT", None);
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
