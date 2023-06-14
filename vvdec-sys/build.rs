use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");

    pkg_config::Config::new()
        .atleast_version("2.0.0")
        .probe("libvvdec")
        .unwrap();

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .allowlist_type("vvdec.*")
        .allowlist_function("vvenc_.*")
        .allowlist_var("VVDEC.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
