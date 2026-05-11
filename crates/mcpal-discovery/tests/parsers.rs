//! Parser unit tests using inline JSON fixtures.

use std::path::PathBuf;

use mcpal_core::ServerSpec;
use mcpal_discovery::{Scope, Source, sources};

fn run(src: &dyn Source, path: &str, body: &str) -> Vec<mcpal_discovery::DiscoveredServer> {
    src.parse(&PathBuf::from(path), body.as_bytes()).unwrap()
}

#[test]
fn claude_code_user_and_project_scopes() {
    let body = r#"{
        "mcpServers": {
            "github": { "command": "npx", "args": ["-y", "@modelcontextprotocol/server-github"] }
        },
        "projects": {
            "/x": { "mcpServers": { "p1": { "url": "https://p1.example/sse" } } }
        }
    }"#;
    let out = run(&sources::ClaudeCode, "/Users/pawelb/.claude.json", body);
    assert_eq!(out.len(), 2);

    let github = out.iter().find(|d| d.name == "github").unwrap();
    assert_eq!(github.scope, Scope::Global);
    assert!(matches!(github.spec, ServerSpec::Stdio { .. }));

    let p1 = out.iter().find(|d| d.name == "p1").unwrap();
    assert_eq!(p1.scope, Scope::Project);
    assert!(matches!(p1.spec, ServerSpec::Http { .. }));
}

#[test]
fn claude_desktop_basic() {
    let body = r#"{ "mcpServers": { "fs": { "command": "filesystem-mcp", "args": ["/tmp"] } } }"#;
    let out = run(
        &sources::ClaudeDesktop,
        "/Users/pawelb/Library/.../claude_desktop_config.json",
        body,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].name, "fs");
    assert_eq!(out[0].source, "claude-desktop");
}

#[test]
fn cursor_with_http_entry() {
    let body = r#"{ "mcpServers": { "linear": { "url": "https://mcp.linear.app/sse" } } }"#;
    let out = run(&sources::Cursor, "/tmp/.cursor/mcp.json", body);
    assert_eq!(out.len(), 1);
    let s = &out[0];
    assert_eq!(s.name, "linear");
    match &s.spec {
        ServerSpec::Http { url, .. } => assert_eq!(url, "https://mcp.linear.app/sse"),
        _ => panic!("expected http"),
    }
}

#[test]
fn lm_studio_empty_or_missing_key_is_ok() {
    let out = run(&sources::LmStudio, "/tmp/.lmstudio/mcp.json", "{}");
    assert!(out.is_empty());
}

#[test]
fn zed_jsonc_with_comments() {
    let body = r#"{
        // Zed user settings
        "context_servers": {
            "demo": { "command": "demo-mcp", "args": ["--flag"] }
        },
        "theme": "One Dark"
    }"#;
    let out = run(
        &sources::Zed,
        "/Users/pawelb/.config/zed/settings.json",
        body,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].source, "zed");
}

#[test]
fn opencode_local_and_remote() {
    let body = r#"{
        "$schema": "https://opencode.ai/config.json",
        "mcp": {
            "fs":  { "type": "local",  "command": ["fs-mcp", "/tmp"], "environment": { "X": "1" } },
            "lin": { "type": "remote", "url": "https://mcp.linear.app/sse" }
        }
    }"#;
    let out = run(
        &sources::Opencode,
        "/Users/pawelb/.config/opencode/opencode.json",
        body,
    );
    assert_eq!(out.len(), 2);
    let fs = out.iter().find(|d| d.name == "fs").unwrap();
    match &fs.spec {
        ServerSpec::Stdio { command, args, env } => {
            assert_eq!(command, "fs-mcp");
            assert_eq!(args, &["/tmp"]);
            assert_eq!(env.get("X").map(String::as_str), Some("1"));
        }
        _ => panic!("expected stdio"),
    }
}

#[test]
fn malformed_entry_is_skipped_not_fatal() {
    let body = r#"{ "mcpServers": {
        "ok": { "command": "echo" },
        "junk": { "foo": "bar" }
    } }"#;
    let out = run(&sources::ClaudeDesktop, "/tmp/x.json", body);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].name, "ok");
}
