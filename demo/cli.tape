# mcpal CLI — the AWS-CLI-for-MCP pitch in 20 seconds. Register
# a server, list its tools, call one with --query for JMESPath
# extraction.
#
# Regenerate:  vhs demo/cli.tape
# Prereqs:     cargo build --release; PATH includes ./target/release; npx

Output assets/cli.gif

Require mcpal
Require npx

Set Shell "bash"
Set FontFamily "JetBrains Mono"
Set FontSize 24
Set Width 1300
Set Height 700
Set Theme "Catppuccin Mocha"
Set TypingSpeed 40ms
Set Prompt "$ "

Hide
Type "export MCPAL_CONFIG=$(mktemp -t mcpal-demo.XXXXXX.toml)"
Enter
Type "export PS1='$ '"
Enter
Type "mcpal config init >/dev/null"
Enter
Type "clear"
Enter
Show

# Register a stdio MCP server in one line
Type "mcpal server add ev -- npx -y @modelcontextprotocol/server-everything"
Enter
Sleep 1500ms

# List what it offers
Type "mcpal tool list ev --names-only"
Enter
Wait+Line /\$\s*$/
Sleep 2000ms

# Call it. Plain.
Type 'mcpal tool call ev echo --message "hello from mcpal"'
Enter
Wait+Line /\$\s*$/
Sleep 2500ms

# Same call, JMESPath-extracted (AWS-CLI-style)
Type "mcpal --query 'content[0].text' tool call ev echo --message hi"
Enter
Wait+Line /\$\s*$/
Sleep 3000ms
