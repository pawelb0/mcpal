# Recipes

Copy-paste cookbook. Replace `<ref>` with any server reference (see
[Concepts](./concepts.md#1-server-reference-ref)).

## I just installed mcpal. How do I see what's already on my machine?

```bash
mcpal discover
```

## I have an `mcp.json` from Claude Desktop / Cursor. How do I use it without re-registering?

```bash
mcpal --mcp-json ./mcp.json tool list <name>
```

The flag is global; it overlays servers into the session config without
writing anything to disk.

## I want to call a tool with complex JSON arguments

Use `--cli-input-json`:

```bash
echo '{"message":"hi","count":3}' \
  | mcpal tool call ev some-tool --cli-input-json -
```

Or read from a file:

```bash
mcpal tool call ev some-tool --cli-input-json @args.json
```

Or use `mcpal tool template` to get a starting skeleton:

```bash
mcpal --output json tool template ev some-tool \
  | jq '.field = "value"' \
  | mcpal tool call ev some-tool --cli-input-json -
```

## I need just one field out of a tool response

```bash
mcpal --query 'content[0].text' tool call ev echo --message hi
```

`--query` is JMESPath. See the [tutorial](https://jmespath.org/tutorial.html).

## I want to watch a long-running tool's progress

```bash
# terminal 1
mcpal watch ev

# terminal 2
mcpal tool call ev trigger-long-running-operation --duration 10 --steps 5
```

The `watch` terminal prints one YAML doc per server notification
(progress, log, resource-updated, list-changed) until you Ctrl-C.

## I need to send a JSON-RPC method mcpal doesn't have a verb for

```bash
mcpal raw <ref> some/new-method --params '{"foo":"bar"}'
mcpal raw <ref> some/method --params @payload.json
mcpal raw <ref> some/method --params -    # read stdin
```

## I want to invoke a tool 50 times across different inputs

```bash
for q in rust go python; do
  mcpal tool call github search --q "$q stars:>1000" --per_page 3
done
```

For parallelism:

```bash
seq 1 50 | xargs -P 8 -I {} \
  mcpal tool call worker process --batch-id {}
```

## I want to script error handling

Check the exit code:

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

## I'm using mcpal in CI; I don't want interactive prompts

```bash
mcpal --no-interactive tool call <ref> …
```

Elicitation requests from the server auto-decline. Bearer tokens come
from `MCPAL_BEARER` or `--bearer`, never a TTY prompt.

## My tool wants to call an LLM (sampling)

Plug in any LLM CLI via `--sampling-handler`:

```bash
mcpal --sampling-handler "claude --output json" \
  tool call <ref> trigger-sampling-request --prompt "summarize"
```

mcpal pipes the `CreateMessageRequestParams` JSON to your handler's
stdin and reads a `CreateMessageResult` JSON from its stdout.

## I want to expose two workspace roots to the server

```bash
mcpal --root ~/src/my-project --root /tmp \
  tool call <ref> get-roots-list
```

## I want to read a resource

```bash
mcpal resource list <ref>
mcpal resource read <ref> demo://resource/static/document/architecture.md
mcpal resource template list <ref>
mcpal resource subscribe <ref> some://uri
```

## I want to fetch a prompt with arguments

```bash
mcpal prompt list <ref>
mcpal prompt get <ref> some-prompt --city Dallas --state Texas
```

## I want to compare two servers

```bash
diff \
  <(mcpal --output json tool list <ref-a> | jq -S) \
  <(mcpal --output json tool list <ref-b> | jq -S)
```

A built-in `mcpal diff` may land in a future release.

## I want shell completions

```bash
mcpal completion zsh > ~/.zfunc/_mcpal
# bash: bash
# fish: fish
```
