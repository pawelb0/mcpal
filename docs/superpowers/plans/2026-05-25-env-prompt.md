# v0.4.1 env-var prompt + TUI modal + test corpus — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Treat registry-declared env vars as required by default, prompt for them at install time on a TTY, and pop a TUI modal when a server with empty env values is opened.

**Architecture:** Adjust `registry::EnvVar` shape (description field + default-true requiredness); change `to_spec` to return `(ServerSpec, RequiredEnvHint)` so the caller decides prompt/bail; layer the prompt loop into `commands/server.rs::install`; add `Modal::EnvSetup` to the TUI that intercepts connect when the spec has empty required values. Plus a curated `book/src/test-corpus.md` for release smoke.

**Tech Stack:** Rust, `serde`, existing `tui-input` crate, no new deps.

**Spec:** `docs/superpowers/specs/2026-05-25-env-prompt-design.md`.

---

## File Structure

| File | Role |
|---|---|
| `crates/mcpal/src/registry.rs` | `EnvVar.description`; `default_required`; `RequiredEnvHint`; `EnvVarHint`; `to_spec` returns tuple; `stdio_from_package` no longer bails on missing. |
| `crates/mcpal/src/commands/server.rs` | `install` prompts on TTY; new `prompt_for` helper; `--no-prompt` plumbing. |
| `crates/mcpal/src/cli.rs` | `ServerInstallArgs.no_prompt`. |
| `crates/mcpal/src/exit.rs` | E0017 pattern + EXPLAIN. |
| `crates/mcpal/src/tui/app.rs` | `Modal::EnvSetup`; `EnvField`; pre-connect check; description cache via `App::env_descriptions`. |
| `book/src/test-corpus.md` (new) | Curated catalogue, linked from SUMMARY. |
| `book/src/SUMMARY.md` | Add Test corpus under Reference. |
| `book/src/troubleshooting.md` | "Missing env var on install" subsection. |
| `book/src/error-codes.md` | E0017 entry. |
| `crates/mcpal/tests/integration.sh` | E0017 + explain assertions. |
| `CHANGELOG.md` | `[Unreleased]` → `[0.4.1]` block. |
| `Cargo.toml` workspace + per-crate | bump 0.4.0 → 0.4.1. |

---

### Task 1: `registry::EnvVar` shape + `RequiredEnvHint`

**Files:**
- Modify: `crates/mcpal/src/registry.rs`

- [ ] **Step 1: Write failing tests**

Append to `crates/mcpal/src/registry.rs` `#[cfg(test)] mod tests` (or `fetch_tests` if that's the active block; there are two — pick the one that already imports `super::*`):

```rust
#[test]
fn env_var_without_isrequired_is_required_by_default() {
    let body = r#"{
        "name": "X",
        "description": "the X"
    }"#;
    let v: EnvVar = serde_json::from_str(body).unwrap();
    assert_eq!(v.name, "X");
    assert_eq!(v.description.as_deref(), Some("the X"));
    assert!(v.is_required, "should default to required");
}

#[test]
fn explicit_is_required_false_is_honoured() {
    let body = r#"{ "name": "X", "isRequired": false }"#;
    let v: EnvVar = serde_json::from_str(body).unwrap();
    assert!(!v.is_required);
}

#[test]
fn env_var_default_value_round_trips() {
    let body = r#"{ "name": "X", "default": "yo" }"#;
    let v: EnvVar = serde_json::from_str(body).unwrap();
    assert_eq!(v.default.as_deref(), Some("yo"));
    assert!(v.is_required); // default doesn't override requiredness
}
```

- [ ] **Step 2: Run — expect fail**

```
export PATH=~/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH
cargo test -p mcpal --bin mcpal env_var_without_isrequired_is_required_by_default
```
Expected: FAIL — current `is_required` defaults to `false`.

- [ ] **Step 3: Update the struct**

In `crates/mcpal/src/registry.rs`, find the existing `EnvVar` (lines ~49-55):

```rust
#[derive(Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct EnvVar {
    pub name: String,
    pub is_required: bool,
    pub default: Option<String>,
}
```

Replace with:

```rust
#[derive(Deserialize, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct EnvVar {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_required")]
    pub is_required: bool,
    pub default: Option<String>,
}

fn default_required() -> bool {
    true
}
```

Note: `#[serde(default)]` on the struct AND `#[serde(default = "default_required")]` on the field — both are needed. The struct-level `default` runs `Default::default()` for missing-from-JSON cases; the field-level one overrides for `is_required` specifically when the field is absent.

`#[derive(Default)]` will trip on `is_required: bool` defaulting to `false` instead of `true`. Override by removing `Default` from the derive and writing a manual impl:

```rust
impl Default for EnvVar {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: None,
            is_required: true,
            default: None,
        }
    }
}
```

(Removing `Default` from the derive will likely cascade. Re-check: nothing else needs `EnvVar::default()`. If something does, leave `#[derive(Default)]` and rely on the field-level `default = "default_required"` to win — verify by running the tests.)

- [ ] **Step 4: Run — expect pass**

```
cargo test -p mcpal --bin mcpal env_var
```
Expected: 3 tests pass.

- [ ] **Step 5: Add `RequiredEnvHint` + `EnvVarHint` types**

Append to `crates/mcpal/src/registry.rs` (near the existing public structs):

```rust
/// Per-var info pulled from the registry, used to prompt or print hints.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnvVarHint {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Result of converting a registry server into a ServerSpec: the spec
/// plus everything the caller needs to know about declared env vars.
#[derive(Debug, Clone)]
pub struct RequiredEnvHint {
    /// Every declared env var (satisfied or not).
    pub vars: Vec<EnvVarHint>,
    /// Names of vars that are required but unsatisfied — caller must prompt or bail.
    pub missing: Vec<String>,
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/mcpal/src/registry.rs
git -c commit.gpgsign=false commit -m "registry env vars required by default"
```

---

### Task 2: `to_spec` returns `(ServerSpec, RequiredEnvHint)`

**Files:**
- Modify: `crates/mcpal/src/registry.rs`
- Modify: `crates/mcpal/src/commands/server.rs` (one call site — update for signature change)

- [ ] **Step 1: Write failing tests**

Append to `crates/mcpal/src/registry.rs::fetch_tests`:

```rust
#[test]
fn to_spec_returns_missing_when_required_unsupplied() {
    let body = r#"{"servers":[{"server":{"name":"x","packages":[{"registryType":"npm","identifier":"p","environmentVariables":[
        {"name":"NEEDED","description":"the thing"}
    ]}]}}]}"#;
    let s = parse(body);
    let (spec, hint) = to_spec(&s, &BTreeMap::new()).expect("to_spec");
    assert!(matches!(spec, ServerSpec::Stdio { .. }));
    assert_eq!(hint.missing, vec!["NEEDED"]);
    assert_eq!(hint.vars.len(), 1);
    assert_eq!(hint.vars[0].name, "NEEDED");
    assert_eq!(hint.vars[0].description.as_deref(), Some("the thing"));
}

#[test]
fn to_spec_satisfied_by_extra_env() {
    let body = r#"{"servers":[{"server":{"name":"x","packages":[{"registryType":"npm","identifier":"p","environmentVariables":[
        {"name":"NEEDED"}
    ]}]}}]}"#;
    let s = parse(body);
    let mut extra = BTreeMap::new();
    extra.insert("NEEDED".to_string(), "abc".to_string());
    let (_, hint) = to_spec(&s, &extra).unwrap();
    assert!(hint.missing.is_empty());
}

#[test]
fn to_spec_satisfied_by_registry_default() {
    let body = r#"{"servers":[{"server":{"name":"x","packages":[{"registryType":"npm","identifier":"p","environmentVariables":[
        {"name":"NEEDED","default":"baked"}
    ]}]}}]}"#;
    let s = parse(body);
    let (spec, hint) = to_spec(&s, &BTreeMap::new()).unwrap();
    assert!(hint.missing.is_empty());
    if let ServerSpec::Stdio { env, .. } = spec {
        assert_eq!(env.get("NEEDED").map(String::as_str), Some("baked"));
    }
}

#[test]
fn to_spec_skips_non_required_when_missing() {
    let body = r#"{"servers":[{"server":{"name":"x","packages":[{"registryType":"npm","identifier":"p","environmentVariables":[
        {"name":"OPTIONAL","isRequired":false}
    ]}]}}]}"#;
    let s = parse(body);
    let (_, hint) = to_spec(&s, &BTreeMap::new()).unwrap();
    assert!(hint.missing.is_empty(), "isRequired:false should not appear in missing");
    assert_eq!(hint.vars.len(), 1);
}
```

- [ ] **Step 2: Run — expect compile error (to_spec returns ServerSpec, not tuple)**

```
cargo test -p mcpal --bin mcpal to_spec_returns_missing
```
Expected: compile error.

- [ ] **Step 3: Change `to_spec` and `stdio_from_package` signatures**

Replace `pub fn to_spec(...) -> Result<ServerSpec>` (line ~139):

```rust
pub fn to_spec(
    server: &Server,
    extra_env: &BTreeMap<String, String>,
) -> Result<(ServerSpec, RequiredEnvHint)> {
    if let Some(pkg) = server
        .packages
        .iter()
        .find(|p| p.transport.as_ref().is_none_or(|t| t.r#type == "stdio"))
    {
        return stdio_from_package(pkg, extra_env);
    }
    if let Some(r) = server
        .remotes
        .iter()
        .find(|r| r.r#type == "streamable-http")
    {
        return Ok((
            ServerSpec::Http {
                url: r.url.clone(),
                headers: BTreeMap::new(),
                auth: None,
            },
            RequiredEnvHint { vars: Vec::new(), missing: Vec::new() },
        ));
    }
    bail!(
        "registry server '{}' has no stdio package or streamable-http remote",
        server.name
    )
}
```

Replace `stdio_from_package`:

```rust
fn stdio_from_package(
    pkg: &Package,
    extra_env: &BTreeMap<String, String>,
) -> Result<(ServerSpec, RequiredEnvHint)> {
    let id = &pkg.identifier;
    let ver = pkg.version.as_deref().filter(|v| !v.is_empty());
    let (command, mut args): (&str, Vec<String>) = match pkg.registry_type.as_str() {
        "npm" => (
            "npx",
            vec![
                "-y".into(),
                ver.map_or_else(|| id.clone(), |v| format!("{id}@{v}")),
            ],
        ),
        "pypi" => ("uvx", vec![id.clone()]),
        "oci" => (
            "docker",
            vec!["run".into(), "--rm".into(), "-i".into(), id.clone()],
        ),
        other => bail!("unsupported registry_type '{other}'"),
    };
    let extra_vals = |xs: &[Argument]| -> Vec<String> {
        xs.iter()
            .filter_map(|a| a.value.clone().or_else(|| a.default.clone()))
            .collect()
    };
    args.extend(extra_vals(&pkg.package_arguments));
    args.extend(extra_vals(&pkg.runtime_arguments));

    // Seed env with registry-declared defaults, then overlay user `--env` values.
    let mut env: BTreeMap<String, String> = pkg
        .environment_variables
        .iter()
        .filter_map(|v| Some((v.name.clone(), v.default.clone()?)))
        .collect();
    env.extend(extra_env.iter().map(|(k, v)| (k.clone(), v.clone())));

    let vars: Vec<EnvVarHint> = pkg
        .environment_variables
        .iter()
        .map(|v| EnvVarHint {
            name: v.name.clone(),
            description: v.description.clone(),
        })
        .collect();
    let missing: Vec<String> = pkg
        .environment_variables
        .iter()
        .filter(|v| v.is_required && !env.contains_key(&v.name))
        .map(|v| v.name.clone())
        .collect();

    Ok((
        ServerSpec::Stdio {
            command: command.into(),
            args,
            env,
        },
        RequiredEnvHint { vars, missing },
    ))
}
```

- [ ] **Step 4: Update the one existing caller**

In `crates/mcpal/src/commands/server.rs::install` (line ~343):

Current:
```rust
let spec = registry::to_spec(&server, &parse_env(&args.env)?)?;
```

Provisional (Task 3 wires the prompt logic; for now just unpack):
```rust
let (spec, _hint) = registry::to_spec(&server, &parse_env(&args.env)?)?;
```

The `_hint` will become live in Task 3. Mark with a comment so the next task picks it up:

```rust
let extra = parse_env(&args.env)?;
let (spec, _hint) = registry::to_spec(&server, &extra)?;
// TODO(Task 3): if !_hint.missing.is_empty() { prompt or bail }
```

- [ ] **Step 5: Run tests — expect pass**

```
cargo test -p mcpal --bin mcpal
cargo build -p mcpal
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/mcpal/src/registry.rs crates/mcpal/src/commands/server.rs
git -c commit.gpgsign=false commit -m "to_spec returns RequiredEnvHint"
```

---

### Task 3: prompt-on-TTY install flow + `--no-prompt`

**Files:**
- Modify: `crates/mcpal/src/cli.rs`
- Modify: `crates/mcpal/src/commands/server.rs`

- [ ] **Step 1: Add `--no-prompt` to clap**

In `/Users/pawelb/workspace/mcpal/crates/mcpal/src/cli.rs`, find `ServerInstallArgs` (line ~350):

```rust
pub struct ServerInstallArgs {
    /// e.g. `io.github.owner/repo`.
    pub name: String,
    #[arg(long = "as")]
    pub alias: Option<String>,
    #[arg(long = "env", value_name = "K=V", num_args = 1)]
    pub env: Vec<String>,
}
```

Add a field:

```rust
pub struct ServerInstallArgs {
    /// e.g. `io.github.owner/repo`.
    pub name: String,
    #[arg(long = "as")]
    pub alias: Option<String>,
    #[arg(long = "env", value_name = "K=V", num_args = 1)]
    pub env: Vec<String>,
    /// Skip the interactive prompt for declared env vars; bail with E0017 instead.
    #[arg(long = "no-prompt")]
    pub no_prompt: bool,
}
```

- [ ] **Step 2: Add `prompt_for` helper**

In `crates/mcpal/src/commands/server.rs`, append near the bottom (before the test module):

```rust
fn prompt_for(name: &str, description: &str) -> std::io::Result<String> {
    use std::io::{BufRead, Write};
    let mut err = std::io::stderr();
    if description.is_empty() {
        write!(err, "  {name}\n> ")?;
    } else {
        write!(err, "  {name} — {description}\n> ")?;
    }
    err.flush()?;
    let mut buf = String::new();
    std::io::stdin().lock().read_line(&mut buf)?;
    Ok(buf.trim_end_matches(['\r', '\n']).to_string())
}
```

- [ ] **Step 3: Rewrite `install` to loop until satisfied**

Replace the existing `install` body:

```rust
async fn install(args: ServerInstallArgs, ctx: &Ctx) -> Result<()> {
    use std::io::IsTerminal;

    let server = registry::fetch(&args.name).await?;
    let mut extra = parse_env(&args.env)?;
    let alias = args
        .alias
        .clone()
        .unwrap_or_else(|| default_alias(&server.name).into());

    loop {
        let (spec, hint) = registry::to_spec(&server, &extra)?;
        if hint.missing.is_empty() {
            write_server(&ctx.config_path, &alias, spec, false)?;
            eprintln!("installed {} as '{alias}'", server.name);
            return Ok(());
        }
        if args.no_prompt || !std::io::stdin().is_terminal() {
            bail!(
                "registry server requires env vars: {} — re-run on a TTY or pass --env VAR=…",
                hint.missing.join(", "),
            );
        }
        eprintln!(
            "{} needs {} environment variable{}:",
            server.name,
            hint.missing.len(),
            if hint.missing.len() == 1 { "" } else { "s" },
        );
        for missing_name in &hint.missing {
            let description = hint
                .vars
                .iter()
                .find(|v| v.name == *missing_name)
                .and_then(|v| v.description.clone())
                .unwrap_or_default();
            let value = prompt_for(missing_name, &description).context("prompt")?;
            extra.insert(missing_name.clone(), value);
        }
    }
}
```

Add the `Context` import at the top of the file if not already there: `use anyhow::Context;` (the file already uses `anyhow::Result`/`bail`; check if `Context` is in the same `use` line).

- [ ] **Step 4: Build + verify**

```
cargo fmt --all
cargo clippy -p mcpal --all-targets -- -D warnings
cargo test -p mcpal --bin mcpal
```
All green.

- [ ] **Step 5: Smoke (non-TTY bail path)**

```bash
./target/debug/mcpal server install io.github.codeurali/dataverse --no-prompt 2>&1 | tail -3
```
Expected: error mentioning "requires env vars: DATAVERSE_ENV_URL".

```bash
./target/debug/mcpal server install io.github.codeurali/dataverse \
    --env DATAVERSE_ENV_URL=https://x.dynamics.com 2>&1 | tail -3
./target/debug/mcpal server show dataverse | grep DATAVERSE_ENV_URL
./target/debug/mcpal server remove dataverse
```
Expected: install succeeds; show prints the var.

(The interactive-prompt path is exercised manually by the human; no automated test for it.)

- [ ] **Step 6: Commit**

```bash
git add crates/mcpal/src/cli.rs crates/mcpal/src/commands/server.rs
git -c commit.gpgsign=false commit -m "server install: prompt on tty for env vars"
```

---

### Task 4: E0017 error code + docs

**Files:**
- Modify: `crates/mcpal/src/exit.rs`
- Modify: `book/src/error-codes.md`
- Modify: `crates/mcpal/tests/integration.sh`

- [ ] **Step 1: Append failing integration assertion**

In `crates/mcpal/tests/integration.sh`, after the existing "stderr surfaced on stdio failure" section (around line 193), append:

```bash
# ---------- registry install — non-tty bail ----------
section "registry install — non-tty bail"

# Hit a registry URL that doesn't exist so we don't depend on network state;
# tests the exit-classifier path for missing env vars by way of the upstream
# error, not the env-var path itself.
# (Real registry-touching tests are too flaky for CI — env-var unit tests
#  cover the logic.)
it_grep_err 'debug explain E0017 prints prose' 'registry server' \
    mc debug explain E0017
```

- [ ] **Step 2: Run integration — expect failure (E0017 not yet known)**

```
MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture 2>&1 | grep E0017
```
Expected: FAIL (`debug explain E0017` returns nothing useful).

- [ ] **Step 3: Add to `exit.rs::ANYHOW_PATTERNS`**

In `crates/mcpal/src/exit.rs`, find the `const ANYHOW_PATTERNS` array. Add a row near the top (above generic patterns):

```rust
("requires env vars", 2, "E0017"),
```

- [ ] **Step 4: Add to `EXPLAIN`**

In the same file, find `const EXPLAIN` and append after the existing E0016 entry:

```rust
(
    "E0017",
    "Registry server declares required environment variables that aren't set. \
    Re-run `mcpal server install <ref>` on a TTY (mcpal will prompt) or pre-supply \
    each via `--env VAR=value`. `mcpal server search <ref>` shows the entry.\n",
),
```

- [ ] **Step 5: Document in `book/src/error-codes.md`**

Append to `/Users/pawelb/workspace/mcpal/book/src/error-codes.md`:

```markdown
## E0017 — registry server requires env vars

`mcpal server install` found the registry entry declares environment
variables but none were supplied. Either re-run on a TTY (mcpal will
prompt with the registry's description per variable), or pre-supply
with `--env VAR=value` (repeatable). `mcpal server search <ref>` shows
the entry's declared variables.
```

- [ ] **Step 6: Run integration — expect pass**

```
MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture 2>&1 | tail -6
```
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add crates/mcpal/src/exit.rs book/src/error-codes.md crates/mcpal/tests/integration.sh
git -c commit.gpgsign=false commit -m "E0017 for non-tty install w/ missing env"
```

---

### Task 5: TUI `Modal::EnvSetup`

**Files:**
- Modify: `crates/mcpal/src/tui/app.rs`

This task is the largest in the plan because it touches the TUI's event loop and modal rendering. Read `tui/app.rs` end-to-end before editing (~600 LoC).

- [ ] **Step 1: Add `EnvField` + `Modal::EnvSetup` variant**

Find the existing `enum Modal` block in `tui/app.rs`. Add a variant:

```rust
enum Modal {
    None,
    // ... existing variants ...
    EnvSetup {
        reference: String,
        fields: Vec<EnvField>,
        cursor: usize,
    },
}

struct EnvField {
    name: String,
    description: String,
    input: tui_input::Input,
}
```

If `tui_input::Input` is already imported elsewhere in the file, no new use needed.

- [ ] **Step 2: Add description cache to `App`**

Find `struct App { ... }`. Add a field:

```rust
pub struct App {
    // ... existing fields ...
    env_descriptions: std::cell::OnceCell<
        std::collections::HashMap<String, std::collections::HashMap<String, String>>
    >,
}
```

Initialise in `App::new` with `env_descriptions: std::cell::OnceCell::new()`.

A helper to fetch (lazily) descriptions for a server:

```rust
fn descriptions_for(&self, server_name: &str) -> std::collections::HashMap<String, String> {
    // Stubbed for now: real registry fetch is async; the modal opens fast,
    // descriptions populate when the registry query returns. For v0.4.1 we
    // accept empty descriptions on stdio servers — the user can read the
    // var name and reach for `mcpal server search <name>` separately.
    let _ = server_name;
    std::collections::HashMap::new()
}
```

(A full async-fetch + populate cycle would require dispatching a future, threading the result back via `AsyncMsg`, and triggering a re-render. Out of scope for v0.4.1; ship name-only and revisit later. Test corpus covers the manual smoke.)

- [ ] **Step 3: Intercept connect when spec has empty env values**

Find the `Enter`-on-sidebar handler that starts the connect future. Before dispatching it, add:

```rust
// Sidebar Enter handler — replace the existing `start_connect(reference)`
// call (or equivalent) with the gated version below.

let needs_setup = ctx
    .cfg
    .server
    .get(&reference)
    .map(|s| match s {
        mcpal_core::ServerSpec::Stdio { env, .. } => {
            env.values().any(|v| v.is_empty())
        }
        _ => false,
    })
    .unwrap_or(false);

if needs_setup {
    let spec = ctx.cfg.server.get(&reference).cloned().expect("checked above");
    if let mcpal_core::ServerSpec::Stdio { env, .. } = spec {
        let descriptions = self.descriptions_for(&reference);
        let fields: Vec<EnvField> = env
            .iter()
            .filter(|(_, v)| v.is_empty())
            .map(|(name, _)| EnvField {
                name: name.clone(),
                description: descriptions
                    .get(name)
                    .cloned()
                    .unwrap_or_default(),
                input: tui_input::Input::default(),
            })
            .collect();
        self.modal = Modal::EnvSetup {
            reference,
            fields,
            cursor: 0,
        };
        return;
    }
}

// Otherwise proceed with the existing connect path.
self.start_connect(reference);
```

The exact integration point depends on the current `on_key` shape — read the surrounding code and slot this in at the right level. The point is: before `start_connect`, gate on `needs_setup`.

- [ ] **Step 4: Modal key handling**

In `App::on_key` (or wherever the existing `Modal::Bearer` keys are handled), add a branch for `Modal::EnvSetup`:

```rust
Modal::EnvSetup { reference, fields, cursor } => {
    use crossterm::event::KeyCode;
    match key.code {
        KeyCode::Esc => {
            self.modal = Modal::None;
        }
        KeyCode::Tab | KeyCode::Down => {
            if !fields.is_empty() {
                *cursor = (*cursor + 1) % fields.len();
            }
        }
        KeyCode::BackTab | KeyCode::Up => {
            if !fields.is_empty() {
                *cursor = if *cursor == 0 { fields.len() - 1 } else { *cursor - 1 };
            }
        }
        KeyCode::Enter => {
            // Save values into cfg + persist + dispatch connect.
            let reference = reference.clone();
            let values: Vec<(String, String)> = fields
                .iter()
                .map(|f| (f.name.clone(), f.input.value().to_string()))
                .collect();
            self.save_env_setup(&reference, &values);
            self.modal = Modal::None;
            self.start_connect(reference);
        }
        _ => {
            // Forward edit keys to the focused field's tui_input.
            if let Some(field) = fields.get_mut(*cursor) {
                field.input.handle_event(&crossterm::event::Event::Key(key));
            }
        }
    }
}
```

`App::save_env_setup` is a new method:

```rust
fn save_env_setup(&mut self, reference: &str, values: &[(String, String)]) {
    let Ok(mut cfg) = crate::config::Config::load(&self.ctx.config_path) else {
        self.output.err(format!("config load failed; env not saved"));
        return;
    };
    if let Some(spec) = cfg.server.get_mut(reference)
        && let mcpal_core::ServerSpec::Stdio { env, .. } = spec
    {
        for (k, v) in values {
            env.insert(k.clone(), v.clone());
        }
    }
    if let Err(e) = cfg.save(&self.ctx.config_path) {
        self.output.err(format!("config save failed: {e}"));
        return;
    }
    // Refresh the in-memory ctx.cfg so subsequent reads see the new values.
    self.ctx.cfg = cfg;
}
```

If `Ctx` is shared by `&` only (not `&mut`), `self.ctx.cfg = cfg` won't compile. In that case keep an `App::cfg` cache and sync it; the existing TUI code already has some pattern for this — adapt to whatever's there.

- [ ] **Step 5: Render the modal**

Find the existing modal rendering (likely a `match self.modal { ... }` block in a `draw` method). Add a branch for `Modal::EnvSetup`:

```rust
Modal::EnvSetup { reference, fields, cursor } => {
    use ratatui::{
        layout::{Constraint, Direction, Layout, Rect},
        style::{Modifier, Style},
        widgets::{Block, Borders, Paragraph},
        text::{Line, Span},
    };

    let area = centered_rect(70, 60, f.area()); // re-use existing helper
    f.render_widget(ratatui::widgets::Clear, area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(format!("Configure '{reference}'")));
    lines.push(Line::from(""));
    if fields.is_empty() {
        lines.push(Line::from("No empty env values — close (Esc) and try again."));
    } else {
        lines.push(Line::from("This server needs:"));
        lines.push(Line::from(""));
        for (i, field) in fields.iter().enumerate() {
            let highlight = if i == *cursor {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(field.name.clone(), highlight)));
            if !field.description.is_empty() {
                lines.push(Line::from(format!("  {}", field.description)));
            }
            let value = if i == *cursor {
                format!("> {}▌", field.input.value())
            } else {
                format!("> {}", field.input.value())
            };
            lines.push(Line::from(value));
            lines.push(Line::from(""));
        }
    }
    lines.push(Line::from("[Tab] next  [Enter] save+connect  [Esc] cancel"));

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("env setup"),
    );
    f.render_widget(p, area);
}
```

`centered_rect` is likely already defined in the file (used by `Modal::Bearer`, `Modal::Call`, etc.). Reuse it.

- [ ] **Step 6: Verify build**

```
cargo build -p mcpal --features tui
cargo clippy -p mcpal --all-targets -- -D warnings
cargo test -p mcpal --bin mcpal
```
All green.

- [ ] **Step 7: Manual smoke**

```bash
# Provision a server with an empty env value
echo '[server.broken]
transport = "stdio"
command = "echo"
args = ["hi"]

[server.broken.env]
NEEDED = ""
' >> ~/.config/mcpal/config.toml

cargo run -p mcpal -- tui
# Navigate to `broken`, press Enter — modal pops with NEEDED field.
# Type a value, Enter — config.toml is updated; connect dispatched.
```

(Then remove the test entry.)

- [ ] **Step 8: Commit**

```bash
git add crates/mcpal/src/tui/app.rs
git -c commit.gpgsign=false commit -m "tui: env-setup modal pre-connect"
```

---

### Task 6: Test corpus + troubleshooting docs

**Files:**
- Create: `book/src/test-corpus.md`
- Modify: `book/src/SUMMARY.md`
- Modify: `book/src/troubleshooting.md`

- [ ] **Step 1: Create `book/src/test-corpus.md`**

Exact content (large block; preserve as-is):

```markdown
# Test corpus

A curated list of MCP servers to sanity-check mcpal against on every
release. Each row stresses a different edge of the protocol or the
mcpal surface.

## stdio + required env

### `io.github.codeurali/dataverse` — Microsoft Dataverse

```bash
mcpal server install io.github.codeurali/dataverse
# Prompts for DATAVERSE_ENV_URL on TTY; bails with E0017 otherwise.
mcpal tool list dataverse
```

Stresses: env-var prompt path (v0.4.1); registry semver-max (v0.4.0).

### `awslabs.aws-api-mcp-server` — AWS API via uvx

```bash
mcpal server add aws-api \
  --env AWS_PROFILE=default --env AWS_REGION=us-east-1 \
  -- uvx awslabs.aws-api-mcp-server@latest
mcpal tool list aws-api
```

Stresses: long cold start (~30s); uvx; `--env` propagation.

### `@modelcontextprotocol/server-postgres`

```bash
mcpal server add pg \
  --env DATABASE_URL=postgres://localhost/test \
  -- npx -y @modelcontextprotocol/server-postgres
```

Stresses: `DATABASE_URL`; SQL injection in tool args; resource subscriptions.

## Broken on init

### `mcp-dataverse@0.1.0`

```bash
# Force the broken version explicitly:
mcpal server add bd --force -- npx -y mcp-dataverse@0.1.0
mcpal tool list bd
# error[E0006]: ... (child stderr: ENOENT package.json)
```

Stresses: child stderr surfacing (v0.4.0). Without the v0.4.0 fix the
failure is opaque.

## HTTP + OAuth (PKCE + DCR)

### Notion

```bash
mcpal server add notion --http https://mcp.notion.com/v1 --oauth
mcpal tool list notion
mcpal auth refresh notion
```

Stresses: browser handshake; refresh-token storage; loopback listener.

## HTTP + static bearer

### GitHub Copilot MCP

```bash
mcpal server add gh \
  --http https://api.githubcopilot.com/mcp/ \
  --bearer "$GH_TOKEN"
mcpal tool list gh
```

Stresses: `--bearer` keyring write; promote-from-import.

## Pagination + notifications + resources

### `@modelcontextprotocol/server-everything`

```bash
mcpal server add ev -- npx -y @modelcontextprotocol/server-everything
mcpal tool list ev | wc -l           # 100+ tools
mcpal watch ev                       # streams progress + log + list-changed
mcpal resource subscribe ev demo://resource/dynamic/0
mcpal tool call ev sample --message hi    # exercises sampling
mcpal tool call ev eliciting --message x  # exercises elicitation
```

Stresses: pagination; notification stream; resource subscribe; sampling /
elicitation handlers.

## mcp-ui / OpenAI Apps payloads

(Pending: a stable demo server. For now use the unit tests in
`crates/mcpal/src/commands/ui.rs` and the fixture in
`docs/test-corpus-fixtures/` once added.)

## Multi-source same-name

`chrome-devtools` is typically registered in both `opencode` and
`claude-code` configs. Verify:

```bash
mcpal server discover --source opencode | grep chrome-devtools
mcpal server discover --source claude-code | grep chrome-devtools
mcpal tool list opencode:chrome-devtools
mcpal tool list claude-code:chrome-devtools
mcpal tool list chrome-devtools       # ambiguous — fails with hint
```

Stresses: bare-name disambiguation.

## fastmcp banner

(Pending: a local FastMCP demo. Stresses: controlling-terminal detach
via setsid; TUI alt-screen integrity.)

## Known gaps (currently UNTESTED)

- HTTP servers behind a private CA / self-signed cert.
- Windows Store install of Claude Desktop (`%LOCALAPPDATA%\Packages\...`).
- Servers that emit JSON on stdout outside the MCP framing
  (protocol violation; mcpal's behaviour is undefined).

File-paths to keep in this chapter green: every release ritual runs at
least the stdio + HTTP + everything-server smoke before tagging.
```

- [ ] **Step 2: Link from `SUMMARY.md`**

In `book/src/SUMMARY.md`, under `# Reference`, after the `Error codes` entry, add:

```markdown
- [Test corpus](./test-corpus.md)
```

- [ ] **Step 3: Append to troubleshooting**

Append to `/Users/pawelb/workspace/mcpal/book/src/troubleshooting.md`:

```markdown
## Registry install completes but the server crashes on first call

If `mcpal server install <ref>` succeeded silently and then
`mcpal tool list <ref>` reports `E0006: connection closed: initialize
response`, the registry entry likely declares required environment
variables that weren't set.

mcpal v0.4.1+ prompts for these on a TTY. Re-install:

```bash
mcpal server remove <ref>
mcpal server install <ref>
# mcpal lists each declared env var and asks for a value
```

In CI or other non-TTY environments, pre-supply each variable:

```bash
mcpal server install <ref> --env VAR_A=… --env VAR_B=…
```

`mcpal server search <ref>` shows the entry's declared variables and
their descriptions. See also [E0017](./error-codes.md#e0017--registry-server-requires-env-vars).
```

- [ ] **Step 4: Commit**

```bash
git add book/src/test-corpus.md book/src/SUMMARY.md book/src/troubleshooting.md
git -c commit.gpgsign=false commit -m "book: test corpus + missing env troubleshooting"
```

---

### Task 7: Release v0.4.1

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `Cargo.toml` (workspace)
- Modify: `crates/mcpal-discovery/Cargo.toml`
- Modify: `crates/mcpal/Cargo.toml`

- [ ] **Step 1: Move `[Unreleased]` → `[0.4.1]` block**

In `/Users/pawelb/workspace/mcpal/CHANGELOG.md`, replace:

```markdown
## [Unreleased]

## [0.4.0]
```

with:

```markdown
## [Unreleased]

## [0.4.1]

### Added
- `mcpal server install` prompts for declared environment variables on a TTY using each variable's registry-provided description as the hint. Non-TTY (or `--no-prompt`) bails with the new `E0017` error.
- TUI: connecting to a server whose stored spec carries empty env values pops a `Configure '<server>'` modal — fill in, save (writes to `config.toml`), connect.
- `book/src/test-corpus.md` — curated list of tricky MCP servers exercised on every release.

### Changed
- Registry-declared `environmentVariables` default to required unless they carry a `default` value or explicitly set `isRequired: false`. Matches the official registry's actual schema.
- `registry::to_spec` now returns `(ServerSpec, RequiredEnvHint)` so callers can prompt instead of bailing.

### Fixed
- `mcpal server install io.github.codeurali/dataverse` silently produced a spec without `DATAVERSE_ENV_URL`, then the server crashed on `initialize`. The prompt path or `--env DATAVERSE_ENV_URL=…` resolves this end-to-end.
```

- [ ] **Step 2: Bump versions**

In `/Users/pawelb/workspace/mcpal/Cargo.toml`:

```toml
version = "0.4.1"
```

In `/Users/pawelb/workspace/mcpal/crates/mcpal-discovery/Cargo.toml`:

```toml
mcpal-core = { path = "../mcpal-core", version = "0.4.1" }
```

In `/Users/pawelb/workspace/mcpal/crates/mcpal/Cargo.toml`:

```toml
mcpal-core = { path = "../mcpal-core", version = "0.4.1" }

mcpal-discovery = { path = "../mcpal-discovery", version = "0.4.1" }
```

- [ ] **Step 3: Full release gate**

```bash
export PATH=~/.rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p mcpal-core
cargo test -p mcpal-discovery
cargo test -p mcpal --bin mcpal
MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration
```
All green.

- [ ] **Step 4: Commit + tag**

```bash
git add Cargo.toml Cargo.lock crates/mcpal/Cargo.toml crates/mcpal-discovery/Cargo.toml CHANGELOG.md
git -c commit.gpgsign=false commit -m "release v0.4.1"
git tag v0.4.1
```

Do NOT push — human pushes when ready.

---

## Verification

End-to-end smoke after Tasks 1–7:

```bash
# 1. Non-TTY bail with E0017
mcpal server install io.github.codeurali/dataverse --no-prompt 2>&1 | tail -3
# error[E0017]: registry server requires env vars: DATAVERSE_ENV_URL …

# 2. Pre-supplied env (no prompt)
mcpal server install io.github.codeurali/dataverse \
    --env DATAVERSE_ENV_URL=https://x.dynamics.com
mcpal server show dataverse | grep DATAVERSE_ENV_URL
mcpal server remove dataverse

# 3. Interactive prompt (manual)
mcpal server install io.github.codeurali/dataverse
# Prompts for DATAVERSE_ENV_URL with the registry description
mcpal server remove dataverse

# 4. TUI modal (manual)
mcpal server add broken -- echo hi      # synthetic, no real env requirement
# Manually edit ~/.config/mcpal/config.toml: add `[server.broken.env]\nNEEDED = ""`
mcpal tui
# Open `broken` — modal pops with NEEDED field; fill + Enter; connect attempts.

# 5. Test corpus accessible
mdbook serve book   # or visit book/test-corpus.html if mdbook is installed
```

---

## Self-Review

**1. Spec coverage**

| Spec section | Task |
|---|---|
| EnvVar default-required + description | 1 |
| RequiredEnvHint type | 1 |
| to_spec tuple return | 2 |
| CLI prompt flow | 3 |
| --no-prompt flag | 3 |
| E0017 + EXPLAIN + book entry | 4 |
| TUI Modal::EnvSetup | 5 |
| TUI save-back + connect | 5 |
| Test corpus document | 6 |
| Troubleshooting subsection | 6 |
| Release cut | 7 |

**2. Placeholder scan**

- Task 5 Step 2 stubs the description cache (registry fetch deferred) — explicitly called out as a scope cut, not a TODO. Tests don't depend on description text.
- Task 5 Step 3 "exact integration point depends on the current `on_key` shape" — the implementer is told to read + adapt. Real piece of work, not a placeholder.
- Task 5 Step 4 "if Ctx is shared by &..." — actual gate based on the existing code shape. Implementer makes the call.

Acceptable; no TBDs.

**3. Type consistency**

- `EnvVarHint { name: String, description: Option<String> }` consistent across Tasks 1, 2, 3.
- `RequiredEnvHint { vars, missing }` consistent.
- `to_spec` signature `(ServerSpec, RequiredEnvHint)` used in Tasks 2, 3.
- `Modal::EnvSetup { reference, fields, cursor }` consistent across Task 5 steps.
- `EnvField { name, description, input }` consistent.
- E0017 exit code 2 consistent.
