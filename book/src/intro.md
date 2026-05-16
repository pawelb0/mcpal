# mcpal

mcpal is a command-line client for the
[Model Context Protocol](https://modelcontextprotocol.io). It connects
to MCP servers over stdio or HTTP and calls tools, reads resources,
gets prompts, runs raw JSON-RPC, and streams notifications.

```bash
mcpal server list --all
mcpal tool call cursor:linear get-issue --id ENG-123
mcpal auth login notion --oauth
mcpal --query 'content[0].text' tool call ev echo --message hi
```

MCP is a JSON-RPC contract between LLM-aware clients and servers that
expose tools, resources, and prompts. mcpal plays the client role from
outside any specific LLM app.

## Chapters

- [Concepts](./concepts.md) — references, transports, auth, output.
- [Getting started](./getting-started.md) — install and first commands.
- [Recipes](./recipes.md) — short task-driven snippets.
- [Auth](./auth.md) — bearer and OAuth 2.1.
- [Scripting](./scripting.md) — exit codes, `--query`, JSON output.
- [Troubleshooting](./troubleshooting.md) — `mcpal debug doctor`,
  common errors.
- [Protocol matrix](./protocol-matrix.md) — every MCP method and the
  verb that calls it.
- [Error codes](./error-codes.md) — every `E####` in long form.

## License

MIT
