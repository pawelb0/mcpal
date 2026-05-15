# mcpal

`aws` for the [Model Context Protocol](https://modelcontextprotocol.io).

`mcpal` is a single static Rust binary that lets you drive any MCP server
from a terminal, a CI job, or a `Makefile`:

```bash
mcpal server list --all
mcpal tool call cursor:linear get-issue --id ENG-123
mcpal auth login notion --oauth
mcpal --query 'content[0].text' tool call ev echo --message hi
```

It speaks both local **stdio** MCP servers (e.g. `npx -y @modelcontextprotocol/server-everything`)
and remote **Streamable HTTP** servers (bearer or OAuth 2.1 auth). It
**discovers** MCP servers configured by other clients on your machine —
Claude Code, Claude Desktop, Cursor, Zed, opencode, LM Studio, Windsurf,
Cline — so you don't have to re-enter anything.

## What this book covers

- [**Concepts**](./concepts.md) — refs, transports, discovery, auth, output
  formats. Read this first.
- [**Getting started**](./getting-started.md) — 60-second tour from `cargo install`
  to your first tool call.
- [**Recipes**](./recipes.md) — copy-paste cookbook organized by task.
- [**Auth deep dive**](./auth.md) — bearer tokens, OAuth 2.1 + PKCE + DCR,
  keyring storage, the `MCPAL_BEARER` escape hatch.
- [**Scripting & exit codes**](./scripting.md) — stable exit codes, `--output
  json`, `--query <jmespath>`, piping into `jq`, error code taxonomy.
- [**Troubleshooting**](./troubleshooting.md) — `mcpal doctor`, error code
  cookbook, common gotchas.
- [**Protocol compliance matrix**](./protocol-matrix.md) — which MCP methods
  mcpal exposes as first-class verbs vs `mcpal raw <method>` passthrough.
- [**Error codes**](./error-codes.md) — every E#### code with prose, mirrors
  `mcpal explain E0001`.

## Who this is for

- **You're debugging an MCP server you're building.** `mcpal tool list`,
  `tool describe`, `tool call`, `watch`, `raw` are your friends.
- **You're scripting an integration.** `--output json` + `--query` + stable
  exit codes + `--cli-input-json -` cover every pipeline shape.
- **You configured an MCP server in Claude/Cursor/opencode and want to
  drive it from elsewhere.** `mcpal discover` finds it; `mcpal tool call
  <source>:<name>` reuses the config without re-entering anything.

## Who this isn't for

- You want a browser GUI. Use [MCP Inspector](https://github.com/modelcontextprotocol/inspector).
- You want an LLM-bundled agent shell. Use [Goose](https://github.com/block/goose),
  [chrishayuk/mcp-cli](https://github.com/chrishayuk/mcp-cli), or [mcphost](https://github.com/mark3labs/mcphost).
- You want to build MCP servers in Python. Use [FastMCP](https://github.com/jlowin/fastmcp).

## License

MIT OR Apache-2.0, at your option.
