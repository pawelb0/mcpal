# mcpal

`curl` for the [Model Context Protocol](https://modelcontextprotocol.io).

```
$ mcpal server list --all
$ mcpal tool call cursor:linear get-issue --id ENG-123
$ mcpal auth login notion --oauth
$ mcpal --query 'content[0].text' tool call ev echo --message hi
```

A scriptable command-line client for MCP. No browser, no LLM, no Node or
Python runtime. Single static Rust binary.

## What it does

Three things, well.

1. **Reuses servers already on your machine.** Claude Code, Claude
   Desktop, Cursor, Zed, opencode, LM Studio, Windsurf, and Cline all
   store their MCP server configs on disk. mcpal reads every one of
   them and lets you call those servers without copying their config:
   `mcpal tool list cursor:linear` works the moment Cursor knows about
   `linear`.
2. **Speaks the whole protocol.** Tools, resources, resource templates,
   prompts, subscriptions, logging set-level, server-initiated requests
   (`roots/list`, `elicitation/create`, `sampling/createMessage`), and a
   `raw` passthrough for any JSON-RPC method that doesn't yet have a
   first-party verb.
3. **Survives pipelines.** Stable exit codes per failure class,
   `--output json|yaml`, AWS-CLI-compatible `--query <jmespath>`,
   rustc-style error blocks with stable `E####` codes, and
   `mcpal explain E####` for the long-form prose. `case $?` works.

## Install

```
cargo install --path crates/mcpal-cli
```

Prebuilt binaries: pending first tag. The `cargo-dist` workflow lands
under `.github/workflows/release.yml`; once tagged it produces macOS
(arm64 + x86_64), Linux (glibc + musl), Windows binaries, a Homebrew
tap, and a `curl | sh` installer.

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
mcpal server test ev
mcpal server test ev --full          # also enumerates capabilities
```

Tokens after `--` are the command and its args (Claude Code / mcptools
style). The older `--stdio <cmd> --arg <a> --arg <b>` form still works.

### Add a remote HTTP server

```bash
mcpal server add ctx7 --http https://mcp.context7.com/mcp
mcpal server test ctx7

mcpal server add github --http https://api.githubcopilot.com/mcp/
mcpal auth login github --bearer ghp_xxx     # token → OS keyring
mcpal tool list github

mcpal server add notion --http https://mcp.notion.com/v1
mcpal auth login notion --oauth              # PKCE + DCR + loopback callback
mcpal auth refresh notion                    # mint a new access token later
```

Tokens always live in the OS keyring (Keychain on macOS, Secret Service
on Linux, Credential Manager on Windows), never in `config.toml`.

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
mcpal discover                           # full dump
mcpal discover --source cursor           # one client
mcpal server list --all                  # mcpal-owned + discovered
mcpal tool list opencode:tavily          # call directly, no copy step
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
| 3 | server ref not found | `mcpal discover` |
| 4 | auth required | `mcpal auth login <ref>` |
| 5 | auth expired | `mcpal auth refresh <ref>` |
| 6 | transport | network or stdio failure |
| 7 | server error | check args vs `tool describe` |
| 8 | timed out | retry; raise `--timeout` |
| 130 | interrupted (Ctrl-C) | — |

Each error renders with a stable `E####` code:

```
error[E0001]: server 'foo' not found (owned, URL, path, or discovered)
help: run `mcpal discover` to scan installed MCP clients for servers
help: or `mcpal server list --all` to see what's already configured
help: or add one: `mcpal server add <alias> --stdio <command>`

For more information about this error, try `mcpal explain E0001`.
```

Eleven codes today: E0000–E0011. `mcpal explain <code>` prints the
long-form prose for each.

### Sanity check

```bash
mcpal doctor
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
url = "https://mcp.linear.app/sse"
auth = "oauth"

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
