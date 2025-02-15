use std::path::PathBuf;

const VVDEC_VERSION: &str = "3.0.0";

mod vendored {
    use super::*;
    use std::str::FromStr;

    pub fn build_from_src(
        lib: &str,
        _version: &str,
    ) -> Result<system_deps::Library, system_deps::BuildInternalClosureError> {
        println!("cargo:rerun-if-changed=vvdec");
        let source = PathBuf::from_str("vvdec").expect("submodule is initialized");
        let install_dir = cmake::Config::new(source)
            .define("VVDEC_TOPLEVEL_OUTPUT_DIRS", "OFF")
            .build();
        let pkg_dir = install_dir.join("lib/pkgconfig");
        system_deps::Library::from_internal_pkg_config(pkg_dir, lib, VVDEC_VERSION)
    }
}

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");

    #[cfg(docsrs)]
    std::env::set_var("SYSTEM_DEPS_LIBVVDEC_BUILD_INTERNAL", "always");

    system_deps::Config::new()
        .add_build_internal("libvvdec", vendored::build_from_src)
        .probe()
        .unwrap();

    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
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
