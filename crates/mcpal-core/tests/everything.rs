//! End-to-end smoke test against `@modelcontextprotocol/server-everything`.
//!
//! Skipped at runtime if `npx` is not on PATH. Validates that connect()
//! actually speaks MCP, not just that the types compile.

use std::collections::BTreeMap;

use mcpal_core::{ServerSpec, connect};

#[tokio::test]
async fn list_tools_against_everything_server() {
    if which::which("npx").is_err() {
        eprintln!("skipping: npx not on PATH");
        return;
    }

    let spec = ServerSpec::Stdio {
        command: "npx".into(),
        args: vec![
            "-y".into(),
            "@modelcontextprotocol/server-everything".into(),
        ],
        env: BTreeMap::new(),
    };

    let client = connect(&spec).await.expect("connect");
    let tools = client.list_all_tools().await.expect("list_all_tools");
    assert!(!tools.is_empty(), "everything server exposes tools");
    assert!(tools.iter().any(|t| t.name == "echo"));

    client.cancel().await.expect("cancel");
}
