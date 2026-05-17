# mcpal TUI — hero demo. Drill into the everything-server,
# fire a tool call from the auto-built form, and watch the
# result land in-pane plus the output log.
#
# Regenerate:  vhs demo/tui.tape
# Prereqs:     cargo build --release; PATH includes ./target/release; npx

Output assets/tui.gif

Require mcpal
Require npx

Set Shell "bash"
Set FontFamily "JetBrains Mono"
Set FontSize 22
Set Width 1400
Set Height 880
Set Theme "Catppuccin Mocha"
Set TypingSpeed 35ms
Set PlaybackSpeed 1.0

# --- silent setup: isolated config, two stdio fixtures, warm cache ---
Hide
Type "export MCPAL_CONFIG=$(mktemp -t mcpal-demo.XXXXXX.toml)"
Enter
Sleep 200ms
Type "mcpal config init >/dev/null"
Enter
Sleep 200ms
Type "mcpal server add ev -- npx -y @modelcontextprotocol/server-everything"
Enter
Sleep 200ms
Type "mcpal server add fs -- npx -y @modelcontextprotocol/server-filesystem $HOME"
Enter
Sleep 200ms
Type "mcpal server ping ev >/dev/null 2>&1 || true"
Enter
Sleep 500ms
Ctrl+L
Sleep 600ms
Show
Sleep 400ms

# --- demo proper ---
Type "mcpal tui"
Enter
Wait+Screen /Servers/

Sleep 1800ms

# ev is selected by default (alphabetical) — open it
Enter
Wait+Screen /Tools/
Sleep 2200ms

# echo is the first tool — open the call form directly
Type "c"
Wait+Screen /call echo/
Sleep 1500ms

# Fill the message field; form's first field is `message` (string).
Type "hello from mcpal"
Sleep 1500ms

# Submit
Enter
Wait+Screen /tool call ev echo/
Sleep 3500ms

# Esc: CallResult → Server (tab Tools, selection kept)
Escape
Sleep 1200ms

# Esc again: Server view stays warm, focus jumps to sidebar
Escape
Sleep 1000ms

# Show the help overlay
Type "?"
Sleep 2800ms
Escape
Sleep 800ms

# Quit
Type "q"
Sleep 1500ms
