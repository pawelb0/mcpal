# Your first MCP call

A guided walk-through. By the end you will have mcpal installed,
a reference MCP server registered, and a tool call returning a real
response.

Time: about five minutes once `npx` is cached.

You will need: a shell, `cargo`, and `npx` (Node.js 18+).

## 1. Install

```bash
cargo install --path crates/mcpal
mcpal --version
```

Output:

```
mcpal 0.1.1
```

Prebuilt binaries: GitHub Releases (planned).

## 2. Register a stdio server

```bash
mcpal server add ev -- npx -y @modelcontextprotocol/server-everything
```

Output:

```
added server 'ev'
```

Tokens after `--` form the spawned command. `ev` is the local alias.

## 3. Verify it speaks MCP

```bash
mcpal server ping ev
```

Output:

```yaml
ok: true
ref: ev
```

## 4. List the server's tools

```bash
mcpal tool list ev
```

The reference server exposes about a dozen tools — `echo`,
`get-sum`, `trigger-long-running-operation`, and so on.

## 5. Call one

```bash
mcpal tool call ev echo --message hi
```

Output:

```yaml
content:
- type: text
  text: 'Echo: hi'
```

That round-trip is a real MCP `tools/call` request and response.

## 6. Filter the response

```bash
mcpal --query 'content[0].text' tool call ev echo --message hi
```

Output:

```
'Echo: hi'
```

`--query` runs JMESPath on the response before printing.

## Next

- [Recipes](./recipes.md) — copy-paste snippets per task.
- [Authenticate to an HTTP server](./auth.md) — bearer or OAuth 2.1.
- [Concepts](./concepts.md) — how `<ref>` resolves, transports,
  output formats.
