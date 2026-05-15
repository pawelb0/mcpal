# mcpal

`curl` for the [Model Context Protocol](https://modelcontextprotocol.io).

```bash
mcpal server list --all
mcpal tool call cursor:linear get-issue --id ENG-123
mcpal auth login notion --oauth
mcpal --query 'content[0].text' tool call ev echo --message hi
```

A scriptable command-line client for MCP. No browser, no LLM, no Node or
Python runtime. Single static Rust binary.

## What it does

Three things, well.

1. **Reuses servers already on disk.** Claude Code, Claude Desktop,
   Cursor, Zed, opencode, LM Studio, Windsurf, and Cline all store
   their MCP server configs on disk. mcpal reads every one of them and
   lets you talk to those servers without copying their config.
2. **Speaks the whole protocol.** Tools, resources, resource templates,
   prompts, subscriptions, logging, server-initiated requests, and
   `mcpal raw` for any JSON-RPC method without a first-party verb yet.
3. **Survives pipelines.** Stable exit codes per failure class,
   `--output json|yaml`, AWS-CLI `--query <jmespath>`, rustc-style
   error blocks with stable `E####` codes, `--timeout SECS`, Ctrl-C
   cancellation. `case $?` works.

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

## License

MIT OR Apache-2.0, at your option.
