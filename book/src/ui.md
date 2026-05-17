# UI-rich MCP servers

Some MCP servers don't just return text — they return **UI**. A
weather server may hand back an interactive chart. A booking
agent may serve a confirm-this-trip form. A dashboard tool may
embed a React component you can poke at right inside the host
application. This chapter is about the two ways MCP servers
ship that UI today, what those payloads look like on the wire,
and how `mcpal ui inspect` lets you debug them from a terminal
without spinning up a full MCP client.

## Why UI started showing up in MCP

The base MCP protocol was built for text. A tool returns
`content: [{ type: "text", text: "…" }]`, the client renders
it, the LLM reads it, end of story. That works for code
assistants and chatty agents. It falls apart the moment your
tool wants to do something a paragraph of Markdown can't do:

- Render a chart with the actual numbers, not their textual
  description.
- Let the user pick from a long, dynamic list (calendars,
  catalogs, dashboards) without forcing the LLM to enumerate
  options.
- Capture a structured action — a "buy this", "approve that",
  "set these dates" — that the LLM can then continue acting on.

To paper over that gap, two parallel UI standards appeared on
top of MCP:

- **mcp-ui** is an open standard. A tool result includes one or
  more *embedded resources* whose URI starts with `ui://`. The
  resource body is HTML (or an iframe pointer). The client
  renders the HTML in a webview and routes any user actions
  back to the server as fresh tool calls.

- **OpenAI Apps SDK** is OpenAI's flavour, used by apps that
  live inside ChatGPT. A tool result includes an embedded
  resource with MIME type `application/vnd.openai.app+json`.
  That JSON describes a component tree (their own runtime),
  which ChatGPT renders natively.

Both ride the standard MCP wire format — they just stuff their
payload inside a resource block. Neither is part of the MCP
spec proper. mcpal handles them the same way a curious user
would: detect the payload, classify it, and let you peel it
open.

## What a UI response looks like

Strip a tool call response of its envelope and you get a
`CallToolResult`:

```jsonc
{
  "content": [
    {
      "type": "text",
      "text": "Here is your weather:"
    },
    {
      "type": "resource",
      "resource": {
        "uri": "ui://weather/london",
        "mimeType": "text/html",
        "text": "<html>…interactive forecast…</html>"
      }
    }
  ],
  "isError": false
}
```

The `text` block is for the LLM ("describe this"). The
`resource` block is for the user's eyeballs. A naive MCP client
that doesn't speak mcp-ui prints the text, drops the resource
on the floor, and the rich UI is invisible to the user.

For OpenAI Apps the resource looks like:

```jsonc
{
  "type": "resource",
  "resource": {
    "uri": "openai://app/booking-confirm/3a91…",
    "mimeType": "application/vnd.openai.app+json",
    "text": "{\"component\":\"BookingConfirm\",\"props\":{…}}"
  }
}
```

Same shape, different MIME, different runtime needed to render.

## `mcpal ui inspect`: triage from a terminal

`mcpal ui inspect` calls a tool and tells you exactly what
came back, block by block:

```bash
mcpal ui inspect demo-server show_weather --params '{"city":"London"}'
```

Output (YAML by default; `--output json` for JSON):

```yaml
reference: demo-server
tool: show_weather
ui_resources: 1
is_error: false
hits:
  - index: 0
    kind: text
    size_bytes: 32
  - index: 1
    kind: mcp_ui
    uri: ui://weather/london
    mime_type: text/html
    size_bytes: 2814
```

`kind` is the headline classification:

| `kind` | Meaning |
|---|---|
| `text` | Plain `text/text` content block. |
| `image` / `audio` | Base64 content with a media MIME. |
| `mcp_ui` | Embedded resource whose URI starts with `ui://`. |
| `openai_app` | Embedded resource with `vnd.openai.app+json` MIME. |
| `resource` | Embedded resource that isn't UI (data attachment). |
| `resource_link` | Pointer to a resource the server doesn't inline. |

`ui_resources` is the count of `mcp_ui` + `openai_app` blocks.
That's the number you care about when asking *did this server
actually return any UI?*.

### Save the payload to disk

By default `inspect` only prints the summary. Pass `--save`
to dump UI/app payloads to `/tmp/mcpal-ui-<pid>-<index>.{html,json,js}`:

```bash
mcpal ui inspect demo-server show_weather --params '{"city":"London"}' --save
```

The summary lines now end with paths you can `cat`, `grep`,
diff, or pipe into a linter.

### Open in a browser

`--open` implies `--save` and additionally hands each file to
your OS opener (`open` on macOS, `xdg-open` on Linux,
`explorer` on Windows):

```bash
mcpal ui inspect demo-server show_weather --params '{"city":"London"}' --open
```

A mcp-ui HTML payload will load straight into a browser as a
standalone file — most demos work this way out of the box. An
OpenAI Apps JSON payload won't render directly: it needs
OpenAI's runtime. You get the descriptor on disk so you can
read it, validate it, or hand it to whatever harness you're
building.

## The TUI badge

`mcpal tui` paints a magenta `UI ✦` next to the tool name in
the Detail pane whenever a call result carries an mcp-ui or
OpenAI Apps block. No keystroke needed — the classifier runs
on every result. To save the payload from a TUI session, drop
back to the CLI:

```bash
mcpal ui inspect <ref> <tool> --params '<the args>' --save
```

A future release may bind a key to that directly. For now the
badge is the cue; the saving is one shell away.

## When this is useful

- **Building an mcp-ui server.** You wrote a tool that returns
  HTML in a `ui://` resource and your client doesn't render it.
  `mcpal ui inspect --save --open` proves whether the payload
  is well-formed and whether the HTML stands up on its own,
  before you start blaming the client.

- **Validating an OpenAI App.** Apps SDK components are opaque
  JSON descriptors. `mcpal ui inspect --save` lets you diff
  what your tool returned today against what it returned
  yesterday — easy regression check without booting ChatGPT.

- **Security review.** UI resources are arbitrary HTML or JS
  served by an MCP server you may not fully trust. `mcpal ui
  inspect --save` writes the payload to a regular file you can
  feed to whatever scanner or linter you'd run on third-party
  code: grep for `script src=`, run an HTML validator, check
  for inline event handlers, whatever your threat model
  demands.

- **Debugging a client that should render UI but doesn't.**
  If `mcpal ui inspect` reports `ui_resources: 0`, the server
  isn't sending UI — talk to the server team. If it reports
  `ui_resources: 1` and your client still shows nothing, the
  bug lives in the client.

- **Capturing fixtures.** Once `--save` produces a file, you
  have a golden HTML/JSON artifact to check into a test
  suite. Replay against a stub, snapshot-test the renderer,
  done.

## What `mcpal ui inspect` does *not* do

- It does not render the UI itself. mcpal is a terminal tool;
  it tells you what's there and hands the file to your real
  browser or a downstream harness.

- It does not relay user interactions back to the server. A
  full mcp-ui experience includes `postMessage` round-trips
  from the rendered iframe back to the MCP server, which then
  may issue new tool calls. Implementing that bridge would
  turn mcpal into a webview host. Out of scope.

- It does not validate against either standard's spec. The
  classifier is pattern-match on URI prefix + MIME, nothing
  deeper. Adding a `--strict` validator is on the list.

## See also

- mcp-ui spec and reference servers: <https://github.com/idosal/mcp-ui>
- OpenAI Apps SDK: <https://platform.openai.com/docs/guides/apps-sdk>
- [Recipes → Real-server cookbook](./recipes.md) for non-UI tool calls
- [Authenticate to an HTTP server](./auth.md) for the bearer / OAuth
  flow you'll need before calling most production UI servers
