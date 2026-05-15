# Concepts

The five things you need to know before reading anything else.

## 1. Server reference (`<ref>`)

Every command that talks to an MCP server takes a `<ref>` positional. It
resolves in this order:

1. **mcpal-owned alias** — anything you registered with `mcpal server add`.
2. **`http://` or `https://` URL** — anonymous HTTP MCP server.
3. **Path to a JSON file** containing one `ServerSpec`.
4. **`<source>:<name>`** — a discovered server, fully qualified
   (`cursor:linear`, `opencode:tavily`).
5. **Bare `<name>`** — discovered server if it's unambiguous across sources.

Examples:

```bash
mcpal tool list ev                            # owned alias
mcpal tool list https://mcp.example/mcp       # raw URL
mcpal tool list ./spec.json                   # file
mcpal tool list cursor:linear                 # discovered
mcpal tool list tavily                        # bare; only works if unambiguous
```

## 2. Transports

Two are first-class:

- **stdio** — mcpal spawns a child process and speaks JSON-RPC over its
  stdin/stdout. Use for local servers like `@modelcontextprotocol/server-everything`.
- **Streamable HTTP** — a single endpoint with optional SSE for server-sent
  events. Use for hosted servers. Uses rustls (no system OpenSSL).

SSE (the legacy 2024-11-05 transport) is not exposed; rmcp covers it
behind a feature flag we don't enable.

## 3. Discovery

mcpal reads the MCP server configs that other clients on your machine
already wrote:

| Client | macOS path | Linux path | Windows path |
|---|---|---|---|
| Claude Code | `~/.claude.json` | same | same |
| Claude Desktop | `~/Library/Application Support/Claude/...` | `~/.config/Claude/...` | `%APPDATA%/Claude/...` |
| Cursor | `~/.cursor/mcp.json` | same | same |
| Zed | `~/.config/zed/settings.json` | same | same |
| opencode | `~/.config/opencode/opencode.json` | same | same |
| LM Studio | `~/.lmstudio/mcp.json` | same | same |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` | same | same |
| Cline | `~/Library/.../Code/User/globalStorage/...` | `~/.config/Code/User/globalStorage/...` | `%APPDATA%/Code/User/globalStorage/...` |

`mcpal discover` lists everything found. `mcpal server list --all` shows
owned + discovered together. Discovered servers are usable directly
(no copy step) as `<source>:<name>`.

## 4. Auth

| Mode | Where the secret lives | How to set |
|---|---|---|
| Inline bearer | OS keyring | `mcpal auth login <ref> --bearer <TOKEN>` |
| `BearerEnv` | environment variable | `auth = { type = "bearer_env", env = "MY_TOKEN" }` in config |
| OAuth 2.1 | OS keyring (StoredCredentials JSON) | `mcpal auth login <ref> --oauth` |
| One-shot | environment | `MCPAL_BEARER=… mcpal tool list <ref>` |

Secrets never live in `config.toml`. They live in the OS keyring
(Keychain on macOS, Secret Service on Linux, Credential Manager on
Windows). The OAuth flow runs locally — mcpal opens a browser to the
authorization URL, listens on a loopback port for the callback,
exchanges the code for tokens, and stores them.

See the [Auth deep dive](./auth.md) for the full lifecycle.

## 5. Output

```
mcpal tool list <ref>                # yaml (default)
mcpal --output json tool list <ref>  # pretty JSON
mcpal --query 'content[0].text' …    # JMESPath filter before output
```

YAML is the default because it is both human-readable and machine-parseable
in one shot. `--output json` is pretty JSON, suitable for `jq`. `--query`
is AWS-CLI-compatible JMESPath, evaluated before rendering.

Exit codes are stable — see [Scripting & exit codes](./scripting.md).
