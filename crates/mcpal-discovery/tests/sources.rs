use std::path::PathBuf;

use mcpal_discovery::{DiscoveryCtx, discover};

fn fake_ctx(tempdir: &tempfile::TempDir) -> DiscoveryCtx {
    let root = tempdir.path().to_path_buf();
    DiscoveryCtx {
        home: root.join("home"),
        config_dir: root.join("config"),
        data_dir: root.join("data"),
        cwd: root.join("cwd"),
        custom_paths: Vec::new(),
    }
}

fn write(path: PathBuf, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

#[test]
fn vscode_workspace_mcp_json() {
    let d = tempfile::tempdir().unwrap();
    let ctx = fake_ctx(&d);
    write(
        ctx.cwd.join(".vscode/mcp.json"),
        r#"{ "servers": { "fetch": { "command": "uvx", "args": ["mcp-server-fetch"] } } }"#,
    );
    let servers = discover(&ctx);
    assert!(
        servers
            .iter()
            .any(|s| s.source == "vscode" && s.name == "fetch"),
        "expected vscode/fetch in {servers:?}"
    );
}

#[test]
fn vscode_user_chat_mcp_servers() {
    let d = tempfile::tempdir().unwrap();
    let ctx = fake_ctx(&d);
    write(
        ctx.config_dir.join("Code/User/settings.json"),
        r#"{
            "chat": { "mcp": { "servers": {
                "fs": { "command": "npx", "args": ["-y", "@mcp/fs"] }
            } } }
        }"#,
    );
    let servers = discover(&ctx);
    assert!(
        servers
            .iter()
            .any(|s| s.source == "vscode-user" && s.name == "fs"),
        "expected vscode-user/fs in {servers:?}"
    );
}

#[test]
fn continue_extension_globalstorage() {
    let d = tempfile::tempdir().unwrap();
    let ctx = fake_ctx(&d);
    write(
        ctx.config_dir
            .join("Code/User/globalStorage/continue.continue/config.json"),
        r#"{ "mcpServers": { "fs": { "command": "uvx", "args": ["mcp-server-fs"] } } }"#,
    );
    let servers = discover(&ctx);
    assert!(
        servers
            .iter()
            .any(|s| s.source == "continue" && s.name == "fs"),
        "expected continue/fs in {servers:?}"
    );
}

#[test]
fn codex_config_toml() {
    let d = tempfile::tempdir().unwrap();
    let ctx = fake_ctx(&d);
    write(
        ctx.home.join(".codex/config.toml"),
        r#"
[mcp_servers.fs]
command = "uvx"
args = ["mcp-server-fs"]
"#,
    );
    let servers = discover(&ctx);
    assert!(
        servers
            .iter()
            .any(|s| s.source == "codex" && s.name == "fs"),
        "expected codex/fs in {servers:?}"
    );
}

#[test]
fn empty_nested_key_yields_zero_not_error() {
    let d = tempfile::tempdir().unwrap();
    let ctx = fake_ctx(&d);
    write(
        ctx.config_dir.join("Code/User/settings.json"),
        r#"{ "chat": { "mcp": { "servers": {} } } }"#,
    );
    let servers = discover(&ctx);
    assert!(
        !servers.iter().any(|s| s.source == "vscode-user"),
        "vscode-user entries should be empty"
    );
}

#[test]
fn custom_paths_pick_up_extra_files() {
    let d = tempfile::tempdir().unwrap();
    let mut ctx = fake_ctx(&d);
    let extra = d.path().join("team-mcp.json");
    write(
        extra.clone(),
        r#"{ "mcpServers": { "team": { "command": "uvx", "args": ["mcp-team"] } } }"#,
    );
    let missing = d.path().join("does-not-exist.json");
    ctx.custom_paths = vec![extra, missing];
    let servers = discover(&ctx);
    assert!(
        servers
            .iter()
            .any(|s| s.source == "custom" && s.name == "team"),
        "expected custom/team in {servers:?}"
    );
}

/// Every built-in source produces at least one path against a synthetic
/// `DiscoveryCtx`. Catches typos / missing roots. `custom` is skipped
/// because it depends on `ctx.custom_paths`.
#[test]
fn all_sources_produce_paths() {
    let d = tempfile::tempdir().unwrap();
    let ctx = fake_ctx(&d);
    let mut missing = Vec::new();
    for src in mcpal_discovery::sources::registry() {
        if src.id() == "custom" {
            continue;
        }
        if src.paths(&ctx).is_empty() {
            missing.push(src.id());
        }
    }
    assert!(missing.is_empty(), "sources with no paths: {missing:?}");
}
