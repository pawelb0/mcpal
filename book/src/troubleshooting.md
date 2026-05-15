# Troubleshooting

First step is always:

```bash
mcpal doctor
```

It checks: config readable, keyring round-trips, every owned server's
auth state, discovery counts per source. Output is YAML by default;
`--output json` for paste-into-issue.

## "server 'X' not found"

```
error[E0001]: server 'foo' not found (owned, URL, path, or discovered)
help: run `mcpal discover` to scan installed MCP clients for servers
help: or `mcpal server list --all` to see what's already configured
help: or add one: `mcpal server add <alias> --stdio <command>`
```

- `mcpal discover` to see all eight client configs mcpal scans.
- If you copied a config from Cursor/Claude Desktop, try `mcpal --mcp-json
  ./mcp.json tool list <name>` to skip the registration step.
- `mcpal explain E0001` for the full resolver order.

## "auth required" / "auth expired"

E0003 = no credentials at all. Run `mcpal auth login <ref> --bearer …` or
`--oauth`. E0004 = the server rejected the token mcpal sent. Run
`mcpal auth refresh <ref>` (uses the refresh token to mint a new access
token). If refresh also fails, do a full re-login.

`mcpal auth status <ref>` shows what's stored.

## "transport error"

E0005. The server didn't even start responding.

- For HTTP: verify with `curl -I <url>` that you can reach it.
- For stdio: confirm the command runs standalone. `npx -y` first runs
  download a package on a cold cache; takes 10–60s. Subsequent runs are
  fast.
- Run with `-v` (or `-vv`) for the tracing log of the request.

## "server error" — the server said no

E0006 = a well-formed JSON-RPC error from the server. Common causes:

- The tool/resource/prompt name is wrong. Check
  `mcpal tool list <ref>`.
- The arguments don't match `inputSchema`. Verify with
  `mcpal tool describe <ref> <name>` and re-build with
  `mcpal tool template <ref> <name>`.

## "request timed out"

E0007. Usually the first run of `npx -y @some-pkg` doing a cold install.
Retry — subsequent runs hit the npx cache and complete in <5s.

## "not yet supported"

E0008. The MCP feature you're hitting isn't wired in mcpal yet. The
`mcpal raw <ref> <method> --params <...>` escape hatch lets you send the
JSON-RPC directly while we wait.

## "query: …" — JMESPath errors

E0009. Your `--query` expression didn't compile or returned an error.

```
error[E0009]: query: search: …
help: JMESPath syntax — see https://jmespath.org/tutorial.html
help: common: `.field` projects, `[]` flattens, `[?cond]` filters
help: preview without the filter to inspect the shape first
```

Print the unfiltered response first to see the shape:

```bash
mcpal --output json tool list <ref>     # without --query
# then layer in the query
mcpal --query '[].name' tool list <ref>
```

## "JSON payload didn't parse"

E0010. Shell quoting issues are the #1 cause:

```bash
# wrong: shell strips the inner quotes
mcpal raw ev tools/call --params {"name":"echo"}

# right
mcpal raw ev tools/call --params '{"name":"echo","arguments":{"message":"hi"}}'
```

Use `mcpal tool template <ref> <name>` to get a known-good skeleton, or
use `--cli-input-json @file.json`.

## ANSI / weird terminal output

Spawned stdio servers print to stderr by default — and that stderr is
silenced (sent to `/dev/null`). If you want to see it, set:

```bash
MCPAL_CHILD_STDERR=inherit mcpal tool call <ref> …
```

## Keyring failures on Linux

If `mcpal doctor` reports `keyring round-trip failed`, your session may
not have a running Secret Service daemon. Install `gnome-keyring` or
`kwallet`, or use `MCPAL_BEARER=…` as a one-shot env override.

## "no Secret Service / D-Bus running"

Same fix as above. Or in unattended environments (CI, containers), skip
keyring entirely with `MCPAL_BEARER`.

## Filing a bug

Paste the output of:

```bash
mcpal --version
mcpal --output json doctor
```

Plus the failing command and its `-vv` trace output. The
`error[E####]` prefix is stable; please quote it verbatim.
