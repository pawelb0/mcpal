# Error codes

Every error mcpal raises carries a stable `E####` code. At the command
line you see a rustc-style block; this page is the long-form reference.
`mcpal explain E0001` prints the same prose.

## E0000 — generic

mcpal couldn't classify this failure into a known category, so the
message ends up here as a catch-all. The displayed text is whatever the
underlying library reported. If you can reproduce it, open an issue with
the command, the full message, and the `-v` trace output.

Exit code **1**.

## E0001 — server reference not found

mcpal didn't recognise the `<ref>` you passed. A reference resolves in
this order:

1. mcpal-owned alias (registered via `mcpal server add`)
2. an `http://` or `https://` URL
3. a path to a JSON file containing one `ServerSpec`
4. `<source>:<name>` from discovery (e.g. `cursor:linear`)
5. a bare `<name>` if it's unambiguous across discovered sources

To fix:

- `mcpal discover` — list everything installed clients already configured
- `mcpal server list --all` — see mcpal-owned + discovered together
- `mcpal server add <alias> --stdio <command>` — register a stdio server
- `mcpal server add <alias> --http <url>` — register an HTTP server

Exit code **3**.

## E0002 — usage / invalid arguments

mcpal couldn't parse the arguments you supplied. Most commonly this is a
malformed `--key value` flag pair or an unknown flag.

To fix:

- use AWS-CLI style flags: `mcpal tool call ev echo --message hi`
- for nested JSON, use `--cli-input-json @args.json` (or `-` for stdin)
- `mcpal tool template <ref> <name>` prints an example body you can pipe in
- `mcpal <subcommand> --help` for the full grammar

Exit code **2**.

## E0003 — auth required

The server (or the tool/resource you're calling) needs credentials and
none are configured.

To fix:

- bearer: `mcpal auth login <ref> --bearer <TOKEN>`
- OAuth: `mcpal auth login <ref> --oauth`
- one-shot env: `MCPAL_BEARER=… mcpal tool list <ref>`

Tokens persist in the OS keyring. They never touch the TOML config.

Exit code **4**.

## E0004 — auth expired

The server rejected the credentials mcpal sent. The access token has
likely expired.

To fix:

- `mcpal auth refresh <ref>` — use the refresh token to mint a new one
- `mcpal auth login <ref> --oauth` — full re-authorize when refresh fails
- `mcpal auth status <ref>` — see what's currently stored

Exit code **5**.

## E0005 — transport error

mcpal couldn't talk to the server. For stdio, the spawned process may
have failed to start; for HTTP, the URL may be wrong or unreachable.

To fix:

- verify the URL with `curl -I <url>` (HEAD should return 200/4xx, not a
  network error)
- for stdio: confirm the command is on `$PATH` and runs standalone
- re-run with `-v` (or `-vv`) to see the underlying request
- `mcpal server test <ref>` is the smallest reproducer

Exit code **6**.

## E0006 — server returned a JSON-RPC error

mcpal got a well-formed response, but the server returned an error code
inside the JSON-RPC payload. Common causes:

- the tool/resource/prompt doesn't exist on this server
- the arguments don't match `inputSchema`
- a server-side runtime failure

To fix:

- `mcpal tool describe <ref> <name>` — confirm the input schema
- `mcpal tool template <ref> <name>` — get a valid skeleton to fill in
- re-run with `-v` for the raw JSON-RPC frame

Exit code **7**.

## E0007 — request timed out

The server didn't respond within the deadline. For stdio servers the
most common cause is `npx -y @some-pkg` doing a fresh install (~30s on a
cold cache).

To fix:

- simply retry; subsequent runs hit the npx cache
- check the server isn't waiting on input (some stdio servers prompt
  interactively for config when first launched)

Exit code **8**.

## E0008 — not yet supported

mcpal recognised the request but the underlying rmcp library (or mcpal
itself) doesn't implement the necessary plumbing yet.

To fix:

- check `mcpal --version` and update if a newer release is out
- for advanced flows, the `mcpal raw <ref> <method> --params …` escape
  hatch sends arbitrary JSON-RPC directly

Exit code **6**.

## E0009 — bad JMESPath query

`--query` couldn't compile your expression or it ran but returned an
error. Common causes:

- unbalanced brackets or quotes (`tools[0` instead of `tools[0]`)
- trying to flatten a non-array (`foo[]` where `foo` is an object)
- a function call against a missing field

To fix:

- run the same command without `--query` to inspect the actual shape
- cheat sheet: `field`, `field.subfield`, `arr[]`, `arr[0]`,
  `arr[].field`, `arr[?field == 'x'].name`
- full tutorial: https://jmespath.org/tutorial.html

Exit code **2**.

## E0010 — JSON payload didn't parse

mcpal expected a JSON document and got something else. This happens with
`mcpal raw --params <inline|@file|->` and `mcpal tool call
--cli-input-json <file|->`.

To fix:

- for inline JSON, quote it correctly for your shell:
  ```
  mcpal raw ev tools/call --params '{"name":"echo","arguments":{"message":"hi"}}'
  ```
- for files: use `@/absolute/or/relative/path.json` (note the `@`)
- use `mcpal tool template <ref> <name>` to print a known-good skeleton

Exit code **2**.
