//! Integration test runner. Delegates every operation assertion to
//! `tests/integration.sh` (a bash script). Gated behind
//! `MCPAL_INTEGRATION_TESTS=1` so default CI runs the unit suite only;
//! local + nightly integration jobs opt in.

use std::process::Command;

#[test]
fn integration_script() {
    if std::env::var_os("MCPAL_INTEGRATION_TESTS").is_none() {
        eprintln!("skipping: set MCPAL_INTEGRATION_TESTS=1 to run");
        return;
    }
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
