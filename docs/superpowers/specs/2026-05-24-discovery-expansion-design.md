# v0.4.0 — discovery expansion + two fast-follow bugfixes

Status: approved · 2026-05-24

## Context

Two prompts collapsed into one release:

1. **Discovery expansion.** Built-in sources today cover claude-desktop,
   cursor, lm-studio, windsurf, cline, zed, claude-code, opencode. Gaps:
   VS Code (native MCP, settings.json, Continue extension) and Codex
   CLI. Users also need a way to point mcpal at arbitrary `mcp.json`
   files in custom locations.

2. **Two bugs surfaced by a Dataverse debug run:**
   - `registry::fetch` returns the first match from the registry API.
     The API returns versions in ascending order; we pick the **oldest**.
     `mcpal server install io.github.codeurali/dataverse` installs the
     broken `0.1.0` instead of the working `0.1.4`.
   - Stdio child stderr is nulled by default (a fix from the TUI work to
     stop `uv`/`fastmcp` installer noise from corrupting the alt-screen).
     That fix applies to every stdio command, including `mcpal tool
     list`. When a server dies on startup, the user sees
     `E0006 connection closed: initialize response` with **zero**
     diagnostic context. The `MCPAL_CHILD_STDERR=inherit` escape hatch
     exists but is undocumented.

## Goals

1. Add discovery sources: `vscode` (workspace + user `mcp.json`),
   `vscode-user` (`chat.mcp.servers` key in `settings.json`),
   `continue` (Continue extension storage), `codex` (`~/.codex/config.toml`).
2. `--discover-from PATH` global flag (repeatable) for custom file paths.
3. Audit + verify every existing source resolves correctly on Windows.
4. Fix `registry::fetch` to return the latest semver-compatible version.
5. Capture the last N lines of child stderr on stdio connect failure
   and surface them in the error chain — without changing TUI behaviour
   (which still wants stderr suppressed to avoid alt-screen corruption).

Non-goals:
- Discovery from Windows Store builds of Claude Desktop
  (`%LOCALAPPDATA%\Packages\…`). Documented limitation.
- Auto-update of installed servers when registry has a newer version.
- A registry caching layer.

## Schema extensions

`SimpleSource` (in `crates/mcpal-discovery/src/sources/mod.rs`)
gains:

```rust
pub struct SimpleSource {
    pub id: &'static str,
    pub key_path: &'static [&'static str], // was: key: &'static str
    pub global: &'static [(Location, &'static str)],
    pub project: &'static [&'static str],
    pub format: SourceFormat,              // was: jsonc: bool
}

pub enum SourceFormat { Json, Jsonc, Toml }
```

Existing entries migrate: `key: "mcpServers"` → `key_path: &["mcpServers"]`;
`jsonc: false` → `format: SourceFormat::Json`; `jsonc: true` → `Jsonc`.

The parse step walks `key_path` step-by-step before pulling the servers
map. Empty `key_path` is rejected at compile time (zero-length slice
on a `const` entry — would never get past code review anyway, but lint
via a `debug_assert!`).

## New built-in sources

| id | global path (Location-rooted) | project path | key_path | format |
|---|---|---|---|---|
| `vscode` | `Code/User/mcp.json` (Config) | `.vscode/mcp.json` | `["servers"]` | Json |
| `vscode-user` | `Code/User/settings.json` (Config) | — | `["chat", "mcp", "servers"]` | Jsonc |
| `continue` | `Code/User/globalStorage/continue.continue/config.json` (Config) | — | `["mcpServers"]` | Json |
| `codex` | `.codex/config.toml` (Home) | — | `["mcp_servers"]` | Toml |

`vscode-user`'s nested `chat.mcp.servers` exercises the new
`key_path` plumbing. `codex` exercises TOML parsing.

VS Code's `mcp.json` schema:
```json
{ "servers": { "fetch": { "type": "stdio", "command": "uvx", "args": ["mcp-server-fetch"] } } }
```
The per-server shape matches Cursor's spec; existing `parse::servers_map`
handles it.

## Custom paths

New global clap arg on `Cli`:

```rust
/// Additional `mcp.json` file to include in discovery (repeatable).
#[arg(long = "discover-from", global = true, value_name = "PATH")]
pub discover_from: Vec<PathBuf>,
```

`DiscoveryCtx` gains `custom_paths: Vec<PathBuf>`. A new `CustomFile`
source iterates `ctx.custom_paths` and parses each as
`{ "mcpServers": { ... } }` (de-facto standard). Files that don't
exist are silently skipped (matches built-in behaviour). Source id is
`"custom"`; scope is `Global`.

Threading:
- `Cli::discover_from` → `Ctx::new(...)` new arg → stored on `Ctx` →
  `Ctx::discovered()` builds `DiscoveryCtx::current().with_custom_paths(...)`
  before invoking the registry.

## Bugfix A — `registry::fetch` semver-max

`crates/mcpal/src/registry.rs::fetch` currently:

```rust
search(name, 20).await?
    .servers.into_iter().map(|w| w.server)
    .find(|s| s.name == name)
    .ok_or_else(|| anyhow!("registry: no exact match for '{name}'"))
```

Replace with:

```rust
let mut hits: Vec<Server> = search(name, 20).await?
    .servers.into_iter().map(|w| w.server)
    .filter(|s| s.name == name)
    .collect();
hits.sort_by(|a, b| {
    let av = semver::Version::parse(a.version.as_deref().unwrap_or("0.0.0")).ok();
    let bv = semver::Version::parse(b.version.as_deref().unwrap_or("0.0.0")).ok();
    av.cmp(&bv)
});
hits.pop().ok_or_else(|| anyhow!("registry: no exact match for '{name}'"))
```

Adds `semver = "1"` workspace dep. Unparseable versions sort to `None`
(below all real versions) — defensive; in practice every registry entry
has a SemVer string.

Unit test: feed `search` a stub with versions out of order, assert
`fetch` returns the highest.

## Bugfix B — capture child stderr on connect failure

Current state in `crates/mcpal-core/src/client.rs::connect_stdio`:
the stdio child has stderr nulled by default (or set via
`MCPAL_CHILD_STDERR=inherit` / `capture`). When `handler.serve(transport)`
fails (e.g. the child exits before the `initialize` response), the
error reaching the caller is the rmcp `Service` error — no child
context.

New behaviour:
- Default mode becomes **`capture`** — child stderr piped into a
  bounded ring buffer (last 64 lines, ~8KB cap).
- `MCPAL_CHILD_STDERR=null` opt-out preserves the current behaviour
  for TUI / scripted environments that explicitly want silence.
- `inherit` unchanged — passes through to the parent's stderr.
- On connect failure, the captured tail is appended to the error chain:
  `connection closed: initialize response (child stderr: …)`.
- On success, the captured buffer is dropped silently — no
  side-channel pollution to stdout, no log spam.

Implementation:
- `TokioChildProcess::builder(cmd).stderr(Stdio::piped()).spawn()` returns
  `(transport, Option<ChildStderr>)`. Drain the `ChildStderr` in a
  background task into an `Arc<Mutex<VecDeque<String>>>` bounded to 64
  lines. On failure, format the deque and attach to the error.
- TUI must opt back into `null` to keep alt-screen clean. Easiest
  knob: have the TUI's `runtime::connect_with_handler` set
  `MCPAL_CHILD_STDERR=null` for its own connection scope. Or set the
  env from inside `tui::run` before spawning child. Either way the
  effect is the existing TUI experience is preserved.

Documentation:
- Book `troubleshooting.md` gets a section: "Server connection closes
  immediately — read the stderr". Includes `MCPAL_CHILD_STDERR=inherit`
  for live streaming.
- `--help` for `tool list` / `tool call` / `server ping` mentions the
  env var in the after_help when one exists for the verb.

## Windows audit

`directories::BaseDirs` already resolves to platform-native roots:

| Location | Win | macOS | Linux |
|---|---|---|---|
| Home | `%USERPROFILE%` | `~` | `~` |
| Config | `%APPDATA%` | `~/Library/Application Support` | `~/.config` |
| Data | `%APPDATA%` | `~/Library/Application Support` | `~/.local/share` |

Every existing + new source uses Home or Config and resolves correctly
on Windows by construction. Specific verifications baked into the
test suite:

| Source | Windows-resolved path |
|---|---|
| `claude-desktop` | `%APPDATA%\Claude\claude_desktop_config.json` |
| `cursor` | `%USERPROFILE%\.cursor\mcp.json` |
| `cline` | `%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json` |
| `vscode` | `%APPDATA%\Code\User\mcp.json` |
| `vscode-user` | `%APPDATA%\Code\User\settings.json` |
| `continue` | `%APPDATA%\Code\User\globalStorage\continue.continue\config.json` |
| `codex` | `%USERPROFILE%\.codex\config.toml` |

`#[cfg(windows)]` smoke in `mcpal-discovery/tests/`: build a
`DiscoveryCtx` with tempdir-rooted Home + Config, drop a fixture under
each Windows-style relative path, assert it's found. Other platforms
get the same scaffold with their own roots.

## Files

| File | Change |
|---|---|
| `crates/mcpal-discovery/src/sources/mod.rs` | Extend `SimpleSource` (`key_path`, `format`); add 4 entries. |
| `crates/mcpal-discovery/src/sources/custom.rs` (new) | `CustomFile` source. |
| `crates/mcpal-discovery/src/lib.rs` | `DiscoveryCtx::custom_paths`; `with_custom_paths` builder. |
| `crates/mcpal-discovery/src/parse.rs` | TOML parsing path (use `toml` crate, already in workspace). |
| `crates/mcpal-discovery/Cargo.toml` | `toml.workspace = true`. |
| `crates/mcpal-discovery/tests/` | Snapshot tests for the 4 new sources + nested-key + Windows audit smoke. |
| `crates/mcpal/src/cli.rs` | `--discover-from PATH` global flag. |
| `crates/mcpal/src/runtime.rs` | Plumb `discover_from` into `Ctx`. |
| `crates/mcpal/src/main.rs` | Ctx::new call site. |
| `crates/mcpal/src/registry.rs` | Semver-max `fetch`. |
| `crates/mcpal/Cargo.toml` | `semver.workspace = true`. |
| `Cargo.toml` workspace | `semver = "1"`. |
| `crates/mcpal-core/src/client.rs` | Capture child stderr by default into a bounded ring; attach to error on failure. |
| `crates/mcpal/src/tui/...` | Set `MCPAL_CHILD_STDERR=null` for TUI scope (or thread a `Handler` flag). |
| `crates/mcpal/tests/integration.sh` | `--discover-from` assertion; stderr-capture assertion against a known-failing child. |
| `book/src/troubleshooting.md` | New section: "Server dies on initialize — read the stderr". |
| `book/src/concepts.md` (or new `discovery.md`) | List of supported clients + `--discover-from`. |
| `CHANGELOG.md` | `[Unreleased]` → `[0.4.0]` block. |

## Tests

**Unit (mcpal-discovery):**
- Each new source: write fixture under tempdir, assert `parse` returns
  expected `DiscoveredServer` list (name + transport).
- Nested-key walk: `vscode-user` fixture with `chat.mcp.servers.{foo,bar}` → 2 servers.
- Empty `chat.mcp.servers: {}` → 0 servers, no error.
- TOML codex fixture → expected servers.
- `CustomFile`: two paths, one exists with 2 servers, one absent → 2 servers + no error.
- Windows-rooted paths smoke (#[cfg(windows)] + Unix sibling).

**Unit (mcpal/registry):**
- Stub `search` returning versions `[0.1.0, 0.1.4, 0.1.2, 0.1.3, 0.1.1]` → `fetch` returns `0.1.4`.
- Unparseable version filtered, others picked correctly.

**Unit (mcpal-core/client):**
- Spawn a child that writes "boom\n" to stderr and exits 1.
  `connect_stdio` returns Err whose chain contains "boom".
- `MCPAL_CHILD_STDERR=null` short-circuits capture (env-set test).

**Integration (`tests/integration.sh`):**
- New section "custom discovery":
  - Build tempdir `custom.json` with one server entry. Run
    `mcpal --discover-from $TMP/custom.json server list --discovered`
    and grep for the server name + source `custom`.
- New section "stderr surfaced on stdio failure":
  - `mcpal tool list $REF` against a deliberately-broken stub command
    (e.g. `mcpal server add boom -- bash -c 'echo boom-message >&2; exit 1'`).
    Assert exit 6/7 (transport/service error) AND `boom-message`
    visible in stderr.

## Rollout (small commits)

1. `extend SimpleSource with nested key_path + format enum`
2. `add vscode + vscode-user + continue + codex sources`
3. `--discover-from PATH plumbed into DiscoveryCtx`
4. `fetch picks latest semver from registry`
5. `capture child stderr by default; attach to connect failures`
6. `tui pins MCPAL_CHILD_STDERR=null for its scope`
7. `windows path audit smoke + docs`
8. `book chapter for discovery sources + custom-paths`
9. `release v0.4.0`

~1.5 weeks. Each commit individually shippable.

## Verification

End-to-end smoke after rollout:

```bash
# 1. Custom path
echo '{"mcpServers":{"smoke":{"type":"stdio","command":"echo","args":["hi"]}}}' > /tmp/c.json
mcpal --discover-from /tmp/c.json server list --discovered | grep smoke

# 2. Semver-max
mcpal server install io.github.codeurali/dataverse
mcpal server show dataverse | grep -F '0.1.4'   # not 0.1.0

# 3. Stderr surfaced
mcpal server add boom -- bash -c 'echo "kaboom" >&2; exit 2'
mcpal -v tool list boom 2>&1 | grep kaboom

# 4. VS Code workspace
mkdir -p /tmp/proj/.vscode && cd /tmp/proj
echo '{"servers":{"ev":{"type":"stdio","command":"npx","args":["-y","@modelcontextprotocol/server-everything"]}}}' > .vscode/mcp.json
mcpal server discover --source vscode | grep ev
```

`cargo fmt --all -- --check`, `clippy -D warnings`, `cargo test -p
mcpal-discovery`, `cargo test -p mcpal --bin mcpal`,
`MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration`
all green.
