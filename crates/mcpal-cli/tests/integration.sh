#!/usr/bin/env bash
# Integration harness for mcpal. Drives the binary at $MCPAL_BIN against the
# `@modelcontextprotocol/server-everything` reference server and exercises one
# named operation per `it`. Output goes to $OUT; assertions are grep / `[ ]`.
#
# Skipped (by the parent Rust shim) if `npx` is not on PATH.

set -u

BIN="${MCPAL_BIN:?MCPAL_BIN is required}"
CFG="$(mktemp -t mcpal-test.XXXXXX)"
rm -f "$CFG"
OUT="$(mktemp -t mcpal-test-out.XXXXXX)"
ERR="$(mktemp -t mcpal-test-err.XXXXXX)"
trap 'rm -f "$CFG" "$OUT" "$ERR"' EXIT

REF=ev
pass=0
fail=0

mc() { "$BIN" --config "$CFG" "$@"; }

it() {
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

# Pipe a literal payload via stdin to the wrapped command.
it_grep_stdin() {
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

section() { printf '\n# %s\n' "$1"; }

# ---------- config ----------
section config
it          'config init writes default config' mc config init
it_grep     'config path prints absolute path'  '^/' mc config path
it          'config show parses TOML'           mc config show

# ---------- server lifecycle ----------
section server
it          'server add stdio via `-- cmd`' \
            mc server add "$REF" -- npx -y @modelcontextprotocol/server-everything
it_grep     'server list shows the alias'   "$REF"      mc server list
it_grep     'server show prints transport'  'stdio'     mc server show "$REF"
it_exit     'server add duplicate fails (E0000)' 1 \
            mc server add "$REF" -- npx -y @modelcontextprotocol/server-everything

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

# ---------- cleanup ----------
section cleanup
it          'server remove'                  mc server remove "$REF"

printf '\n%d passed, %d failed\n' "$pass" "$fail"
test "$fail" -eq 0
