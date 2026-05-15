# mcpal

A scriptable command-line client for the
[Model Context Protocol](https://modelcontextprotocol.io). Single static
Rust binary. No browser, no LLM, no Node or Python runtime.

```bash
mcpal server list --all
mcpal tool call cursor:linear get-issue --id ENG-123
mcpal auth login notion --oauth
mcpal --query 'content[0].text' tool call ev echo --message hi
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
   `mcpal explain E####` for the long-form prose, `--timeout SECS` and
   Ctrl-C cancellation.

Read [Concepts](./concepts.md) next.

## License

MIT OR Apache-2.0, at your option.
