use std::env;
use std::path::PathBuf;

const VVDEC_VERSION: &str = "2.1.2";

mod build {
    use super::*;

    use std::str::FromStr;

    pub fn build_from_src() -> PathBuf {
        let source = PathBuf::from_str("vvdec").expect("submodule is initialized");
        let install_dir = cmake::Config::new(source).generator("Ninja").build();
        install_dir.join("lib/pkgconfig")
    }
}

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");

    // Try to find vvdec in the system. Else, build vvdec from source.
    let library = match pkg_config::Config::new()
        .atleast_version(VVDEC_VERSION)
        .probe("libvvdec")
    {
        Ok(library) => library,
        Err(_) => {
            if env::var("VVDEC_SYS_BUILD_DEP_FROM_SRC").is_err() && env::var("DOCS_RS").is_err() {
                panic!(
                    "libvvdec not found in the system. To allow building it from source, \
                    set environment variable VVDEC_SYS_BUILD_DEP_FROM_SRC=1"
                );
            }
            let pkg_config_dir = build::build_from_src();
            env::set_var("PKG_CONFIG_PATH", pkg_config_dir.as_os_str());
            pkg_config::Config::new()
                .atleast_version(VVDEC_VERSION)
                .probe("libvvdec")
                .unwrap()
        }
    };

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
