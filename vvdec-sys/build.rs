use std::path::PathBuf;

mod build {
    use super::*;
    use std::env;
    use std::path::Path;
    use std::process::{Command, Stdio};

    const REPO: &str = "https://github.com/fraunhoferhhi/vvdec.git";

    macro_rules! runner {
        ($cmd:expr, $($arg:expr),*) => {
            Command::new($cmd)
                $(.arg($arg))*
                .stderr(Stdio::inherit())
                .stdout(Stdio::inherit())
                .output()
                .expect(concat!($cmd, " failed"));

        };
    }

    pub fn build_from_src(
        lib: &str,
        version: &str,
    ) -> Result<system_deps::Library, system_deps::BuildInternalClosureError> {
        let mut tag = "v".to_string();
        tag.push_str(version);

        let source = PathBuf::from(env::var("OUT_DIR").unwrap()).join("vvdec");

        if !Path::new(&source.join(".git")).exists() {
            runner!("git", "clone", "--depth", "1", "-b", tag, REPO, &source);
        } else {
            runner!(
                "git",
                "-C",
                source.to_str().unwrap(),
                "fetch",
                "--depth",
                "1",
                "origin",
                tag
            );
            runner!(
                "git",
                "-C",
                source.to_str().unwrap(),
                "checkout",
                "FETCH_HEAD"
            );
        }

        let install_dir = cmake::build(source);
        let pkg_config_dir = install_dir.join("lib/pkgconfig");
        system_deps::Library::from_internal_pkg_config(&pkg_config_dir, lib, version)
    }
}

fn main() {
    println!("cargo:rerun-if-changed=wrapper.h");

    system_deps::Config::new()
        .add_build_internal("libvvdec", build::build_from_src)
        .probe()
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
