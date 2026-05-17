# Demo recordings

Three [VHS](https://github.com/charmbracelet/vhs) tapes that render the
GIFs under `../assets/`:

| Tape | Output | What it shows |
|---|---|---|
| `tui.tape` | `assets/tui.gif` | TUI flythrough: sidebar → tools → filter → call form (auto-filled from schema) → result + output log |
| `cli.tape` | `assets/cli.gif` | "AWS CLI for MCP": register a stdio server, list tools, call one with JMESPath |
| `scripting.tape` | `assets/scripting.gif` | Pipe `--output json` into `jq`, branch on `$?`, extract with `--query` |

## Regenerating

```bash
# 1. install vhs once
brew install vhs                  # macOS
# 2. build a release binary the tapes can find
cargo build --release
export PATH="$PWD/target/release:$PATH"
# 3. record
vhs demo/tui.tape
vhs demo/cli.tape
vhs demo/scripting.tape
```

Each tape writes its own GIF into `assets/`. Commit the GIFs alongside
any tape change so the README never references a stale asset.

## Notes

- Tapes use a throw-away `MCPAL_CONFIG=$(mktemp …)` so they never
  touch your real `~/.config/mcpal/config.toml`.
- The first run of any `npx -y @modelcontextprotocol/server-*` pulls
  the package, which can take 5–15 s. Tapes `ping` once during the
  hidden setup so the recorded call is warm.
- `Wait+Line /\$\s*$/` waits for the prompt to reappear before
  proceeding — keeps timing honest across machines.
