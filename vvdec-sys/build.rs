use std::path::PathBuf;

const VVDEC_VERSION: &str = "3.0.0";

#[cfg(feature = "vendored")]
mod vendored {
    use super::*;
    use std::str::FromStr;

    pub fn build_from_src() {
        let source = PathBuf::from_str("vvdec").expect("submodule is initialized");
        let install_dir = cmake::Config::new(source)
            .define("VVDEC_TOPLEVEL_OUTPUT_DIRS", "OFF")
            .build();
        let pkg_config_dir = install_dir.join("lib/pkgconfig");
        std::env::set_var("PKG_CONFIG_PATH", pkg_config_dir.as_os_str());
        println!("cargo:rerun-if-changed=vvdec");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");

    #[cfg(feature = "vendored")]
    vendored::build_from_src();

    let library = pkg_config::Config::new()
        .atleast_version(VVDEC_VERSION)
        .probe("libvvdec")
        .expect("libvvdec not found in the system. To allow building it from source, use the \"vendored\" feature");

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_args(
            library
                .include_paths
                .iter()
                .map(|path| format!("-I{}", path.to_string_lossy())),
        )
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .allowlist_type("vvdec.*")
        .allowlist_function("vvdec_.*")
        .allowlist_var("VVDEC.*")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
