# Getting started

Replace `npx` with whatever command spawns servers on your machine.

## Install

```bash
cargo install --path crates/mcpal-cli
mcpal --version
```

Prebuilt binaries will be published to GitHub Releases on the first
tagged release.

## Talk to the reference server

```bash
mcpal server add ev \
  --stdio npx --arg -y --arg @modelcontextprotocol/server-everything
mcpal server test ev
```

```yaml
ref: ev
ok: true
server:
  name: mcp-servers/everything
  version: 2.0.0
peerInfo:
  protocolVersion: …
```

## List, describe, call

```bash
mcpal tool list ev
mcpal tool describe ev echo
mcpal tool call ev echo --message hi
```

`tool list` returns `{name, description, required}`. Use `tool describe`
for the full schema.

```yaml
content:
- type: text
  text: 'Echo: hi'
```

## Discover

```bash
mcpal discover
mcpal server list --all
```

If servers are configured in Claude Code, Cursor, opencode, or any of
the other supported clients, they show up. Call them with
`<source>:<name>`:

```bash
mcpal tool list cursor:linear
```

## Pipe through jq

```bash
mcpal --output json tool list ev | jq -r '.[].name'
```

Or skip `jq` with `--query`:

```bash
mcpal --query '[].name' tool list ev
```

## Auth

Bearer:

```bash
mcpal auth login github --bearer ghp_xxx
mcpal tool list github
```

OAuth 2.1:

```bash
mcpal server add notion --http https://mcp.notion.com/v1
mcpal auth login notion --oauth
mcpal tool list notion
```

Tokens go to the OS keyring, not the TOML config.

## Health check

```bash
mcpal doctor
```

Errors render in rustc style:

```
error[E0001]: server 'foo' not found …
help: run `mcpal discover` …

For more information about this error, try `mcpal explain E0001`.
```

Recipes are in [Recipes](./recipes.md).
