# Scripting & exit codes

mcpal is built for pipelines. Three guarantees:

- **stdout is data.** Everything informational goes to stderr.
- **stable exit codes per failure class** — script `case $?` and don't
  worry about message changes.
- **`--output json` + `--query <jmespath>`** cover the 80% case so you
  don't need `jq` (but pipe to `jq` if you want).

## Stable exit codes

| Code | Meaning | Common fix |
|---|---|---|
| 0 | success | — |
| 1 | generic error | check stderr; consider `mcpal explain E0000` |
| 2 | usage / invalid arguments | `mcpal <subcommand> --help` |
| 3 | server reference not found | `mcpal discover` or `mcpal server list --all` |
| 4 | auth required | `mcpal auth login <ref>` (`--bearer` or `--oauth`) |
| 5 | auth expired | `mcpal auth refresh <ref>` |
| 6 | transport error | network unreachable / pipe broken |
| 7 | server returned a JSON-RPC error | check args against `tool describe` |
| 8 | request timed out | retry; `npx -y` cold cache is ~30s |

Each error renders in rustc style with `error[E####]:` plus actionable
hints. `mcpal explain E0001` (etc.) prints the long-form prose.

## `--output json` for machine pipelines

```bash
mcpal --output json tool list <ref> | jq -r '.[].name'
mcpal --output json server test <ref> | jq -r '.peerInfo.serverInfo.version'
```

YAML is the default because it stays readable on a terminal while still
being parseable. Pipelines should set `--output json` explicitly to make
the data shape unambiguous.

## `--query` (JMESPath)

For one-liners where `jq` is overkill:

```bash
mcpal --query 'content[0].text' tool call ev echo --message hi
mcpal --query '[].name' tool list ev
mcpal --query 'peerInfo.serverInfo.{name:name,version:version}' server test ev
```

Syntax mirrors AWS-CLI `--query` (both use the official JMESPath grammar).
[Tutorial](https://jmespath.org/tutorial.html).

## Reading args from stdin / files

`tool call` accepts:

- `--key value` flags — typed JSON values (numbers, booleans, JSON literals).
- `--cli-input-json @path/to.json` — read a base object from a file.
- `--cli-input-json -` — read from stdin.
- Mix: file/stdin first, then `--key value` overrides individual fields.

```bash
echo '{"a":1,"b":2}' | mcpal tool call ev some --cli-input-json - --b 99
```

## `raw` for unmapped methods

```bash
mcpal raw <ref> some/method --params '{"k":"v"}'
mcpal raw <ref> some/method --params @payload.json
mcpal raw <ref> some/method --params -
```

Composes with `--query` and `--output`:

```bash
mcpal --query 'tools[].name' --output json raw <ref> tools/list
```

## `watch` for streaming notifications

```bash
mcpal watch <ref>
```

One YAML doc per server-initiated notification (progress, log,
resource-updated, list-changed). Ctrl-C exits. Compose with another
terminal driving requests against the same server.

## Env vars

| Var | Effect |
|---|---|
| `MCPAL_CONFIG` | path to `config.toml` |
| `MCPAL_PROFILE` | reserved for future per-profile settings |
| `MCPAL_BEARER` | one-shot bearer for any HTTP server |
| `MCPAL_SAMPLING_HANDLER` | shell command for `sampling/createMessage` |
| `MCPAL_CHILD_STDERR=inherit` | un-silence the spawned stdio server's stderr |
| `RUST_LOG` | tracing filter (`info,mcpal=debug` etc.) |

## CI patterns

GitHub Actions:

```yaml
- run: cargo install --path crates/mcpal-cli
- run: |
    mcpal server add api --http $MCP_URL
    mcpal doctor --output json
    mcpal --output json tool list api > tools.json
  env:
    MCPAL_BEARER: ${{ secrets.MCP_TOKEN }}
    MCPAL_CONFIG: ${{ runner.temp }}/mcpal.toml
```

## Don't

- Don't parse human stderr. mcpal's wording changes; the exit code and
  the `error[E####]` prefix don't.
- Don't rely on TTY colors in scripts. mcpal already disables ANSI on
  non-TTY stdout.
- Don't rely on argument order beyond positionals. Use `--key value`
  flags throughout.
