use assert_cmd::Command;

#[test]
fn basic() {
    // TODO: this just tests that the cli didn't crash.
    // more robust testing could be
    // 1. use insta_cmd and provide a report on stdout to assert on
    // 2. PSNR on the decoded output or plain hash checking
    Command::cargo_bin("vvdecli")
        .unwrap()
        .args(&[
            "-i",
            std::env::current_dir()
                .unwrap()
                .parent()
                .unwrap()
                .join("tests")
                .join("short.vvc")
                .to_str()
                .unwrap(),
            "-o",
            tempfile::NamedTempFile::new()
                .unwrap()
                .path()
                .to_str()
                .unwrap(),
        ])
        .assert()
        .success();
}
