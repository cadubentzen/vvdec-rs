use assert_cmd::Command;

#[test]
fn basic() {
    // TODO: this just tests that the cli didn't crash.
    // more robust testing could be
    // 1. use insta_cmd and provide a report on stdout to assert on
    // 2. PSNR on the decoded output or plain hash checking
    Command::cargo_bin("vvdecli")
        .unwrap()
        .args(&["-i", "../tests/short.vvc", "-o", "/tmp/decoded.y4m"])
        .assert()
        .success();
}
