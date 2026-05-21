# `mcpal server add` one-liner — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the two-command flow (`server add` + `auth login`) with a single `mcpal server add` that takes `--bearer / --bearer-env / --header / --oauth`, persists credentials to the OS keyring (DPAPI on Windows, Keychain on macOS, Secret Service on Linux), and adds `--force` + `E0013` for collision handling.

**Architecture:** Five additive tasks. Task 1 grows `ServerAddArgs` and turns the `add` fn async with a file-local `AuthIntent` derivation. Task 2 wires literal/env bearers + `--header` to the existing `extract_bearer` + `keyring::put`. Task 3 extracts an `oauth_login_inline` helper from `commands/auth.rs` so `add --oauth` and `auth login --oauth` share one code path. Task 4 plumbs `--force` through `write_server` + adds `E0013` to `exit.rs`. Task 5 collapses docs. Tests are TDD: unit tests for `AuthIntent` derivation and `materialise_auth` (intent dispatcher), integration shell assertions for end-to-end.

**Tech Stack:** Rust, clap 4 (`ArgGroup`, `trailing_var_arg`), `keyring` crate, `tokio`, existing `oauth.rs` (PKCE+DCR), `tests/integration.sh` harness.

**Spec:** `docs/superpowers/specs/2026-05-20-server-add-one-liner-design.md`.

---

## File Structure

| File | Role |
|---|---|
| `crates/mcpal/src/cli.rs` | Grow `ServerAddArgs`: `bearer`, `bearer_env`, `oauth`, `header: Vec<String>`, `no_login`, `force`. Clap `ArgGroup` enforces exclusivity. |
| `crates/mcpal/src/commands/server.rs` | `add` becomes `async`. Add `AuthIntent` enum (file-local). Add `materialise_auth(name, intent, no_login, ctx)`. `write_server` gains `force: bool`. |
| `crates/mcpal/src/commands/auth.rs` | Extract `oauth_login_inline(reference: &str, override_url: Option<&str>, no_browser: bool, ctx: &Ctx) -> Result<()>` — both `add` and `login` call it. |
| `crates/mcpal/src/commands/mod.rs` | Dispatch of `Server::Add` to `add(args, ctx).await`. |
| `crates/mcpal/src/exit.rs` | New `ANYHOW_PATTERNS` row + `EXPLAIN` row for `E0013 server already exists`. |
| `crates/mcpal/tests/integration.sh` | New "server add — one-liner" section. Update existing `E0000`-duplicate test to `E0013`. |
| `README.md`, `book/src/getting-started.md`, `book/src/auth.md`, `book/src/error-codes.md` | Collapse two-command pairs to one line; document `E0013`. |

---

### Task 1: Clap surface + `AuthIntent` derivation (pure, no side-effects)

**Files:**
- Modify: `crates/mcpal/src/cli.rs:326-345` (extend `ServerAddArgs`)
- Modify: `crates/mcpal/src/commands/server.rs:107-134` (split into pure intent derivation + side-effecting `add`)
- Modify: `crates/mcpal/src/commands/mod.rs` (dispatch becomes `.await`)
- Test: `crates/mcpal/src/commands/server.rs` `#[cfg(test)] mod tests` (already exists, append)

- [ ] **Step 1: Write the failing intent-derivation tests**

Append to `crates/mcpal/src/commands/server.rs` `mod tests`:

```rust
fn args(alias: &str) -> crate::cli::ServerAddArgs {
    crate::cli::ServerAddArgs {
        alias: alias.into(),
        stdio: None,
        args: vec![],
        env: vec![],
        http: None,
        bearer: None,
        bearer_env: None,
        oauth: false,
        header: vec![],
        no_login: false,
        force: false,
        command: vec![],
    }
}

#[test]
fn intent_none_when_no_auth_flags() {
    let mut a = args("x");
    a.http = Some("https://x".into());
    let (spec, intent) = derive(a).expect("derive");
    assert!(matches!(intent, AuthIntent::None));
    assert!(matches!(spec, ServerSpec::Http { .. }));
}

#[test]
fn intent_literal_from_bearer_flag() {
    let mut a = args("x");
    a.http = Some("https://x".into());
    a.bearer = Some("abc".into());
    let (_, intent) = derive(a).expect("derive");
    assert!(matches!(intent, AuthIntent::Literal(ref t) if t == "abc"));
}

#[test]
fn intent_env_from_bearer_env_flag() {
    let mut a = args("x");
    a.http = Some("https://x".into());
    a.bearer_env = Some("GH_TOKEN".into());
    let (spec, intent) = derive(a).expect("derive");
    assert!(matches!(intent, AuthIntent::Env(ref v) if v == "GH_TOKEN"));
    if let ServerSpec::Http { auth, .. } = spec {
        assert!(matches!(
            auth,
            Some(mcpal_core::AuthSpec::BearerEnv { env }) if env == "GH_TOKEN"
        ));
    } else {
        panic!("expected http spec");
    }
}

#[test]
fn intent_oauth_from_oauth_flag() {
    let mut a = args("x");
    a.http = Some("https://x".into());
    a.oauth = true;
    let (spec, intent) = derive(a).expect("derive");
    assert!(matches!(intent, AuthIntent::Oauth));
    if let ServerSpec::Http { auth, .. } = spec {
        assert!(matches!(auth, Some(mcpal_core::AuthSpec::Oauth)));
    }
}

#[test]
fn header_authorization_bearer_promotes_to_literal() {
    let mut a = args("x");
    a.http = Some("https://x".into());
    a.header = vec!["Authorization: Bearer abc".into()];
    let (spec, intent) = derive(a).expect("derive");
    assert!(matches!(intent, AuthIntent::Literal(ref t) if t == "abc"));
    if let ServerSpec::Http { headers, .. } = spec {
        assert!(!headers.keys().any(|k| k.eq_ignore_ascii_case("authorization")));
    }
}

#[test]
fn header_authorization_env_promotes_to_env() {
    let mut a = args("x");
    a.http = Some("https://x".into());
    a.header = vec!["Authorization: Bearer ${GH_TOKEN}".into()];
    let (_, intent) = derive(a).expect("derive");
    assert!(matches!(intent, AuthIntent::Env(ref v) if v == "GH_TOKEN"));
}

#[test]
fn header_non_authorization_kept_in_spec() {
    let mut a = args("x");
    a.http = Some("https://x".into());
    a.header = vec!["X-Api-Key: k1".into()];
    let (spec, intent) = derive(a).expect("derive");
    assert!(matches!(intent, AuthIntent::None));
    if let ServerSpec::Http { headers, .. } = spec {
        assert_eq!(headers.get("X-Api-Key").map(String::as_str), Some("k1"));
    }
}

#[test]
fn stdio_with_bearer_is_rejected() {
    let mut a = args("x");
    a.command = vec!["echo".into(), "hi".into()];
    a.bearer = Some("abc".into());
    let err = derive(a).unwrap_err();
    assert!(err.to_string().contains("--http"));
}

#[test]
fn header_missing_colon_is_rejected() {
    let mut a = args("x");
    a.http = Some("https://x".into());
    a.header = vec!["NoColonHere".into()];
    let err = derive(a).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("header"));
}
```

- [ ] **Step 2: Run tests — expect failure (symbols undefined)**

Run: `cargo test -p mcpal --lib commands::server::tests`
Expected: compile error — `bearer`, `bearer_env`, `oauth`, `header`, `no_login`, `force` fields don't exist on `ServerAddArgs`; `derive` and `AuthIntent` don't exist.

- [ ] **Step 3: Extend `ServerAddArgs` in `cli.rs`**

Replace `crates/mcpal/src/cli.rs:326-345` `ServerAddArgs` with:

```rust
#[derive(clap::Args, Debug)]
#[command(group(
    clap::ArgGroup::new("auth-mode")
        .args(["bearer", "bearer_env", "oauth"])
        .multiple(false)
        .required(false)
))]
pub struct ServerAddArgs {
    pub alias: String,
    #[arg(long, conflicts_with = "http")]
    pub stdio: Option<String>,
    #[arg(
        long = "arg",
        value_name = "ARG",
        num_args = 1,
        allow_hyphen_values = true
    )]
    pub args: Vec<String>,
    #[arg(long = "env", value_name = "K=V", num_args = 1)]
    pub env: Vec<String>,
    #[arg(long)]
    pub http: Option<String>,
    /// Literal token (or `-` for stdin) → OS keyring.
    #[arg(long, value_name = "TOKEN|-")]
    pub bearer: Option<String>,
    /// Spec auth = bearer_env { env = VAR } — token read from env at runtime.
    #[arg(long = "bearer-env", value_name = "VAR")]
    pub bearer_env: Option<String>,
    /// Run the OAuth 2.1 (PKCE + DCR) browser flow inline.
    #[arg(long)]
    pub oauth: bool,
    /// Pass `K: V` to the HTTP server. `Authorization: Bearer …` is auto-promoted to keyring/bearer_env.
    #[arg(long = "header", value_name = "K: V", num_args = 1)]
    pub header: Vec<String>,
    /// With `--oauth`: write the spec but skip the browser handshake.
    #[arg(long = "no-login")]
    pub no_login: bool,
    /// Overwrite an existing entry of the same name.
    #[arg(long)]
    pub force: bool,
    /// `mcpal server add ev -- npx -y @mcp/server-everything`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
    pub command: Vec<String>,
}
```

- [ ] **Step 4: Add `AuthIntent` + `derive` in `commands/server.rs`**

After `BearerSource` (~line 153) add:

```rust
/// Pure derivation of the persisted side-effects from CLI input.
/// No I/O — every test above pins behaviour deterministically.
#[derive(Debug, PartialEq, Eq)]
enum AuthIntent {
    None,
    Literal(String),
    Env(String),
    Oauth,
}

fn derive(args: ServerAddArgs) -> Result<(ServerSpec, AuthIntent)> {
    let (command, stdio_args) = match (args.stdio, args.command.split_first()) {
        (Some(_), Some(_)) => bail!("can't combine `--stdio` with a trailing `-- <cmd>`"),
        (Some(cmd), None) => (Some(cmd), args.args),
        (None, Some((c, rest))) => {
            if !args.args.is_empty() {
                bail!("can't combine `--arg` with a trailing `-- <cmd>`");
            }
            (Some(c.clone()), rest.to_vec())
        }
        (None, None) => (None, args.args),
    };
    let is_stdio = command.is_some();
    let auth_flags_present = args.bearer.is_some()
        || args.bearer_env.is_some()
        || args.oauth
        || args
            .header
            .iter()
            .any(|h| h.split_once(':').is_some_and(|(k, _)| k.eq_ignore_ascii_case("authorization")));
    if is_stdio && auth_flags_present {
        bail!("auth flags require --http (stdio servers carry no Authorization)");
    }

    let mut spec = match (command, args.http) {
        (Some(_), Some(_)) => bail!("--stdio/`-- cmd` and --http are mutually exclusive"),
        (Some(cmd), None) => ServerSpec::Stdio {
            command: cmd,
            args: stdio_args,
            env: parse_env(&args.env)?,
        },
        (None, Some(url)) => {
            let mut headers = BTreeMap::new();
            for h in &args.header {
                let (k, v) = h.split_once(':').ok_or_else(|| {
                    anyhow!("--header needs `K: V`, got: {h}")
                })?;
                headers.insert(k.trim().to_string(), v.trim().to_string());
            }
            ServerSpec::Http {
                url,
                headers,
                auth: None,
            }
        }
        (None, None) => bail!("provide a stdio command (`-- cmd args…`) or `--http <url>`"),
    };

    // 1) header-derived Authorization wins as the *baseline*.
    let header_intent = match extract_bearer(&mut spec) {
        BearerSource::None => AuthIntent::None,
        BearerSource::Literal(t) => AuthIntent::Literal(t),
        BearerSource::Env(v) => AuthIntent::Env(v),
    };

    // 2) explicit --bearer / --bearer-env / --oauth override the header path.
    let intent = if let Some(t) = args.bearer {
        AuthIntent::Literal(t)
    } else if let Some(v) = args.bearer_env {
        if let ServerSpec::Http { auth, .. } = &mut spec {
            *auth = Some(mcpal_core::AuthSpec::BearerEnv { env: v.clone() });
        }
        AuthIntent::Env(v)
    } else if args.oauth {
        if let ServerSpec::Http { auth, .. } = &mut spec {
            *auth = Some(mcpal_core::AuthSpec::Oauth);
        }
        AuthIntent::Oauth
    } else {
        header_intent
    };

    Ok((spec, intent))
}
```

Also add the import at the top of the file (next to other `mcpal_core::*` uses):
```rust
use mcpal_core::AuthSpec;
```
(if not already in scope — check the existing `use` block; `ServerSpec` is already imported.)

- [ ] **Step 5: Rewrite `add` to call `derive` then write (stub side-effects for now)**

Replace `crates/mcpal/src/commands/server.rs:107-134` with:

```rust
async fn add(args: ServerAddArgs, ctx: &Ctx) -> Result<()> {
    let alias = args.alias.clone();
    let no_login = args.no_login;
    let force = args.force;
    let (spec, intent) = derive(args)?;
    let transport = match &spec {
        ServerSpec::Http { .. } => "http",
        ServerSpec::Stdio { .. } => "stdio",
    };
    write_server(&ctx.config_path, &alias, spec, force)?;
    materialise_auth(&alias, &intent, no_login, ctx).await?;
    ctx.render_one(&json!({
        "ok": true,
        "ref": alias,
        "transport": transport,
        "auth": auth_label(&intent),
    }))?;
    Ok(())
}

fn auth_label(intent: &AuthIntent) -> &'static str {
    match intent {
        AuthIntent::None => "none",
        AuthIntent::Literal(_) => "bearer",
        AuthIntent::Env(_) => "bearer_env",
        AuthIntent::Oauth => "oauth",
    }
}

async fn materialise_auth(
    _alias: &str,
    _intent: &AuthIntent,
    _no_login: bool,
    _ctx: &Ctx,
) -> Result<()> {
    // Filled out in Task 2 + Task 3. Keeping a no-op stub here so the
    // intent-derivation tests can run.
    Ok(())
}
```

Update `write_server` signature (still at server.rs:237-246) to accept `force`:

```rust
fn write_server(path: &std::path::Path, alias: &str, spec: ServerSpec, force: bool) -> Result<()> {
    let mut cfg = Config::load(path)?;
    if cfg.server.contains_key(alias) && !force {
        bail!("server '{alias}' already exists");
    }
    cfg.server.insert(alias.into(), spec);
    cfg.save(path)?;
    eprintln!("added server '{alias}'");
    Ok(())
}
```

Update the one other caller in `crates/mcpal/src/commands/server.rs::install` (line 232) and `import` (line 145) to pass `false`:

```rust
write_server(&ctx.config_path, &alias, spec, false)?;
```

- [ ] **Step 6: Switch dispatch to async**

Find the `Server::Add` arm in `crates/mcpal/src/commands/mod.rs` (or wherever `ServerAction` is matched). Change:

```rust
ServerAction::Add(args) => add(args, ctx),
```

to:

```rust
ServerAction::Add(args) => add(args, ctx).await,
```

If the surrounding function is not async, add `async`/`.await` accordingly.

- [ ] **Step 7: Run tests — expect pass on derive, stub on side-effects**

Run: `cargo test -p mcpal --lib commands::server::tests`
Expected: all `intent_*` and `header_*` tests pass. Build clean. Stubbed `materialise_auth` exercises no I/O yet.

- [ ] **Step 8: Commit**

```bash
git add crates/mcpal/src/cli.rs crates/mcpal/src/commands/server.rs crates/mcpal/src/commands/mod.rs
git commit -m "feat(cli): auth flags on server add"
```

---

### Task 2: Materialise bearer / bearer-env to keyring

**Files:**
- Modify: `crates/mcpal/src/commands/server.rs` (`materialise_auth` body, stdin handling)
- Test: `crates/mcpal/tests/integration.sh`

- [ ] **Step 1: Write the failing integration assertions**

Append to `crates/mcpal/tests/integration.sh` immediately after the existing `server add stdio via -- cmd` section (after line ~95). Use the existing `it` / `it_grep` / `it_exit` helpers — match the surrounding style. Use a fresh `MCPAL_CONFIG` tempfile per assertion if the harness needs isolation; otherwise re-use `MCPAL_TMPDIR` already in scope:

```bash
section "server add — one-liner with auth"

ADD_DIR="$(mktemp -d -t mcpal-add.XXXXXX)"
ADD_CFG="$ADD_DIR/config.toml"
add() { MCPAL_CONFIG="$ADD_CFG" mc "$@"; }

it 'add --bearer (literal) writes keyring + spec has no Authorization' \
    add server add T1 --http http://example.invalid/mcp --bearer abc
it_grep 'T1 spec keeps auth field absent' '^\[server\.T1\]' \
    cat "$ADD_CFG"
it_grep 'T1 spec is http' 'transport = "http"' \
    cat "$ADD_CFG"
it_no_grep 'T1 spec has no Authorization' 'Authorization' \
    cat "$ADD_CFG"
it 'auth status reports bearer present' \
    add auth status T1
# Cleanup the keyring entry so reruns are idempotent.
add auth logout T1 >/dev/null 2>&1 || true

it 'add --bearer-env sets bearer_env in spec' \
    add server add T2 --http http://example.invalid/mcp --bearer-env GH_TOKEN
it_grep 'T2 spec has bearer_env' 'type = "bearer_env"' \
    cat "$ADD_CFG"
it_grep 'T2 spec carries env var' 'env = "GH_TOKEN"' \
    cat "$ADD_CFG"

it 'add --header Authorization: Bearer literal == --bearer' \
    add server add T3 --http http://example.invalid/mcp --header 'Authorization: Bearer xyz'
it_grep 'T3 auth status bearer present' '"bearer": true' \
    add --output json auth status T3
add auth logout T3 >/dev/null 2>&1 || true

it 'add --header X-Api-Key kept in spec, no auth' \
    add server add T4 --http http://example.invalid/mcp --header 'X-Api-Key: k1'
it_grep 'T4 spec has X-Api-Key' 'X-Api-Key' \
    cat "$ADD_CFG"
it_no_grep 'T4 spec has no bearer_env' 'bearer_env' \
    cat "$ADD_CFG"

it 'add stdio (no auth flags)' \
    add server add T5 -- echo hi
it_exit 'add stdio + --bearer is rejected' 2 \
    add server add T6 --bearer x -- echo hi

it 'add --bearer - (stdin)' \
    bash -c "echo stdintok | MCPAL_CONFIG=$ADD_CFG '$MCPAL_BIN' server add T7 --http http://example.invalid/mcp --bearer -"
it_grep 'T7 bearer present via stdin' '"bearer": true' \
    add --output json auth status T7
add auth logout T7 >/dev/null 2>&1 || true

rm -rf "$ADD_DIR"
```

Note: `it_no_grep` may not exist in the harness. If absent, define it once near the top of the new section:

```bash
it_no_grep() {
    local label="$1" pattern="$2"; shift 2
    local out; out="$("$@" 2>&1 || true)"
    if printf '%s\n' "$out" | grep -q -- "$pattern"; then
        echo "FAIL: $label (matched '$pattern')"; fails=$((fails+1))
    else
        echo "ok:   $label"; oks=$((oks+1))
    fi
}
```

- [ ] **Step 2: Run integration script — expect failure (materialise is a no-op)**

Run: `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture`
Expected: assertions like "T1 bearer present", "T3 bearer present" FAIL because `materialise_auth` is still a stub.

- [ ] **Step 3: Implement `materialise_auth` for bearer / bearer-env**

Replace the stub in `crates/mcpal/src/commands/server.rs` with:

```rust
async fn materialise_auth(
    alias: &str,
    intent: &AuthIntent,
    no_login: bool,
    _ctx: &Ctx,
) -> Result<()> {
    match intent {
        AuthIntent::None => Ok(()),
        AuthIntent::Literal(token) => {
            let token = if token == "-" {
                read_token_stdin()?
            } else {
                token.clone()
            };
            if token.is_empty() {
                bail!("no token on stdin");
            }
            keyring::put(alias, keyring::Kind::Bearer, &token)?;
            Ok(())
        }
        AuthIntent::Env(_) => Ok(()), // spec already carries bearer_env
        AuthIntent::Oauth => {
            let _ = no_login;
            // Filled out in Task 3.
            Ok(())
        }
    }
}

fn read_token_stdin() -> Result<String> {
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).context("read stdin")?;
    Ok(buf.trim().to_string())
}
```

Add `use anyhow::Context;` if not already at the top of the file. Add `use crate::keyring;` if not already imported (it may already be — check the existing `use` block).

- [ ] **Step 4: Run integration script again — expect bearer-related rows green, oauth row still no-op**

Run: `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture`
Expected: all T1–T7 rows pass.

- [ ] **Step 5: Commit**

```bash
git add crates/mcpal/src/commands/server.rs crates/mcpal/tests/integration.sh
git commit -m "feat(server-add): materialise bearer / bearer-env"
```

---

### Task 3: Inline `--oauth` shares the `auth login --oauth` path

**Files:**
- Modify: `crates/mcpal/src/commands/auth.rs` (extract `oauth_login_inline`)
- Modify: `crates/mcpal/src/commands/server.rs` (call it from `materialise_auth`)
- Test: unit test in `crates/mcpal/src/commands/server.rs`

- [ ] **Step 1: Write the failing unit test**

Append to `crates/mcpal/src/commands/server.rs` `mod tests`:

```rust
#[tokio::test]
async fn oauth_with_no_login_does_not_call_browser() {
    // We can't reach the network here. `--no-login` must short-circuit
    // before any OAuth attempt, so this returns Ok immediately.
    let ctx = crate::runtime::Ctx::test_default();
    let r = materialise_auth("oa1", &AuthIntent::Oauth, true, &ctx).await;
    assert!(r.is_ok(), "expected --no-login to short-circuit: {r:?}");
}
```

If `Ctx::test_default()` does not exist, define one in the test-only path in `crates/mcpal/src/runtime.rs` returning a `Ctx` pointed at a tempfile config (search the codebase for an existing test ctor — there may already be one). Skip this test if Ctx is not easily constructible and rely on integration coverage instead — note that decision in the commit message.

- [ ] **Step 2: Run test — expect fail**

Run: `cargo test -p mcpal --lib commands::server::tests::oauth_with_no_login`
Expected: FAIL (materialise_auth's oauth arm is currently a no-op so this might pass, but we'll bring it to a real fail by tightening the arm next).

- [ ] **Step 3: Extract `oauth_login_inline` in `auth.rs`**

In `crates/mcpal/src/commands/auth.rs`, refactor:

```rust
pub(crate) async fn oauth_login_inline(
    reference: &str,
    override_url: Option<&str>,
    no_browser: bool,
    ctx: &Ctx,
) -> Result<()> {
    let url = http_url(reference, override_url, ctx)?;
    oauth::login(reference, &url, !no_browser).await?;
    Ok(())
}
```

Then update the existing `login` function's `if oauth_flag { … }` branch to call it:

```rust
if oauth_flag {
    oauth_login_inline(reference, url, no_browser, ctx).await?;
    ctx.render_one(&json!({"ok": true, "ref": reference, "method": "oauth"}))?;
    return Ok(());
}
```

`http_url` is already at `auth.rs:85` — no change needed.

- [ ] **Step 4: Wire `materialise_auth`'s oauth arm**

In `crates/mcpal/src/commands/server.rs`, replace the `AuthIntent::Oauth` arm of `materialise_auth`:

```rust
AuthIntent::Oauth => {
    if no_login {
        return Ok(());
    }
    crate::commands::auth::oauth_login_inline(alias, None, false, _ctx).await
}
```

Rename `_ctx` → `ctx` in the function signature and at the call site (it now has a real consumer).

- [ ] **Step 5: Run unit + integration tests**

Run: `cargo test -p mcpal --lib`
Expected: PASS.

Run: `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture`
Expected: PASS — `auth login --oauth` behaviour against `oauth_mock` is unchanged.

- [ ] **Step 6: Add an integration assertion for `add --oauth --no-login`**

Insert near the other one-liner section in `tests/integration.sh`:

```bash
it 'add --oauth --no-login writes spec only (no browser)' \
    add server add T8 --http http://example.invalid/mcp --oauth --no-login
it_grep 'T8 spec has auth = oauth' 'type = "oauth"' \
    cat "$ADD_CFG"
```

Run: `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/mcpal/src/commands/auth.rs crates/mcpal/src/commands/server.rs crates/mcpal/tests/integration.sh
git commit -m "feat(server-add): inline --oauth"
```

---

### Task 4: `--force` + `E0013 server already exists`

**Files:**
- Modify: `crates/mcpal/src/exit.rs` (new `ANYHOW_PATTERNS` row + new `EXPLAIN` row)
- Modify: `crates/mcpal/src/commands/server.rs::write_server` (already accepts `force` from Task 1 — verify it bails with a stable phrase)
- Modify: `crates/mcpal/tests/integration.sh` (existing duplicate-test rewires to `E0013`)
- Modify: `book/src/error-codes.md` (add `E0013`)

- [ ] **Step 1: Write the failing integration assertion**

Replace the existing `it_exit 'server add duplicate fails (E0000)' 1` block (`tests/integration.sh` ~line 91) with:

```bash
it_exit     'server add duplicate fails (E0013)' 2 \
            mc server add "$REF" -- npx -y @modelcontextprotocol/server-everything
it_grep     'server add duplicate names E0013' 'E0013' \
            mc server add "$REF" -- npx -y @modelcontextprotocol/server-everything
it          'server add --force overwrites existing' \
            mc server add "$REF" --force -- npx -y @modelcontextprotocol/server-everything
```

- [ ] **Step 2: Run integration script — expect failure**

Run: `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture`
Expected: FAIL — exit code is currently 1/E0000, not 2/E0013.

- [ ] **Step 3: Add `E0013` to `exit.rs`**

In `crates/mcpal/src/exit.rs`, add to `ANYHOW_PATTERNS` (before the generic patterns):

```rust
("already exists", 2, "E0013"),
```

Add to `EXPLAIN` (alphabetical-by-code; goes after E0012):

```rust
(
    "E0013",
    "Server name already registered. Run `mcpal server list` to see what \
    you have, or re-run with `--force` to overwrite. `mcpal server remove \
    <name>` deletes the entry first.\n",
),
```

- [ ] **Step 4: Run integration script again**

Run: `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture`
Expected: PASS — duplicate is exit 2 with E0013, `--force` succeeds.

- [ ] **Step 5: Document in `book/src/error-codes.md`**

Append to `book/src/error-codes.md`:

```markdown
## E0013 — server already exists

`mcpal server add <name>` failed because `<name>` is already in the
config. Pick a different name, run `mcpal server remove <name>` first,
or re-run with `--force` to overwrite. `mcpal server list` shows the
current entries.
```

- [ ] **Step 6: Commit**

```bash
git add crates/mcpal/src/exit.rs crates/mcpal/tests/integration.sh book/src/error-codes.md
git commit -m "feat(server-add): --force + E0013"
```

---

### Task 5: Documentation — collapse two commands to one line

**Files:**
- Modify: `README.md` (Quickstart "Add your own")
- Modify: `book/src/getting-started.md` (step 4)
- Modify: `book/src/auth.md` (top)

- [ ] **Step 1: README Quickstart**

In `README.md`, find the "Add your own" subsection of Quickstart. Replace:

```markdown
# HTTP + OAuth 2.1 (PKCE + DCR)
mcpal server add notion --http https://mcp.notion.com/v1
mcpal auth login notion --oauth
mcpal tool list notion
```

with:

```markdown
# HTTP + bearer (literal token → OS keyring)
mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer $GH_TOKEN

# HTTP + OAuth 2.1 (PKCE + DCR) — browser opens inline
mcpal server add notion --http https://mcp.notion.com/v1 --oauth
mcpal tool list notion
```

- [ ] **Step 2: `book/src/getting-started.md`**

Locate any step where a separate `mcpal auth login` follows a `server add`. Collapse to a single `server add … --bearer $TOKEN` (or `--oauth`) line. Keep the prose explanation short — point readers at `book/src/auth.md` for the long form.

- [ ] **Step 3: `book/src/auth.md` lead**

Prepend a "Most users want the one-liner" block right under the H1:

```markdown
## One-liner

```bash
mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer $GH_TOKEN
mcpal server add notion --http https://mcp.notion.com/v1 --oauth
```

`mcpal auth login` is for rotating a token later or recovering from a
mid-OAuth failure — see below.
```

- [ ] **Step 4: Build the book locally (optional but recommended)**

Run: `mdbook build book` (skip if `mdbook` isn't on PATH — CI `book.yml` validates).
Expected: no broken-link warnings; the page renders.

- [ ] **Step 5: Commit**

```bash
git add README.md book/src/getting-started.md book/src/auth.md
git commit -m "docs: collapse add+login to one liner"
```

---

---

### Task 6: Strip AWS-CLI references + enrich `--help` with examples

mcpal's docs and clap doc-strings repeatedly call out "AWS-CLI style"
/ "AWS-CLI JMESPath filter" / "AWS CLI for MCP" — drop the comparison.
The names `--cli-input-json` and `--query` stay (the *flag names* are
fine, they are widely understood); only the *phrasing* changes.

In the same pass, add clap `after_help` blocks to the four most
copy-pasted subcommands so `mcpal <cmd> --help` shows working
examples.

**Files:**
- Modify: `crates/mcpal/src/cli.rs` (rename doc on `--query`; add `after_help` blocks)
- Modify: `crates/mcpal/src/kv.rs:10` (doc comment)
- Modify: `crates/mcpal/src/exit.rs:115` (E0002 explain text)
- Modify: `book/src/recipes.md:198`, `book/src/scripting.md:51`, `book/src/error-codes.md:43`
- Modify: `demo/README.md:9`

- [ ] **Step 1: Write the failing test (snapshot expectation in integration)**

Append to `tests/integration.sh`:

```bash
section "help text contains key examples"
it_grep 'server add --help shows --bearer example' 'mcpal server add gh --http' \
    mc server add --help
it_grep 'tool call --help shows --params example' '--params' \
    mc tool call --help
it_no_grep 'no AWS-CLI references in --query help' 'AWS-CLI' \
    mc server add --help
it_no_grep 'no AWS-CLI references in tool call --help' 'AWS-CLI' \
    mc tool call --help
```

- [ ] **Step 2: Run — expect failure**

Run: `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture`
Expected: FAIL (no examples in `--help`; AWS-CLI string still present).

- [ ] **Step 3: Rename `--query` help text**

In `crates/mcpal/src/cli.rs` (search for `AWS-CLI JMESPath filter`):

```rust
/// JMESPath filter applied to the response.
#[arg(long = "query", value_name = "JMESPATH", global = true)]
pub query: Option<String>,
```

Replace `AWS-CLI JMESPath filter` with `JMESPath filter applied to the response.`

- [ ] **Step 4: Add `after_help` to `server add`**

In `crates/mcpal/src/cli.rs`, decorate `ServerAddArgs`:

```rust
#[derive(clap::Args, Debug)]
#[command(
    after_help = "Examples:\n  \
        mcpal server add ev -- npx -y @modelcontextprotocol/server-everything\n  \
        mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer $GH_TOKEN\n  \
        mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer-env GH_TOKEN\n  \
        mcpal server add notion --http https://mcp.notion.com/v1 --oauth\n  \
        mcpal server add aws-api --env AWS_PROFILE=default -- uvx awslabs.aws-api-mcp-server@latest\n  \
        echo $TOKEN | mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer -",
    group(
        clap::ArgGroup::new("auth-mode")
            .args(["bearer", "bearer_env", "oauth"])
            .multiple(false)
            .required(false)
    ),
)]
pub struct ServerAddArgs { /* …unchanged… */ }
```

(Merges with the `ArgGroup` from Task 1 — both go inside the same `#[command(...)]`.)

- [ ] **Step 5: Add `after_help` to `tool call`**

Find the `ToolAction::Call` variant in `crates/mcpal/src/cli.rs`. Add `#[command(after_help = "Examples:\n  …")]`:

```rust
/// Call a tool.
#[command(after_help = "Examples:\n  \
    mcpal tool call ev echo --message hi\n  \
    mcpal tool call ev echo --params '{\"message\":\"hi\"}'\n  \
    echo '{\"message\":\"hi\"}' | mcpal tool call ev echo --params -\n  \
    mcpal tool call ev echo --cli-input-json @body.json\n  \
    mcpal --query 'content[0].text' tool call ev echo --message hi")]
Call {
    /* …unchanged fields… */
},
```

- [ ] **Step 6: Add `after_help` to `auth login`**

In `crates/mcpal/src/cli.rs`, on the `AuthAction::Login` variant:

```rust
/// Store a bearer or run the OAuth 2.1 flow.
#[command(after_help = "Examples:\n  \
    mcpal auth login gh --bearer $GH_TOKEN\n  \
    echo $TOKEN | mcpal auth login gh --bearer -\n  \
    mcpal auth login notion --oauth\n\nMost users want `mcpal server add … --bearer` instead — this is the rotation entry-point.")]
Login {
    /* …unchanged fields… */
},
```

- [ ] **Step 7: Add `after_help` to `raw`**

```rust
/// Send arbitrary JSON-RPC.
#[command(after_help = "Examples:\n  \
    mcpal raw ev tools/list\n  \
    mcpal raw ev some/method --params '{\"k\":\"v\"}'\n  \
    mcpal raw ev some/method --params @payload.json\n  \
    cat payload.json | mcpal raw ev some/method --params -")]
Raw { /* … */ },
```

(If `Raw` is not a `ToolAction`-style enum variant but a top-level
`Commands::Raw { … }`, attach `after_help` to that variant instead —
search for `Send arbitrary JSON-RPC`.)

- [ ] **Step 8: Strip remaining AWS-CLI strings**

| File:line | Current | Replacement |
|---|---|---|
| `crates/mcpal/src/kv.rs:10` | `Walk \`--key value\` pairs (AWS-CLI style) into a typed JSON object.` | `Walk \`--key value\` pairs into a typed JSON object.` |
| `crates/mcpal/src/exit.rs:115` | `"Bad arguments. Use AWS-CLI style \`--key value\`; …` | `"Bad arguments. Pass \`--key value\` pairs; …` |
| `book/src/recipes.md:198` | `\`--cli-input-json\` is the AWS-CLI-compatible alias.` | `\`--cli-input-json\` accepts a base body from a path or \`-\` (stdin).` |
| `book/src/scripting.md:51` | `Same syntax as AWS-CLI \`--query\`.` | `Standard JMESPath.` |
| `book/src/error-codes.md:43` | `Pass \`--key value\` pairs (AWS-CLI style): \`mcpal tool call ev echo …\`` | `Pass \`--key value\` pairs: \`mcpal tool call ev echo …\`` |
| `demo/README.md:9` | `"AWS CLI for MCP": register a stdio server, …` | `"CLI for MCP": register a stdio server, …` |

- [ ] **Step 9: Run tests**

Run: `cargo test -p mcpal && MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture`
Expected: PASS.

- [ ] **Step 10: Visually inspect `--help` outputs**

```bash
./target/debug/mcpal server add --help
./target/debug/mcpal tool call --help
./target/debug/mcpal auth login --help
./target/debug/mcpal raw --help
```

Confirm each ends with an "Examples:" block. No "AWS-CLI" anywhere.

- [ ] **Step 11: Commit**

```bash
git add crates/mcpal/src/cli.rs crates/mcpal/src/kv.rs crates/mcpal/src/exit.rs \
        book/src/recipes.md book/src/scripting.md book/src/error-codes.md \
        demo/README.md crates/mcpal/tests/integration.sh
git commit -m "docs: drop AWS-CLI mentions, add --help examples"
```

---

## Verification

Run the full suite end-to-end:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test -p mcpal --lib
MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture
```

Manual smoke (macOS Keychain — substitute Notion or a stub for the OAuth path):

```bash
mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer "$GH_TOKEN"
mcpal --output json auth status gh   # { bearer: true, oauth: false }
mcpal tool list gh                    # returns tools
mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer "$GH_TOKEN"
# → error[E0013]: server already exists
mcpal server add gh --force --http https://api.githubcopilot.com/mcp/ --bearer "$GH_TOKEN"
# → succeeds
```

Windows smoke (run from PowerShell on a Win11 box once a build is available — DPAPI is the only OS-specific surface):

```powershell
$env:GH_TOKEN = '<ghp_...>'
mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer $env:GH_TOKEN
mcpal --output json auth status gh
```

---

## Self-Review

**1. Spec coverage**
- Surface (all flags) → Task 1.
- `--bearer` literal + stdin → Task 2.
- `--bearer-env` → Task 2.
- `--header` passthrough + Authorization promotion → Task 1 (derive) + Task 2 (materialise).
- `--oauth` inline + `--no-login` → Task 3.
- `--force` + `E0013` → Task 4.
- Worked examples (AWS / Dataverse) → already covered by `--env` (unchanged); Task 5 docs surface them in `getting-started`; Task 6 surfaces them in `--help`.
- Future work (Entra static-client OAuth, SigV4) → out of scope, called out in spec; no task.
- AWS-CLI doc-string cleanup + `--help` examples → Task 6 (added post-spec on user feedback).

**2. Placeholder scan** — no "TBD" / "implement later" remain.

**3. Type consistency** — `AuthIntent` variants stable across Tasks 1–3 (`None`, `Literal`, `Env`, `Oauth`). `materialise_auth(alias, intent, no_login, ctx)` signature stable from Task 1 onward (`_ctx` → `ctx` in Task 3).
