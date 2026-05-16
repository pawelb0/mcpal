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

## How this book is organised

The chapters follow the [Diátaxis](https://diataxis.fr) framework: a
single tutorial, problem-driven how-tos, factual reference, and
explanation.

**Tutorial** — start here if you have never used mcpal.

- [Your first MCP call](./getting-started.md) — install, register the
  reference server, list and call tools.

**How-to guides** — find the recipe for the problem you have.

- [Recipes](./recipes.md) — short task-driven snippets.
- [Authenticate to an HTTP server](./auth.md) — bearer or OAuth 2.1.
- [Script around mcpal](./scripting.md) — exit codes, `--query`, JSON
  output, CI patterns.
- [Troubleshoot](./troubleshooting.md) — `mcpal debug doctor`, common
  failures.

**Reference** — look up specifics.

- [Protocol compliance matrix](./protocol-matrix.md) — every MCP
  method and the verb that calls it.
- [Error codes](./error-codes.md) — every `E####` in long form.

**Explanation** — read once for the model.

- [Concepts](./concepts.md) — references, transports, auth modes,
  output.

## License

MIT
