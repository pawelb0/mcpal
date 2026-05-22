# `mcpal.yml` collection file — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a `mcpal.yml` collection file. Users define `profiles:` + `calls:`, then `mcpal run NAME --profile prod` to invoke a saved tool call with `{{profile.X}}` / `{{env.X}}` substitution.

**Architecture:** New `crates/mcpal/src/collection/` module — three files (`parse`, `template`, `mod`) covering YAML parsing, walk-parents lookup, and string-tree templating. New `commands/run.rs` glues them together: load → lookup → render → `ctx.open(server).call_tool(...)`. Reuses existing runtime + render path. No new code in `mcpal-core`.

**Tech Stack:** Rust, `serde_yaml` (already in workspace), new `regex` workspace dep, `clap` 4 (`#[arg(global = true)]`), existing `Ctx::open` / `client.call_tool` from `mcpal-core`.

**Spec:** `docs/superpowers/specs/2026-05-22-mcpal-yml-collection-design.md`.

---

## File Structure

| File | Role |
|---|---|
| `Cargo.toml` (workspace) | Add `regex = "1"` under `[workspace.dependencies]`. |
| `crates/mcpal/Cargo.toml` | `regex.workspace = true` under `[dependencies]`. |
| `crates/mcpal/src/collection/mod.rs` | Pub re-exports + `find_collection(start, override) -> Result<PathBuf>`. |
| `crates/mcpal/src/collection/parse.rs` | `Collection`, `Call` structs (serde). `Collection::load(&Path)`. |
| `crates/mcpal/src/collection/template.rs` | `render(&mut Value, &profile_map) -> Result<(), TemplateError>`. |
| `crates/mcpal/src/commands/run.rs` | `run(args, ctx)` — the glue. |
| `crates/mcpal/src/commands/mod.rs` | Add `pub mod run;`. |
| `crates/mcpal/src/main.rs` | Dispatch `Command::Run`. |
| `crates/mcpal/src/cli.rs` | `Command::Run { name, dry_run, params_override }` + global `--collection PATH`. |
| `crates/mcpal/src/exit.rs` | Patterns + EXPLAIN entries for `E0014`, `E0015`, `E0016`. |
| `crates/mcpal/src/runtime.rs` | `Ctx` carries `collection_override: Option<PathBuf>` (set from `--collection`). |
| `book/src/collection.md` | NEW how-to chapter. |
| `book/src/SUMMARY.md` | Insert `collection.md` between Recipes and Authenticate. |
| `book/src/error-codes.md` | Append E0014–E0016 entries. |
| `README.md` | Append a 4th Quickstart block. |
| `crates/mcpal/tests/integration.sh` | New `mcpal run` section. |

---

### Task 1: Add `regex` workspace dep

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/mcpal/Cargo.toml`

- [ ] **Step 1: Confirm regex is not already present**

Run: `grep -n 'regex' Cargo.toml crates/*/Cargo.toml 2>/dev/null || echo "absent"`
Expected: `absent`.

- [ ] **Step 2: Add to workspace `[workspace.dependencies]`**

In `/Users/pawelb/workspace/mcpal/Cargo.toml`, find the `[workspace.dependencies]` block (it exists — `serde_yaml = "0.9.34"` is there). Add a row in alphabetical position:

```toml
regex = "1"
```

- [ ] **Step 3: Wire into the binary crate**

In `/Users/pawelb/workspace/mcpal/crates/mcpal/Cargo.toml`, under `[dependencies]` (where other `*.workspace = true` lines live), add:

```toml
regex.workspace = true
```

- [ ] **Step 4: Verify it resolves**

Run: `cargo check -p mcpal 2>&1 | tail -3`
Expected: `Finished` line; no errors.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates/mcpal/Cargo.toml
git -c commit.gpgsign=false commit -m "add regex dep for templating"
```

---

### Task 2: Collection schema + YAML parser

**Files:**
- Create: `crates/mcpal/src/collection/mod.rs`
- Create: `crates/mcpal/src/collection/parse.rs`
- Modify: `crates/mcpal/src/main.rs` (add `mod collection;`)

- [ ] **Step 1: Write failing parser tests**

Create `crates/mcpal/src/collection/parse.rs` with:

```rust
//! Parse the `mcpal.yml` collection format.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    #[serde(default, rename = "default-profile")]
    pub default_profile: Option<String>,
    #[serde(default)]
    pub profiles: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default)]
    pub calls: BTreeMap<String, Call>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub server: String,
    pub tool: String,
    #[serde(default)]
    pub params: Value,
}

impl Collection {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read collection: {}", path.display()))?;
        serde_yaml::from_str(&text)
            .with_context(|| format!("parse YAML/JSON from {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_collection_ok() {
        let c: Collection = serde_yaml::from_str("").unwrap();
        assert!(c.calls.is_empty());
        assert!(c.profiles.is_empty());
        assert_eq!(c.default_profile, None);
    }

    #[test]
    fn full_collection_round_trips() {
        let src = r#"
default-profile: dev
profiles:
  dev:
    issue_id: ENG-1
    workspace: my-team
  prod:
    issue_id: ENG-999
    workspace: my-team
calls:
  get-issue:
    server: cursor:linear
    tool: get-issue
    params:
      id: "{{profile.issue_id}}"
      workspace: "{{profile.workspace}}"
"#;
        let c: Collection = serde_yaml::from_str(src).unwrap();
        assert_eq!(c.default_profile.as_deref(), Some("dev"));
        assert_eq!(c.profiles.len(), 2);
        assert_eq!(c.profiles["prod"]["issue_id"], "ENG-999");
        let call = &c.calls["get-issue"];
        assert_eq!(call.server, "cursor:linear");
        assert_eq!(call.tool, "get-issue");
        let params = call.params.as_object().expect("params is object");
        assert_eq!(
            params["id"].as_str(),
            Some("{{profile.issue_id}}")
        );
    }

    #[test]
    fn unknown_top_level_key_rejected() {
        let src = "wat: 1\ncalls: {}\n";
        let err = serde_yaml::from_str::<Collection>(src).unwrap_err();
        assert!(err.to_string().contains("wat"), "{err}");
    }

    #[test]
    fn unknown_call_key_rejected() {
        let src = r#"
calls:
  x:
    server: ev
    tool: echo
    nope: 1
"#;
        let err = serde_yaml::from_str::<Collection>(src).unwrap_err();
        assert!(err.to_string().contains("nope"), "{err}");
    }
}
```

Create `crates/mcpal/src/collection/mod.rs`:

```rust
pub mod parse;

pub use parse::{Call, Collection};
```

Add module reference in `crates/mcpal/src/main.rs` — find the existing `mod` block (top of file, alongside `mod cli;`, `mod runtime;`, etc.) and insert (alphabetical):

```rust
mod collection;
```

- [ ] **Step 2: Run tests — expect compile-then-pass**

Run: `cargo test -p mcpal --bin mcpal collection::parse::tests`
Expected: all 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/mcpal/src/collection/ crates/mcpal/src/main.rs
git -c commit.gpgsign=false commit -m "add collection parser + sample yaml"
```

---

### Task 3: Walk-parents lookup

**Files:**
- Modify: `crates/mcpal/src/collection/mod.rs` (add `find_collection`)

- [ ] **Step 1: Write failing tests**

Append to `crates/mcpal/src/collection/mod.rs`:

```rust
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

/// Locate `mcpal.yml`. If `override_` is `Some(p)`, return `p` if it exists
/// or fail with `E0015`. Otherwise walk from `start` up to filesystem root
/// looking for `mcpal.yml`; first hit wins. `Ok(None)` if nothing is found.
pub fn find_collection(start: &Path, override_: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(p) = override_ {
        if p.is_file() {
            return Ok(Some(p.to_path_buf()));
        }
        bail!("collection not found: {} doesn't exist", p.display());
    }
    let mut cur = start.to_path_buf();
    loop {
        let candidate = cur.join("mcpal.yml");
        if candidate.is_file() {
            return Ok(Some(candidate));
        }
        if !cur.pop() {
            return Ok(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn finds_in_cwd() {
        let d = tmp();
        std::fs::write(d.path().join("mcpal.yml"), "").unwrap();
        let got = find_collection(d.path(), None).unwrap();
        assert_eq!(got.as_deref(), Some(d.path().join("mcpal.yml").as_path()));
    }

    #[test]
    fn walks_up_to_ancestor() {
        let root = tmp();
        std::fs::write(root.path().join("mcpal.yml"), "").unwrap();
        let nested = root.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested).unwrap();
        let got = find_collection(&nested, None).unwrap();
        assert_eq!(got.as_deref(), Some(root.path().join("mcpal.yml").as_path()));
    }

    #[test]
    fn none_when_no_file() {
        let d = tmp();
        let nested = d.path().join("sub");
        std::fs::create_dir_all(&nested).unwrap();
        // Skip if any ancestor (e.g. CI runner) happens to have mcpal.yml.
        if find_collection(&nested, None).unwrap().is_some() {
            return;
        }
        assert!(find_collection(&nested, None).unwrap().is_none());
    }

    #[test]
    fn explicit_override_must_exist() {
        let d = tmp();
        let p = d.path().join("nope.yml");
        let err = find_collection(d.path(), Some(&p)).unwrap_err();
        assert!(err.to_string().contains("not found"), "{err}");
    }

    #[test]
    fn explicit_override_returns_as_is() {
        let d = tmp();
        let p = d.path().join("custom.yml");
        std::fs::write(&p, "").unwrap();
        let got = find_collection(d.path(), Some(&p)).unwrap();
        assert_eq!(got.as_deref(), Some(p.as_path()));
    }
}
```

If `tempfile` isn't a dev-dep yet, add to `/Users/pawelb/workspace/mcpal/crates/mcpal/Cargo.toml` under `[dev-dependencies]`:

```toml
tempfile = "3"
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p mcpal --bin mcpal collection::tests`
Expected: all 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/mcpal/src/collection/mod.rs crates/mcpal/Cargo.toml
git -c commit.gpgsign=false commit -m "walk parents to find mcpal.yml"
```

---

### Task 4: Template engine

**Files:**
- Create: `crates/mcpal/src/collection/template.rs`
- Modify: `crates/mcpal/src/collection/mod.rs` (add `pub mod template;`)

- [ ] **Step 1: Write failing tests**

Create `crates/mcpal/src/collection/template.rs`:

```rust
//! Substitute `{{profile.X}}` and `{{env.X}}` into a JSON Value tree.
//! `{{{{` is the literal-`{{` escape.

use std::collections::BTreeMap;

use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

#[derive(Debug, PartialEq, Eq)]
pub enum Ns {
    Profile,
    Env,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Miss {
    pub ns: Ns,
    pub key: String,
}

#[derive(Debug)]
pub struct TemplateError {
    pub misses: Vec<Miss>,
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "template variable not set: ")?;
        for (i, m) in self.misses.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            let ns = match m.ns {
                Ns::Profile => "profile",
                Ns::Env => "env",
            };
            write!(f, "{}.{}", ns, m.key)?;
        }
        Ok(())
    }
}
impl std::error::Error for TemplateError {}

fn pattern() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"\{\{\s*(profile|env)\.([A-Za-z_][A-Za-z0-9_]*)\s*\}\}").unwrap()
    })
}

pub fn render(
    value: &mut Value,
    profile: &BTreeMap<String, String>,
) -> Result<(), TemplateError> {
    let mut misses = Vec::new();
    walk(value, profile, &mut misses);
    if misses.is_empty() {
        Ok(())
    } else {
        Err(TemplateError { misses })
    }
}

fn walk(value: &mut Value, profile: &BTreeMap<String, String>, misses: &mut Vec<Miss>) {
    match value {
        Value::String(s) => {
            *s = render_string(s, profile, misses);
        }
        Value::Array(a) => {
            for v in a {
                walk(v, profile, misses);
            }
        }
        Value::Object(m) => {
            for (_k, v) in m.iter_mut() {
                walk(v, profile, misses);
            }
        }
        _ => {}
    }
}

fn render_string(
    input: &str,
    profile: &BTreeMap<String, String>,
    misses: &mut Vec<Miss>,
) -> String {
    // Handle the `{{{{` -> `{{` escape by splitting on the literal token
    // first, rendering each piece, then re-joining with `{{`.
    let mut out = String::with_capacity(input.len());
    for (i, piece) in input.split("{{{{").enumerate() {
        if i > 0 {
            out.push_str("{{");
        }
        let mut cursor = 0;
        for m in pattern().captures_iter(piece) {
            let whole = m.get(0).unwrap();
            out.push_str(&piece[cursor..whole.start()]);
            let ns = match &m[1] {
                "profile" => Ns::Profile,
                "env" => Ns::Env,
                _ => unreachable!(),
            };
            let key = m[2].to_string();
            let resolved = match ns {
                Ns::Profile => profile.get(&key).cloned(),
                Ns::Env => std::env::var(&key).ok(),
            };
            match resolved {
                Some(v) => out.push_str(&v),
                None => misses.push(Miss { ns, key }),
            }
            cursor = whole.end();
        }
        out.push_str(&piece[cursor..]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn profile() -> BTreeMap<String, String> {
        BTreeMap::from_iter([
            ("issue_id".to_string(), "ENG-1".to_string()),
            ("workspace".to_string(), "my-team".to_string()),
        ])
    }

    #[test]
    fn profile_substitution() {
        let mut v = json!("{{profile.issue_id}}");
        render(&mut v, &profile()).unwrap();
        assert_eq!(v, json!("ENG-1"));
    }

    #[test]
    fn env_substitution() {
        unsafe { std::env::set_var("MCPAL_TPL_TEST_KEY", "hello"); }
        let mut v = json!("{{env.MCPAL_TPL_TEST_KEY}}");
        render(&mut v, &profile()).unwrap();
        assert_eq!(v, json!("hello"));
    }

    #[test]
    fn mixed_string() {
        unsafe { std::env::set_var("MCPAL_TPL_USER", "pawel"); }
        let mut v = json!("user={{env.MCPAL_TPL_USER}} ws={{profile.workspace}}");
        render(&mut v, &profile()).unwrap();
        assert_eq!(v, json!("user=pawel ws=my-team"));
    }

    #[test]
    fn recursive_object_and_array() {
        let mut v = json!({
            "id": "{{profile.issue_id}}",
            "list": ["{{profile.workspace}}", 42, true, "lit"]
        });
        render(&mut v, &profile()).unwrap();
        assert_eq!(
            v,
            json!({
                "id": "ENG-1",
                "list": ["my-team", 42, true, "lit"]
            })
        );
    }

    #[test]
    fn numbers_and_bools_untouched() {
        let mut v = json!({"n": 7, "b": true, "f": 1.5, "null": null});
        render(&mut v, &profile()).unwrap();
        assert_eq!(v, json!({"n": 7, "b": true, "f": 1.5, "null": null}));
    }

    #[test]
    fn missing_var_collects_all() {
        let mut v = json!({
            "a": "{{profile.nope}}",
            "b": "{{env.MCPAL_TPL_DEFINITELY_NOT_SET}}",
            "c": "ok"
        });
        let err = render(&mut v, &profile()).unwrap_err();
        assert_eq!(err.misses.len(), 2);
        let msg = err.to_string();
        assert!(msg.contains("template variable not set"), "{msg}");
        assert!(msg.contains("profile.nope"), "{msg}");
        assert!(msg.contains("env.MCPAL_TPL_DEFINITELY_NOT_SET"), "{msg}");
    }

    #[test]
    fn escape_literal_braces() {
        let mut v = json!("{{{{not a template}}}}");
        render(&mut v, &profile()).unwrap();
        assert_eq!(v, json!("{{not a template}}"));
    }
}
```

In `crates/mcpal/src/collection/mod.rs`, append `pub mod template;`.

- [ ] **Step 2: Run tests**

Run: `cargo test -p mcpal --bin mcpal collection::template`
Expected: 7 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/mcpal/src/collection/template.rs crates/mcpal/src/collection/mod.rs
git -c commit.gpgsign=false commit -m "template engine for profile + env vars"
```

---

### Task 5: CLI surface (`Command::Run` + `--collection`)

**Files:**
- Modify: `crates/mcpal/src/cli.rs`
- Modify: `crates/mcpal/src/runtime.rs` (Ctx carries collection override)

- [ ] **Step 1: Add `--collection PATH` to `Cli` struct**

In `/Users/pawelb/workspace/mcpal/crates/mcpal/src/cli.rs`, find the `pub struct Cli { … }` block and add (alphabetical alongside other globals, near `--mcp-json`):

```rust
    /// Path to a collection file (`mcpal.yml`). Overrides walk-parents lookup.
    #[arg(long, global = true, value_name = "PATH")]
    pub collection: Option<PathBuf>,
```

- [ ] **Step 2: Add `Command::Run` variant**

Same file, in the `pub enum Command { … }` block, after `Raw { … }` and before `Completion { … }`, add:

```rust
    /// Run a saved call from a collection file.
    #[command(after_help = "Examples:\n  \
        mcpal run get-issue --profile prod\n  \
        mcpal --collection ./mcpal.yml run echo --dry-run\n  \
        mcpal run echo --params-override message=override")]
    Run {
        name: String,
        /// Resolve + print the call without opening a connection.
        #[arg(long)]
        dry_run: bool,
        /// Overlay raw `K=V` params after templating (repeatable).
        #[arg(long = "params-override", value_name = "K=V", num_args = 1)]
        params_override: Vec<String>,
    },
```

- [ ] **Step 3: Thread `--collection` into `Ctx`**

In `/Users/pawelb/workspace/mcpal/crates/mcpal/src/runtime.rs`, find the `pub struct Ctx { … }` block (line ~17) and add a field:

```rust
    pub collection_override: Option<PathBuf>,
```

Update the constructor (line ~33 area, the `pub fn …` that builds the Ctx) to take/store it. The exact signature depends on how `Ctx::new` is wired today — read the constructor and propagate the field through. In `crates/mcpal/src/main.rs`, when Ctx is built from `Cli`, pass `cli.collection.clone()`.

- [ ] **Step 4: Verify the build**

```bash
cargo fmt --all
cargo clippy -p mcpal --all-targets -- -D warnings
cargo test -p mcpal --bin mcpal
```

All pass. No new behaviour yet — `Command::Run` has no dispatch arm (Task 6 adds it).

- [ ] **Step 5: Commit**

```bash
git add crates/mcpal/src/cli.rs crates/mcpal/src/runtime.rs crates/mcpal/src/main.rs
git -c commit.gpgsign=false commit -m "CLI surface for mcpal run + --collection"
```

---

### Task 6: `mcpal run` dispatch + glue

**Files:**
- Create: `crates/mcpal/src/commands/run.rs`
- Modify: `crates/mcpal/src/commands/mod.rs` (export `pub mod run;`)
- Modify: `crates/mcpal/src/main.rs` (dispatch `Command::Run`)

- [ ] **Step 1: Create `commands/run.rs`**

```rust
//! `mcpal run <NAME>` — execute a saved call from `mcpal.yml`.

use anyhow::{Context, Result, anyhow, bail};
use mcpal_core::rmcp::model::CallToolRequestParams;
use serde_json::{Map, Value};

use crate::collection::{Call, Collection, find_collection, template};
use crate::kv;
use crate::runtime::Ctx;

pub async fn run(
    name: String,
    dry_run: bool,
    params_override: Vec<String>,
    ctx: &Ctx,
) -> Result<()> {
    let cwd = std::env::current_dir().context("cwd")?;
    let path = find_collection(&cwd, ctx.collection_override.as_deref())?
        .ok_or_else(|| anyhow!("collection not found: no mcpal.yml from {} upward", cwd.display()))?;
    let coll = Collection::load(&path)?;

    let call: &Call = coll
        .calls
        .get(&name)
        .ok_or_else(|| {
            let avail: Vec<&String> = coll.calls.keys().collect();
            anyhow!("not found in mcpal config: call '{name}' (available: {avail:?})")
        })?;

    // Profile selection: --profile NAME > MCPAL_PROFILE > default-profile: > "default".
    let profile_name = ctx.profile.clone();
    let empty: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let profile_vars = match coll.profiles.get(&profile_name) {
        Some(p) => p,
        None if coll.profiles.is_empty() => &empty,
        None => bail!("profile '{}' not in collection", profile_name),
    };

    let mut params = call.params.clone();
    template::render(&mut params, profile_vars)
        .map_err(|e| anyhow!("{}", e))?;

    // --params-override KEY=VAL overlay (raw, after templating).
    if !params_override.is_empty() {
        let mut obj: Map<String, Value> = match params {
            Value::Object(m) => m,
            Value::Null => Map::new(),
            other => {
                params = other;
                bail!("--params-override requires `params:` be an object; call '{name}' has a scalar/array");
            }
        };
        for kv in &params_override {
            let (k, v) = kv
                .split_once('=')
                .ok_or_else(|| anyhow!("--params-override expects K=V: {kv}"))?;
            obj.insert(k.to_string(), Value::String(v.to_string()));
        }
        params = Value::Object(obj);
    }

    if dry_run {
        ctx.render_one(&serde_json::json!({
            "dry_run": true,
            "server": call.server,
            "tool": call.tool,
            "params": params,
        }))?;
        return Ok(());
    }

    let arguments = match params {
        Value::Object(m) => m,
        Value::Null => Map::new(),
        _ => bail!("`params:` must be an object for call '{name}'"),
    };

    let (_, client) = ctx.open(&call.server).await?;
    let mut req = CallToolRequestParams::new(call.tool.clone());
    if !arguments.is_empty() {
        req = req.with_arguments(arguments);
    }
    let result = ctx
        .under_deadline(client.call_tool(req))
        .await?
        .context("tools/call")?;
    ctx.render_one(&result)?;
    if result.is_error.unwrap_or(false) {
        bail!("server returned tools/call result with isError: true");
    }
    let _ = kv::parse_flag_args; // import retained for symmetry with tool::call — silences unused warning if any
    Ok(())
}
```

If `kv::parse_flag_args` isn't actually used here, drop that line — it's only a guard against an unused-import warning that may not fire.

- [ ] **Step 2: Wire the module**

In `/Users/pawelb/workspace/mcpal/crates/mcpal/src/commands/mod.rs`, add (alphabetical):

```rust
pub mod run;
```

In `/Users/pawelb/workspace/mcpal/crates/mcpal/src/main.rs`, find the dispatch on `cli.command` and add (alphabetical alongside other arms):

```rust
Command::Run {
    name,
    dry_run,
    params_override,
} => crate::commands::run::run(name, dry_run, params_override, &ctx).await,
```

- [ ] **Step 3: Verify build**

```bash
cargo fmt --all
cargo clippy -p mcpal --all-targets -- -D warnings
cargo test -p mcpal --bin mcpal
```

All clean.

- [ ] **Step 4: Smoke (manual)**

```bash
TMP=$(mktemp -d)
cat >"$TMP/mcpal.yml" <<'EOF'
profiles:
  dev: { msg: hello }
calls:
  echo:
    server: ev
    tool: echo
    params: { message: "{{profile.msg}}" }
EOF
# Requires `mcpal server add ev -- npx -y @modelcontextprotocol/server-everything`
mcpal --collection "$TMP/mcpal.yml" --profile dev run echo --dry-run
```

Expected: prints JSON with `"dry_run": true`, `"server": "ev"`, `"tool": "echo"`, `"params": {"message": "hello"}`.

- [ ] **Step 5: Commit**

```bash
git add crates/mcpal/src/commands/run.rs crates/mcpal/src/commands/mod.rs crates/mcpal/src/main.rs
git -c commit.gpgsign=false commit -m "mcpal run wires call to tool invocation"
```

---

### Task 7: Error codes E0014 / E0015 / E0016

**Files:**
- Modify: `crates/mcpal/src/exit.rs`
- Modify: `book/src/error-codes.md`

- [ ] **Step 1: Add patterns to `ANYHOW_PATTERNS`**

In `/Users/pawelb/workspace/mcpal/crates/mcpal/src/exit.rs`, find the `const ANYHOW_PATTERNS: &[(&str, i32, &str)] = &[ … ];` array and add three rows ABOVE the generic `("not found …)` patterns (first match wins):

```rust
    ("template variable not set", 2, "E0014"),
    ("collection not found", 2, "E0015"),
    ("not in collection", 2, "E0016"),
```

- [ ] **Step 2: Add EXPLAIN entries**

In the same file's `const EXPLAIN: &[(&str, &str)] = &[ … ];`, append three entries after E0013:

```rust
    (
        "E0014",
        "Template variable not set. `mcpal.yml` references `{{profile.X}}` or \
        `{{env.X}}` that didn't resolve. Add the key to the active profile, set \
        the env var, or pass `--params-override KEY=VAL` to bypass.\n",
    ),
    (
        "E0015",
        "Collection not found. `mcpal run` looked for `mcpal.yml` in the current \
        directory and every parent, found none. Create one at your project root \
        or pass `--collection PATH` to point at a specific file.\n",
    ),
    (
        "E0016",
        "Active profile isn't declared in the collection. Either add a `profiles.<name>:` \
        block to `mcpal.yml`, pick a different `--profile`, or set `MCPAL_PROFILE`. \
        `default-profile:` at the top of `mcpal.yml` sets the fallback.\n",
    ),
```

- [ ] **Step 3: Update the book error-codes chapter**

In `/Users/pawelb/workspace/mcpal/book/src/error-codes.md`, append:

```markdown
## E0014 — template variable not set

`mcpal run` couldn't resolve a `{{profile.X}}` or `{{env.X}}` placeholder.
The error lists the unset variables. Fix by adding the key to the active profile
(`profiles.<name>.<key>:`) in `mcpal.yml`, exporting the env var, or passing
`--params-override KEY=VAL` to bypass.

## E0015 — collection not found

`mcpal run` walked from the current directory up to the filesystem root looking
for `mcpal.yml` and didn't find one. Drop a `mcpal.yml` at your project root or
pass `--collection PATH` to point at an explicit file.

## E0016 — profile not in collection

The active profile (`--profile NAME`, `MCPAL_PROFILE`, or `default-profile:`)
isn't declared in the collection's `profiles:` block. Add it, pick a different
profile, or remove the `default-profile:` key.
```

- [ ] **Step 4: Verify build**

```bash
cargo test -p mcpal --bin mcpal
```

Pass.

- [ ] **Step 5: Commit**

```bash
git add crates/mcpal/src/exit.rs book/src/error-codes.md
git -c commit.gpgsign=false commit -m "E0014 E0015 E0016 for collection errors"
```

---

### Task 8: Book chapter + README quickstart

**Files:**
- Create: `book/src/collection.md`
- Modify: `book/src/SUMMARY.md` (insert link)
- Modify: `README.md` (Quickstart 4th block)

- [ ] **Step 1: Write `book/src/collection.md`**

```markdown
# Collections

Drop a `mcpal.yml` at your project root, define saved tool calls and
profile-scoped variables, then run them by name:

```bash
mcpal run get-issue --profile prod
```

The collection file is plain YAML — check it into git, share it with
teammates, switch environments with `--profile`. Secrets stay out of
the file (`{{env.X}}` reads them at runtime from your shell or
`.envrc`).

## Minimal example

```yaml
default-profile: dev

profiles:
  dev:
    issue_id: ENG-1
    workspace: my-team
  prod:
    issue_id: ENG-999
    workspace: my-team

calls:
  get-issue:
    server: cursor:linear
    tool: get-issue
    params:
      id: "{{profile.issue_id}}"
      workspace: "{{profile.workspace}}"

  echo-token:
    server: gh
    tool: list_repos
    params:
      owner: "{{env.GH_USER}}"
```

`server` accepts any `<ref>` mcpal already understands — an alias from
`mcpal server add`, a `<source>:<name>` pair from `mcpal server
discover`, or an `https://` URL.

## Lookup

`mcpal run` walks from the current directory up to the filesystem
root looking for `mcpal.yml`. First hit wins. Override with
`--collection PATH`:

```bash
mcpal --collection ./mcpal.staging.yml run get-issue
```

If no file is found, `E0015`.

## Profiles

Pick which one is active with (in precedence order):

1. `--profile NAME` on the command line
2. `MCPAL_PROFILE` env var
3. `default-profile:` key in `mcpal.yml`
4. literal `default`

If the active name isn't a profile in the file, `E0016`.

## Templating

Two namespaces, nothing else:

- `{{profile.X}}` — reads from the active profile's key/value map.
- `{{env.X}}` — reads from your OS environment.

Substitution happens before the call is sent. Unresolved variables
fail loudly with `E0014` (all misses listed in one message); the
request never reaches the server.

Escape literal `{{` with `{{{{`.

## Dry-run

```bash
mcpal run echo --dry-run
```

Prints the resolved `(server, tool, params)` JSON and exits without
opening a connection. Useful for CI assertions on what a call *would*
do.

## One-off overrides

```bash
mcpal run echo --params-override message="custom value"
```

`--params-override` overlays raw `K=V` pairs onto the rendered params
*after* templating. Repeatable. Useful for tweaking a saved call
without editing the file.
```

(The inner code fences need to match — if your editor breaks the
nested ```` ``` ```` blocks, write them carefully.)

- [ ] **Step 2: Insert SUMMARY link**

In `/Users/pawelb/workspace/mcpal/book/src/SUMMARY.md`, under `# How-to guides`, between `- [Recipes](./recipes.md)` and `- [Authenticate to an HTTP server](./auth.md)`, insert:

```markdown
- [Collections](./collection.md)
```

- [ ] **Step 3: Append README Quickstart block**

In `/Users/pawelb/workspace/mcpal/README.md`, in the `## Quickstart` section, after the `### Browse interactively` subsection's code block (the `mcpal tui` one), add:

```markdown
### Run a saved call

```bash
# Repo has mcpal.yml at root
mcpal run get-issue --profile prod
```
```

- [ ] **Step 4: Verify markdown renders**

If `mdbook` is on PATH: `mdbook build book 2>&1 | tail -10`. Expected: no warnings. Otherwise skip — CI `book.yml` validates.

- [ ] **Step 5: Commit**

```bash
git add book/src/collection.md book/src/SUMMARY.md README.md
git -c commit.gpgsign=false commit -m "book chapter for collections"
```

---

### Task 9: Integration assertions

**Files:**
- Modify: `crates/mcpal/tests/integration.sh`

- [ ] **Step 1: Append new section**

At the end of `/Users/pawelb/workspace/mcpal/crates/mcpal/tests/integration.sh`, before the final `pass`/`fail` summary print (read the file end to confirm placement — the summary lives at the very bottom and references `$pass`/`$fail`), insert:

```bash
# ---------- collection + mcpal run ----------
section "collection + mcpal run"

COLL_DIR="$(mktemp -d -t mcpal-coll.XXXXXX)"
COLL="$COLL_DIR/mcpal.yml"
cat > "$COLL" <<'YAML'
default-profile: dev

profiles:
  dev:
    msg: "hello-dev"
  prod:
    msg: "hello-prod"

calls:
  echo:
    server: ev
    tool: echo
    params:
      message: "{{profile.msg}}"

  echo-env:
    server: ev
    tool: echo
    params:
      message: "{{env.MCPAL_RUN_TEST_VAR}}"
YAML

run_cmd() { "$BIN" --config "$CFG" --collection "$COLL" "$@"; }

it_grep 'run --dry-run prints resolved params' 'hello-dev' \
    run_cmd --output json run echo --dry-run
it_grep 'run --dry-run dry_run flag present' 'dry_run' \
    run_cmd --output json run echo --dry-run

it_grep 'run --profile prod swaps the value' 'hello-prod' \
    run_cmd --output json --profile prod run echo --dry-run

it 'run echo end-to-end (live tool call)' \
    run_cmd run echo
it_grep 'run echo response contains hello-dev' 'hello-dev' \
    run_cmd --query 'content[0].text' run echo

it_exit 'unknown call name → exit 3 (E0001)' 3 \
    run_cmd run nope
it_grep 'unknown call message names E0001' 'E0001' \
    run_cmd run nope || true

it_exit 'unknown profile → exit 2 (E0016)' 2 \
    run_cmd --profile missing run echo
it_grep_err 'unknown profile shows E0016' 'E0016' \
    run_cmd --profile missing run echo

it_exit 'missing env var → exit 2 (E0014)' 2 \
    env -u MCPAL_RUN_TEST_VAR run_cmd run echo-env
it_grep_err 'missing env var shows E0014' 'E0014' \
    env -u MCPAL_RUN_TEST_VAR run_cmd run echo-env

it_exit 'missing collection → exit 2 (E0015)' 2 \
    "$BIN" --config "$CFG" --collection "$COLL_DIR/nope.yml" run echo
it_grep_err 'missing collection shows E0015' 'E0015' \
    "$BIN" --config "$CFG" --collection "$COLL_DIR/nope.yml" run echo

it 'run --params-override overlays raw value' \
    run_cmd --query 'content[0].text' run echo --params-override message=overridden
it_grep 'override took effect' 'overridden' \
    run_cmd --query 'content[0].text' run echo --params-override message=overridden

rm -rf "$COLL_DIR"
```

The `run_cmd` shell function shadows the harness convention (other sections use `mc`) because it injects `--collection`. The pattern matches the existing `add` helper in the "server add — one-liner with auth" section.

- [ ] **Step 2: Run the suite**

```bash
MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture
```

Expected: all new rows pass; total green.

- [ ] **Step 3: Commit**

```bash
git add crates/mcpal/tests/integration.sh
git -c commit.gpgsign=false commit -m "integration assertions for mcpal run"
```

---

## Verification

After Tasks 1–9:

- `cargo fmt --all -- --check` clean.
- `cargo clippy -p mcpal --all-targets -- -D warnings` clean.
- `cargo test -p mcpal --bin mcpal` — all old + ~17 new tests pass.
- `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration` — all rows green including the new section.
- Manual smoke (using `mcpal server add ev -- npx -y @modelcontextprotocol/server-everything` to provision the reference server):

  ```bash
  TMP=$(mktemp -d)
  cat > "$TMP/mcpal.yml" <<'EOF'
  profiles:
    dev: { msg: hi }
  calls:
    echo:
      server: ev
      tool: echo
      params: { message: "{{profile.msg}}" }
  EOF
  cd "$TMP"
  mcpal --profile dev run echo --dry-run    # → {"dry_run":true,"server":"ev",…}
  mcpal --profile dev run echo              # → {"content":[{"text":"Echo: hi", …}]}
  ```

- `mcpal run --help` shows the `Examples:` after_help block.
- `mcpal debug explain E0014` (and 15, 16) prints the new prose.

CHANGELOG (post-merge, before tag): move "Added: collection file (`mcpal.yml`) + `mcpal run`" under `## [Unreleased]`. Tag `v0.3.0` once merged.

---

## Self-Review

**1. Spec coverage**

| Spec deliverable | Task |
|---|---|
| YAML schema (`profiles` + `calls`) | 2 |
| Walk-parents lookup + `--collection PATH` | 3, 5 |
| Template engine (`{{profile.X}}` + `{{env.X}}` + escape) | 4 |
| `mcpal run NAME` verb | 5, 6 |
| `--dry-run` | 5 (clap), 6 (logic) |
| `--params-override KEY=VAL` | 5 (clap), 6 (logic) |
| Profile selection precedence | 6 |
| `E0014` / `E0015` / `E0016` | 7 |
| Book chapter | 8 |
| README Quickstart block | 8 |
| Integration coverage | 9 |
| `regex` dep | 1 |

**2. Placeholder scan** — every step has runnable code or an exact shell command. The one slightly soft spot is Task 5 Step 3 ("read the constructor and propagate the field") — that's necessary because `Ctx::new`'s exact signature isn't pinned in the spec and varies per how the existing main.rs builds it. The implementer reads two short files and adds one field; not a placeholder, but a real piece of mechanical glue.

**3. Type consistency**

- `Collection`, `Call`, `find_collection`, `template::render`, `template::TemplateError`, `template::Ns`, `template::Miss` — names stable across Tasks 2–6.
- `Ctx::collection_override: Option<PathBuf>` — referenced by name in Task 6's `commands/run.rs`. Set in Task 5.
- `Ctx::profile: String` — assumed to exist as a field. Today `Cli::profile` is `String`; need to add `pub profile: String` to `Ctx` in Task 5 alongside `collection_override`. (Confirm during Task 5 Step 3; this is the glue piece called out above.)
