# Phase 2 — `mcpal.yml` collection file

Status: approved · 2026-05-22

## Context

Editors configure dozens of MCP servers; mcpal already drives them
from a shell. What's missing is the **repo-portable** layer: a YAML
file you check into a project that says "this project uses Linear +
GitHub like *this*", with parameterised tool calls behind short names.
Slumber's request collection is the proof of concept (HTTP); we want
the MCP equivalent.

`mcpal run get-issues --profile prod` replaces the long
`mcpal --query 'content[0].text' tool call cursor:linear get-issue
--id ENG-999 --workspace my-team`. The collection file is shared
between teammates via git. Source-first.

## Goals

1. Drop a `mcpal.yml` in any repo, define profiles + saved calls, run
   them by name.
2. `{{profile.X}}` and `{{env.X}}` template substitution in `params`,
   nothing more.
3. `--profile NAME` (or `MCPAL_PROFILE`) chooses environment.
4. Walk-parents lookup; `--collection PATH` overrides.
5. Missing variables are an error before any RPC fires.

Non-goals (locked YAGNI fence):
- Chained-response templating (`{{response.X.body.id}}`).
- Shell-command templating (`{{shell:date +%F}}`).
- File-load templating (`{{file:./body.json}}`).
- Saved `resource read` or `prompt get` calls (tool calls only, MVP).
- Editing the collection from the CLI (`mcpal run --add`).
- Importing from Postman / Insomnia / OpenAPI.

These are obvious follow-ups; designing them now adds surface we
don't need yet.

## Schema (`mcpal.yml`)

```yaml
default-profile: dev          # optional; falls back to "default"

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

- Top-level keys (all optional): `default-profile`, `profiles`,
  `calls`.
- `profiles[name]` is a flat `String -> String` map. No nesting.
- `calls[name]` requires `server` + `tool`. `params` optional (an
  empty mapping for tools that take no args).
- `server` is any `<ref>` mcpal understands today (alias, URL,
  `<source>:<name>`, JSON path).

## Surface

```
mcpal [--profile NAME] [--collection PATH] run <CALL_NAME>
  [--dry-run] [--params-override KEY=VAL...]
```

`--profile` already exists as a global flag (was dead code); we give
it meaning. `--collection PATH` is new, global.

`--dry-run` resolves the call and prints the rendered
`(server, tool, params)` JSON to stdout, no network.

`--params-override KEY=VAL` (repeatable) overlays raw values onto the
rendered params *after* templating. Same `K=V` parsing as today's
`tool call --key value` form.

## Run flow

```
run(name):
  1. find_collection(cwd, --collection)
     -> walk parents until mcpal.yml found
     -> if --collection given, use it verbatim (must exist)
     -> miss => E0015
  2. Collection::load(path)
     -> serde_yaml; map collisions => YAML parse error => E0010
  3. calls.get(name) or E0001 (list available)
  4. select profile (precedence, first non-empty wins):
        a. `--profile NAME` if user passed one explicitly
        b. `MCPAL_PROFILE` env var if set
        c. `default-profile:` key from the collection
        d. literal `"default"` (matches today's clap default)
     If the resolved name isn't a key of `profiles:`, that's E0016 —
     unless `profiles:` is entirely absent AND no template refers to
     `{{profile.X}}` (i.e. the call only uses `{{env.X}}` or no
     templates at all). In that degenerate case the call runs with an
     empty profile map.
  5. render(params, &profile_vars):
        walk Value tree; substitute strings via regex.
        any miss => E0014 (collect all misses; report in one message).
        {{{{ => literal {{ (escape).
  6. apply --params-override (raw string overlay after template).
  7. if --dry-run: render_one({server, tool, params}); exit 0.
  8. ctx.open(server) -> call_tool(tool, params) -> render result.
     (Same code path as `tool call`.)
```

## Components

| File | Role |
|---|---|
| `crates/mcpal/src/collection/mod.rs` | Re-exports + `find_collection(start, override) -> Result<PathBuf>`. |
| `crates/mcpal/src/collection/parse.rs` | `Collection`, `Call` structs (serde). `Collection::load(&Path)`. |
| `crates/mcpal/src/collection/template.rs` | `render(&mut Value, &profile_map) -> Result<()>`. Regex-driven. |
| `crates/mcpal/src/commands/run.rs` | CLI dispatch. Glue: load → lookup → render → open → call. |
| `crates/mcpal/src/cli.rs` | `Command::Run { name, dry_run, params_override }`; global `--collection PATH`. |
| `crates/mcpal/src/exit.rs` | Patterns + EXPLAIN for E0014 / E0015 / E0016. |
| `book/src/collection.md` | NEW how-to chapter. |
| `book/src/SUMMARY.md` | Add `collection.md` in How-to between Recipes and Authenticate. |
| `book/src/error-codes.md` | Append E0014–E0016. |
| `crates/mcpal/tests/integration.sh` | New section: `mcpal run` against a tempdir collection. |
| `Cargo.toml` workspace | Add `regex = "1"` (confirm not already present). |

## Templating grammar

- `\{\{[\s]*(profile|env)\.([A-Za-z_][A-Za-z0-9_]*)[\s]*\}\}`
- Two namespaces only: `profile`, `env`. Nothing else parses.
- `{{{{` escapes to literal `{{`.
- Substitution is recursive over `serde_json::Value`: walk every
  `Value::String`, replace matches in place. Numbers / bools / arrays
  / objects descend unchanged.
- Lookup: `env.X` reads OS env only (no fallback into profile).
  `profile.X` reads the selected profile only.
- Multiple misses collapse to one `E0014` message listing all of them.

## Errors

| Code | Exit | Title | When |
|---|---|---|---|
| `E0014` | 2 | template variable not set | unresolved `{{ns.key}}` |
| `E0015` | 2 | collection not found | walk hit `/` with no `mcpal.yml`, or `--collection PATH` missing |
| `E0016` | 2 | profile not in collection | `--profile prod` but `profiles:` has no `prod` |
| `E0001` (reuse) | 3 | not found | `mcpal run unknown` — error lists available call names |
| `E0010` (reuse) | 2 | bad YAML/JSON | YAML parse failure |

## Tests

**Unit** (`crates/mcpal/src/collection/`):

- `parse::tests`: round-trip the example YAML; empty `calls:` ok;
  `default-profile` absent ok; `default-profile: dev` round-trips.
- `template::tests`:
  - `"{{profile.x}}"` with profile `{x: "v"}` → `"v"`.
  - `"a {{env.HOME}} b"` mixed with `"{{profile.k}}"` → both
    substituted.
  - Object `{a: "{{profile.x}}", b: ["{{profile.y}}", "k"]}` → all
    leaves substituted, structure unchanged.
  - Missing var → `TemplateError::Missing { ns, key }`; multiple
    misses collected.
  - `"{{{{"` → `"{{"` (escape).
  - Value::Number / Value::Bool untouched.
- `find_collection::tests`:
  - CWD has `mcpal.yml` → found at CWD.
  - Three-deep nested CWD, root has `mcpal.yml` → found at root.
  - No file anywhere → `Ok(None)`.
  - Explicit `--collection PATH` to missing file → `Err(E0015)`.

**Integration** (`tests/integration.sh`, new section "collection +
mcpal run"):

- Write a tempdir `mcpal.yml` with a `ping` call against `$REF`. Run
  `mcpal --collection .../mcpal.yml run ping` → exits 0; output
  matches `mcpal server ping $REF`.
- `--dry-run` prints resolved `(server, tool, params)` JSON; does not
  open a connection (assert via `time`/process count? — simpler:
  assert stdout contains `"dry_run": true` and `"server"`).
- `--profile dev` substitutes the `dev` variant; `--profile prod` the
  `prod` variant; assertions grep the rendered output.
- `mcpal run unknown` → exit 3 + `E0001` + lists available calls.
- Missing `{{profile.x}}` → exit 2 + `E0014`.
- `--profile missing` → exit 2 + `E0016`.
- `--collection /no/such.yml` → exit 2 + `E0015`.

## Documentation

`book/src/collection.md` — How-to (Diátaxis):
- Pitch (2 paragraphs): source-first; commit `mcpal.yml`, teammates
  run by name.
- Minimal example (profiles + calls).
- Lookup rules (walk parents, `--collection PATH`).
- Templating (`{{profile.X}}`, `{{env.X}}`, escape, errors).
- `--dry-run`, `--params-override`.
- Cross-link to error codes E0014–E0016.

`README.md` Quickstart: append 4th block:

```bash
# Repo has mcpal.yml at root
mcpal run get-issue --profile prod
```

## Rollout (small commits)

1. `add collection parser + sample yaml`
2. `walk parents to find mcpal.yml`
3. `template engine for {{profile.X}} + {{env.X}}`
4. `mcpal run <name> wiring`
5. `--dry-run + --params-override`
6. `E0014/E0015/E0016 with explain text`
7. `book chapter for collections`
8. `integration assertions for mcpal run`

Eight commits, ~1 week. Each individually shippable.

## Verification

- `cargo test -p mcpal --bin mcpal` green (new unit tests pass).
- `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration`
  green (new section passes).
- Manual smoke against a real server-everything fixture:
  ```bash
  printf 'calls:\n  echo:\n    server: ev\n    tool: echo\n    params:\n      message: "{{profile.greeting}}"\nprofiles:\n  dev:\n    greeting: hello\n' > /tmp/mcpal.yml
  mcpal --collection /tmp/mcpal.yml --profile dev run echo
  ```
  Expected: server echoes `hello`.
- `mcpal run --help` shows the new flags + an Examples block (carry
  the after_help pattern from `server add`).
