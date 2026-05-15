# mcpal

A command-line client for the [Model Context Protocol](https://modelcontextprotocol.io).

Single static binary. Talks to MCP servers from a shell, a CI job, or a
Makefile:

```bash
mcpal server list --all
mcpal tool call cursor:linear get-issue --id ENG-123
mcpal auth login notion --oauth
mcpal --query 'content[0].text' tool call ev echo --message hi
```

Two transports: stdio (child process) and Streamable HTTP (rustls). mcpal
reads server configs from Claude Code, Claude Desktop, Cursor, Zed,
opencode, LM Studio, Windsurf, and Cline; you can call those servers
without copying their config.

## Chapters

- [Concepts](./concepts.md). Refs, transports, discovery, auth, output.
- [Getting started](./getting-started.md). Install plus a first tool call.
- [Recipes](./recipes.md). Task-indexed snippets.
- [Auth deep dive](./auth.md). Bearer tokens, OAuth 2.1 + PKCE + DCR,
  keyring storage, `MCPAL_BEARER`.
- [Scripting & exit codes](./scripting.md). Exit codes, `--output json`,
  `--query`, error codes.
- [Troubleshooting](./troubleshooting.md). `mcpal doctor` and the common
  failures.
- [Protocol compliance matrix](./protocol-matrix.md). MCP methods mcpal
  exposes as verbs versus `raw` passthrough.
- [Error codes](./error-codes.md). Every `E####` with long-form prose.

## Audience

- Driving a server you're building (`tool list`, `describe`, `call`,
  `watch`, `raw`).
- Scripting integrations (`--output json`, `--query`, stable exit codes,
  `--cli-input-json`).
- Reusing servers already configured in another client (`discover`,
  `<source>:<name>`).

If you want a browser GUI, use [MCP Inspector](https://github.com/modelcontextprotocol/inspector).
For an LLM-bundled agent shell, see [Goose](https://github.com/block/goose),
[chrishayuk/mcp-cli](https://github.com/chrishayuk/mcp-cli), or
[mcphost](https://github.com/mark3labs/mcphost). For a Python server SDK,
see [FastMCP](https://github.com/jlowin/fastmcp).

## License

MIT OR Apache-2.0, at your option.
