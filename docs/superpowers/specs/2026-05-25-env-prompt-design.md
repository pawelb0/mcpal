# v0.4.1 — env-var prompt + TUI env-setup modal + test corpus

Status: approved · 2026-05-25

## Context

`mcpal server install io.github.codeurali/dataverse` completes
silently but `mcpal tool list dataverse` then dies with
`environmentUrl: Required`. Root cause: `registry::EnvVar.is_required`
defaults to `false`, and current registry entries omit the
`isRequired` field entirely, so mcpal never detects required env vars
and writes a spec with an empty `env` map. The server crashes on
`initialize`.

Bonus fallout: even when the user knows env vars are needed, mcpal
gives no hint about names or descriptions. The user has to dig into
`registry.modelcontextprotocol.io` JSON to find them. The TUI is
worse — it just shows the connection failure with no path to fix it.

## Goals

1. Treat every declared `environmentVariables[]` entry as required
   unless it has a `default` or explicitly sets `isRequired: false`.
2. Carry the env-var `description` from the registry into mcpal so
   it can prompt with context.
3. `mcpal server install`: on a TTY, prompt for each missing required
   var with its description as the hint, write answers into the spec.
   Non-TTY: bail with a new error code listing the missing names.
4. TUI: when the user opens a server whose stored spec has any
   declared env var with an empty value, pop a modal listing the
   vars + descriptions (fetched fresh from the registry), let the
   user fill them, write back to `config.toml`, then connect.
5. Ship a `docs/test-corpus.md` catalogue of tricky MCPs we
   should sanity-check against on every release.

Non-goals:
- Auto-discovering env vars from servers that don't publish via the
  registry (no protocol for it).
- A standalone `mcpal env edit <ref>` command. Editing happens at
  install time, via `--env`, via `mcpal config edit`, or via the
  TUI modal. Don't add a third CLI surface.
- Migration of existing v0.4.0 specs that have empty env values — they
  already render in `server show`; the TUI modal will pick them up on
  next open.

## Registry detection

`crates/mcpal/src/registry.rs::EnvVar` becomes:

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

fn default_required() -> bool { true }
```

A var is "satisfied" when any of these is true:
- `extra_env` (CLI `--env`) contains its name.
- It has a non-`None` `default`.
- `is_required` is explicitly `false` in the registry JSON.

Otherwise it's "missing" and must be prompted / bailed on.

`to_spec` signature changes:

```rust
pub struct RequiredEnvHint {
    pub vars: Vec<EnvVarHint>,   // every declared var, satisfied or not
    pub missing: Vec<String>,    // names of unsatisfied required vars
}

pub struct EnvVarHint {
    pub name: String,
    pub description: Option<String>,
}

pub fn to_spec(
    server: &Server,
    extra_env: &BTreeMap<String, String>,
) -> Result<(ServerSpec, RequiredEnvHint)>;
```

The existing `bail!` path for missing vars goes away — the caller
decides whether to prompt, bail, or accept.

## CLI install flow

`crates/mcpal/src/commands/server.rs::install`:

```
let server = registry::fetch(&args.name).await?;
let extra = parse_env(&args.env)?;
loop {
    let (spec, hint) = registry::to_spec(&server, &extra)?;
    if hint.missing.is_empty() {
        write_server(&ctx.config_path, &alias, spec, false)?;
        eprintln!("installed {} as '{alias}'", server.name);
        break Ok(());
    }
    if !std::io::stdin().is_terminal() || args.no_prompt {
        bail!(
            "registry server requires env vars: {} \
             — re-run on a TTY or pass --env VAR=…",
            hint.missing.join(", "),
        );
    }
    // Print the per-var description, prompt one at a time.
    eprintln!(
        "{} needs {} environment variable{}:",
        server.name, hint.missing.len(), if hint.missing.len() == 1 {""} else {"s"},
    );
    for name in &hint.missing {
        let desc = hint
            .vars
            .iter()
            .find(|v| v.name == *name)
            .and_then(|v| v.description.clone())
            .unwrap_or_default();
        let value = prompt_for(name, &desc)?;
        extra.insert(name.clone(), value);
    }
}
```

`prompt_for` writes `"  NAME — DESCRIPTION\n> "` to stderr, reads a
single line from stdin, trims `\r\n`, returns the result. Empty
input is allowed (user explicitly skips); we write the empty string
into the spec and the TUI's connect-time modal will pick it up if
they later try the TUI.

New CLI surface:

- `ServerInstallArgs.no_prompt: bool` — `--no-prompt`. Skips the
  interactive path even on a TTY. Useful for shell scripts that want
  the bail behaviour explicitly.

New error code in `crates/mcpal/src/exit.rs`:

```rust
("requires env vars", 2, "E0017"),
```

and EXPLAIN:

```rust
(
    "E0017",
    "Registry server declares required environment variables that aren't set. \
    Re-run `mcpal server install <ref>` on a TTY (mcpal will prompt) or pre-supply \
    each via `--env VAR=value`. `mcpal server search <ref>` shows the entry.\n",
),
```

## TUI connect-time modal

Trigger location: `tui/app.rs` — sidebar `Enter` handler, before
dispatching the connect future. Read the entry's `ServerSpec`. If
it's `ServerSpec::Stdio { env, .. }` and **any** value in `env` is the
empty string, intercept the connect: open `Modal::EnvSetup` instead.

```rust
enum Modal {
    None,
    Filter(...),
    Call(CallForm),
    Confirm(...),
    Bearer { reference: String, buf: String },
    EnvSetup {                                  // NEW
        reference: String,
        fields: Vec<EnvField>,
        cursor: usize,
    },
    Help,
}

struct EnvField {
    name: String,
    description: String,
    input: tui_input::Input,
}
```

`EnvField`s are built from the spec's env map (one field per
empty-valued entry, preserving spec order). Descriptions are pulled
from a lazy `App::env_descriptions: OnceCell<HashMap<server_name,
HashMap<var_name, String>>>` fetched via `registry::search(server_name,
1)` on first modal open. If the registry call fails or the server
isn't a registry-installed one, descriptions are empty — modal still
shows the names.

Keymap:

| Key | Effect |
|---|---|
| `Tab` / `Down` | next field |
| `Shift-Tab` / `Up` | previous field |
| Edit keys | passed to `tui_input::Input` of the focused field |
| `Enter` | save + connect (works from any field) |
| `Esc` | cancel; sidebar focus restored, no connect |

Save path:

```rust
// 1. Apply field values back to cfg
let mut cfg = Config::load(&ctx.config_path)?;
if let Some(ServerSpec::Stdio { env, .. }) = cfg.server.get_mut(&reference) {
    for f in &fields {
        env.insert(f.name.clone(), f.input.value().to_string());
    }
}
cfg.save(&ctx.config_path)?;
// 2. Refresh Ctx's cached cfg
self.refresh_cfg(cfg);
// 3. Dispatch the connect (same path as a normal Enter)
self.start_connect(reference);
```

`App::refresh_cfg` already exists or needs to be added — it replaces
`ctx.cfg` and any sidebar entries derived from it. (Verify by reading
the file; if absent, add a 5-line method.)

Failure modes:
- File-system error during `cfg.save`: pop a transient error line into
  the output pane, keep the modal open.
- Registry fetch fails: the modal still opens with empty
  descriptions; user fills in by name alone.

## Test corpus

New file `docs/test-corpus.md` linked from `book/src/SUMMARY.md`
under Reference. Contains the matrix from the brainstorm:

| Category | Server | What it stresses |
|---|---|---|
| stdio + required env | `io.github.codeurali/dataverse` | env-var prompt (this PR); semver-max (v0.4.0) |
| stdio + uvx | `awslabs.aws-api-mcp-server` | cold-start latency; AWS_PROFILE env |
| stdio + npx | `@modelcontextprotocol/server-postgres` | DATABASE_URL env |
| broken on init | `mcp-dataverse@0.1.0` | stderr surfacing (v0.4.0); ENOENT package.json |
| HTTP + OAuth | `https://mcp.notion.com/v1` | PKCE+DCR; refresh token |
| HTTP + bearer | `https://api.githubcopilot.com/mcp/` | --bearer; promote-on-import |
| HTTP + custom header | (any API-key gated server) | --header round-trip |
| pagination | server-everything | 100+ tools; --names-only |
| notifications | server-everything | watch, progress, list-changed |
| resource subscriptions | server-everything | `resource subscribe` |
| sampling / elicitation | server-everything `sample` / `eliciting` | --sampling-handler; --no-interactive |
| mcp-ui / OpenAI Apps | weather-demo / openai-apps-demo | `mcpal ui inspect` |
| multi-source name | chrome-devtools (opencode + claude-code) | bare-name disambiguation |
| fastmcp banner | local FastMCP demo | TUI alt-screen integrity (setsid) |
| HTTP custom CA | (corp HTTPS) | currently UNTESTED — known gap |

Each row also gets a 5-line "manual smoke" block underneath in the
file (install command, expected output, gotchas).

## Tests

**Unit (`registry.rs`):**
- Var without default + no extra_env → in `hint.missing`.
- Var with default → satisfied, in `hint.vars` but not `hint.missing`.
- Var with explicit `isRequired: false` and no default → not in
  `hint.missing` even though no extra_env covers it.
- Var with description → carried through to `hint.vars[i].description`.

**Unit (`commands/server.rs`):**
- `install` non-TTY with missing vars → returns Err whose `to_string`
  contains "requires env vars" (so exit classifier maps to E0017).
- `install` non-TTY with all vars supplied via `--env` → succeeds.
- The TTY-prompt path is exercised indirectly by integration; a unit
  test would need stdin mocking which Rust makes awkward. Skip.

**Unit (`tui/app.rs`):**
- Given `ServerSpec::Stdio { env: { "X": "", "Y": "v" } }`, building
  `EnvSetup` modal produces one field for `X`, none for `Y`.
- Description lookup misses fall back to empty string without panic.

**Integration:**
- `mc server install` with no real registry (`MCPAL_REGISTRY_URL=http://127.0.0.1:9`)
  fails fast — exercises the classifier, not the full prompt path.
- E0017 explain text accessible via `mc debug explain E0017`.

## Files

| File | Change |
|---|---|
| `crates/mcpal/src/registry.rs` | `EnvVar.description`; `default_required`; `RequiredEnvHint`; `to_spec` returns tuple. |
| `crates/mcpal/src/commands/server.rs` | `install` flow; `prompt_for` helper; `--no-prompt` plumbing. |
| `crates/mcpal/src/cli.rs` | `ServerInstallArgs.no_prompt`. |
| `crates/mcpal/src/exit.rs` | E0017 pattern + EXPLAIN. |
| `crates/mcpal/src/tui/app.rs` | `Modal::EnvSetup`; `EnvField`; pre-connect check; description cache. |
| `crates/mcpal/src/tui/mod.rs` | (Possibly) wire new modal — most likely covered in `app.rs`. |
| `docs/test-corpus.md` | New file. |
| `book/src/SUMMARY.md` | Add `[Test corpus](../../docs/test-corpus.md)` under Reference — or copy the file under `book/src/test-corpus.md` (cleaner; pick during plan). |
| `book/src/troubleshooting.md` | "Missing env var on install" subsection. |
| `book/src/error-codes.md` | E0017 entry. |
| `crates/mcpal/tests/integration.sh` | E0017 + explain assertions. |
| `CHANGELOG.md` | `[0.4.1]` block. |

## Rollout (small commits)

1. `registry: descriptions + required-by-default on env vars`
2. `registry: to_spec returns (ServerSpec, RequiredEnvHint)`
3. `server install: prompt on TTY for missing env vars`
4. `E0017 for non-tty install with missing env`
5. `tui: env-setup modal pre-connect`
6. `book: test corpus + missing-env troubleshooting`
7. `release v0.4.1`

Seven commits, ~1 day. Each individually shippable.

## Verification

End-to-end smoke:

```bash
# 1. Prompt path
echo "https://myorg.crm.dynamics.com" | mcpal server install io.github.codeurali/dataverse
mcpal server show dataverse           # env has DATAVERSE_ENV_URL filled

# 2. Non-tty bail (E0017)
mcpal server remove dataverse
mcpal server install io.github.codeurali/dataverse --no-prompt
# → error[E0017]: registry server requires env vars: DATAVERSE_ENV_URL …

# 3. Pre-supplied (no prompt)
mcpal server install io.github.codeurali/dataverse --env DATAVERSE_ENV_URL=https://x.dynamics.com
mcpal tool list dataverse             # initialises

# 4. TUI modal
# Manual: open mcpal tui, navigate to a server with empty-string env value,
# Enter, modal appears, fill in, Enter saves + connects.
```

`cargo fmt --all -- --check`, `clippy -D warnings`,
`cargo test -p mcpal --bin mcpal`, integration suite all green.
