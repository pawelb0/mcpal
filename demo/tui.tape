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
Sleep 200ms
Type "clear"
Enter
Sleep 200ms
Show

# --- demo proper ---
Type "mcpal tui"
Enter
Wait+Screen /Servers/

Sleep 1500ms

# Move to ev, open it
Type "j"
Sleep 600ms
Enter
Wait+Screen /Tools/
Sleep 1500ms

# Filter tools for echo
Type "/"
Sleep 400ms
Type "echo"
Sleep 800ms
Enter
Sleep 800ms

# Open the call form
Type "c"
Wait+Screen /call echo/
Sleep 1200ms

# Fill the message field
Type "hello from mcpal"
Sleep 1200ms

# Submit
Enter
Wait+Screen /tool call.*echo/
Sleep 3500ms

# Back to server view, then back to sidebar
Escape
Sleep 1200ms
Escape
Sleep 1200ms

# Show the help overlay briefly
Type "?"
Sleep 2500ms
Escape
Sleep 800ms

# Quit
Type "q"
Sleep 1500ms
