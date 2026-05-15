# Getting started

A 60-second tour. Replace `npx` with whatever spawn command works on your
machine.

## Install

```bash
cargo install --path crates/mcpal-cli
mcpal --version
```

Prebuilt binaries land at GitHub Releases once the first tag ships.

## 1. Talk to the reference server

```bash
mcpal server add ev \
  --stdio npx --arg -y --arg @modelcontextprotocol/server-everything
mcpal server test ev
```

Expected output:

```yaml
ref: ev
ok: true
server:
  name: mcp-servers/everything
  version: 2.0.0
peerInfo:
  protocolVersion: 2025-11-25
  …
```

## 2. List + describe + call tools

```bash
mcpal tool list ev
mcpal tool describe ev echo
mcpal tool call ev echo --message hi
```

The compact `tool list` returns just `{name, description, required}`.
For the full schema use `tool describe`.

```yaml
content:
- type: text
  text: 'Echo: hi'
```

## 3. Discover what other clients already have

```bash
mcpal discover
mcpal server list --all
```

If you've configured servers in Claude Code, Cursor, opencode, etc.,
they show up — usable by `<source>:<name>`:

```bash
mcpal tool list cursor:linear
```

## 4. Pipe results into jq

```bash
mcpal --output json tool list ev | jq -r '.[].name'
```

Or skip jq with `--query` (JMESPath):

```bash
mcpal --query '[].name' tool list ev
```

## 5. Auth

Bearer:

```bash
mcpal auth login github --bearer ghp_xxx
mcpal tool list github
```

OAuth 2.1:

```bash
mcpal server add notion --http https://mcp.notion.com/v1 --auth oauth
mcpal auth login notion --oauth
mcpal tool list notion
```

Token storage is the OS keyring, never the TOML config.

## 6. Sanity check

If anything looks odd:

```bash
mcpal doctor
```

If you hit an error, the rustc-style block tells you what to fix:

```
error[E0001]: server 'foo' not found …
help: run `mcpal discover` …

For more information about this error, try `mcpal explain E0001`.
```

That's the whole tour. Recipes for specific tasks live in
[Recipes](./recipes.md).
