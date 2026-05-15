# Scripting & exit codes

mcpal is meant for pipelines:

- stdout is data. Informational output goes to stderr.
- Exit codes are stable per failure class; the wording around them is
  not.
- `--output json` and `--query <jmespath>` handle most pipelines without
  `jq`.

## Exit codes

| Code | Meaning | Error code(s) | Common fix |
|---|---|---|---|
| 0 | success | — | — |
| 1 | generic error | E0000 | check stderr |
| 2 | usage / invalid arguments | E0002, E0009, E0010 | `mcpal <subcommand> --help` |
| 3 | server reference not found | E0001 | `mcpal discover` |
| 4 | auth required | E0003 | `mcpal auth login <ref>` |
| 5 | auth expired | E0004 | `mcpal auth refresh <ref>` |
| 6 | transport / not yet supported | E0005, E0008 | network unreachable or `mcpal raw` |
| 7 | server returned a JSON-RPC error | E0006 | check args against `tool describe` |
| 8 | request timed out | E0007 | retry; with `--timeout`, raise the value |
| 130 | interrupted (Ctrl-C) | E0011 | — |

Each error prints `error[E####]:` plus hints. `mcpal explain E####`
shows the long form.

## `--output json`

```bash
mcpal --output json tool list <ref> | jq -r '.[].name'
mcpal --output json server test <ref> | jq -r '.peerInfo.serverInfo.version'
```

YAML is the default for human reading. Set `--output json` in pipelines.

## `--query` (JMESPath)

For one-liners:

```bash
mcpal --query 'content[0].text' tool call ev echo --message hi
mcpal --query '[].name' tool list ev
mcpal --query 'peerInfo.serverInfo.{name:name,version:version}' server test ev
```

Same syntax as AWS-CLI `--query`.
[Tutorial](https://jmespath.org/tutorial.html).

## Reading args from stdin or files

`tool call` accepts:

- `--key value` flags — typed JSON values (numbers, booleans, JSON
  literals).
- `--cli-input-json @path/to.json` — read a base object from a file.
- `--cli-input-json -` — read from stdin.
- Mix: base from file or stdin, override with `--key value`.

```bash
echo '{"a":1,"b":2}' | mcpal tool call ev some --cli-input-json - --b 99
```

## `raw` for unmapped methods

```bash
mcpal raw <ref> some/method --params '{"k":"v"}'
mcpal raw <ref> some/method --params @payload.json
mcpal raw <ref> some/method --params -
```

With `--query` and `--output`:

```bash
mcpal --query 'tools[].name' --output json raw <ref> tools/list
```

## `watch`

```bash
mcpal watch <ref>
```

One YAML doc per notification (progress, log, resource-updated,
list-changed). Run alongside another terminal that drives requests.

## Env vars

| Var | Effect |
|---|---|
| `MCPAL_CONFIG` | path to `config.toml` |
| `MCPAL_PROFILE` | accepted but unused (will gate profile selection later) |
| `MCPAL_BEARER` | one-shot bearer for any HTTP server |
| `MCPAL_SAMPLING_HANDLER` | shell command for `sampling/createMessage` |
| `MCPAL_CHILD_STDERR=inherit` | un-silence the spawned stdio server's stderr |
| `RUST_LOG` | tracing filter (e.g. `info,mcpal=debug`) |

## GitHub Actions

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

- Parse human stderr. mcpal's wording changes; the exit code and the
  `error[E####]` prefix don't.
- Rely on TTY colors in scripts. mcpal already disables ANSI on non-TTY
  stdout.
- Rely on argument order beyond positionals. Use `--key value`
  throughout.
