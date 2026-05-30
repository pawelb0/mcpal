# One-line MCP

You can drive an MCP server in a single shell command. No `server add`,
no config file edits, no auth flow up front. Pick a `<ref>` shape that
matches what you have:

| You have | One-line `<ref>` | Example |
|---|---|---|
| A local stdio command (npx, uvx, docker, anything) | `cmd:<command> [args]` | `mcpal tool call "cmd:npx -y @modelcontextprotocol/server-everything" echo --message hi` |
| An HTTP(S) URL | the URL plus `--auth MODE` | `mcpal --auth env:GH_TOKEN tool list https://api.githubcopilot.com/mcp/` |
| A `ServerSpec` JSON file on disk | the path | `mcpal tool call ./spec.json read_file --path README.md` |
| A server one of your editors already configured | `<source>:<name>` | `mcpal tool call cursor:linear get-issue --id ENG-123` |
| A bare name that's unambiguous across discovered sources | the name | `mcpal tool list linear` |
| A registry server | install first, then call | `mcpal server install io.github.foo/bar && mcpal tool list bar` |

The order above is the resolution order. `mcpal debug explain E0001`
prints the same precedence in long form.

## `cmd:` — ephemeral stdio

`cmd:<command> [args]` spawns the named program over stdio for the
duration of the call. The spec is never written to disk. Tokens after
`cmd:` are split on whitespace.

```bash
# everything server
mcpal tool list "cmd:npx -y @modelcontextprotocol/server-everything"
mcpal tool call "cmd:npx -y @modelcontextprotocol/server-everything" \
    echo --message hi

# filesystem sandbox at $HOME/projects
mcpal tool call "cmd:npx -y @modelcontextprotocol/server-filesystem $HOME/projects" \
    read_file --path README.md

# uv-managed Python server
mcpal --query 'content[0].text' \
    tool call "cmd:uvx awslabs.aws-api-mcp-server@latest" describe_regions

# docker
mcpal tool list "cmd:docker run --rm -i ghcr.io/example/mcp"
```

Quote the whole `cmd:…` string so your shell groups it as one argument.
Values that need their own spaces, glob characters, or shell escapes
won't survive whitespace-splitting — for anything that fancy, use
`mcpal server add` and persist the spec.

`cmd:` carries no environment variables. Pass them via your shell
(`API_KEY=… mcpal …`) or persist with `mcpal server add … --env K=V`.

## `https://…` — ephemeral HTTP

A literal URL resolves to an HTTP `ServerSpec`. By default the spec
carries `auth = oauth`. The first call without a stored token will
print a warning telling you to run `mcpal auth login --oauth <url>`.

```bash
mcpal tool list https://mcp.context7.com/mcp
mcpal --output json tool call https://mcp.context7.com/mcp \
    search --query 'Rust async runtimes'
```

### `--auth MODE` — pick the auth flavour inline

Override the default with the global `--auth` flag. Modes:

| `--auth` | Behaviour |
|---|---|
| `oauth` (default) | Send the stored OAuth access token |
| `none` / `anon` | Send no `Authorization` header |
| `env:VAR` | Read the bearer token from `$VAR` at call time |
| `bearer:TOKEN` | Send the literal token (leaks to shell history) |

```bash
# anonymous HTTP MCP — no warning about a missing OAuth token
mcpal --auth none tool list https://mcp.context7.com/mcp

# token comes from an env var, never touches the spec file or history
GH_TOKEN=ghp_… mcpal --auth env:GH_TOKEN \
    tool list https://api.githubcopilot.com/mcp/

# literal token (avoid — lives in shell history)
mcpal --auth bearer:ghp_… tool list https://api.githubcopilot.com/mcp/
```

For repeated use, persist with `mcpal server add … --bearer-env GH_TOKEN`
so future calls don't need the `--auth` flag at all.

## `./spec.json` — ephemeral file

A path to a JSON `ServerSpec` resolves inline. Useful when a teammate
hands you a saved spec or when you generate one in CI:

```bash
cat > /tmp/ev.json <<'EOF'
{ "transport": "stdio",
  "command": "npx",
  "args": ["-y", "@modelcontextprotocol/server-everything"] }
EOF
mcpal tool list /tmp/ev.json
```

## `<source>:<name>` and bare names — already-configured

`mcpal server discover` (or `mcpal server list --all`) prints the
`<source>:<name>` form of every server your editors already know about.
Either form is a valid `<ref>`:

```bash
mcpal server discover
mcpal tool list cursor:linear
mcpal tool list linear           # if only one source has 'linear'
```

## Registry — one extra line

The MCP Registry returns servers that often need env vars, so a true
one-liner isn't always safe. The minimum is two:

```bash
mcpal server install io.github.modelcontextprotocol/server-everything --no-prompt
mcpal tool list server-everything
```

If you pre-supply env vars with `--env K=V`, you can install
non-interactively:

```bash
mcpal server install io.github.modelcontextprotocol/server-filesystem \
    --env FS_ROOT=$HOME/projects --no-prompt
```

For full registry behaviour see [Recipes](./recipes.md).

## What doesn't work in one line

- stdio with arguments that contain whitespace or shell metacharacters
  — `cmd:` is a whitespace split. Persist the spec with `server add`.
- Registry servers that declare required env vars and you didn't pass
  `--env` — `mcpal server install` exits with `E0017`.

Static bearer tokens via `--auth bearer:TOKEN` *do* work in one line
but are noisy: the token lands in shell history and process listings.
`--auth env:VAR` is the safer one-line form.

## Quick reference

```bash
# stdio ephemeral
mcpal tool list "cmd:npx -y @modelcontextprotocol/server-everything"

# HTTP — pick auth inline
mcpal --auth none      tool list https://mcp.context7.com/mcp
mcpal --auth env:TOKEN tool list https://api.githubcopilot.com/mcp/
mcpal --auth oauth     tool list https://mcp.notion.com/v1   # default

# spec file
mcpal tool list /tmp/ev.json

# already configured
mcpal tool list cursor:linear
mcpal tool list linear        # if unambiguous
```
