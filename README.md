# mcpal

A scriptable command-line client for the
[Model Context Protocol](https://modelcontextprotocol.io). Single static
Rust binary. No browser, no LLM, no Node or Python runtime.

```
$ mcpal server list --all
$ mcpal tool call cursor:linear get-issue --id ENG-123
$ mcpal auth login notion --oauth
$ mcpal --query 'content[0].text' tool call ev echo --message hi
```

## What it does

1. **Reuses servers already configured by other clients.** Claude Code,
   Claude Desktop, Cursor, Zed, opencode, LM Studio, Windsurf, and
   Cline all store their MCP server configs on disk. mcpal reads every
   one of them, so `mcpal tool list cursor:linear` works the moment
   Cursor knows about `linear`.
2. **Speaks the full protocol.** Tools, resources, resource templates,
   prompts, subscriptions, logging set-level, server-initiated requests
   (`roots/list`, `elicitation/create`, `sampling/createMessage`), and
   a `raw` passthrough for any JSON-RPC method that doesn't yet have a
   first-party verb.
3. **Works in pipelines.** Stable exit codes per failure class,
   `--output json|yaml`, AWS-CLI-compatible `--query <jmespath>`,
   rustc-style error blocks with stable `E####` codes,
   `mcpal debug explain E####` for the long-form prose, `--timeout SECS` and
   Ctrl-C cancellation.

## Install

Homebrew (tracks `main` until the first tagged release):

```
brew tap pawelb0/tap
brew install --HEAD pawelb0/tap/mcpal
```

From source:

```
cargo install --git https://github.com/pawelb0/mcpal --path crates/mcpal-cli
```

After the first tagged release, the curl installer pulls a prebuilt
binary into `$HOME/.local/bin` (override with `MCPAL_INSTALL_DIR`):

```
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/pawelb0/mcpal/main/dist/install.sh | sh
```

The release workflow at `.github/workflows/release.yml` builds macOS
(arm64 + x86_64), Linux (x86_64 GNU), and Windows binaries.

## Documentation

The user manual lives at <https://pawelb0.github.io/mcpal/> (built
from `book/` via mdBook).

## 60-second tour

Replace `<ref>` with any of:

- an alias you registered with `mcpal server add`
- `<source>:<name>` from discovery (`cursor:linear`, `opencode:tavily`)
- a bare `<name>` if unambiguous across discovered sources
- a raw `https://…` URL
- a path to a JSON file containing one `ServerSpec`

### Add a stdio server

```bash
mcpal server add ev -- npx -y @modelcontextprotocol/server-everything
mcpal server ping ev
mcpal server capabilities ev          # also enumerates capabilities
```

Tokens after `--` are the command and its args. The older `--stdio
<cmd> --arg <a> --arg <b>` form still works.

### Add a remote HTTP server

```bash
mcpal server add ctx7 --http https://mcp.context7.com/mcp
mcpal server ping ctx7

mcpal server add github --http https://api.githubcopilot.com/mcp/
mcpal auth login github --bearer ghp_xxx     # token → OS keyring
mcpal tool list github

mcpal server add notion --http https://mcp.notion.com/v1
mcpal auth login notion --oauth              # PKCE + DCR
mcpal auth refresh notion                    # mint a new access token later
```

Tokens live in the OS keyring, never in `config.toml`.

### Tools

```bash
mcpal tool list ev                    # name + description + required args
mcpal tool describe ev echo           # full input schema
mcpal tool template ev echo           # known-good skeleton JSON
mcpal tool call ev echo --message hi
mcpal tool call ev echo --params '{"message":"hi"}'      # inline JSON
mcpal tool call ev echo --cli-input-json @body.json      # from a file
echo '{"message":"hi"}' | mcpal tool call ev echo --params -
```

### Resources, prompts, logging

```bash
mcpal resource list ev
mcpal resource read ev demo://resource/static/document/architecture.md
mcpal resource template list ev
mcpal resource subscribe ev some://uri

mcpal prompt list ev
mcpal prompt get ev args-prompt --city Dallas --state Texas

mcpal logging set-level ev debug
```

### Watch notifications

```bash
mcpal watch ev      # one YAML doc per progress / log / list_changed
                    # notification; Ctrl-C to exit
```

### Raw passthrough

```bash
mcpal raw ev some/method --params '{"foo":"bar"}'
mcpal raw ev some/method --params @payload.json
mcpal raw ev some/method --params -
```

### Discover servers from other clients

```bash
mcpal server discover                           # full dump
mcpal server discover --source cursor           # one client
mcpal server list --all                  # mcpal-owned + discovered
mcpal tool list opencode:tavily          # call directly
mcpal --mcp-json ./mcp.json tool list x  # use a Claude/Cursor config inline
mcpal server import --from opencode tavily --as tav
```

### Pipelines

```bash
mcpal --output json tool list ev | jq -r '.[].name'
mcpal --query '[].name' tool list ev
mcpal --timeout 5 tool call ev trigger-long-running-operation --duration 3 --steps 5

for q in rust go python; do
  mcpal tool call github search --q "$q stars:>1000" --per_page 3
done
```

### Exit codes + error system

| Code | Meaning | Common fix |
|---|---|---|
| 0 | success | — |
| 1 | generic | check stderr |
| 2 | usage | `mcpal <cmd> --help` |
| 3 | server ref not found | `mcpal server discover` |
| 4 | auth required | `mcpal auth login <ref>` |
| 5 | auth expired | `mcpal auth refresh <ref>` |
| 6 | transport | network or stdio failure |
| 7 | server error | check args vs `tool describe` |
| 8 | timed out | retry; with `--timeout`, raise the value |
| 130 | interrupted (Ctrl-C) | — |

Each error renders with a stable `E####` code:

```
error[E0001]: server 'foo' not found (owned, URL, path, or discovered)
help: run `mcpal server discover` to scan installed MCP clients for servers
help: or `mcpal server list --all` to see what's already configured
help: or add one: `mcpal server add <alias> --stdio <command>`

For more information about this error, try `mcpal debug explain E0001`.
```

Codes E0000–E0011 today. `mcpal debug explain <code>` prints the long-form
prose for each.

### Sanity check

```bash
mcpal debug doctor
```

Checks: config readable, keyring round-trip, auth state per server,
discovery counts. `--output json` for bug reports.

## Configuration

`~/.config/mcpal/config.toml` (Linux), `~/Library/Application
Support/mcpal/config.toml` (macOS), `%APPDATA%\mcpal\config.toml`
(Windows). Override with `MCPAL_CONFIG=/path/to/file`.

```toml
[server.everything]
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-everything"]

[server.linear]
transport = "http"
url = "https://mcp.linear.app/mcp"
auth = { type = "oauth" }

[server.notion]
transport = "http"
url = "https://mcp.notion.com/v1"
auth = { type = "bearer_env", env = "NOTION_MCP_TOKEN" }
```

Secrets never live in this file; they go to the OS keyring via
`mcpal auth login`.

## Status

M1–M7 shipped. The full manual lives under `book/` (mdBook). Roadmap to
`v0.1.0`: registry-aware `server install`, dynamic tool-name completion,
cargo-dist release artifacts.

## License

MIT OR Apache-2.0, at your option.
