fn main() {
    let imgui = "vendor/cimgui/imgui";
    let node = "vendor/imgui-node-editor";
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .include(imgui)
        .include(node)
        .include("vendor/cimgui");
    for file in [
        "imgui.cpp",
        "imgui_draw.cpp",
        "imgui_tables.cpp",
        "imgui_widgets.cpp",
        "imgui_demo.cpp",
    ] {
        build.file(format!("{imgui}/{file}"));
    }
    for file in [
        "imgui_node_editor.cpp",
        "imgui_node_editor_api.cpp",
        "imgui_canvas.cpp",
        "crude_json.cpp",
    ] {
        build.file(format!("{node}/{file}"));
    }
    build
        .file("vendor/cimgui/cimgui.cpp")
        .file("shim/bite_c.cpp")
        .warnings(false)
        .compile("bite_imgui");
    println!("cargo:rerun-if-changed=shim");
    println!("cargo:rerun-if-changed=vendor/cimgui");
    println!("cargo:rerun-if-changed=vendor/imgui-node-editor");
}
