# mcpal

`aws` for the [Model Context Protocol](https://modelcontextprotocol.io).

```
$ mcpal server list --all
$ mcpal tool call cursor:linear get-issue --arg id=ENG-123
$ mcpal auth login --oauth notion
```

Status: early. M1–M5 shipped (stdio + Streamable HTTP, 8-client discovery, bearer + OAuth 2.1, server-initiated handler).

## Install

```
cargo install --path crates/mcpal-cli
```

Prebuilt binaries: pending (cargo-dist).

## Features

| Surface | Status |
|---|---|
| Local stdio transport (`@modelcontextprotocol/server-everything` etc.) | ✓ |
| Streamable HTTP transport, rustls | ✓ |
| Bearer auth (inline, env var, OS keyring) | ✓ |
| OAuth 2.1 + PKCE + Dynamic Client Registration | ✓ |
| `list_roots` handler — `--root <path>` | ✓ |
| Elicitation prompts + server log forwarding | ✓ |
| Discovery across Claude Desktop, Claude Code, Cursor, Zed, opencode, LM Studio, Windsurf, Cline | ✓ (macOS/Linux; Windows pending) |
| Sampling plugin (`--sampling-handler <cmd>`) | pending |
| `mcpal repl` | pending |

## Usage

### Add and inspect a server

```
mcpal init
mcpal server add everything --stdio npx --arg -y --arg @modelcontextprotocol/server-everything
mcpal server list --all
mcpal server show everything
mcpal server test everything
```

### Discover what other clients already configured

```
mcpal discover
mcpal discover --source opencode
mcpal server list --all       # mcpal-owned + discovered
```

A discovered server is referenceable as `<source>:<name>`, or bare `<name>` if unambiguous:

```
mcpal tool list opencode:tavily
mcpal tool call cursor:linear get-issue --arg id=ENG-123
```

Copy a discovered server into mcpal config so you can override env/auth/alias:

```
mcpal server import --from opencode tavily --as tav
```

### Call tools, read resources, fetch prompts

Flags follow AWS-CLI style: `--key value` pairs. Values parse as typed JSON
(numbers, booleans, JSON literals) when possible, otherwise stay strings.

```
mcpal tool list <ref>
mcpal tool describe <ref> <name>
mcpal tool call <ref> <name> --key value ...
mcpal tool call <ref> <name> --cli-input-json args.json --override-key new-value
echo '{"k":"v"}' | mcpal tool call <ref> <name> --cli-input-json -

mcpal resource list <ref>
mcpal resource read <ref> <uri>
mcpal resource template list <ref>

mcpal prompt list <ref>
mcpal prompt get <ref> <name> --city Dallas
mcpal ping <ref>
```

### Interactive shell

```
$ mcpal repl <ref>
mcpal> tool list
mcpal> tool describe echo
mcpal> tool call echo --message hi
mcpal> resource read demo://resource/static/document/architecture.md
mcpal> quit
```

Arrow-up history, line editing, persisted across sessions.

### Auth

Bearer — stored in OS keyring (Keychain / Secret Service / Credential Manager):

```
mcpal auth login <ref> --bearer <TOKEN>
mcpal auth login <ref> --bearer -        # read stdin
mcpal auth login <ref>                    # prompt on TTY
MCPAL_BEARER=... mcpal tool list <ref>    # one-shot env
```

OAuth 2.1 — discovery + DCR + loopback callback + PKCE:

```
mcpal auth login <ref> --oauth
mcpal auth refresh <ref>
mcpal auth status <ref>
mcpal auth logout <ref>
```

### Output

Default detects TTY: human (comfy-table) on a terminal, JSONL when piped. Override:

```
mcpal --output json tool list <ref>
mcpal --output jsonl resource list <ref>
```

## Configuration

`~/.config/mcpal/config.toml` on Linux, `~/Library/Application Support/mcpal/config.toml` on macOS. Override with `MCPAL_CONFIG=/path/to/file`.

```toml
[server.everything]
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-everything"]

[server.linear]
transport = "http"
url = "https://mcp.linear.app/sse"
auth = "oauth"
```

Secrets never live in this file; they go to the OS keyring via `mcpal auth login`.

## License

MIT OR Apache-2.0, at your option.
