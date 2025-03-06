use std::path::PathBuf;

use bindgen::{RustEdition, RustTarget};

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
        let mut config = cmake::Config::new(source);
        config.define("VVDEC_TOPLEVEL_OUTPUT_DIRS", "OFF");
        // https://github.com/rust-lang/cmake-rs/issues/178
        #[cfg(target_os = "windows")]
        config.profile("Release");

        let install_dir = config.build();
        let pkg_dir = install_dir.join("lib/pkgconfig");
        system_deps::Library::from_internal_pkg_config(pkg_dir, lib, VVDEC_VERSION)
    }
}

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");

    #[cfg(feature = "docsrs")]
    std::env::set_var("SYSTEM_DEPS_LIBVVDEC_BUILD_INTERNAL", "always");

    let dependencies = system_deps::Config::new()
        .add_build_internal("libvvdec", vendored::build_from_src)
        .probe()
        .unwrap();

    let library = dependencies.get_by_name("libvvdec").unwrap();

    let bindings = bindgen::Builder::default()
        // FIXME: InvalidRustTarget doesn't implement Debug?!
        .rust_target(RustTarget::stable(80, 1).map_err(|_| ()).unwrap())
        .rust_edition(RustEdition::Edition2021)
        .header("wrapper.h")
        .clang_args(
            library
                .include_paths
                .iter()
                .map(|path| format!("-I{}", path.to_string_lossy())),
        )
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_type("vvdec.*")
        .allowlist_function("vvdec_.*")
        .allowlist_var("VVDEC.*")
        // FIXME: this fixes build issue on Windows with MSVC.
        // If we need to expose this type at some point, we'd need to fix it.
        .blocklist_type("vvdecSEIDependentRapIndication")
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
