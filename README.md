# mcpal

`aws` for the [Model Context Protocol](https://modelcontextprotocol.io).

```
$ mcpal server list --all
$ mcpal tool call cursor:linear get-issue --id ENG-123
$ mcpal auth login --oauth notion
```

Status: early. M1–M6 shipped (stdio + Streamable HTTP, 8-client discovery, bearer + OAuth 2.1 + DCR, server-initiated handler, sampling plugin, CI matrix, prebuilt-binaries workflow).

## Install

```
cargo install --path crates/mcpal-cli
```

Prebuilt binaries: pending first tag (workflow ready in `.github/workflows/release.yml`).

## Features

| Surface | Status |
|---|---|
| Local stdio transport (`@modelcontextprotocol/server-everything` etc.) | ✓ |
| Streamable HTTP transport, rustls | ✓ |
| Bearer auth (inline, env var, OS keyring) | ✓ |
| OAuth 2.1 + PKCE + Dynamic Client Registration | ✓ |
| `list_roots` handler — `--root <path>` | ✓ |
| Elicitation prompts + server log forwarding (via `tracing`) | ✓ |
| Sampling plugin (`--sampling-handler <cmd>`) | ✓ |
| Discovery across Claude Desktop, Claude Code, Cursor, Zed, opencode, LM Studio, Windsurf, Cline | ✓ |
| YAML (default) + JSON output | ✓ |

## Quick tour

Every example below is copy-paste ready. Replace `<ref>` with any of:
- a mcpal-owned alias (`mcpal server add foo …`)
- `<source>:<name>` from discovery (`cursor:linear`, `opencode:tavily`)
- a bare name if unambiguous across discovered servers
- a raw `https://…` URL
- a path to a JSON file containing one `ServerSpec`

### 0. Init

```bash
mcpal init
```
Writes a default config to `~/Library/Application Support/mcpal/config.toml` (macOS) or `~/.config/mcpal/config.toml` (Linux).

### 1. Add a stdio server (local process)

```bash
mcpal server add everything \
  --stdio npx --arg -y --arg @modelcontextprotocol/server-everything
mcpal server test everything
```

`server test` returns a yaml record with `ok: true` + serverInfo:

```yaml
ref: everything
ok: true
server:
  name: mcp-servers/everything
  version: 2.0.0
peerInfo:
  …
```

### 2. Add a remote HTTP server

No auth:

```bash
mcpal server add ctx7 --http https://mcp.context7.com/mcp
mcpal server test ctx7
```

With a bearer token (token goes to the OS keyring, never the TOML):

```bash
mcpal server add github --http https://api.githubcopilot.com/mcp/
mcpal auth login github --bearer ghp_xxx
mcpal tool list github
```

With OAuth (browser flow + DCR + PKCE, tokens persisted to keyring):

```bash
mcpal server add notion --http https://mcp.notion.com/v1 --auth oauth
mcpal auth login notion --oauth
mcpal tool list notion
# later, if access token expires:
mcpal auth refresh notion
```

### 3. List + describe + call tools

`tool list` is a compact summary (name, description, required args). The full schema lives in `tool describe`.

```bash
mcpal tool list everything
```
```yaml
- name: echo
  description: Echoes back the input string
  required:
  - message
- name: get-sum
  description: Returns the sum of two numbers
  required:
  - a
  - b
- name: get-tiny-image
  description: Returns a tiny MCP logo image.
```

```bash
mcpal tool describe everything echo
```
```yaml
name: echo
title: Echo Tool
description: Echoes back the input string
inputSchema:
  $schema: http://json-schema.org/draft-07/schema#
  properties:
    message:
      description: Message to echo
      type: string
  required:
  - message
  type: object
```

Call a tool — AWS-CLI style flags, values typed automatically:

```bash
mcpal tool call everything echo --message "hello world"
mcpal tool call everything get-sum --a 2 --b 40
mcpal tool call everything trigger-long-running-operation --duration 3 --steps 5
```

Pass a JSON object as the base, override individual fields:

```bash
echo '{"message":"piped"}' | mcpal tool call everything echo --cli-input-json -
mcpal tool call everything echo --cli-input-json args.json --message override
```

### 4. Resources

```bash
mcpal resource list everything
mcpal resource read everything demo://resource/static/document/architecture.md
mcpal resource template list everything
```

### 5. Prompts

```bash
mcpal prompt list everything
mcpal prompt get everything simple-prompt
mcpal prompt get everything args-prompt --city Dallas --state Texas
```

### 6. Discover servers from other clients

Scan every supported client on your machine:

```bash
mcpal discover
```
```yaml
- source: claude-code
  source_path: /Users/pawelb/.claude.json
  name: chrome-devtools
  spec:
    transport: stdio
    command: npx
    args: [chrome-devtools-mcp@latest]
  scope: global
- source: opencode
  …
```

Filter to one client:

```bash
mcpal discover --source cursor
```

Use a discovered entry directly — no copy needed:

```bash
mcpal tool list cursor:linear
mcpal tool call opencode:tavily search --query "rust async runtimes"
```

Or copy it into your own config so you can override env / auth / alias:

```bash
mcpal server import --from opencode tavily --as tav
mcpal auth login tav --bearer $TAVILY_KEY
mcpal tool call tav search --query rust
```

`server list --all` shows mcpal-owned + discovered together:

```bash
mcpal server list --all
mcpal server list --discovered --source claude-code
```

### 7. Pipe into other tools

YAML is the default for humans. Add `--output json` whenever you want machine-readable output:

```bash
# pick the tool with the longest description
mcpal --output json tool list everything | jq -r 'max_by(.description|length).name'

# extract the access token from a `server test` result
mcpal --output json server test ctx7 | jq -r .peerInfo.serverInfo.version

# script a series of calls
for q in rust go python; do
  mcpal tool call github search --q "$q stars:>1000" --per_page 3
done
```

### 8. Auth lifecycle

```bash
mcpal auth login github --bearer ghp_xxx     # bearer → keyring
mcpal auth login notion --oauth              # OAuth flow → keyring
mcpal auth status github                      # { bearer: true, oauth: false }
mcpal auth refresh notion                     # refresh access token
mcpal auth logout github                      # wipe both kinds for the ref
```

One-shot env override (no keyring write):

```bash
MCPAL_BEARER=ghp_xxx mcpal tool list github
```

### 9. Server-initiated requests

Some servers ask the client for things during a tool call. mcpal handles them by default:

| Method | Default behavior |
|---|---|
| `roots/list` | Returns whatever paths you pass via `--root` |
| `elicitation/create` (form) | Prompts on stderr; reads one line; or declines with `--no-interactive` |
| `elicitation/create` (url) | Prints the URL + accepts |
| `sampling/createMessage` | Without a handler: method-not-found. With `--sampling-handler <cmd>`: pipes the request JSON to that program on stdin, parses `CreateMessageResult` JSON from its stdout |
| `logging/message` | Routed through `tracing`; respect `RUST_LOG` |

Expose two workspace roots to the server:

```bash
mcpal --root ~/src/my-project --root /tmp tool call everything get-roots-list
```

Wire sampling to a local LLM CLI (the contract is JSON-in, JSON-out):

```bash
mcpal --sampling-handler "claude --output json" tool call ev trigger-sampling-request --prompt "summarize"
```

## Output

YAML by default — readable + parseable in one shot. JSON when you need it:

```bash
mcpal tool list <ref>                # yaml
mcpal --output json tool list <ref>  # pretty JSON
```

## Configuration

`~/.config/mcpal/config.toml` (Linux), `~/Library/Application Support/mcpal/config.toml` (macOS), `%APPDATA%\mcpal\config.toml` (Windows). Override with `MCPAL_CONFIG=/path/to/file`.

```toml
[server.everything]
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-everything"]

[server.linear]
transport = "http"
url = "https://mcp.linear.app/sse"
auth = "oauth"

[server.notion]
transport = "http"
url = "https://mcp.notion.com/v1"
auth = { type = "bearer_env", env = "NOTION_MCP_TOKEN" }
```

Secrets never live in this file; they go to the OS keyring via `mcpal auth login`.

## License

MIT OR Apache-2.0, at your option.
