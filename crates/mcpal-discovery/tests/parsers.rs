use std::path::PathBuf;

use mcpal_core::ServerSpec;
use mcpal_discovery::{DiscoveredServer, Scope, sources};

fn parse(id: &str, path: &str, scope: Scope, body: &str) -> Vec<DiscoveredServer> {
    let src = sources::by_id(id).unwrap_or_else(|| panic!("no source {id}"));
    src.parse(&PathBuf::from(path), scope, body.as_bytes())
        .unwrap()
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
    let out = parse(
        "claude-code",
        "/Users/pawelb/.claude.json",
        Scope::Global,
        body,
    );
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
    let out = parse(
        "claude-desktop",
        "/Users/pawelb/Library/.../claude_desktop_config.json",
        Scope::Global,
        body,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].name, "fs");
    assert_eq!(out[0].source, "claude-desktop");
}

#[test]
fn cursor_with_http_entry() {
    let body = r#"{ "mcpServers": { "linear": { "url": "https://mcp.linear.app/sse" } } }"#;
    let out = parse("cursor", "/tmp/.cursor/mcp.json", Scope::Project, body);
    assert_eq!(out.len(), 1);
    let s = &out[0];
    assert_eq!(s.name, "linear");
    assert_eq!(s.scope, Scope::Project);
    match &s.spec {
        ServerSpec::Http { url, .. } => assert_eq!(url, "https://mcp.linear.app/sse"),
        _ => panic!("expected http"),
    }
}

#[test]
fn lm_studio_empty_or_missing_key_is_ok() {
    let out = parse("lm-studio", "/tmp/.lmstudio/mcp.json", Scope::Global, "{}");
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
    let out = parse(
        "zed",
        "/Users/pawelb/.config/zed/settings.json",
        Scope::Global,
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
    let out = parse(
        "opencode",
        "/Users/pawelb/.config/opencode/opencode.json",
        Scope::Global,
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
    let out = parse("claude-desktop", "/tmp/x.json", Scope::Global, body);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].name, "ok");
}
