//! Integration test runner. Delegates every operation assertion to
//! `tests/integration.sh` (a bash script). Skipped if `npx` or `bash`
//! aren't on PATH.

use std::process::Command;

#[test]
fn integration_script() {
    if which::which("npx").is_err() || which::which("bash").is_err() {
        eprintln!("skipping: integration tests need `npx` and `bash` on PATH");
        return;
    }
    // Build the oauth_mock example so the OAuth section can find it next to
    // the mcpal binary. Skips that section if the build fails (e.g. offline).
    let _ = Command::new(env!("CARGO"))
        .args(["build", "--quiet", "--example", "oauth_mock"])
        .status();

    let bin = assert_cmd::cargo::cargo_bin("mcpal");
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("integration.sh");

    let mut cmd = Command::new("bash");
    cmd.arg(&script).env("MCPAL_BIN", &bin).env("NO_COLOR", "1");
    let status = cmd.status().expect("spawn bash");
    assert!(status.success(), "integration.sh failed");
}
