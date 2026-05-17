# Interactive TUI

`mcpal tui` opens a three-pane curses-style browser over every MCP
server that mcpal can see (owned + discovered). Useful when you do
not remember a tool's exact name or schema and want to call it
without composing a shell command first.

```text
┌─ Servers ──────────────┬─ Detail ───────────────────────────┐
│ > opencode:linear   ⚡ │ Tools (12)  Resources (3)  Prompts │
│   cursor:notion     🔒 │   list_issues                       │
│   ev                ●  │   create_comment                    │
│   mcp.context7.com  ⚡ │   add_assignee                      │
│   fs (stdio)        ●  │   …                                 │
├─ Output ───────────────┴─────────────────────────────────────┤
│ $ connect opencode:linear                                    │
│ ✓ connected to opencode:linear (12 tools)                    │
│ $ tool call opencode:linear list_issues                      │
│ ✓ opencode:linear list_issues                                │
└ Tab cycle · Enter open · / filter · ? help · q quit ─────────┘
```

## Layout

The three panes are **Sidebar** (servers), **Detail** (tools /
resources / prompts of the selected server, or a tool schema /
result), and **Output** (a 200-line ring of command echoes plus
live notifications from connected servers).

Icons in the sidebar tag the transport:

- `●` stdio
- `⚡` HTTP, no auth required
- `🔒` HTTP, OAuth required

## Key map

| Key | Action |
| --- | --- |
| `Tab` / `Shift+Tab` | cycle pane focus |
| `j`/`k`, `↓`/`↑` | move selection |
| `gg` / `G` | jump to top / bottom |
| `Enter` | drill in (sidebar → detail tabs → schema / form) |
| `Esc` | drill back, or close a modal |
| `/` | filter the sidebar; `Esc` clears, `Enter` accepts |
| `c` | call the selected tool (Detail focus, Tools tab) |
| `l` / `Right` | next tab in Detail (Tools → Resources → Prompts) |
| `b` | open a bearer-token input for the selected server |
| `?` | toggle help overlay |
| `q`, `Ctrl-C` | quit |

## Calling a tool

`c` on a tool opens a form modal whose fields come from the tool's
`inputSchema`. Each field is labelled with its type (`str`, `int`,
`num`, `bool`, `json`) and an `*` when required. `Tab` cycles
fields. `Enter` submits. The terminal renders the response inline
in the Detail pane; the Output pane gets a one-line echo with a
`✓` or `✗`.

If the schema declares `object` or `array`, the field stores a raw
JSON literal — paste it directly.

## Notifications

mcpal forwards every notification it sees from a connected server
into the Output pane:

- `progress` becomes `→ progress N/M`.
- `log` becomes `→ log <level>: <message>`.
- list-changed / resource-updated become a generic `→ <kind>` line.

The buffer is bounded at 200 lines.

## Building without the TUI

The TUI is gated behind the `tui` feature, which is on by default.
To get a smaller `mcpal` binary without `ratatui`, `crossterm`,
`tui-input`, and `tui-textarea`:

```bash
cargo install mcpal --no-default-features
```

`mcpal tui` then prints "unrecognized command" and exits 2.

## What's not in v1

- In-TUI OAuth flow. If a server returns 401, drop out and run
  `mcpal auth login <ref> --oauth` in another shell.
- `:` command bar.
- Persistent layout / theme overrides.

File issues at <https://github.com/pawelb0/mcpal/issues>.
