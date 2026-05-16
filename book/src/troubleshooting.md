# Troubleshooting

## `mcpal debug doctor`

```bash
mcpal debug doctor
```

Checks: config readable, keyring round-trip, auth state per server,
discovery counts. YAML default; `--output json` for bug reports.

## E0001 — "server 'X' not found"

```
error[E0001]: server 'foo' not found (owned, URL, path, or discovered)
help: run `mcpal server discover` to scan installed MCP clients for servers
help: or `mcpal server list --all` to see what's already configured
help: or add one: `mcpal server add <alias> --stdio <command>`
```

- `mcpal server discover` lists every client config mcpal scans.
- If you copied a config from Cursor or Claude Desktop, try
  `mcpal --mcp-json ./mcp.json tool list <name>` and skip registration.
- `mcpal debug explain E0001` for the resolver order.

## E0003 / E0004 — auth

- E0003: no credentials. `mcpal auth login <ref> --bearer …` or
  `--oauth`.
- E0004: server rejected the token. `mcpal auth refresh <ref>`; if
  refresh fails, re-login.

`mcpal auth status <ref>` shows what's stored.

## E0005 — transport error

No response from the server.

- HTTP: verify with `curl -I <url>` that the host is reachable.
- stdio: confirm the command runs standalone. `npx -y` on a cold cache
  installs the package (10–60s); subsequent runs complete in <5s.
- Re-run with `-v` (or `-vv`) for the request trace.
- `mcpal server test <ref>` is the smallest reproducer.

## E0006 — server-returned error

A well-formed JSON-RPC error from the server.

- The tool, resource, or prompt name is wrong. Check
  `mcpal tool list <ref>`.
- The arguments don't match `inputSchema`. Verify with
  `mcpal tool describe <ref> <name>` and rebuild with
  `mcpal tool template <ref> <name>`.

## E0007 — request timed out

Triggered when no response arrives within the deadline. First `npx -y`
runs commonly hit this on a cold cache. Retry; subsequent runs hit the
cache and complete in <5s. Also check the server isn't waiting on
stdin.

## E0008 — not yet supported

The MCP feature isn't wired in mcpal yet. Use
`mcpal raw <ref> <method> --params <…>` to send the JSON-RPC directly.

## E0009 — JMESPath errors

```
error[E0009]: query: search: …
help: JMESPath syntax — see https://jmespath.org/tutorial.html
help: common: `.field` projects, `[]` flattens, `[?cond]` filters
help: preview without the filter to inspect the shape first
```

Print the unfiltered response first to see the shape:

```bash
mcpal --output json tool list <ref>
mcpal --query '[].name' tool list <ref>
```

## E0010 — JSON payload didn't parse

Shell quoting is the common cause:

```bash
# wrong: shell strips the inner quotes
mcpal raw ev tools/call --params {"name":"echo"}

# right
mcpal raw ev tools/call --params '{"name":"echo","arguments":{"message":"hi"}}'
```

Use `mcpal tool template <ref> <name>` for a known-good skeleton, or
`--cli-input-json @file.json`.

## Spawned server stderr is hidden

Spawned stdio servers' stderr is redirected to `/dev/null`. To see it:

```bash
MCPAL_CHILD_STDERR=inherit mcpal tool call <ref> …
```

## Keyring failures on Linux

If `mcpal debug doctor` reports `keyring round-trip failed`, the session has
no running Secret Service daemon. Install `gnome-keyring` or `kwallet`.
In CI or containers, skip the keyring entirely with `MCPAL_BEARER=…`.

## Filing a bug

```bash
mcpal --version
mcpal --output json doctor
```

Include the failing command and its `-vv` trace. The `error[E####]`
prefix is stable; quote it verbatim.
