#!/usr/bin/env bash
# Integration harness for mcpal. Drives the binary at $MCPAL_BIN against a
# pinned `@modelcontextprotocol/server-everything` (installed once into the
# temp root, so individual calls skip npx/registry entirely) and exercises one
# named operation per `it`. Output goes to $OUT; assertions are grep / `[ ]`.
#
# MCPAL_IT_ONLY=tools,oauth runs only sections whose name contains one of the
# comma-separated (case-insensitive) substrings.
#
# Skipped (by the parent Rust shim) if `npm` is not on PATH.

set -u

BIN="${MCPAL_BIN:?MCPAL_BIN is required}"
# Some tests cd elsewhere before invoking the binary.
case "$BIN" in /*) ;; *) BIN="$(pwd)/$BIN" ;; esac

TMPROOT="$(mktemp -d -t mcpal-it.XXXXXX)"
CFG="$TMPROOT/config.toml"
OUT="$TMPROOT/out"
ERR="$TMPROOT/err"

# Aliases that may have written to the OS keyring; logged out on exit.
KEYRING_REFS=""
# Background processes (oauth mock, login, watch); killed on exit.
PIDS=""

cleanup() {
    for p in $PIDS; do kill "$p" 2>/dev/null; done
    for r in $KEYRING_REFS; do
        "$BIN" --config "$CFG" auth logout "$r" >/dev/null 2>&1
    done
    rm -rf "$TMPROOT"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

REF=ev
pass=0
fail=0

mc() { "$BIN" --config "$CFG" "$@"; }

# Poll until $pat appears in $file, max $deadline seconds. No fixed sleeps:
# fast when the event is fast, tolerant when the machine is loaded.
wait_for_grep() {
    local pat="$1" file="$2" deadline="${3:-15}" waited=0
    until grep -qE -- "$pat" "$file" 2>/dev/null; do
        sleep 0.1
        waited=$((waited + 1))
        if [ "$waited" -ge $((deadline * 10)) ]; then
            return 1
        fi
    done
    return 0
}

ONLY="$(printf '%s' "${MCPAL_IT_ONLY:-}" | tr '[:upper:]' '[:lower:]')"
ACTIVE=1

section() {
    local name_lc
    name_lc="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')"
    ACTIVE=1
    if [ -n "$ONLY" ]; then
        ACTIVE=0
        local IFS=','
        for f in $ONLY; do
            case "$name_lc" in *"$f"*) ACTIVE=1 ;; esac
        done
    fi
    [ "$ACTIVE" = 1 ] && printf '\n# %s\n' "$1"
}

it() {
    [ "$ACTIVE" = 1 ] || return 0
    local name="$1"; shift
    if "$@" >"$OUT" 2>"$ERR"; then
        printf '  ok   %s\n' "$name"
        pass=$((pass + 1))
    else
        local code=$?
        printf '  FAIL %s (exit %d)\n' "$name" "$code"
        sed 's/^/      | /' "$ERR" >&2
        fail=$((fail + 1))
    fi
}

it_grep() {
    [ "$ACTIVE" = 1 ] || return 0
    local name="$1" pat="$2"; shift 2
    "$@" >"$OUT" 2>"$ERR" || true
    if grep -iqE -- "$pat" "$OUT"; then
        printf '  ok   %s\n' "$name"
        pass=$((pass + 1))
    else
        printf '  FAIL %s (no /%s/ in stdout)\n' "$name" "$pat"
        sed 's/^/      | /' "$OUT" >&2
        fail=$((fail + 1))
    fi
}

it_grep_err() {
    [ "$ACTIVE" = 1 ] || return 0
    local name="$1" pat="$2"; shift 2
    "$@" >"$OUT" 2>"$ERR" || true
    if grep -iqE -- "$pat" "$ERR"; then
        printf '  ok   %s\n' "$name"
        pass=$((pass + 1))
    else
        printf '  FAIL %s (no /%s/ in stderr)\n' "$name" "$pat"
        sed 's/^/      | /' "$ERR" >&2
        fail=$((fail + 1))
    fi
}

# Pipe a literal payload via stdin to the wrapped command.
it_grep_stdin() {
    [ "$ACTIVE" = 1 ] || return 0
    local name="$1" pat="$2" payload="$3"; shift 3
    printf '%s' "$payload" | "$@" >"$OUT" 2>"$ERR" || true
    if grep -iqE -- "$pat" "$OUT"; then
        printf '  ok   %s\n' "$name"
        pass=$((pass + 1))
    else
        printf '  FAIL %s (no /%s/ in stdout)\n' "$name" "$pat"
        sed 's/^/      | /' "$OUT" >&2
        fail=$((fail + 1))
    fi
}

it_exit() {
    [ "$ACTIVE" = 1 ] || return 0
    local name="$1" want="$2"; shift 2
    "$@" >"$OUT" 2>"$ERR"
    local got=$?
    if [ "$got" = "$want" ]; then
        printf '  ok   %s\n' "$name"
        pass=$((pass + 1))
    else
        printf '  FAIL %s (got exit %d, want %s)\n' "$name" "$got" "$want"
        sed 's/^/      | /' "$ERR" >&2
        fail=$((fail + 1))
    fi
}

it_no_grep() {
    [ "$ACTIVE" = 1 ] || return 0
    local name="$1" pat="$2"; shift 2
    local out; out="$("$@" 2>&1 || true)"
    if printf '%s\n' "$out" | grep -q -- "$pat"; then
        printf '  FAIL %s (matched /%s/)\n' "$name" "$pat"
        fail=$((fail + 1))
    else
        printf '  ok   %s\n' "$name"
        pass=$((pass + 1))
    fi
}

# ---------- setup: pinned everything-server, provisioned once ----------
# Pinned so an upstream publish can't break the suite; installed into the
# temp root so every mcpal invocation execs a local script instead of npx.
EV_PKG="@modelcontextprotocol/server-everything@2026.1.26"
EV="$TMPROOT/npm/node_modules/.bin/mcp-server-everything"

printf '# setup: npm install %s\n' "$EV_PKG"
if ! npm install --prefix "$TMPROOT/npm" --prefer-offline --no-audit --no-fund \
    --loglevel=error "$EV_PKG" >"$OUT" 2>"$ERR"; then
    echo "FATAL: npm install $EV_PKG failed" >&2
    sed 's/^/      | /' "$ERR" >&2
    exit 1
fi
[ -x "$EV" ] || { echo "FATAL: $EV not executable after install" >&2; exit 1; }

# Every section assumes $REF exists; provision it outside any section so
# MCPAL_IT_ONLY can run sections standalone.
if ! mc server add "$REF" -- "$EV" >"$OUT" 2>"$ERR"; then
    echo "FATAL: could not provision '$REF' in $CFG" >&2
    sed 's/^/      | /' "$ERR" >&2
    exit 1
fi

# ---------- config ----------
section config
CFG_INIT="$TMPROOT/cfg-init/config.toml"
co() { "$BIN" --config "$CFG_INIT" "$@"; }
it          'config init writes default config' co config init
it_grep     'config path prints absolute path'  '^/' co config path
it          'config show parses TOML'           co config show

# ---------- server lifecycle ----------
section server
REF2=ev2
it          'server add stdio via `-- cmd`' mc server add "$REF2" -- "$EV"
it_grep     'server list shows the alias'   "$REF2"     mc server list
it          'server list --owned still works'           mc server list --owned
it_grep     'server list --owned shows alias' "$REF2"   mc server list --owned
it          'server list --all kept for back-compat'    mc server list --all
it_grep     'server show prints transport'  'stdio'     mc server show "$REF2"
it_exit     'server add duplicate fails (E0013)' 2 \
            mc server add "$REF2" -- "$EV"
it_grep_err 'server add duplicate names E0013' 'E0013' \
            mc server add "$REF2" -- "$EV"
it          'server add --force overwrites existing' \
            mc server add "$REF2" --force -- "$EV"

# ---------- server add — one-liner with auth ----------
section "server add — one-liner with auth"

# Unique alias suffix: keyring entries are keyed by alias, so fixed names
# would collide across concurrent runs and leak on a crashed one.
T1="t1-$$"; T2="t2-$$"; T3="t3-$$"; T4="t4-$$"; T5="t5-$$"
T6="t6-$$"; T6B="t6b-$$"; T7="t7-$$"; T8="t8-$$"
ADD_CFG="$TMPROOT/add/config.toml"
add() { "$BIN" --config "$ADD_CFG" "$@"; }

KEYRING_REFS="$KEYRING_REFS $T1 $T3 $T7"

it "add --bearer (literal) writes keyring + spec has no Authorization" \
    add server add "$T1" --http http://example.invalid/mcp --bearer abc
it_grep "T1 spec has [server.$T1] section" "^\\[server\\.$T1\\]" \
    cat "$ADD_CFG"
it_grep 'T1 spec is http' 'transport = "http"' \
    cat "$ADD_CFG"
it_no_grep 'T1 spec has no Authorization header' 'Authorization' \
    cat "$ADD_CFG"
it_no_grep 'T1 spec has no auth = key' '^auth' \
    cat "$ADD_CFG"
it 'auth status reports bearer present' \
    add auth status "$T1"

it 'add --bearer-env sets bearer_env in spec' \
    add server add "$T2" --http http://example.invalid/mcp --bearer-env GH_TOKEN
it_grep 'T2 spec has bearer_env' 'type = "bearer_env"' \
    cat "$ADD_CFG"
it_grep 'T2 spec carries env var' 'env = "GH_TOKEN"' \
    cat "$ADD_CFG"

it 'add --header Authorization: Bearer literal == --bearer' \
    add server add "$T3" --http http://example.invalid/mcp --header 'Authorization: Bearer xyz'
it_grep 'T3 auth status bearer present' '"bearer": true' \
    add --output json auth status "$T3"

it 'add --header X-Api-Key kept in spec, no auth' \
    add server add "$T4" --http http://example.invalid/mcp --header 'X-Api-Key: k1'
it_grep 'T4 spec has X-Api-Key' 'X-Api-Key' \
    cat "$ADD_CFG"
it_no_grep 'T4 spec has no bearer_env' 'bearer_env' \
    add server show "$T4"

it 'add stdio (no auth flags)' \
    add server add "$T5" -- echo hi
it_exit 'add stdio + --bearer is rejected' 2 \
    add server add "$T6" --bearer x -- echo hi
it_grep_err 'add stdio + --bearer shows E0002' 'E0002' \
    add server add "$T6B" --bearer x -- echo hi

it 'add --bearer - (stdin)' \
    bash -c "echo stdintok | '$BIN' --config '$ADD_CFG' server add '$T7' --http http://example.invalid/mcp --bearer -"
it_grep 'T7 bearer present via stdin' '"bearer": true' \
    add --output json auth status "$T7"

it 'add --oauth --no-login writes spec only (no browser)' \
    add server add "$T8" --http http://example.invalid/mcp --oauth --no-login
it_grep 'T8 spec has auth = oauth' 'type = "oauth"' \
    cat "$ADD_CFG"

# ---------- one-line ephemeral refs ----------
section 'one-line ephemeral refs (cmd:)'
it_grep     'cmd: lists tools on the everything server' '\becho\b' \
            mc tool list "cmd:$EV"
it_grep     'cmd: tool call returns echoed text' 'Echo: cmdmode' \
            mc --query 'content[0].text' tool call "cmd:$EV" echo --message cmdmode
it_grep_err 'cmd: with no command after prefix' 'needs a command' \
            mc tool list 'cmd:'

it          '--auth none resolves URL without OAuth warning' \
            mc --auth none server show 'https://example.test/mcp'
it_grep     '--auth env:VAR is preserved in spec' 'bearer_env' \
            mc --auth env:GH_TOKEN --output json server show 'https://example.test/mcp'
it_exit     '--auth unknown mode → E0002 exit 2' 2 \
            mc --auth magic server show 'https://example.test/mcp'

# ---------- stderr surfaced on stdio failure ----------
section "stderr surfaced on stdio failure"

it 'boom server registers (setup)' \
    mc server add boom --force -- bash -c 'echo "kaboom-marker" >&2; exit 2'
it_exit 'boom server fails (service error exit 7)' 7 \
    mc tool list boom
it_grep_err 'failure error chain contains stderr marker' 'kaboom-marker' \
    mc tool list boom
it 'boom server removes cleanly' mc server remove boom

# ---------- server properties ----------
section 'server properties'
it_grep     'server info has serverInfo.name' 'mcp-servers/everything' mc server info "$REF"
it_grep     'server protocol has a version'   '^[0-9]'              mc server protocol "$REF"
it_grep     'server capabilities lists tools' 'tools'               mc server capabilities "$REF"
it          'server instructions returns scalar'                    mc server instructions "$REF"
it_grep     'server ping → ok: true'          'ok: true'            mc server ping "$REF"

# ---------- discover / doctor ----------
section discovery
it          'server discover (may be empty)'  mc server discover
it          'debug doctor (may flag issues)'  mc debug doctor
it_grep     'debug explain E0001'             'server reference'    mc debug explain E0001

# ---------- tools ----------
section tools
it_grep     'tool list lists echo'            '\becho\b'            mc tool list "$REF"
it_grep     'tool list --names-only echo'     '^echo$'              mc tool list "$REF" --names-only
it_grep     'tool describe echo has schema'   'inputSchema'         mc tool describe "$REF" echo
it_grep     'tool template echo has message'  'message'             mc tool template "$REF" echo
it_grep     'tool call echo via flag'         'Echo: hi'   mc --query 'content[0].text' tool call "$REF" echo --message hi
it_grep     'tool call echo via --params'     'Echo: from-params' \
            mc --query 'content[0].text' tool call "$REF" echo --params '{"message":"from-params"}'
it_grep_stdin 'tool call echo via --params -' 'Echo: from-stdin' \
              '{"message":"from-stdin"}' \
              mc --query 'content[0].text' tool call "$REF" echo --params -
it_exit     'tool call schema-validation fails (E0012, exit 2)' 2 \
            mc tool call "$REF" get-sum --a notanumber --b 2
it          'tool call --skip-validation bypasses' \
            mc --query 'content[0].text' tool call "$REF" echo --skip-validation --message ok

# ---------- resources ----------
section resources
it_grep     'resource list has uri'             'demo://'             mc resource list "$REF"
it          'resource list --names-only one URI per line'             mc resource list "$REF" --names-only
it          'resource template list runs'                             mc resource template list "$REF"

# ---------- prompts ----------
section prompts
it_grep     'prompt list returns simple-prompt' 'simple-prompt'       mc prompt list "$REF"
it          'prompt list --names-only'                                mc prompt list "$REF" --names-only
it          'prompt get simple-prompt'                                mc prompt get "$REF" simple-prompt

# ---------- logging ----------
section logging
it          'logging set-level info' mc logging set-level "$REF" info

# ---------- raw ----------
section raw
it_grep     'raw tools/list returns tools' '\becho\b' mc raw "$REF" tools/list

# ---------- diff (server vs itself) ----------
section diff
it          'diff <ref> <ref> empty added/removed/changed' mc diff "$REF" "$REF"
it          'diff --only tools'                            mc diff "$REF" "$REF" --only tools

# ---------- output / query / timeout / errors ----------
section pipelines
it_grep     '--output json tool list is JSON array' '^\[' mc --output json tool list "$REF"
it_grep     '--query selects names'                 '^- echo$' mc --query '[].name' tool list "$REF"
it_exit     '--timeout 1 → E0007 exit 8'            8 \
            mc --timeout 1 tool call "$REF" trigger-long-running-operation --duration 5 --steps 5
it_exit     'unknown ref → E0001 exit 3'            3 mc tool list nope-no-such-server
it_exit     'bad subcommand → exit 2'               2 mc not-a-command

# ---------- auth ----------
section auth
it          'auth status of an unknown ref'  mc auth status nope
it          'auth logout (idempotent)'       mc auth logout nope

# ---------- server import promotes Authorization -----
section 'server import (bearer extraction)'
LIT="lit-$$"
ENVREF="envref-$$"
KEYRING_REFS="$KEYRING_REFS $LIT"
IMPORT_DIR="$TMPROOT/import"
mkdir -p "$IMPORT_DIR"
cat >"$IMPORT_DIR/.mcp.json" <<JSON
{
  "mcpServers": {
    "$LIT": {
      "url": "https://example.test/mcp",
      "headers": { "Authorization": "Bearer TESTLITERAL-INTEGRATION" }
    },
    "$ENVREF": {
      "url": "https://example.test/mcp",
      "headers": { "Authorization": "Bearer \${MY_TOK}" }
    }
  }
}
JSON
it_import_grep() {
    [ "$ACTIVE" = 1 ] || return 0
    # Run mc inside $IMPORT_DIR, capture stdout+stderr together.
    local name="$1" pat="$2"; shift 2
    ( cd "$IMPORT_DIR" && mc "$@" ) >"$OUT" 2>&1 || true
    if grep -iqE -- "$pat" "$OUT"; then
        printf '  ok   %s\n' "$name"
        pass=$((pass + 1))
    else
        printf '  FAIL %s (no /%s/ in stdout+stderr)\n' "$name" "$pat"
        sed 's/^/      | /' "$OUT" >&2
        fail=$((fail + 1))
    fi
}
it_import_grep 'import literal moves token to keyring' 'stored in keyring' \
               server import --from claude-code "$LIT"
it_import_grep 'import env-ref produces bearer_env'    'comes from \$MY_TOK' \
               server import --from claude-code "$ENVREF"
it_grep 'literal lands as bearer in keyring'  'bearer: true'  mc auth status "$LIT"
it_grep 'env-ref has no keyring entry'        'bearer: false' mc auth status "$ENVREF"
it_grep 'env-ref spec records bearer_env'     'bearer_env'    mc server show "$ENVREF"
it_grep 'literal spec drops Authorization'    '^url:'         mc server show "$LIT"
it_no_grep 'literal scrubs Authorization'     'authorization' mc server show "$LIT"
it 'cleanup: logout the test bearer' mc auth logout "$LIT"

# ---------- tool input variants ----------
section 'tool input variants'
ARGS_JSON="$TMPROOT/args.json"
printf '{"message":"file"}' >"$ARGS_JSON"
it_grep     'tool call --cli-input-json @path'  'Echo: file' \
            mc --query 'content[0].text' tool call "$REF" echo --cli-input-json "@$ARGS_JSON"
it_grep     'tool call --cli-input-json bare path' 'Echo: file' \
            mc --query 'content[0].text' tool call "$REF" echo --cli-input-json "$ARGS_JSON"
it_grep_stdin 'tool call --cli-input-json -' 'Echo: from-stdin' \
              '{"message":"from-stdin"}' \
              mc --query 'content[0].text' tool call "$REF" echo --cli-input-json -
it_exit     'tool call missing required arg → E0012 exit 2' 2 mc tool call "$REF" echo
it_exit     'tool call bad --params JSON → E0010 exit 2'    2 \
            mc tool call "$REF" echo --params '{bad json'

# ---------- resources extended ----------
section 'resources extended'
it_grep     'resource read returns contents' 'contents' \
            mc resource read "$REF" demo://resource/static/document/extension.md

# ---------- prompts extended ----------
section 'prompts extended'
it_grep     'prompt complete returns values' 'values' \
            mc prompt complete "$REF" completable-prompt --arg fruit=a

# ---------- diff alt categories ----------
section 'diff alt categories'
it          'diff --only resources' mc diff "$REF" "$REF" --only resources
it          'diff --only prompts'   mc diff "$REF" "$REF" --only prompts

# ---------- pipelines: error codes ----------
section 'error codes'
it_exit     'bad --query → E0009 exit 2'    2 mc --query 'not[valid' tool list "$REF"

# ---------- completion scripts ----------
section 'shell completions'
it_grep     'completion zsh non-empty'  'compdef'  mc completion zsh
it_grep     'completion bash non-empty' 'complete' mc completion bash
it_grep     'completion fish non-empty' 'complete' mc completion fish

# ---------- doctor JSON schema ----------
section 'doctor JSON'
it_grep     'doctor --output json has ok'      '"ok"'      mc --output json debug doctor
it_grep     'doctor --output json has servers' '"servers"' mc --output json debug doctor

# ---------- discovery filter ----------
section 'discover filter'
it          'discover --source unknown returns empty' \
            mc server discover --source not-a-real-source

# ---------- roots flag ----------
section roots
it_grep     'tool list with --root flag works' 'echo' \
            mc --root /tmp tool list "$REF"

# ---------- mcp-json overlay ----------
section 'mcp-json overlay'
MCPJ="$TMPROOT/mcp.json"
printf '{"mcpServers":{"ovr":{"command":"%s"}}}' "$EV" >"$MCPJ"
it_grep     '--mcp-json overlays a server' 'echo' \
            mc --mcp-json "$MCPJ" tool list ovr

# ---------- http alias lifecycle ----------
section 'http alias'
it          'server add --http registers' \
            mc server add fake-http --http https://example.invalid/mcp
it_grep     'server show fake-http → http' 'http' mc server show fake-http
it          'server remove fake-http' mc server remove fake-http

# ---------- typed arg parsing ----------
section 'typed args'
it_grep     'integer typed correctly' '42' \
            mc --query 'content[0].text' tool call "$REF" get-sum --a 40 --b 2
it_exit     'string where number expected → E0012 exit 2' 2 \
            mc tool call "$REF" get-sum --a 40 --b notanumber

# ---------- output shapes ----------
section 'output shapes'
it_grep     'tool list JSON has [].name'   '"name"'        mc --output json tool list "$REF"
it_grep     'tool list JSON has required'  '"required"'    mc --output json tool list "$REF"
it_grep     'server list JSON has source'  '"source"'      mc --output json server list
it_grep     'doctor reports mcpal version' '"version"'     mc --output json debug doctor

# ---------- config edge cases ----------
section 'config edge cases'
BAD_CFG="$TMPROOT/bad-config.toml"
printf 'this is not toml = =\n' >"$BAD_CFG"
it_exit     'malformed TOML → exit 1 not panic' 1 \
            "$BIN" --config "$BAD_CFG" server list

it          'missing config: server list works (empty)' \
            "$BIN" --config "$TMPROOT/never-written.toml" server list

it_exit     'config init twice fails' 1 mc config init

# ---------- bearer env one-shot ----------
section 'MCPAL_BEARER env'
it          'MCPAL_BEARER set is a no-op for stdio' \
            env MCPAL_BEARER=ignored "$BIN" --config "$CFG" server ping "$REF"

# ---------- OAuth flow (mocked) ----------
section 'OAuth flow'
if [ "$ACTIVE" = 1 ]; then
    OAUTH_REF="mcpal-oauth-test-$$"
    KEYRING_REFS="$KEYRING_REFS $OAUTH_REF"
    MOCK_BIN="$(dirname "$BIN")/examples/oauth_mock"
    [ -x "$MOCK_BIN" ] || MOCK_BIN="$(dirname "$BIN")/../examples/oauth_mock"
    if [ ! -x "$MOCK_BIN" ]; then
        printf '  skip OAuth (oauth_mock binary not built at %s)\n' "$MOCK_BIN"
    else
        MOCK_LOG="$TMPROOT/oauth-mock.log"
        "$MOCK_BIN" 0 >"$MOCK_LOG" 2>&1 &
        MOCK_PID=$!
        PIDS="$PIDS $MOCK_PID"
        if ! wait_for_grep 'port=[0-9]+' "$MOCK_LOG" 15; then
            printf '  FAIL OAuth (mock never bound)\n'
            fail=$((fail + 1))
            kill "$MOCK_PID" 2>/dev/null
        else
            PORT="$(grep -oE 'port=[0-9]+' "$MOCK_LOG" | head -1 | cut -d= -f2)"
            MOCK_URL="http://127.0.0.1:$PORT"
            # Run login in background; capture authorize URL from stderr.
            LOGIN_LOG="$TMPROOT/oauth-login.log"
            "$BIN" --config "$CFG" auth login --oauth "$OAUTH_REF" \
                --url "$MOCK_URL" --no-browser >"$LOGIN_LOG" 2>&1 &
            LOGIN_PID=$!
            PIDS="$PIDS $LOGIN_PID"
            if ! wait_for_grep 'http://127\.0\.0\.1:[0-9]+/authorize' "$LOGIN_LOG" 20; then
                printf '  FAIL OAuth (mcpal never printed authorize URL)\n'
                sed 's/^/      | /' "$LOGIN_LOG" >&2
                fail=$((fail + 1))
                kill "$LOGIN_PID" 2>/dev/null
            else
                AUTH_URL="$(grep -oE 'http://127.0.0.1:[0-9]+/authorize[^ ]*' "$LOGIN_LOG" | head -1)"
                # Drive the consent step: curl follows the mock's redirect to
                # mcpal's loopback callback; mcpal exchanges the code for tokens.
                curl -sSL "$AUTH_URL" >/dev/null 2>&1 || true
                wait "$LOGIN_PID"
                login_rc=$?
                if [ "$login_rc" -eq 0 ]; then
                    printf '  ok   auth login --oauth (full flow)\n'
                    pass=$((pass + 1))
                else
                    printf '  FAIL auth login exited %d\n' "$login_rc"
                    sed 's/^/      | /' "$LOGIN_LOG" >&2
                    fail=$((fail + 1))
                fi
                it_grep 'auth status shows oauth: true' 'oauth: true' \
                        mc auth status "$OAUTH_REF"
                it       'auth refresh' \
                         mc auth refresh "$OAUTH_REF" --url "$MOCK_URL"
                it       'auth logout cleans up' mc auth logout "$OAUTH_REF"
            fi
            kill "$MOCK_PID" 2>/dev/null
            wait "$MOCK_PID" 2>/dev/null
        fi
    fi
fi

# ---------- watch ----------
section watch
if [ "$ACTIVE" = 1 ]; then
    WATCH_OUT="$TMPROOT/watch.out"
    WATCH_ERR="$TMPROOT/watch.err"
    "$BIN" --config "$CFG" watch "$REF" >"$WATCH_OUT" 2>"$WATCH_ERR" &
    WATCH_PID=$!
    PIDS="$PIDS $WATCH_PID"
    # "watching <ref>" on stderr marks a live session; then poll for the
    # first rendered notification instead of sleeping a fixed amount.
    if ! wait_for_grep 'watching' "$WATCH_ERR" 20; then
        printf '  FAIL watch never connected\n'
        sed 's/^/      | /' "$WATCH_ERR" >&2
        fail=$((fail + 1))
    else
        mc tool call "$REF" toggle-simulated-logging --enable true >/dev/null 2>&1 || true
        if wait_for_grep 'kind:' "$WATCH_OUT" 30; then
            printf '  ok   watch emits at least one kind: notification\n'
            pass=$((pass + 1))
        else
            printf '  FAIL watch emitted no notifications\n'
            sed 's/^/      | /' "$WATCH_OUT" >&2
            fail=$((fail + 1))
        fi
    fi
    kill "$WATCH_PID" 2>/dev/null
    wait "$WATCH_PID" 2>/dev/null
fi

# ---------- cleanup ----------
section cleanup
it          'server remove'                  mc server remove "$REF2"

# ---------- help text ----------
section "help text contains key examples"
it_grep 'server add --help shows --bearer example' 'mcpal server add gh --http' \
    mc server add --help
it_grep 'tool call --help shows --params example' '--params' \
    mc tool call --help

# No AWS-CLI mentions in user-visible help text.
it_no_grep 'no AWS-CLI in server add --help' 'AWS-CLI' \
    mc server add --help
it_no_grep 'no AWS-CLI in tool call --help' 'AWS-CLI' \
    mc tool call --help

# ---------- E0017 explain ----------
section "E0017 explain"
it_grep 'debug explain E0017 prints prose' 'registry server' \
    mc debug explain E0017

# ---------- collection + mcpal run ----------
section "collection + mcpal run"

COLL_DIR="$TMPROOT/coll"
mkdir -p "$COLL_DIR"
COLL="$COLL_DIR/mcpal.yml"
cat > "$COLL" <<'YAML'
default-profile: dev

profiles:
  dev:
    msg: "hello-dev"
  prod:
    msg: "hello-prod"

calls:
  echo:
    server: ev
    tool: echo
    params:
      message: "{{profile.msg}}"

  echo-env:
    server: ev
    tool: echo
    params:
      message: "{{env.MCPAL_RUN_TEST_VAR}}"
YAML

run_cmd() { "$BIN" --config "$CFG" --collection "$COLL" "$@"; }

it_grep 'run --dry-run prints resolved params' 'hello-dev' \
    run_cmd --output json run echo --dry-run
it_grep 'run --dry-run dry_run flag present' 'dry_run' \
    run_cmd --output json run echo --dry-run

it_grep 'run --profile prod swaps the value' 'hello-prod' \
    run_cmd --output json --profile prod run echo --dry-run

it 'run echo end-to-end (live tool call)' \
    run_cmd run echo
it_grep 'run echo response contains hello-dev' 'hello-dev' \
    run_cmd --query 'content[0].text' run echo

it_exit 'unknown call name exits 3 (E0001)' 3 \
    run_cmd run nope

it_exit 'unknown profile exits 2 (E0016)' 2 \
    run_cmd --profile missing run echo
it_grep_err 'unknown profile shows E0016' 'E0016' \
    run_cmd --profile missing run echo

unset MCPAL_RUN_TEST_VAR
it_exit 'missing env var exits 2 (E0014)' 2 \
    run_cmd run echo-env
it_grep_err 'missing env var shows E0014' 'E0014' \
    run_cmd run echo-env

it_exit 'missing collection exits 2 (E0015)' 2 \
    "$BIN" --config "$CFG" --collection "$COLL_DIR/nope.yml" run echo
it_grep_err 'missing collection shows E0015' 'E0015' \
    "$BIN" --config "$CFG" --collection "$COLL_DIR/nope.yml" run echo

it 'run --params-override overlays raw value' \
    run_cmd --query 'content[0].text' run echo --params-override message=overridden
it_grep 'override took effect' 'overridden' \
    run_cmd --query 'content[0].text' run echo --params-override message=overridden

printf '\n%d passed, %d failed\n' "$pass" "$fail"
test "$fail" -eq 0
