# Error codes

Every error has a stable `E####` code. At the CLI you see a rustc-style
block; this page is the long form. `mcpal debug explain E####` prints the
same text.

## E0000 — generic

mcpal couldn't classify the failure. The displayed text is whatever the
underlying library reported. If you can reproduce it, open an issue
with the command, the full message, and the `-v` trace output.

Exit code **1**.

## E0001 — server reference not found

mcpal didn't recognise the `<ref>`. References resolve in this order:

1. Alias registered via `mcpal server add`.
2. An `http://` or `https://` URL.
3. Path to a JSON file with one `ServerSpec`.
4. `<source>:<name>` from discovery (e.g. `cursor:linear`).
5. A bare `<name>` if unambiguous across discovered sources.

Fixes:

- `mcpal server discover` — list everything installed clients already
  configured.
- `mcpal server list --all` — owned + discovered.
- `mcpal server add <alias> --stdio <command>` — register a stdio
  server.
- `mcpal server add <alias> --http <url>` — register an HTTP server.

Exit code **3**.

## E0002 — usage / invalid arguments

mcpal couldn't parse the arguments. Most commonly a malformed
`--key value` pair or an unknown flag.

Fixes:

- Pass `--key value` pairs (AWS-CLI style): `mcpal tool call ev echo
  --message hi`.
- For nested JSON, use `--cli-input-json @args.json` (or `-` for stdin).
- `mcpal tool template <ref> <name>` prints a valid skeleton.
- `mcpal <subcommand> --help` for the full grammar.

Exit code **2**.

## E0003 — auth required

The server (or the tool, resource, or prompt) needs credentials and
none are configured.

Fixes:

- Bearer: `mcpal auth login <ref> --bearer <TOKEN>`.
- OAuth: `mcpal auth login <ref> --oauth`.
- One-shot env: `MCPAL_BEARER=… mcpal tool list <ref>`.

Tokens persist in the OS keyring, not the TOML config.

Exit code **4**.

## E0004 — auth expired

The server rejected the credentials mcpal sent. The access token has
likely expired.

Fixes:

- `mcpal auth refresh <ref>` to mint a new access token.
- `mcpal auth login <ref> --oauth` for a full re-authorize when refresh
  fails.
- `mcpal auth status <ref>` to see what's stored.

Exit code **5**.

## E0005 — transport error

mcpal couldn't talk to the server. For stdio, the spawned process may
have failed to start; for HTTP, the URL is wrong or unreachable.

Fixes:

- Verify the URL with `curl -I <url>` (HEAD should return 200/4xx, not
  a network error).
- For stdio: confirm the command is on `$PATH` and runs standalone.
- Re-run with `-v` (or `-vv`) to see the underlying request.
- `mcpal server ping <ref>` is the smallest reproducer.

Exit code **6**.

## E0006 — server returned a JSON-RPC error

A well-formed JSON-RPC error from the server. Common causes:

- The tool, resource, or prompt name is wrong.
- The arguments don't match `inputSchema`.
- A server-side runtime failure.

Fixes:

- `mcpal tool describe <ref> <name>` — confirm the input schema.
- `mcpal tool template <ref> <name>` — get a valid skeleton.
- Re-run with `-v` for the raw JSON-RPC frame.

Exit code **7**.

## E0007 — request timed out

The server didn't respond within the deadline. For stdio servers, the
common cause is `npx -y @some-pkg` doing a fresh install (~30s on a
cold cache).

Fixes:

- Retry; subsequent runs hit the npx cache.
- Check the server isn't waiting on stdin (some stdio servers prompt
  for config on first launch).

Exit code **8**.

## E0008 — not yet supported

mcpal recognised the request but the plumbing isn't in place yet.

Fixes:

- Check `mcpal --version` and update if a newer release is out.
- For advanced flows, `mcpal raw <ref> <method> --params …` sends
  arbitrary JSON-RPC directly.

Exit code **6**.

## E0009 — bad JMESPath query

`--query` couldn't compile or returned an error. Common causes:

- Unbalanced brackets or quotes (`tools[0` instead of `tools[0]`).
- Trying to flatten a non-array (`foo[]` where `foo` is an object).
- A function call against a missing field.

Fixes:

- Run the same command without `--query` to inspect the actual shape.
- Cheat sheet: `field`, `field.subfield`, `arr[]`, `arr[0]`,
  `arr[].field`, `arr[?field == 'x'].name`.
- Tutorial: https://jmespath.org/tutorial.html.

Exit code **2**.

## E0010 — JSON payload didn't parse

mcpal expected a JSON document and got something else. This happens
with `mcpal raw --params <inline|@file|->` and
`mcpal tool call --cli-input-json <file|->`.

Fixes:

- Quote inline JSON for your shell:
  ```
  mcpal raw ev tools/call --params '{"name":"echo","arguments":{"message":"hi"}}'
  ```
- For files: `@/absolute/or/relative/path.json` (note the `@`).
- `mcpal tool template <ref> <name>` for a known-good skeleton.

Exit code **2**.

## E0012 — schema validation failed

mcpal validates the arguments you pass to `tool call` against the
tool's `inputSchema` before sending the request. A schema check turned
up one or more violations (missing required field, wrong type, value
outside the allowed enum, unknown property when the schema is strict).

Fixes:

- `mcpal tool describe <ref> <name>` shows the full schema.
- `mcpal tool template <ref> <name>` prints a known-good skeleton.
- Pass `--skip-validation` to dispatch the call without checking
  (useful if the server's schema is buggy or stricter than reality).

Exit code **2**.

## E0011 — interrupted by Ctrl-C

You pressed Ctrl-C while mcpal was waiting on a response from the
server. mcpal drops the in-flight request and exits with code 130 (the
conventional code for SIGINT-terminated programs).

The server may still complete the operation on its end — mcpal just
stops waiting. There is no MCP method today to tell the server "never
mind" once the request is in flight. For a hard deadline instead, pass
`--timeout <SECS>`.

Exit code **130**.
