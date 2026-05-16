# Concepts

The model behind mcpal. Read once. The four moving parts are
references, transports, auth modes, and output. Each is explained
below; the how-to pages link back here whenever they refer to one of
them.

## Server reference (`<ref>`)

Every command that talks to an MCP server takes a `<ref>` positional. It
resolves in this order:

1. Alias from `mcpal server add`.
2. An `http(s)://` URL (anonymous HTTP server).
3. Path to a JSON file with one `ServerSpec`.
4. `<source>:<name>` — a discovered server (`cursor:linear`).
5. Bare `<name>` if unambiguous across discovered sources.

```bash
mcpal tool list ev
mcpal tool list https://mcp.example/mcp
mcpal tool list ./spec.json
mcpal tool list cursor:linear
mcpal tool list tavily
```

## Transports

- stdio: mcpal spawns a child process and exchanges JSON-RPC over its
  stdin/stdout.
- Streamable HTTP: single endpoint, optional SSE stream. TLS via
  rustls.

The legacy 2024-11-05 SSE transport is not enabled.

## Discovery

mcpal reads other clients' MCP config files:

| Client | macOS | Linux | Windows |
|---|---|---|---|
| Claude Code | `~/.claude.json` | same | same |
| Claude Desktop | `~/Library/Application Support/Claude/` | `~/.config/Claude/` | `%APPDATA%/Claude/` |
| Cursor | `~/.cursor/mcp.json` | same | same |
| Zed | `~/.config/zed/settings.json` | same | same |
| opencode | `~/.config/opencode/opencode.json` | same | same |

Also: LM Studio (`~/.lmstudio/mcp.json`), Windsurf
(`~/.codeium/windsurf/mcp_config.json`), Cline (VS Code
`globalStorage`).

`mcpal server discover` lists everything found. `mcpal server list
--all` shows owned and discovered together. Discovered servers are
addressable as `<source>:<name>` without copying their config.

## Auth

| Mode | Storage | Command |
|---|---|---|
| Inline bearer | OS keyring | `mcpal auth login <ref> --bearer <TOKEN>` |
| `BearerEnv` | environment variable | TOML: `auth = { type = "bearer_env", env = "MY_TOKEN" }` |
| OAuth 2.1 | OS keyring (stored credentials) | `mcpal auth login <ref> --oauth` |
| One-shot | environment | `MCPAL_BEARER=… mcpal tool list <ref>` |

Tokens live in the OS keyring (Keychain on macOS, Secret Service on
Linux, Credential Manager on Windows), never in `config.toml`. OAuth
flow: open browser → loopback callback → token exchange → store in
keyring.

The full lifecycle is in [Auth deep dive](./auth.md).

## Output

```
mcpal tool list <ref>                # yaml (default)
mcpal --output json tool list <ref>  # pretty JSON
mcpal --query 'content[0].text' …    # JMESPath filter applied first
```

YAML is the default. Use `--output json` for `jq`. `--query` applies
a JMESPath expression before rendering.

Exit codes: see [Scripting & exit codes](./scripting.md).
