# mcpal scripting — pipe into jq, extract via JMESPath, branch
# on exit codes. The "it's just a Unix tool" story.
#
# Regenerate:  vhs demo/scripting.tape
# Prereqs:     cargo build --release; PATH includes ./target/release; npx; jq

Output assets/scripting.gif

Require mcpal
Require npx
Require jq

Set Shell "bash"
Set FontFamily "JetBrains Mono"
Set FontSize 22
Set Width 1300
Set Height 760
Set Theme "Catppuccin Mocha"
Set TypingSpeed 35ms

Hide
Type "export MCPAL_CONFIG=$(mktemp -t mcpal-demo.XXXXXX.toml)"
Enter
Type "export PS1='$ '"
Enter
Type "mcpal config init >/dev/null"
Enter
Type "mcpal server add ev -- npx -y @modelcontextprotocol/server-everything"
Enter
Type "mcpal server ping ev >/dev/null 2>&1 || true"
Enter
Type "clear"
Enter
Show

# JMESPath: extract one field
Type "mcpal --query 'content[0].text' tool call ev echo --message hi"
Enter
Wait+Line /\$\s*$/
Sleep 2200ms

# JSON output, piped to jq
Type "mcpal --output json tool list ev --names-only | head -3"
Enter
Wait+Line /\$\s*$/
Sleep 2200ms

# Result extraction via jq
Type "mcpal --output json tool call ev add --a 17 --b 25 | jq '.content[0].text'"
Enter
Wait+Line /\$\s*$/
Sleep 2500ms

# Exit-code branching — bash case on $?
Type "mcpal tool call ev echo --message ping >/dev/null && echo 'ok ('$?')' || echo 'failed ('$?')'"
Enter
Wait+Line /\$\s*$/
Sleep 2800ms
