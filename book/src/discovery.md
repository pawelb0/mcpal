# Discovery

mcpal can pull MCP server definitions from other clients you already
have installed. Run `mcpal server discover` to scan, or `mcpal server
list` (default) to see your registered + discovered entries side by side.

## Supported clients

| Source | Files |
|---|---|
| `claude-code` | `~/.claude.json` |
| `claude-desktop` | `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) / `%APPDATA%\Claude\claude_desktop_config.json` (Windows) |
| `cursor` | `~/.cursor/mcp.json`, project `.cursor/mcp.json` |
| `opencode` | `~/.config/opencode/opencode.json` |
| `vscode` | `<Code config>/User/mcp.json`, project `.vscode/mcp.json` |
| `vscode-user` | `<Code config>/User/settings.json` (`chat.mcp.servers` key) |
| `continue` | `<Code config>/User/globalStorage/continue.continue/config.json` |
| `cline` | `<Code config>/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` |
| `codex` | `~/.codex/config.toml` |
| `lm-studio` | `~/.lmstudio/mcp.json` |
| `windsurf` | `~/.codeium/windsurf/mcp_config.json` |
| `zed` | `~/.config/zed/settings.json` (`context_servers` key) |

`<Code config>` resolves to `%APPDATA%\Code` on Windows,
`~/Library/Application Support/Code` on macOS, `~/.config/Code` on Linux.

Refer to a discovered server with `<source>:<name>` — e.g.
`mcpal tool list cursor:linear`. Bare names resolve when unambiguous.

## Custom paths

```bash
mcpal --discover-from ~/.config/private/team.json server list --discovered
```

`--discover-from` is repeatable and combines with the built-in sources.
Files must use the `{ "mcpServers": { "<name>": { ... } } }` shape.
Missing files are skipped silently; parse errors log under `-v`.
