# Why a CLI for MCP

MCP servers were designed to be driven from inside an LLM client.
The chat app holds the connection, lists every tool the server
exposes, and lets the model call them. That model works well inside
a conversation. It is awkward everywhere else.

This page explains where a shell-level client like mcpal earns its
place, and where it does not.

## Tool definitions cost tokens you may not need

When a chat client connects to an MCP server, it loads every tool's
name, description, and JSON schema into the model's context. A server
with forty tools costs context whether the model uses two of them or
all of them. Connect four such servers and a non-trivial slice of the
context window is gone before the first user message.

mcpal calls into the same servers without paying that price. The
shell types one line:

```bash
mcpal tool call cursor:linear get-issue --id ENG-123
```

When an agent script needs to discover what's available, it asks for
exactly what it needs:

```bash
mcpal tool list cursor:linear --names-only       # names only
mcpal tool describe cursor:linear get-issue      # one schema
mcpal tool template cursor:linear get-issue      # a known-good skeleton
```

No upfront catalogue. Cost scales with what the script invokes, not
with what the server advertises.

## The shell already knows how to compose things

MCP responses are JSON-RPC frames inside a chat-only protocol. Once
the chat tab closes, the call is gone. mcpal returns the same data
to standard output, with stable exit codes:

```bash
mcpal --output json tool list cursor:linear |
    jq -r '.[] | select(.name | startswith("get_")) | .name'

mcpal --query 'content[0].text' tool call ev echo --message hi
```

That's pipes, redirection, `jq`, `xargs`, cron, CI runners, exit-code
branching. None of it needs the model in the loop.

Reproducing a failure is `mcpal server ping <ref>` followed by the
exact command that broke, pasted into an issue with a stack trace
instead of a screenshot.

## One authentication, shared across invocations

`mcpal auth login` runs the OAuth 2.1 + PKCE flow (or stores a
bearer token) once and writes credentials to the OS keyring. Every
subsequent `mcpal tool call`, every CI job that exports the same
profile, and every script in `mcpal.yml` reads from the same place.
There is no per-call browser dance, no token pasted into shell
history.

## Servers your editor already configured

Most teams already have a working `mcp.json` somewhere. Claude
Desktop, Cursor, Zed, opencode, VS Code, Codex CLI — each writes
config to disk. `mcpal server discover` reads those files and makes
the servers addressable:

```bash
mcpal server discover
mcpal tool list cursor:linear
mcpal server list --all                          # owned + discovered
```

No duplication. The Linear server your editor configured is the
same Linear server your shell can now drive.

## When mcpal is the wrong answer

A CLI in front of MCP is not always the right move:

- **Interactive use from inside the chat app.** If the workflow is
  "ask the model, watch it call the tool," the chat client already
  owns that path. mcpal adds nothing.
- **Real-time bidirectional flows.** Sampling, elicitation, and
  long-lived subscriptions live more naturally inside a connected
  client. mcpal exposes them (`mcpal watch`, `--sampling-handler`)
  but they are not its strongest surface.
- **End users without a terminal.** A non-developer running a SaaS
  integration is not the audience.

## Limitations

mcpal does not remove every MCP cost the published critiques point
at:

- Each `mcpal tool call` spawns the server fresh over stdio.
  Initialization (the `initialize` handshake, the `tools/list`
  exchange a server may do on connect) runs every call. A
  long-running `mcpal serve` daemon that holds warm sessions is on
  the roadmap.
- The tool catalogue is not cached on disk. `tool list` round-trips
  to the server. A local cache with a TTL is plausible future work.

Known gaps. The shell surface is what exists today; the project sits
alongside MCP-aware chat clients, not in their place.
