fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let cimgui = root.join("vendor/cimgui");
    let header = root.join("vendor/bite_bindgen.h");
    std::fs::write(
        &header,
        "#define CIMGUI_DEFINE_ENUMS_AND_STRUCTS 1\n#include \"cimgui/cimgui.h\"\n",
    )
    .expect("write bindgen header");
    bindgen::Builder::default()
        .header(header.to_string_lossy())
        .clang_arg(format!("-I{}", root.join("vendor").display()))
        .clang_arg(format!("-I{}", cimgui.display()))
        .clang_arg("-DCIMGUI_DEFINE_ENUMS_AND_STRUCTS=1")
        .allowlist_function("ig.*")
        .allowlist_function("Im.*")
        .allowlist_type("Im.*")
        .allowlist_var("Im.*")
        .derive_default(true)
        .derive_debug(true)
        .prepend_enum_name(false)
        .layout_tests(false)
        .generate()
        .expect("generate bindings")
        .write_to_file(root.join("src/bindings.rs"))
        .unwrap();
    let _ = std::fs::remove_file(&header);
}
