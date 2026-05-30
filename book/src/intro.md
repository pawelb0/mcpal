# mcpal

**Inspect, call, and script any MCP server — over stdio or HTTP, with
OAuth, from one CLI.** The curl-equivalent for the
[Model Context Protocol](https://modelcontextprotocol.io).

> Your editors (Claude Desktop, Cursor, Zed, opencode) configure dozens
> of MCP servers. Once configured, the only way to drive them is from
> inside that chat app. mcpal is the shell tool that was missing: point
> it at any server and call tools, read resources, get prompts, run raw
> JSON-RPC, or stream notifications.

New here? **[Install](./install.md) → [Your first MCP call](./getting-started.md) → [Concepts](./concepts.md)**.

```bash
mcpal server list --all
mcpal tool call cursor:linear get-issue --id ENG-123
mcpal auth login notion --oauth
mcpal --query 'content[0].text' tool call ev echo --message hi
```

MCP is a JSON-RPC contract between LLM-aware clients and servers that
expose tools, resources, and prompts. mcpal plays the client role from
outside any specific LLM app.

## Why mcpal exists

LLM-facing MCP clients (Claude Desktop, Claude Code, Cursor, Zed,
opencode) configure dozens of MCP servers — GitHub, Linear, Notion, a
filesystem sandbox, a Postgres bridge, an internal HTTP service. Once
they're configured, the only way to drive them is from inside that
chat client. There is no curl-like tool for testing, scripting, or
inspecting them.

mcpal fills that gap:

- **Debugging MCP servers you're building.** Run `tool call`,
  `tool describe`, `raw`, and `watch` against the server you just
  started. Skip the round-trip through a chat UI.
- **Scripting integrations in CI.** A nightly job that pulls Linear
  tickets, opens a Notion page, files a GitHub issue, or syncs
  filesystem state — all through the same servers your team's
  editors already use.
- **Calling already-configured servers from outside an LLM app.**
  Cursor configured `linear`? Run `mcpal tool list cursor:linear`.
  No copy-paste of `mcp.json`.
- **Auditing what's installed.** `mcpal server discover` reports
  every server the supported clients know about, with paths,
  transports, and scopes.

The protocol itself is the same MCP that Anthropic published; mcpal
just exposes its surface to a shell prompt.

## Who it's for

- MCP server authors who want a curl for their server.
- Platform engineers wiring MCP servers into pipelines.
- Anyone running multiple MCP-aware editors who wants one tool to
  call any of their servers.
- Operators who'd rather paste a stack trace into an issue than a
  screenshot of a chat error.

## What a real session looks like

Read Linear issues from the editor's already-configured Linear MCP:

```bash
mcpal tool list cursor:linear --names-only
mcpal tool call cursor:linear get-issue --id ENG-123
mcpal --query 'content[0].text' \
  tool call cursor:linear list-my-issues --state in_progress
```

Pull GitHub releases over HTTP:

```bash
mcpal server add gh --http https://api.githubcopilot.com/mcp/
mcpal auth login gh --bearer $GH_TOKEN
mcpal --output json tool call gh list_releases --owner anthropics --repo claude-code
```

Mount a local filesystem sandbox:

```bash
mcpal server add fs -- npx -y @modelcontextprotocol/server-filesystem $HOME/projects
mcpal tool call fs read_file --path README.md
mcpal tool call fs search_files --pattern '*.toml'
```

Talk to an HTTP doc-search service (anonymous):

```bash
mcpal server add ctx7 --http https://mcp.context7.com/mcp
mcpal tool call ctx7 search --query 'Rust async runtimes'
```

Read OAuth-protected Notion:

```bash
mcpal server add notion --http https://mcp.notion.com/v1
mcpal auth login notion --oauth
mcpal --query '[].name' tool list notion
```

## How this book is organised

The chapters follow the [Diátaxis](https://diataxis.fr) framework: a
single tutorial, problem-driven how-tos, factual reference, and
explanation.

**Tutorial** — start here if you have never used mcpal.

- [Your first MCP call](./getting-started.md) — install, register the
  reference server, list and call tools.

**How-to guides** — find the recipe for the problem you have.

- [One-line MCP](./one-liners.md) — drive any server in a single
  shell command: `cmd:`, URL, JSON spec, discovered ref.
- [Recipes](./recipes.md) — short task-driven snippets, including a
  cookbook against real servers.
- [Authenticate to an HTTP server](./auth.md) — bearer or OAuth 2.1,
  with a step-by-step walk-through of the OAuth handshake.
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
- [Why a CLI for MCP](./why-cli.md) — where a shell client earns
  its place next to an MCP-aware chat app, and where it doesn't.

## License

MIT
