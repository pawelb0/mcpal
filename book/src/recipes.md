# Recipes

Replace `<ref>` with any server reference (see
[Concepts](./concepts.md#server-reference-ref)).

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
mcpal discover
```

## Use an `mcp.json` without registering it

```bash
mcpal --mcp-json ./mcp.json tool list <name>
```

Global flag. Overlays servers for the session; nothing is written to
disk.

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

Full code table in [Scripting & exit codes](./scripting.md).

## Disable interactive prompts (CI)

```bash
mcpal --no-interactive tool call <ref> …
```

Elicitation requests auto-decline. Bearer tokens come from
`MCPAL_BEARER` or `--bearer`, never a TTY prompt.

## Plug an LLM into sampling/createMessage

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

## Diff two servers' tool lists

```bash
diff \
  <(mcpal --output json tool list <ref-a> | jq -S) \
  <(mcpal --output json tool list <ref-b> | jq -S)
```

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
