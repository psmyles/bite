fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    bindgen::Builder::default()
        .header(root.join("shim/bite_c.h").to_string_lossy())
        .allowlist_function("bite_.*")
        .allowlist_type("Bite.*")
        .derive_default(true)
        .generate()
        .expect("generate bindings")
        .write_to_file(root.join("src/bindings.rs"))
        .unwrap();
}
