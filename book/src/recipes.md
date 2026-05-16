# Recipes

Short, problem-driven snippets. Each section answers one question.
Replace `<ref>` with any server reference — see
[Concepts → Server reference](./concepts.md#server-reference-ref).

## Real-server cookbook

Concrete examples against publicly known MCP servers. Tokens go to
the OS keyring (`mcpal auth login`); none of the commands write
secrets to disk.

### Filesystem (local sandbox, stdio)

A scoped read/write surface over a directory tree. Useful for any
command that wants file access without giving the LLM the whole
machine.

```bash
mcpal server add fs -- \
  npx -y @modelcontextprotocol/server-filesystem $HOME/projects

mcpal tool list fs --names-only
mcpal tool call fs read_file --path README.md
mcpal tool call fs list_directory --path .
mcpal tool call fs search_files --pattern '*.toml' --path .
mcpal --query 'content[0].text' tool call fs read_file --path Cargo.toml
```

The sandbox is the path passed at spawn time; you can pass multiple
paths after the package name.

### Fetch (HTTP client, stdio)

A bounded HTTP fetcher for one-off requests:

```bash
mcpal server add fetch -- npx -y @modelcontextprotocol/server-fetch
mcpal tool call fetch fetch --url https://httpbin.org/json
```

### Time

```bash
mcpal server add time -- npx -y @modelcontextprotocol/server-time
mcpal tool call time get_current_time --timezone Europe/Warsaw
```

### GitHub (HTTP, bearer)

The hosted GitHub MCP at `api.githubcopilot.com/mcp/` accepts a
personal access token (classic or fine-grained) as a bearer:

```bash
mcpal server add gh --http https://api.githubcopilot.com/mcp/
mcpal auth login gh --bearer ghp_xxx          # or use $GITHUB_TOKEN
mcpal tool list gh --names-only | head

mcpal --query '[].name' \
  tool call gh list_repositories_for_user --username anthropics

mcpal --output json \
  tool call gh search_issues --q 'repo:anthropics/claude-code is:open label:bug' \
  | jq '.[].title'
```

For CI, put the token in an env var and reference it from `config.toml`:

```toml
[server.gh]
transport = "http"
url = "https://api.githubcopilot.com/mcp/"
auth = { type = "bearer_env", env = "GITHUB_TOKEN" }
```

### Linear (HTTP, OAuth)

Linear's MCP authenticates users via OAuth 2.1. mcpal runs the full
PKCE + DCR flow for you; no developer-console step:

```bash
mcpal server add linear --http https://mcp.linear.app/mcp
mcpal auth login linear --oauth        # browser → consent → done
mcpal --query '[].name' tool list linear

# Find issues assigned to me, in progress:
mcpal --query 'content[0].text' \
  tool call linear list_my_issues --state in_progress

# Comment on one:
mcpal tool call linear create_comment \
  --issueId ENG-123 --body 'updated branch is up'
```

`mcpal auth refresh linear` rotates the access token when it expires;
mcpal also refreshes eagerly within 30 s of expiry. See the
[OAuth walk-through](./auth.md#oauth-21--pkce--dcr) for what each
step does on the wire.

### Notion (HTTP, OAuth)

```bash
mcpal server add notion --http https://mcp.notion.com/v1
mcpal auth login notion --oauth
mcpal tool list notion --names-only

mcpal --query 'content[0].text' \
  tool call notion search --query 'meeting notes 2026 Q2'

mcpal tool call notion append_block \
  --pageId 8a7…b21 \
  --blocks '[{"type":"paragraph","text":"shipped v0.1"}]'
```

### Context7 (HTTP, anonymous)

A free hosted docs-search MCP — no auth, useful as a sanity check:

```bash
mcpal server add ctx7 --http https://mcp.context7.com/mcp
mcpal tool call ctx7 search --query 'rmcp ServiceExt'
```

### Postgres (stdio, env-injected creds)

```bash
mcpal server add db -- \
  npx -y @modelcontextprotocol/server-postgres \
  postgres://user:pass@localhost:5432/app

mcpal tool list db
mcpal tool call db query --sql 'select count(*) from users'
```

Put the connection string in an env var and `--env` it through:

```bash
mcpal server add db \
  --env DATABASE_URL="$DATABASE_URL" \
  -- npx -y @modelcontextprotocol/server-postgres '$DATABASE_URL'
```

### opencode's already-configured servers

If you run opencode, every server in `~/.config/opencode/opencode.json`
is callable directly:

```bash
mcpal server discover --source opencode
mcpal tool list opencode:tavily
mcpal tool call opencode:tavily search --query 'Rust async runtimes'
```

Same pattern for `cursor:`, `claude-code:`, `zed:`, etc. See
[Concepts → Discovery](./concepts.md#discovery) for the source list.

## Install a server from the MCP Registry

```bash
mcpal server search filesystem --limit 5
mcpal server install io.github.Oncorporation/filesystem-server --as fs
mcpal server install io.github.foo/bar --env API_KEY=$KEY
mcpal tool list fs
```

`server install` resolves the registry entry's package
(npm → `npx`, pypi → `uvx`, oci → `docker run`) or its
streamable-http remote into a `ServerSpec` and writes it to your config.
Required env vars without defaults must be supplied via `--env K=V`.

## List servers already on the machine

```bash
mcpal server discover
```

## Use an `mcp.json` without registering it

```bash
mcpal --mcp-json ./mcp.json tool list <name>
```

Global flag; overlays for the session and writes nothing to disk.

## Call a tool with a JSON arg body

```bash
mcpal tool call ev some-tool --params '{"message":"hi","count":3}'
```

From stdin or file:

```bash
echo '{"message":"hi"}' | mcpal tool call ev some-tool --params -
mcpal tool call ev some-tool --params @args.json
```

`--cli-input-json` is the AWS-CLI-compatible alias. Mix `--params` with
`--key value` overrides; flag values win:

```bash
mcpal tool call ev some-tool --params @base.json --message override
```

Or generate a skeleton:

```bash
mcpal --output json tool template ev some-tool \
  | jq '.field = "value"' \
  | mcpal tool call ev some-tool --cli-input-json -
```

## Extract one field from a response

```bash
mcpal --query 'content[0].text' tool call ev echo --message hi
```

`--query` is JMESPath. See the [tutorial](https://jmespath.org/tutorial.html).

## Watch a long-running tool

```bash
# terminal 1
mcpal watch ev

# terminal 2
mcpal tool call ev trigger-long-running-operation --duration 10 --steps 5
```

One YAML doc per notification (progress, log, resource-updated,
list-changed). Ctrl-C to exit.

## Send a raw JSON-RPC method

```bash
mcpal raw <ref> some/new-method --params '{"foo":"bar"}'
mcpal raw <ref> some/method --params @payload.json
mcpal raw <ref> some/method --params -
```

## Loop a tool over many inputs

```bash
for q in rust go python; do
  mcpal tool call github search --q "$q stars:>1000" --per_page 3
done
```

```bash
seq 1 50 | xargs -P 8 -I {} \
  mcpal tool call worker process --batch-id {}
```

## Branch on exit code

```bash
mcpal tool call ev echo --message hi
case $? in
  0) echo "ok" ;;
  3) echo "ref not found"; exit 1 ;;
  4) mcpal auth login ev --oauth ;;
  5) mcpal auth refresh ev ;;
  *) echo "see above"; exit $? ;;
esac
```

## Disable interactive prompts (CI)

```bash
mcpal --no-interactive tool call <ref> …
```

Elicitation requests auto-decline. Bearer tokens come from
`MCPAL_BEARER` or `--bearer`, never a TTY prompt.

## External sampling handler

```bash
mcpal --sampling-handler "claude --output json" \
  tool call <ref> trigger-sampling-request --prompt "summarize"
```

mcpal pipes `CreateMessageRequestParams` JSON to the handler's stdin and
reads `CreateMessageResult` JSON from its stdout.

## Expose workspace roots

```bash
mcpal --root ~/src/my-project --root /tmp \
  tool call <ref> get-roots-list
```

## Resources

```bash
mcpal resource list <ref>
mcpal resource read <ref> demo://resource/static/document/architecture.md
mcpal resource template list <ref>
mcpal resource subscribe <ref> some://uri
```

## Prompts

```bash
mcpal prompt list <ref>
mcpal prompt get <ref> some-prompt --city Dallas --state Texas
```

## Diff two servers' capabilities

```bash
mcpal diff <ref-a> <ref-b>
mcpal diff <ref-a> <ref-b> --only tools
```

Reports `added`, `removed`, and `changed` entries per category
(`tools`, `resources`, `prompts`). A tool counts as `changed` when its
`inputSchema` differs between the two servers.

## Shell completions

```bash
mcpal completion zsh > ~/.zfunc/_mcpal
```

`bash` and `fish` work the same way.

### Completing tool / resource / prompt names

`tool list`, `resource list`, and `prompt list` accept `--names-only`,
which prints one name (or URI) per line on stdout. Wire it into your
shell's completion. For zsh, with the cursor after `mcpal tool call ev `:

```zsh
_mcpal_tools() {
  # $words: (mcpal tool call <ref> …); the ref is words[-2] from CURRENT.
  local ref=${words[-2]}
  compadd -- $(mcpal tool list "$ref" --names-only 2>/dev/null)
}
compdef _mcpal_tools 'mcpal tool call'
```

Bash equivalent (inside your `complete -F` function, with `$prev` already
set to the ref token):

```bash
COMPREPLY=( $(compgen -W "$(mcpal tool list "$prev" --names-only 2>/dev/null)" -- "$cur") )
```

stdio servers may leak their own stderr (`Starting default (STDIO)
server...` and similar) during completion. The `2>/dev/null` above
suppresses it. Setting `MCPAL_CHILD_STDERR=inherit` un-silences it
again.
