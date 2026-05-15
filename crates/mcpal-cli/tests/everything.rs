//! End-to-end CLI smoke test against `@modelcontextprotocol/server-everything`.
//!
//! Skipped at runtime if `npx` is not on PATH.

use std::path::PathBuf;

use assert_cmd::Command;
use predicates::str;

fn mcpal(config: &PathBuf) -> Command {
    let mut cmd = Command::cargo_bin("mcpal").expect("mcpal binary");
    cmd.env("MCPAL_CONFIG", config);
    cmd.env("NO_COLOR", "1");
    cmd
}

#[test]
fn m1_smoke() {
    if which::which("npx").is_err() {
        eprintln!("skipping: npx not on PATH");
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("mcpal.toml");

    mcpal(&cfg).args(["init"]).assert().success();

    mcpal(&cfg)
        .args([
            "server",
            "add",
            "everything",
            "--stdio",
            "npx",
            "--arg",
            "-y",
            "--arg",
            "@modelcontextprotocol/server-everything",
        ])
        .assert()
        .success();

    mcpal(&cfg)
        .args(["server", "list", "--output", "json"])
        .assert()
        .success()
        .stdout(str::contains("everything"));

    mcpal(&cfg)
        .args(["server", "test", "everything"])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(str::contains("mcp-servers/everything"));

    mcpal(&cfg)
        .args(["tool", "list", "everything", "--output", "json"])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(str::contains("\"echo\""));

    mcpal(&cfg)
        .args([
            "--output",
            "json",
            "tool",
            "call",
            "everything",
            "echo",
            "--message",
            "hello",
        ])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success()
        .stdout(str::contains("hello"));

    mcpal(&cfg)
        .args(["resource", "list", "everything", "--output", "json"])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success();

    mcpal(&cfg)
        .args(["prompt", "list", "everything", "--output", "json"])
        .timeout(std::time::Duration::from_secs(60))
        .assert()
        .success();

    mcpal(&cfg)
        .args(["server", "remove", "everything"])
        .assert()
        .success();
}
