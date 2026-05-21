# `mcpal server add` — one-liner with auth

Status: approved · 2026-05-20

## Problem

Registering an HTTP server today takes two commands:

```
mcpal server add gh --http https://api.githubcopilot.com/mcp/
mcpal auth login gh --bearer $GH_TOKEN
```

Claude Code does the equivalent in one (`claude mcp add … --header
"Authorization: …"`). The split is a real friction point — every
quickstart, GIF, doc, and copy-paste snippet pays the cost.

## Goal

A single `mcpal server add` invocation registers the server **and**
materialises its auth, on macOS / Linux / Windows.

Non-goals:
- Rename verbs to match Claude (`mcpal mcp add …`). Out of scope.
- `mcpal server add-json '<paste>'`. Out of scope.
- Preflight ping before save. Out of scope.
- OAuth providers that **do not** support Dynamic Client Registration
  (Entra ID, Cognito user-pool, Auth0-static-client). Tracked in
  "Future work" below — these need `--oauth-client-id` /
  `--oauth-tenant` / explicit endpoint overrides.
- AWS SigV4 request signing. Workaround: front with
  `aws-sigv4-proxy`, point mcpal at `http://localhost:8080`.

## Surface

```
mcpal server add <name> [transport] [auth] [--force]

transport (mutually exclusive, unchanged):
  --http URL
  --stdio CMD             # or `-- CMD ARGS...`
  -- CMD ARGS...

auth (HTTP only; at most one of bearer / bearer-env / oauth):
  --bearer TOKEN | -      # literal or stdin → OS keyring
  --bearer-env VAR        # spec.auth = bearer_env { env = VAR }
  --oauth                 # inline browser flow → keyring
  --header "K: V"         # repeatable; Authorization: Bearer auto-promoted

other:
  --no-login              # with --oauth: write spec, skip browser
  --force                 # overwrite existing entry (else E0013)
```

Examples after the change:

```
mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer $GH_TOKEN
mcpal server add gh --http https://api.githubcopilot.com/mcp/ --bearer-env GH_TOKEN
mcpal server add notion --http https://mcp.notion.com/v1 --oauth
mcpal server add ev -- npx -y @modelcontextprotocol/server-everything
echo $TOKEN | mcpal server add gh --http URL --bearer -
```

### Worked examples — real servers

The design must hold up against the servers users actually paste in.
Two representative shapes:

**AWS MCP** (`awslabs/mcp` family — stdio via `uvx`, AWS SDK reads
`~/.aws/credentials` from the child):

```
mcpal server add aws-api \
  --env AWS_PROFILE=default --env AWS_REGION=us-east-1 \
  -- uvx awslabs.aws-api-mcp-server@latest
```

The existing `--env K=V` (repeatable) flag is preserved; no new
surface needed. Credentials never reach mcpal.

**Dataverse MCP** (Microsoft Power Platform — stdio + Entra tenant
env vars):

```
mcpal server add dv \
  --env DATAVERSE_URL=https://org.crm.dynamics.com \
  --env AZURE_TENANT_ID=$TENANT_ID \
  -- npx -y @microsoft/dataverse-mcp
```

Works today. The HTTP+Entra variant (static `client_id`, no DCR) is
out of scope — see "Future work".

## Data flow

```
add(args):
  1. validate transport + auth combo  (clap ArgGroups + small match)
  2. build ServerSpec                  (existing builder paths)
  3. fold --header into spec.headers
  4. fold auth inputs into AuthIntent:
       Literal(token) | Env(var) | Oauth | None
     reuse extract_bearer() to demote any Authorization: Bearer header
  5. if registry.exists(name) && !force → Err(E0013)
  6. write_server(name, spec)          (atomic toml write — today)
  7. materialise auth:
       Literal(t)  → keyring::put("bearer", name, t)
       Env(v)      → spec.auth = bearer_env{env=v}; re-save
       Oauth       → spec.auth = oauth; re-save;
                     if !no_login → oauth::login(name)  (browser inline)
       None        → no-op
  8. ctx.render_one(Added { name, transport, auth: <kind> })
```

Spec write precedes credential side-effects so a mid-OAuth failure
leaves a valid spec; the user retries with `mcpal auth login NAME
--oauth` — same recovery path as today.

## Components

| File | Change |
|---|---|
| `crates/mcpal/src/cli.rs` | `ServerAddArgs`: new fields `bearer`, `bearer_env`, `oauth`, `header: Vec<String>`, `no_login`, `force`. Clap `ArgGroup` enforces bearer\|bearer_env\|oauth exclusivity. Stdio + any auth flag → `Args::error(...)` at parse. |
| `crates/mcpal/src/commands/server.rs::add` | Folds inputs into a file-local `AuthIntent` enum, calls reused helpers. New small fn `materialise_auth(name, intent, no_login)`. |
| `crates/mcpal/src/commands/server.rs` | Reuse: `extract_bearer`, `keyring::put`, `write_server`. |
| `crates/mcpal/src/commands/auth.rs::login` | Extract `oauth_login_inline(name) -> Result<()>` so `add --oauth` and `login --oauth` share the path. Public surface of `auth login` unchanged. |
| `crates/mcpal/src/exit.rs` | New entry `E0013 server already exists` (exit code 2). |
| `book/src/error-codes.md` | Document `E0013`. |
| `README.md` Quickstart | Collapse "add then login" pairs to single lines. |
| `book/src/getting-started.md` | Same. |
| `book/src/auth.md` | Update bearer/oauth recipes to one-liners; keep `auth login` as the "update token later" entry-point. |

## Cross-platform

- **Keyring**: `keyring` crate already targets DPAPI (Windows), Keychain (macOS), Secret Service (Linux). No change.
- **Clap `trailing_var_arg` + `last=true`** for `-- CMD ARGS`: identical parse on cmd.exe / PowerShell / bash. Shell quoting is the user's problem (documented).
- **`--bearer -`** stdin: `read_stdin()` already trims `\r\n` and `\n` — PowerShell-safe.
- **`--oauth` browser**: reuses `oauth::login`, which calls `open::that()` (handles `start` / `open` / `xdg-open`). Loopback `127.0.0.1:0` listener — no Win10+ firewall prompt (loopback is exempt).
- **`--header` env expansion**: mcpal does **not** expand `${VAR}`. Shell expands. mcpal only *detects* literal `${VAR}` / `$VAR` to pick `bearer_env`. No platform branch in code.
- **Tokens never to stdout/stderr**: `Added { auth: "bearer" }` reports the *kind*, never the value. Same rule as `auth login`.

## Errors

| Condition | Outcome |
|---|---|
| Name already registered + no `--force` | Exit 2, `E0013 server already exists` |
| Stdio transport + any auth flag | Clap exit 2, hint: "auth flags require --http" |
| Multiple of `--bearer / --bearer-env / --oauth` | Clap exit 2 |
| `--header` value missing `:` | Clap exit 2 |
| `--bearer -` with empty stdin | Exit 2, "no token on stdin" |
| OAuth handshake failure | Spec persisted; stderr: "auth pending — retry with `mcpal auth login NAME --oauth`" |

## Tests

**Unit** (`crates/mcpal/src/commands/server.rs#[cfg(test)] mod tests`):
- `AuthIntent` derivation for every flag combination + `Authorization: Bearer` header promotion (literal / `${VAR}` / `$VAR` / non-Bearer scheme).
- `materialise_auth` with `--no-login` writes spec, performs **no** keyring/browser side-effects (pure intent struct + dispatcher; dispatcher mocked via trait bound or `--no-login` short-circuit).

**Integration** (`crates/mcpal/tests/integration.sh`):
- `mcpal server add T1 --http http://x --bearer abc` → keyring has `bearer:T1`; config `auth` absent.
- `mcpal server add T2 --http http://x --bearer-env FOO` → config `auth = bearer_env{FOO}`; no keyring.
- `mcpal server add T3 --http http://x --header "Authorization: Bearer abc"` → identical state to T1.
- `mcpal server add T4 --http http://x --header "X-API-Key: k"` → spec headers contain it; no auth.
- `mcpal server add T5 -- echo hi` → stdio works (unchanged).
- `mcpal server add T5 -- echo hi --bearer x` → clap exit 2.
- Re-add T1 without `--force` → exit 2, `E0013`. With `--force` → succeeds.
- `echo abc | mcpal server add T6 --http http://x --bearer -` → keyring has `bearer:T6` with value `abc`.

OAuth inline path is already covered end-to-end by `auth login --oauth`
against the `oauth_mock` example; `add --oauth` calls the same
`oauth_login_inline` function — a single assertion confirming the spec
is written and the function is invoked is enough.

## Documentation deltas

- README Quickstart §"Add your own" — collapse the two-line HTTP block
  into one.
- `book/src/getting-started.md` — replace step 4's `auth login` with
  `server add … --bearer $TOKEN`.
- `book/src/auth.md` — top of page: "Most users want the one-liner.
  Use `auth login` to rotate a token later."
- `book/src/error-codes.md` — add `E0013`.

## Rollout

Single commit chain (per `feedback_small_commits.md`):

1. `feat(cli): auth flags on server add` — clap surface + `AuthIntent`.
2. `feat(server-add): materialise bearer / bearer-env` — keyring + spec wiring; integration tests T1–T4.
3. `feat(server-add): inline --oauth` — extract `oauth_login_inline`, dispatch from add.
4. `feat(server-add): --force + E0013` — collision handling.
5. `docs: collapse add+login to one liner` — README + book.

## Future work (out of this spec, captured here so we don't forget)

- **Static-client OAuth providers** (Entra ID, Cognito, Auth0 without
  DCR). New flags: `--oauth-client-id`, `--oauth-client-secret`,
  `--oauth-tenant`, `--oauth-authorize-url`, `--oauth-token-url`.
  Roughly the size of the original OAuth M4 milestone again.
- **AWS SigV4** request signing as a built-in auth mode
  (`--auth sigv4 --aws-profile ...`). Today: document
  `aws-sigv4-proxy` workaround.
- **`mcpal server add-json '<paste>'`** to consume a Claude/Cursor
  `mcp.json` snippet inline (parallel to `--mcp-json` file path).
- **Preflight ping** behind `--check` opt-in: open → ping → rollback
  on failure.

## Verification

- `cargo test -p mcpal` green.
- `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture` green (new assertions added above).
- Manual smoke on macOS (Keychain) **and** at least one Windows box (DPAPI) — covers the only platform-specific surface (keyring).
- `mcpal server add gh --http URL --bearer $TOKEN && mcpal tool list gh` returns tools.
