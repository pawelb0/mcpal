# mcpal

mcpal is a command-line client for the
[Model Context Protocol](https://modelcontextprotocol.io). It connects
to MCP servers over stdio or HTTP and calls tools, reads resources,
gets prompts, runs raw JSON-RPC, and streams notifications.

```
mcpal server list --all
mcpal tool call cursor:linear get-issue --id ENG-123
mcpal auth login notion --oauth
mcpal --query 'content[0].text' tool call ev echo --message hi
```

## Install

    brew tap pawelb0/tap
    brew install --HEAD pawelb0/tap/mcpal

From source:

    cargo install --git https://github.com/pawelb0/mcpal --path crates/mcpal-cli

Once a release is tagged, the `curl | sh` installer drops a prebuilt
binary in `$HOME/.local/bin`:

    curl -fsSL https://raw.githubusercontent.com/pawelb0/mcpal/main/dist/install.sh | sh

## Documentation

Manual: <https://pawelb0.github.io/mcpal/>.

## Synopsis

    mcpal <command> [<ref>] [options]

A `<ref>` is one of:

- a name registered with `mcpal server add`
- `<source>:<name>` from `mcpal server discover` (e.g. `cursor:linear`)
- a bare `<name>` if unambiguous across discovered sources
- an `https://` URL
- a path to a JSON `ServerSpec`

`mcpal discover` reads the MCP server lists that other clients
(Claude Desktop, Cursor, opencode, ...) already wrote to disk; those
servers are addressable directly.

## Examples

Local server, read-only inspection:

    mcpal server add ev -- npx -y @modelcontextprotocol/server-everything
    mcpal server ping ev
    mcpal server capabilities ev
    mcpal tool list ev
    mcpal tool describe ev echo

Call a tool:

    mcpal tool call ev echo --message hi
    mcpal tool call ev echo --params '{"message":"hi"}'
    echo '{"message":"hi"}' | mcpal tool call ev echo --params -

HTTP server with bearer:

    mcpal server add github --http https://api.githubcopilot.com/mcp/
    mcpal auth login github --bearer ghp_xxx
    mcpal tool list github

HTTP server with OAuth 2.1 (PKCE + DCR):

    mcpal server add notion --http https://mcp.notion.com/v1
    mcpal auth login notion --oauth
    mcpal auth refresh notion

Resources, prompts, logging:

    mcpal resource list ev
    mcpal resource read ev demo://resource/static/document/architecture.md
    mcpal resource template list ev
    mcpal resource subscribe ev some://uri
    mcpal prompt list ev
    mcpal prompt get ev args-prompt --city Dallas --state Texas
    mcpal logging set-level ev debug

Notifications:

    mcpal watch ev          # one YAML doc per event, Ctrl-C to exit

Arbitrary JSON-RPC:

    mcpal raw ev some/method --params '{"k":"v"}'
    mcpal raw ev some/method --params @payload.json
    mcpal raw ev some/method --params -

Discovery:

    mcpal server discover
    mcpal server discover --source cursor
    mcpal server list --all
    mcpal tool list opencode:tavily
    mcpal --mcp-json ./mcp.json tool list x

Pipelines:

    mcpal --output json tool list ev | jq -r '.[].name'
    mcpal --query '[].name' tool list ev
    mcpal --timeout 5 tool call ev some-tool

Diagnostics:

    mcpal debug doctor
    mcpal debug explain E0001

## Configuration

`~/.config/mcpal/config.toml` on Linux,
`~/Library/Application Support/mcpal/config.toml` on macOS,
`%APPDATA%\mcpal\config.toml` on Windows. Override with
`MCPAL_CONFIG=/path/to/file`.

```toml
[server.everything]
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-everything"]

[server.notion]
transport = "http"
url = "https://mcp.notion.com/v1"
auth = { type = "bearer_env", env = "NOTION_MCP_TOKEN" }
```

Secrets do not live in this file. `mcpal auth login` writes them to
the OS keyring.

## License

MIT
