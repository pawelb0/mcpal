# mcpal

`mcpal` is a command-line tool for **interacting with MCP servers** —
the JSON-RPC servers spoken to by Claude Desktop, Claude Code, Cursor,
Zed, opencode, LM Studio, Windsurf, Cline, and any other client of the
[Model Context Protocol](https://modelcontextprotocol.io).

MCP servers expose tools, resources, and prompts an LLM client can
call. mcpal calls them too — from a shell, a CI job, or a Makefile —
without writing client code.

```bash
mcpal server list --all
mcpal tool call cursor:linear get-issue --id ENG-123
mcpal auth login notion --oauth
mcpal --query 'content[0].text' tool call ev echo --message hi
```

Single static Rust binary. No browser, no LLM, no Node or Python
runtime.

## What you can do with it

- **Reuse servers other clients already configured.** Claude Code,
  Claude Desktop, Cursor, Zed, opencode, LM Studio, Windsurf, and
  Cline write their MCP server lists to disk. mcpal reads every one of
  them, so `mcpal tool list cursor:linear` works the moment Cursor
  knows about `linear`.
- **Call any part of the protocol.** Tools, resources, resource
  templates, prompts, subscriptions, `logging/setLevel`,
  server-initiated requests (`roots/list`, `elicitation/create`,
  `sampling/createMessage`), and a `raw` escape hatch for any JSON-RPC
  method without a first-party verb.
- **Authenticate.** Bearer tokens (env or OS keyring) and full
  OAuth 2.1 + PKCE + DCR against HTTP MCP servers.
- **Drive pipelines.** `--output json|yaml`, AWS-CLI-compatible
  `--query <jmespath>`, stable exit codes, `--timeout SECS`, Ctrl-C
  cancellation.

## What's "MCP"?

The [Model Context Protocol](https://modelcontextprotocol.io) is a
JSON-RPC contract between an LLM-aware client (Claude Desktop, Cursor,
…) and a server that exposes tools, resources, and prompts. mcpal
plays the client role of that contract from outside any specific LLM
app.

Read [Concepts](./concepts.md) next.

## License

MIT OR Apache-2.0, at your option.
