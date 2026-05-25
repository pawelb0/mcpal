# v0.4.0 discovery expansion + bugfixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add discovery sources for VS Code (3 surfaces), Codex CLI, plus a `--discover-from PATH` flag for ad-hoc files; fix `registry::fetch` to pick the latest version; capture child stderr by default so connection failures surface a real diagnostic.

**Architecture:** Extend the existing `SimpleSource` table (declarative entries) with nested-key paths + format enum, add 4 new entries + a `CustomFile` source. Bugfix A: rewrite `registry::fetch` with semver-max selection. Bugfix B: stdio child stderr piped into a 64-line ring buffer; flushed into the error chain on connect failure; TUI explicitly opts back into null.

**Tech Stack:** Rust, `directories` crate (already in workspace), `toml` crate (already), `serde_yaml`, new `semver = "1"` dep.

**Spec:** `docs/superpowers/specs/2026-05-24-discovery-expansion-design.md`.

---

## File Structure

| File | Role |
|---|---|
| `Cargo.toml` workspace | Add `semver = "1"`. |
| `crates/mcpal/Cargo.toml` | `semver.workspace = true`. |
| `crates/mcpal-discovery/Cargo.toml` | `toml.workspace = true` (if not present). |
| `crates/mcpal-discovery/src/sources/mod.rs` | `SimpleSource` gains `key_path` + `format`; existing 6 entries migrated; 4 new entries added. |
| `crates/mcpal-discovery/src/sources/custom.rs` (new) | `CustomFile` source iterating `ctx.custom_paths`. |
| `crates/mcpal-discovery/src/parse.rs` | New `walk_key_path` helper; TOML parsing branch. |
| `crates/mcpal-discovery/src/lib.rs` | `DiscoveryCtx::custom_paths` field; `with_custom_paths` builder. |
| `crates/mcpal/src/cli.rs` | `--discover-from PATH` global flag. |
| `crates/mcpal/src/runtime.rs` | Thread `discover_from` through `Ctx`. |
| `crates/mcpal/src/main.rs` | `Ctx::new` call site. |
| `crates/mcpal/src/registry.rs` | `fetch` → semver-max. |
| `crates/mcpal-core/src/client.rs` | Stdio child stderr → ring buffer; flushed into error chain on failure. |
| `crates/mcpal/src/tui/mod.rs` | Set `MCPAL_CHILD_STDERR=null` before entering the alt-screen. |
| `crates/mcpal/tests/integration.sh` | New sections: custom discovery + stderr-surfaced-on-failure. |
| `crates/mcpal-discovery/tests/sources.rs` (new) | Unit tests for each new source + nested key + Windows path smoke. |
| `book/src/troubleshooting.md` | "Server dies on initialize" section. |
| `book/src/concepts.md` | Discovery sources list updated. |
| `CHANGELOG.md` | `[Unreleased]` → `[0.4.0]` block. |

---

### Task 1: Extend `SimpleSource` (additive) with nested key path + format

**Files:**
- Modify: `crates/mcpal-discovery/src/sources/mod.rs`
- Modify: `crates/mcpal-discovery/src/parse.rs`

- [ ] **Step 1: Add the helper for nested-key walking**

Append to `crates/mcpal-discovery/src/parse.rs`:

```rust
/// Walk a nested key path against a JSON value, returning the final object map.
pub(crate) fn walk_key_path<'a>(
    root: &'a Value,
    path: &[&str],
) -> Option<&'a serde_json::Map<String, Value>> {
    let mut cur = root;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_object()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn walks_single_level() {
        let v = json!({ "mcpServers": { "a": {} } });
        let m = walk_key_path(&v, &["mcpServers"]).unwrap();
        assert!(m.contains_key("a"));
    }

    #[test]
    fn walks_three_levels() {
        let v = json!({ "chat": { "mcp": { "servers": { "a": {} } } } });
        let m = walk_key_path(&v, &["chat", "mcp", "servers"]).unwrap();
        assert!(m.contains_key("a"));
    }

    #[test]
    fn missing_segment_returns_none() {
        let v = json!({ "chat": { } });
        assert!(walk_key_path(&v, &["chat", "mcp", "servers"]).is_none());
    }

    #[test]
    fn non_object_terminal_returns_none() {
        let v = json!({ "k": 7 });
        assert!(walk_key_path(&v, &["k"]).is_none());
    }
}
```

- [ ] **Step 2: Run new tests — confirm pass**

```
cargo test -p mcpal-discovery parse::tests::walks
cargo test -p mcpal-discovery parse::tests::missing
cargo test -p mcpal-discovery parse::tests::non_object
```
Expected: all 4 pass.

- [ ] **Step 3: Extend `SimpleSource` shape in `sources/mod.rs`**

Replace the current struct + table-header definitions (top of file, lines ~14–25):

```rust
pub enum SourceFormat {
    Json,
    Jsonc,
    Toml,
}

pub struct SimpleSource {
    pub id: &'static str,
    pub key_path: &'static [&'static str],
    pub global: &'static [(Location, &'static str)],
    pub project: &'static [&'static str],
    pub format: SourceFormat,
}
```

Migrate each existing entry in `SIMPLE_SOURCES`:
- `key: "mcpServers"` → `key_path: &["mcpServers"]`
- `key: "context_servers"` → `key_path: &["context_servers"]`
- `jsonc: false` → `format: SourceFormat::Json`
- `jsonc: true` → `format: SourceFormat::Jsonc`

Example for the first entry:

```rust
SimpleSource {
    id: "claude-desktop",
    key_path: &["mcpServers"],
    global: &[(Location::Config, "Claude/claude_desktop_config.json")],
    project: &[],
    format: SourceFormat::Json,
},
```

Repeat the migration for `cursor`, `lm-studio`, `windsurf`, `cline`, `zed` keeping their other fields verbatim.

- [ ] **Step 4: Update the `Source` impl to use `key_path` + `format`**

In the same file, replace the `impl Source for &'static SimpleSource` block's `parse` method:

```rust
fn parse(&self, path: &Path, scope: Scope, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
    let v: Value = match self.format {
        SourceFormat::Json => serde_json::from_slice(bytes)?,
        SourceFormat::Jsonc => json5::from_str(std::str::from_utf8(bytes)?)?,
        SourceFormat::Toml => {
            let s = std::str::from_utf8(bytes)?;
            let t: toml::Value = toml::from_str(s)?;
            serde_json::to_value(t)?
        }
    };
    let Some(map) = crate::parse::walk_key_path(&v, self.key_path) else {
        return Ok(Vec::new());
    };
    Ok(crate::parse::servers_map(map, self.id, path, scope))
}
```

Add the `toml` import (and dep wiring — Task 1 will check) by adding to `crates/mcpal-discovery/Cargo.toml` `[dependencies]` if absent:

```toml
toml.workspace = true
```

Confirm via `grep -n '^toml' crates/mcpal-discovery/Cargo.toml`. If absent, add the line.

- [ ] **Step 5: Verify everything still compiles + passes**

```
cargo build -p mcpal-discovery
cargo test -p mcpal-discovery
cargo test -p mcpal --bin mcpal
```
Expected: all green, no behaviour change for existing sources.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/mcpal-discovery/
git -c commit.gpgsign=false commit -m "extend SimpleSource with key_path + format"
```

---

### Task 2: Add vscode + vscode-user + continue + codex entries

**Files:**
- Modify: `crates/mcpal-discovery/src/sources/mod.rs`
- Create: `crates/mcpal-discovery/tests/sources.rs`

- [ ] **Step 1: Write the failing fixture tests**

Create `/Users/pawelb/workspace/mcpal/crates/mcpal-discovery/tests/sources.rs`:

```rust
use std::path::PathBuf;

use mcpal_discovery::{DiscoveryCtx, Location, discover};

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

fn ensure_dir(p: &PathBuf) {
    std::fs::create_dir_all(p).unwrap();
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
    ensure_dir(&ctx.cwd);
    write(
        ctx.cwd.join(".vscode/mcp.json"),
        r#"{ "servers": { "fetch": { "command": "uvx", "args": ["mcp-server-fetch"] } } }"#,
    );
    let servers = discover(&ctx);
    assert!(
        servers.iter().any(|s| s.source == "vscode" && s.name == "fetch"),
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
        servers.iter().any(|s| s.source == "vscode-user" && s.name == "fs"),
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
        servers.iter().any(|s| s.source == "continue" && s.name == "fs"),
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
        servers.iter().any(|s| s.source == "codex" && s.name == "fs"),
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
```

Add `tempfile` to `crates/mcpal-discovery/Cargo.toml` `[dev-dependencies]` if not already there:

```toml
tempfile.workspace = true
```

- [ ] **Step 2: Run tests — expect failure**

```
cargo test -p mcpal-discovery --test sources
```
Expected: compile error or 5 failures — none of the new sources exist yet.

- [ ] **Step 3: Add the 4 new `SIMPLE_SOURCES` entries**

In `crates/mcpal-discovery/src/sources/mod.rs`, append to the `SIMPLE_SOURCES` slice (before the closing `]`):

```rust
SimpleSource {
    id: "vscode",
    key_path: &["servers"],
    global: &[(Location::Config, "Code/User/mcp.json")],
    project: &[".vscode/mcp.json"],
    format: SourceFormat::Jsonc,
},
SimpleSource {
    id: "vscode-user",
    key_path: &["chat", "mcp", "servers"],
    global: &[(Location::Config, "Code/User/settings.json")],
    project: &[],
    format: SourceFormat::Jsonc,
},
SimpleSource {
    id: "continue",
    key_path: &["mcpServers"],
    global: &[(
        Location::Config,
        "Code/User/globalStorage/continue.continue/config.json",
    )],
    project: &[],
    format: SourceFormat::Json,
},
SimpleSource {
    id: "codex",
    key_path: &["mcp_servers"],
    global: &[(Location::Home, ".codex/config.toml")],
    project: &[],
    format: SourceFormat::Toml,
},
```

- [ ] **Step 4: Update `DiscoveryCtx` literal in the test file**

The fixture `fake_ctx` already includes `custom_paths: Vec::new()` — but `DiscoveryCtx` only gets that field in Task 3. To avoid stalling, either:
  (a) add the `custom_paths` field to `DiscoveryCtx` here (cheap; one line in lib.rs); OR
  (b) drop the field from the fixture for now and add it back in Task 3.

Go with (a). In `crates/mcpal-discovery/src/lib.rs`, add `pub custom_paths: Vec<PathBuf>` to the struct and `custom_paths: Vec::new()` to `DiscoveryCtx::current`. Task 3 will use it; this lets Task 2's tests compile.

- [ ] **Step 5: Run tests — expect pass**

```
cargo test -p mcpal-discovery --test sources
```
Expected: 5 pass.

- [ ] **Step 6: Commit**

```bash
git add crates/mcpal-discovery/
git -c commit.gpgsign=false commit -m "vscode + vscode-user + continue + codex sources"
```

---

### Task 3: `--discover-from PATH` flag + `CustomFile` source

**Files:**
- Create: `crates/mcpal-discovery/src/sources/custom.rs`
- Modify: `crates/mcpal-discovery/src/sources/mod.rs`
- Modify: `crates/mcpal-discovery/src/lib.rs` (builder method only — field added in Task 2)
- Modify: `crates/mcpal/src/cli.rs`
- Modify: `crates/mcpal/src/runtime.rs`
- Modify: `crates/mcpal/src/main.rs`
- Modify: `crates/mcpal-discovery/tests/sources.rs` (add custom-paths test)

- [ ] **Step 1: Write the failing test**

Append to `crates/mcpal-discovery/tests/sources.rs`:

```rust
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
        servers.iter().any(|s| s.source == "custom" && s.name == "team"),
        "expected custom/team in {servers:?}"
    );
}
```

- [ ] **Step 2: Create the `CustomFile` source**

Create `/Users/pawelb/workspace/mcpal/crates/mcpal-discovery/src/sources/custom.rs`:

```rust
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::parse::servers_map;
use crate::{DiscoveredServer, DiscoveryCtx, Scope, Source};

pub struct CustomFile;

impl Source for CustomFile {
    fn id(&self) -> &'static str {
        "custom"
    }

    fn paths(&self, ctx: &DiscoveryCtx) -> Vec<(PathBuf, Scope)> {
        ctx.custom_paths
            .iter()
            .map(|p| (p.clone(), Scope::Global))
            .collect()
    }

    fn parse(&self, path: &Path, scope: Scope, bytes: &[u8]) -> Result<Vec<DiscoveredServer>> {
        let v: Value = serde_json::from_slice(bytes)?;
        let Some(map) = v.get("mcpServers").and_then(Value::as_object) else {
            return Ok(Vec::new());
        };
        Ok(servers_map(map, "custom", path, scope))
    }
}
```

Register it in `sources/mod.rs`:

```rust
mod custom;
pub use custom::CustomFile;
```

And in `registry()`:

```rust
pub fn registry() -> Vec<Box<dyn Source>> {
    let mut v: Vec<Box<dyn Source>> = vec![Box::new(ClaudeCode), Box::new(Opencode), Box::new(CustomFile)];
    for s in SIMPLE_SOURCES {
        v.push(Box::new(s));
    }
    v
}
```

- [ ] **Step 3: Builder method on `DiscoveryCtx`**

In `crates/mcpal-discovery/src/lib.rs`, after `DiscoveryCtx::current`, add:

```rust
impl DiscoveryCtx {
    pub fn with_custom_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.custom_paths = paths;
        self
    }
}
```

- [ ] **Step 4: Add `--discover-from` global flag**

In `crates/mcpal/src/cli.rs`, in the `pub struct Cli` block, near the other global flags (e.g. after `pub collection: Option<PathBuf>`):

```rust
/// Additional `mcp.json` file to include in discovery (repeatable).
#[arg(long = "discover-from", global = true, value_name = "PATH")]
pub discover_from: Vec<PathBuf>,
```

- [ ] **Step 5: Thread through `Ctx`**

In `crates/mcpal/src/runtime.rs`:

Add field to `Ctx`:

```rust
pub discover_from: Vec<PathBuf>,
```

Add to `Ctx::new` signature (already has `#[allow(clippy::too_many_arguments)]`):

```rust
pub fn new(
    cfg: Config,
    format: Format,
    query: Option<String>,
    timeout: Option<u64>,
    config_path: PathBuf,
    collection_override: Option<PathBuf>,
    profile: String,
    discover_from: Vec<PathBuf>,
    handler: Handler,
) -> Self {
    Self {
        cfg, format, query, timeout, config_path,
        collection_override, profile, discover_from, handler,
        discovered: OnceCell::new(),
    }
}
```

Find the `discovered()` method (search for `fn discovered` in `runtime.rs`). It calls `DiscoveryCtx::current()`. Change to:

```rust
let ctx = DiscoveryCtx::current()?.with_custom_paths(self.discover_from.clone());
```

(Or whatever the exact wiring shape — read the existing method to match its style.)

- [ ] **Step 6: Update call site in `main.rs`**

In `crates/mcpal/src/main.rs`, find the `Ctx::new(...)` call and add the new argument:

```rust
let ctx = Ctx::new(
    cfg,
    format,
    cli.query,
    cli.timeout,
    path,
    cli.collection.clone(),
    cli.profile.clone(),
    cli.discover_from.clone(),
    handler,
);
```

- [ ] **Step 7: Run tests**

```
cargo test -p mcpal-discovery --test sources::custom_paths_pick_up_extra_files
cargo test -p mcpal --bin mcpal
cargo build -p mcpal
```
Expected: all pass.

- [ ] **Step 8: Smoke**

```bash
echo '{"mcpServers":{"smoke":{"command":"echo","args":["hi"]}}}' > /tmp/c.json
./target/debug/mcpal --discover-from /tmp/c.json server list --discovered | grep smoke
```
Expected: `smoke` row present, source `custom`.

- [ ] **Step 9: Commit**

```bash
git add crates/mcpal-discovery/ crates/mcpal/src/cli.rs crates/mcpal/src/runtime.rs crates/mcpal/src/main.rs
git -c commit.gpgsign=false commit -m "--discover-from PATH for ad-hoc mcp.json"
```

---

### Task 4: Semver-max `registry::fetch`

**Files:**
- Modify: `Cargo.toml` workspace
- Modify: `crates/mcpal/Cargo.toml`
- Modify: `crates/mcpal/src/registry.rs`

- [ ] **Step 1: Add `semver` workspace dep**

In `/Users/pawelb/workspace/mcpal/Cargo.toml`, alongside `regex = "1"`:

```toml
semver = "1"
```

In `/Users/pawelb/workspace/mcpal/crates/mcpal/Cargo.toml`, `[dependencies]`:

```toml
semver.workspace = true
```

- [ ] **Step 2: Write the failing test**

In `crates/mcpal/src/registry.rs`, find the existing `#[cfg(test)] mod tests` (or add one at the bottom if absent) and append:

```rust
#[cfg(test)]
mod fetch_tests {
    use super::*;

    fn srv(name: &str, ver: &str) -> Server {
        // Replace fields to match the real `Server` struct; this is a minimum
        // viable instance for the version-pick logic.
        Server {
            name: name.into(),
            version: Some(ver.into()),
            description: None,
            packages: vec![],
        }
    }

    #[test]
    fn picks_max_semver() {
        let candidates = vec![
            srv("io.github.x/y", "0.1.0"),
            srv("io.github.x/y", "0.1.4"),
            srv("io.github.x/y", "0.1.2"),
            srv("io.github.other/z", "9.9.9"),
            srv("io.github.x/y", "0.1.3"),
            srv("io.github.x/y", "0.1.1"),
        ];
        let chosen = pick_latest("io.github.x/y", candidates).unwrap();
        assert_eq!(chosen.version.as_deref(), Some("0.1.4"));
    }

    #[test]
    fn unparseable_version_loses_to_real() {
        let candidates = vec![
            srv("p", "not-semver"),
            srv("p", "0.0.1"),
        ];
        let chosen = pick_latest("p", candidates).unwrap();
        assert_eq!(chosen.version.as_deref(), Some("0.0.1"));
    }

    #[test]
    fn no_match_returns_err() {
        let candidates = vec![srv("a", "0.1.0")];
        assert!(pick_latest("b", candidates).is_err());
    }
}
```

If `Server` has additional required fields, fill them with sensible defaults (`Default::default()` if derived, or `vec![]`/`None` per field). Read the struct first via `grep -n 'struct Server' crates/mcpal/src/registry.rs` to confirm shape.

- [ ] **Step 3: Run — expect compile error**

```
cargo test -p mcpal --bin mcpal fetch_tests
```
Expected: compile error — `pick_latest` undefined.

- [ ] **Step 4: Implement `pick_latest` + replace `fetch`**

In `crates/mcpal/src/registry.rs`, locate the existing `pub async fn fetch(...)` (around line 112). Replace with:

```rust
pub async fn fetch(name: &str) -> Result<Server> {
    let env = search(name, 20).await?;
    let candidates: Vec<Server> = env.servers.into_iter().map(|w| w.server).collect();
    pick_latest(name, candidates)
}

fn pick_latest(name: &str, candidates: Vec<Server>) -> Result<Server> {
    use anyhow::anyhow;
    let mut hits: Vec<Server> = candidates.into_iter().filter(|s| s.name == name).collect();
    if hits.is_empty() {
        return Err(anyhow!(
            "registry: no exact match for '{name}' (try `mcpal server search {name}`)"
        ));
    }
    hits.sort_by(|a, b| {
        let av = a
            .version
            .as_deref()
            .and_then(|v| semver::Version::parse(v).ok());
        let bv = b
            .version
            .as_deref()
            .and_then(|v| semver::Version::parse(v).ok());
        av.cmp(&bv)
    });
    Ok(hits.pop().unwrap())
}
```

- [ ] **Step 5: Run tests — expect pass**

```
cargo test -p mcpal --bin mcpal fetch_tests
cargo test -p mcpal --bin mcpal
```

- [ ] **Step 6: Smoke**

```bash
./target/debug/mcpal server install io.github.codeurali/dataverse
./target/debug/mcpal server show dataverse | grep -F '0.1.4' && echo OK || echo "still picking old version"
```
Cleanup: `./target/debug/mcpal server remove dataverse`

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/mcpal/Cargo.toml crates/mcpal/src/registry.rs
git -c commit.gpgsign=false commit -m "registry: pick latest semver, not first match"
```

---

### Task 5: Capture child stderr by default, attach to connect failures

**Files:**
- Modify: `crates/mcpal-core/src/client.rs`
- Modify: `crates/mcpal-core/Cargo.toml` (if `tokio` features need expanding — confirm `io-util` is on)

- [ ] **Step 1: Write the failing integration assertion**

In `crates/mcpal/tests/integration.sh`, after the existing "server" section (around line 119, after the `--force overwrites existing` row), append:

```bash
# ---------- stderr surfaced on stdio failure ----------
section "stderr surfaced on stdio failure"

mc server add boom --force -- bash -c 'echo "kaboom-marker" >&2; exit 2'
it_exit 'boom server fails (exit 6/7 transport/service)' 6 \
    mc tool list boom
it_grep_err 'failure error chain contains stderr marker' 'kaboom-marker' \
    mc tool list boom
mc server remove boom >/dev/null 2>&1 || true
```

Note: the existing `it_grep_err` was added during the Phase-2 work; if not present, define it near `it_grep` at the top of the harness (mirrors `it_grep` but greps `$ERR`).

- [ ] **Step 2: Run — expect failure (stderr is currently swallowed)**

```
MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture 2>&1 | grep -E "kaboom|boom"
```
Expected: `failure error chain contains stderr marker` FAILs (kaboom-marker not in stderr).

- [ ] **Step 3: Rewrite `connect_stdio` to pipe + capture by default**

In `crates/mcpal-core/src/client.rs`, the current shape (per memory `project_rmcp_quirks`):

```rust
let stderr_mode = std::env::var("MCPAL_CHILD_STDERR").unwrap_or_default();
let stderr_stdio = match stderr_mode.as_str() {
    "inherit" => Stdio::inherit(),
    _ => Stdio::null(),
};
let (transport, _stderr) = rmcp::transport::TokioChildProcess::builder(cmd)
    .stderr(stderr_stdio)
    .spawn()?;
```

Replace the surrounding block with:

```rust
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use tokio::io::{AsyncBufReadExt, BufReader};

let stderr_mode = std::env::var("MCPAL_CHILD_STDERR").unwrap_or_default();
let (stderr_stdio, want_capture) = match stderr_mode.as_str() {
    "inherit" => (Stdio::inherit(), false),
    "null" => (Stdio::null(), false),
    _ => (Stdio::piped(), true), // default: capture
};

let (transport, child_stderr) = rmcp::transport::TokioChildProcess::builder(cmd)
    .stderr(stderr_stdio)
    .spawn()?;

let tail: Option<Arc<Mutex<VecDeque<String>>>> = if want_capture {
    let buf = Arc::new(Mutex::new(VecDeque::with_capacity(64)));
    if let Some(err) = child_stderr {
        let buf2 = Arc::clone(&buf);
        tokio::spawn(async move {
            let mut reader = BufReader::new(err).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let mut q = buf2.lock().unwrap();
                if q.len() == 64 {
                    q.pop_front();
                }
                q.push_back(line);
            }
        });
    }
    Some(buf)
} else {
    None
};

match handler.serve(transport).await {
    Ok(client) => Ok(client),
    Err(e) => {
        let mut msg = format!("{e}");
        if let Some(buf) = tail {
            // Give the drain task a tick to catch up.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let q = buf.lock().unwrap();
            if !q.is_empty() {
                let lines: Vec<&str> = q.iter().map(String::as_str).collect();
                msg = format!("{msg} (child stderr: {})", lines.join(" | "));
            }
        }
        Err(Error::Service(msg))
    }
}
```

The 50ms sleep is a coarse drain wait. If integration flake appears, raise to 100ms — bounded; only on the error path.

- [ ] **Step 4: Run unit tests + integration**

```
cargo test -p mcpal-core
MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture
```
Expected: both pass; new stderr-surfaced rows green.

- [ ] **Step 5: Commit**

```bash
git add crates/mcpal-core/
git -c commit.gpgsign=false commit -m "capture child stderr by default; attach on failure"
```

---

### Task 6: TUI keeps child stderr null

**Files:**
- Modify: `crates/mcpal/src/tui/mod.rs` (or wherever the TUI's `run` entry function lives)

- [ ] **Step 1: Locate the TUI entry point**

Run `grep -n 'pub async fn run' crates/mcpal/src/tui/*.rs` to find the entry function (likely in `tui/mod.rs`).

- [ ] **Step 2: Pin the env var before any child spawn**

At the very top of the TUI entry function, before `TerminalGuard::new` or any connect logic, add:

```rust
// SAFETY: env::set_var is process-wide; we run this once at TUI entry.
// Child spawns from the TUI scope must not write to our alt-screen.
unsafe {
    std::env::set_var("MCPAL_CHILD_STDERR", "null");
}
```

Since `unsafe_code = "deny"` lives at workspace level (see memory `feedback_no_ai_slop` / `project_rmcp_quirks`), add `#[allow(unsafe_code)]` either on the surrounding function or as an attribute on the block:

```rust
#[allow(unsafe_code)]
unsafe {
    std::env::set_var("MCPAL_CHILD_STDERR", "null");
}
```

- [ ] **Step 3: Verify TUI behaviour unchanged**

Manual: `cargo run -p mcpal -- tui` against any stdio server (e.g. `ev`). Confirm no stderr text bleeds into the alt-screen. Exit cleanly with `q`.

- [ ] **Step 4: Commit**

```bash
git add crates/mcpal/src/tui/mod.rs
git -c commit.gpgsign=false commit -m "tui pins MCPAL_CHILD_STDERR=null for its scope"
```

---

### Task 7: Windows path audit smoke test

**Files:**
- Modify: `crates/mcpal-discovery/tests/sources.rs`

- [ ] **Step 1: Add a parameterized "every known source resolves" smoke**

Append to `crates/mcpal-discovery/tests/sources.rs`:

```rust
/// Every entry in the source registry should produce at least one path
/// when fed a synthetic `DiscoveryCtx`. Catches typos / missing roots.
#[test]
fn all_sources_produce_paths() {
    let d = tempfile::tempdir().unwrap();
    let ctx = fake_ctx(&d);
    let mut missing = Vec::new();
    for src in mcpal_discovery::sources::registry() {
        if src.id() == "custom" {
            // CustomFile is empty unless custom_paths is set; skip.
            continue;
        }
        let paths = src.paths(&ctx);
        if paths.is_empty() {
            missing.push(src.id());
        }
    }
    assert!(missing.is_empty(), "sources with no paths: {missing:?}");
}
```

Note: this requires `sources::registry()` to be public — check the existing visibility. If it's currently `pub fn registry()` in `sources/mod.rs` and the module is `pub mod sources;` in `lib.rs`, it should already be reachable. If not, expose it.

- [ ] **Step 2: Run**

```
cargo test -p mcpal-discovery --test sources all_sources_produce_paths
```
Expected: pass.

- [ ] **Step 3: Cross-platform note**

The actual Windows path resolution (e.g. `%APPDATA%` → `C:\Users\...\AppData\Roaming`) is delegated to the `directories` crate at runtime. The smoke above uses a tempdir for all roots, so it isolates from real OS paths. CI's Windows job (if present in `.github/workflows/ci.yml`) will validate the real `directories` lookups. If no Windows job exists yet, add a follow-up to enable one — but that's out of scope for this PR.

- [ ] **Step 4: Commit**

```bash
git add crates/mcpal-discovery/tests/sources.rs
git -c commit.gpgsign=false commit -m "smoke: every source produces a path"
```

---

### Task 8: Book chapters — discovery + troubleshooting

**Files:**
- Modify: `book/src/concepts.md` (or add a new `book/src/discovery.md` chapter and link from `SUMMARY.md`)
- Modify: `book/src/troubleshooting.md`

- [ ] **Step 1: Discovery section**

In `book/src/concepts.md`, find the section that mentions discovery (search for `discover` in the file). Replace or extend the supported-clients list to include the new sources. Add a "Custom paths" subsection:

```markdown
## Custom discovery paths

`mcpal --discover-from /path/to/your.json server list --discovered`
adds an ad-hoc `mcp.json`-shaped file to the discovery sweep. The file
must contain `{ "mcpServers": { "<name>": { ... } } }`. Repeatable.
Missing files are skipped silently; parse errors log under `-v`.
```

If `concepts.md` doesn't have a discovery section, create `book/src/discovery.md` instead and add a line to `book/src/SUMMARY.md` under `# Concepts`:

```markdown
- [Discovery](./discovery.md)
```

The chapter content:

```markdown
# Discovery

mcpal can pull MCP server definitions from other clients you already
have installed. Run `mcpal server discover` to scan, or `mcpal server
list` (default) to see your registered + discovered entries side by side.

## Supported clients

| Source | Files |
|---|---|
| `claude-code` | `~/.claude.json` |
| `claude-desktop` | `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) / `%APPDATA%\Claude\claude_desktop_config.json` (Win) |
| `cursor` | `~/.cursor/mcp.json`, project `.cursor/mcp.json` |
| `opencode` | `~/.config/opencode/opencode.json` |
| `vscode` | `<Code config>/User/mcp.json`, project `.vscode/mcp.json` |
| `vscode-user` | `<Code config>/User/settings.json` (`chat.mcp.servers` key) |
| `continue` | `<Code config>/User/globalStorage/continue.continue/config.json` |
| `cline` | `<Code config>/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` |
| `codex` | `~/.codex/config.toml` |
| `lm-studio` | `~/.lmstudio/mcp.json` |
| `windsurf` | `~/.codeium/windsurf/mcp_config.json` |
| `zed` | `~/.config/zed/settings.json` (`context_servers` key) |

Refer to a discovered server with `<source>:<name>` — e.g.
`mcpal tool list cursor:linear`. Bare names resolve when unambiguous.

## Custom paths

```bash
mcpal --discover-from ~/.config/private/team.json server list --discovered
```

`--discover-from` is repeatable and combines with the built-in sources.
Files must use the `{ "mcpServers": { ... } }` shape. Missing files are
skipped silently.
```

- [ ] **Step 2: Troubleshooting section**

In `book/src/troubleshooting.md`, append:

```markdown
## Server dies on initialize — read its stderr

`mcpal tool list <ref>` failing with
`E0006: connection closed: initialize response` means the stdio child
exited before completing the MCP handshake. The error chain now
includes the last lines of the child's stderr — read it.

If the chain is still empty, the child died silently or printed to
stdout (a protocol violation). Run it in inherit mode to stream
stderr live:

```bash
MCPAL_CHILD_STDERR=inherit mcpal tool list <ref>
```

The TUI always nulls child stderr to keep its alt-screen clean. Use
the CLI for diagnosis.

The relevant env var values are:

| Value | Effect |
|---|---|
| (unset) / `capture` | Default. Stderr piped into a 64-line tail; flushed into the error chain on failure. |
| `inherit` | Stream child stderr live to the parent's stderr. Best for diagnosis. |
| `null` | Discard. Used by `mcpal tui` automatically. |
```

- [ ] **Step 3: Commit**

```bash
git add book/src/
git -c commit.gpgsign=false commit -m "book: discovery chapter + stderr troubleshooting"
```

---

### Task 9: Cut v0.4.0

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml` workspace
- Modify: `crates/mcpal/Cargo.toml`
- Modify: `crates/mcpal-discovery/Cargo.toml`

- [ ] **Step 1: Move `[Unreleased]` → `[0.4.0]`**

In `/Users/pawelb/workspace/mcpal/CHANGELOG.md`, replace the `## [Unreleased]` block with:

```markdown
## [Unreleased]

## [0.4.0]

### Added
- Discovery sources for VS Code (workspace `.vscode/mcp.json` + user `settings.json` `chat.mcp.servers` + Continue extension storage) and Codex CLI (`~/.codex/config.toml`).
- `--discover-from PATH` global flag for ad-hoc `mcp.json` files (repeatable).
- `concepts.md` discovery chapter listing every supported client.

### Changed
- `mcpal server install` now picks the highest semver-compatible version from the registry instead of the first match. Fixes installing oldest broken release of a multi-version server.
- Stdio child stderr is captured by default into a 64-line ring buffer; on connect failure the tail is attached to the error chain (`E0006 … (child stderr: …)`). `MCPAL_CHILD_STDERR=null|inherit|capture` controls the mode; the TUI pins `null` to keep its alt-screen clean.

### Fixed
- `mcpal server install io.github.<owner>/<name>` silently picked the lowest version when multiple existed.
- `mcpal tool list <stdio-ref>` failing with `connection closed: initialize response` now includes the child server's stderr instead of an empty error.
```

- [ ] **Step 2: Bump workspace + per-crate pins**

In `/Users/pawelb/workspace/mcpal/Cargo.toml`:

```toml
version = "0.4.0"
```

In `/Users/pawelb/workspace/mcpal/crates/mcpal-discovery/Cargo.toml`:

```toml
mcpal-core = { path = "../mcpal-core", version = "0.4.0" }
```

In `/Users/pawelb/workspace/mcpal/crates/mcpal/Cargo.toml`:

```toml
mcpal-core = { path = "../mcpal-core", version = "0.4.0" }

mcpal-discovery = { path = "../mcpal-discovery", version = "0.4.0" }
```

- [ ] **Step 3: Full release gate**

```bash
cargo fmt --all -- --check
cargo clippy -p mcpal --all-targets -- -D warnings
cargo clippy -p mcpal-discovery --all-targets -- -D warnings
cargo clippy -p mcpal-core --all-targets -- -D warnings
cargo test -p mcpal-discovery
cargo test -p mcpal --bin mcpal
MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration
```
Expected: all green.

- [ ] **Step 4: Commit + tag**

```bash
git add Cargo.toml Cargo.lock crates/mcpal/Cargo.toml crates/mcpal-discovery/Cargo.toml CHANGELOG.md
git -c commit.gpgsign=false commit -m "release v0.4.0"
git tag v0.4.0
```

Do NOT push yet — human decision. Plan stops at "tag created locally".

---

## Verification

End-to-end smoke after Tasks 1–9:

```bash
# 1. VS Code workspace discovery
mkdir -p /tmp/proj/.vscode && cd /tmp/proj
echo '{"servers":{"ev":{"command":"npx","args":["-y","@modelcontextprotocol/server-everything"]}}}' > .vscode/mcp.json
mcpal server discover --source vscode | grep ev

# 2. Codex discovery
mkdir -p ~/.codex
printf '[mcp_servers.demo]\ncommand = "npx"\nargs = ["-y", "@modelcontextprotocol/server-everything"]\n' > ~/.codex/config.toml
mcpal server discover --source codex | grep demo

# 3. Custom paths
echo '{"mcpServers":{"team":{"command":"npx","args":["-y","@mcp/team"]}}}' > /tmp/team.json
mcpal --discover-from /tmp/team.json server list --discovered | grep team

# 4. Semver-max
mcpal server install io.github.codeurali/dataverse
mcpal server show dataverse | grep -F '0.1.4'
mcpal server remove dataverse

# 5. Stderr surfaced
mcpal server add boom --force -- bash -c 'echo "boom-msg" >&2; exit 2'
mcpal tool list boom 2>&1 | grep boom-msg
mcpal server remove boom
```

---

## Self-Review

**1. Spec coverage**

| Spec section | Task |
|---|---|
| `SimpleSource` extensions (key_path, format) | 1 |
| `vscode` / `vscode-user` / `continue` / `codex` entries | 2 |
| `--discover-from PATH` + `CustomFile` | 3 |
| Bugfix A: semver-max `fetch` | 4 |
| Bugfix B: child stderr capture | 5 |
| TUI keeps null stderr | 6 |
| Windows path audit | 7 |
| Book chapters (discovery + troubleshooting) | 8 |
| Release cut | 9 |

**2. Placeholder scan** — every step has runnable code or an exact command. The `Server` struct field-fill in Task 4 Step 2 is the only soft spot ("fill with sensible defaults"); the implementer reads the struct first and matches its actual shape. Not a placeholder — a real two-minute step.

**3. Type consistency**

- `SourceFormat` variants stable across Tasks 1, 2 (`Json` / `Jsonc` / `Toml`).
- `SimpleSource` field renames (`key` → `key_path`, `jsonc` → `format`) applied uniformly in Task 1; every Task 2 entry uses the new shape.
- `DiscoveryCtx::custom_paths: Vec<PathBuf>` added in Task 2 Step 4; consumed in Task 3's `CustomFile`.
- `Ctx::new` 9-arg signature in Task 3; matches the call site in `main.rs`.
- `pick_latest(name, candidates)` signature consistent in Task 4.
- `MCPAL_CHILD_STDERR` values `null` / `inherit` / `capture` (default) consistent across Task 5 + Task 6 + Task 8 docs.
