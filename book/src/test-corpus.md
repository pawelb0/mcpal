# Test corpus

A curated list of MCP servers to sanity-check mcpal against on every
release. Each row stresses a different edge of the protocol or the
mcpal surface.

## stdio + required env

### `io.github.codeurali/dataverse` — Microsoft Dataverse

```bash
mcpal server install io.github.codeurali/dataverse
# Prompts for DATAVERSE_ENV_URL on TTY; bails with E0017 otherwise.
mcpal tool list dataverse
```

Stresses: env-var prompt path (v0.4.1); registry semver-max (v0.4.0).

### `awslabs.aws-api-mcp-server` — AWS API via uvx

```bash
mcpal server add aws-api \
  --env AWS_PROFILE=default --env AWS_REGION=us-east-1 \
  -- uvx awslabs.aws-api-mcp-server@latest
mcpal tool list aws-api
```

Stresses: long cold start (~30s); uvx; `--env` propagation.

### `@modelcontextprotocol/server-postgres`

```bash
mcpal server add pg \
  --env DATABASE_URL=postgres://localhost/test \
  -- npx -y @modelcontextprotocol/server-postgres
```

Stresses: `DATABASE_URL`; SQL injection in tool args; resource subscriptions.

## Broken on init

### `mcp-dataverse@0.1.0`

```bash
mcpal server add bd --force -- npx -y mcp-dataverse@0.1.0
mcpal tool list bd
# error[E0006]: ... (child stderr: ENOENT package.json)
```

Stresses: child stderr surfacing (v0.4.0). Without the v0.4.0 fix the
failure is opaque.

## HTTP + OAuth (PKCE + DCR)

### Notion

```bash
mcpal server add notion --http https://mcp.notion.com/v1 --oauth
mcpal tool list notion
mcpal auth refresh notion
```

Stresses: browser handshake; refresh-token storage; loopback listener.

## HTTP + static bearer

### GitHub Copilot MCP

```bash
mcpal server add gh \
  --http https://api.githubcopilot.com/mcp/ \
  --bearer "$GH_TOKEN"
mcpal tool list gh
```

Stresses: `--bearer` keyring write; promote-from-import.

## Pagination + notifications + resources

### `@modelcontextprotocol/server-everything`

```bash
mcpal server add ev -- npx -y @modelcontextprotocol/server-everything
mcpal tool list ev | wc -l           # 100+ tools
mcpal watch ev                       # streams progress + log + list-changed
mcpal resource subscribe ev demo://resource/dynamic/0
mcpal tool call ev sample --message hi    # exercises sampling
mcpal tool call ev eliciting --message x  # exercises elicitation
```

Stresses: pagination; notification stream; resource subscribe; sampling /
elicitation handlers.

## mcp-ui / OpenAI Apps payloads

(Pending: a stable demo server. For now use the unit tests in
`crates/mcpal/src/commands/ui.rs` and add fixture servers when they
become available.)

## Multi-source same-name

`chrome-devtools` is typically registered in both `opencode` and
`claude-code` configs. Verify:

```bash
mcpal server discover --source opencode | grep chrome-devtools
mcpal server discover --source claude-code | grep chrome-devtools
mcpal tool list opencode:chrome-devtools
mcpal tool list claude-code:chrome-devtools
mcpal tool list chrome-devtools       # ambiguous — fails with hint
```

Stresses: bare-name disambiguation.

## fastmcp banner

(Pending: a local FastMCP demo. Stresses: controlling-terminal detach
via setsid; TUI alt-screen integrity.)

## Known gaps (currently UNTESTED)

- HTTP servers behind a private CA / self-signed cert.
- Windows Store install of Claude Desktop (`%LOCALAPPDATA%\Packages\...`).
- Servers that emit JSON on stdout outside the MCP framing
  (protocol violation; mcpal's behaviour is undefined).

Every release ritual runs at least the stdio + HTTP + everything-server
smoke before tagging.
