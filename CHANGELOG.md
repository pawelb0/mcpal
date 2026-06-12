# Changelog

All notable changes documented here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer](https://semver.org/).

## [Unreleased]

## [0.4.3]

### Changed
- Exit codes derive from a typed `CliError` enum instead of substring-matching error prose. Messages and codes are unchanged; rewording an error can no longer silently change its exit code.
- Integration harness reworked for local runs: the everything-server is pinned and npm-installed once into the temp root (full suite ~20s, was minutes), all state lives under one temp directory with crash-safe cleanup (background processes killed, keyring entries logged out), and keyring-touching aliases carry a PID suffix so concurrent or crashed runs can't poison each other.
- `MCPAL_IT_ONLY=tools,oauth` runs integration sections standalone; `MCPAL_IT_OFFLINE=1` skips everything that needs the live server — 86 assertions in ~2s with no network.
- Fixed waits in the harness (OAuth mock, watch) replaced by deadline polling.
- Dropped unused dev-dependencies: `trycmd`, `insta`, `predicates`.

### Fixed
- `--key` flag missing its value now exits 2 with E0002 like every other usage error, instead of falling through to E0000/exit 1.
- Template errors (`E0014`) reach the classifier as a typed error instead of round-tripping through a string.

## [0.4.2]

### Added
- `cmd:<command> [args]` ephemeral stdio `<ref>` — call any local MCP server in one shell line, no `server add`. `mcpal tool list "cmd:npx -y @mcp/server-everything"`.
- `--auth MODE` global flag pairs with an inline `https://` `<ref>` to pick auth on the fly: `oauth` (default), `none`/`anon`, `env:VAR`, `bearer:TOKEN`.
- `book/src/one-liners.md` — every one-line `<ref>` shape in one table, with auth modes and the limits of each.
- `book/src/why-cli.md` — Explanation chapter on when a shell client earns its place next to MCP-aware chat apps.
- `book/src/protocol-matrix.md` carries a roadmap table for the 2026-07-28 RC.

### Changed
- Resolver order documented and stable: owned alias → `cmd:` → URL → JSON path → `<source>:<name>` → bare name. E0001 message lists the new precedence.
- `mcpal-core::handler::run_sampling_handler` returns `anyhow::Result` instead of a hand-rolled `Result<_, String>`.
- `Config::load` drops the `Path::exists` pre-check and matches `ErrorKind::NotFound` directly; one less stat() per startup.
- `--query` no longer JSON-string-roundtrips its jmespath result.
- Internal registry DTOs (`Envelope`, `Server`, `Package`, `EnvVar`, …) move to `pub(crate)`. `ServerWrapper` renamed to `ServerEntry`. `EnvVarHint` collapsed into `(String, Option<String>)`.
- Unit coverage doubled: 60 → 130 tests (oauth math, JMESPath, resolver order, exit classifier, runtime deadline, TUI focus, sidebar filter, diff edges, discover descriptors).

### Fixed
- `oauth::access_token_refreshing` made one redundant keyring read per call; collapsed to a single load on the hot path.

## [0.4.1]

### Added
- `mcpal server install` prompts for declared environment variables on a TTY using each variable's registry-provided description as the hint. Non-TTY (or `--no-prompt`) bails with the new `E0017` error.
- TUI: connecting to a server whose stored spec carries empty env values pops a `Configure '<server>'` modal — fill in, save (writes to `config.toml`), connect.
- `book/src/test-corpus.md` — curated list of tricky MCP servers exercised on every release.

### Changed
- Registry-declared `environmentVariables` default to required unless they carry a `default` value or explicitly set `isRequired: false`. Matches the official registry's actual schema.
- `registry::to_spec` now returns `(ServerSpec, RequiredEnvHint)` so callers can prompt instead of bailing.

### Fixed
- `mcpal server install io.github.codeurali/dataverse` silently produced a spec without `DATAVERSE_ENV_URL`, then the server crashed on `initialize`. The prompt path or `--env DATAVERSE_ENV_URL=…` resolves this end-to-end.

## [0.4.0]

### Added
- Discovery sources for VS Code (workspace `.vscode/mcp.json` + user `settings.json` `chat.mcp.servers` + Continue extension storage) and Codex CLI (`~/.codex/config.toml`).
- `--discover-from PATH` global flag for ad-hoc `mcp.json` files (repeatable).
- Book `Discovery` chapter listing every supported client; troubleshooting section for stdio servers that die on `initialize`.

### Changed
- `mcpal server install` picks the highest semver-compatible version from the registry instead of the first match.
- Stdio child stderr is captured by default into a 64-line ring buffer; on connect failure the tail is attached to the error chain. `MCPAL_CHILD_STDERR=null|inherit|capture` controls the mode; the TUI pins `null` to keep its alt-screen clean.

### Fixed
- `mcpal server install io.github.<owner>/<name>` silently picked the lowest version when multiple existed.
- `mcpal tool list <stdio-ref>` failing with `connection closed: initialize response` now includes the child's stderr instead of empty context.

## [0.3.1]

### Changed
- `mcpal server list` now shows owned + discovered entries by default. `--owned` narrows to mcpal-registered; `--discovered` narrows to discovery-imported. `--all` is kept (hidden) for back-compat with scripts.

### Fixed
- TUI no longer silently swallows `tools/list`, `resources/list`, or `prompts/list` failures. Errors surface to the output pane (`<ref>: tools/list failed: …`) so an empty tab has a visible explanation.

## [0.3.0]

### Added
- `mcpal.yml` collection file. Define `profiles:` + `calls:`, then `mcpal run NAME --profile prod` to invoke a saved tool call. Source-first: commit the file, share with teammates.
- `mcpal run` verb with `--dry-run` (resolve + print without opening a connection) and `--params-override K=V` (overlay raw values after templating).
- `{{profile.X}}` + `{{env.X}}` substitution inside `params`; `{{{{` escapes a literal `{{`. Unresolved variables collected and reported in one error.
- `E0014` (template variable not set), `E0015` (collection not found), `E0016` (profile not in collection) error codes.
- Book chapter `Collections`; README Quickstart subsection for saved calls.
- Windows install note in the book — DPAPI keyring; MSI / winget roadmap.

## [0.2.0]

### Added
- `mcpal server add` one-liner: `--bearer / --bearer-env / --oauth / --header / --force / --no-login` accepted alongside the transport flags. Writes spec + materialises the credential (keyring for literal bearers, `bearer_env` in the spec for env refs, inline browser flow for OAuth) in one command.
- `E0013 server already exists` error code; `--force` overrides.
- Interactive TUI (`mcpal tui`) — split-pane browser for servers, tools, resources, prompts; live notification stream; bearer + OAuth + tool-call composer.
- `.deb` packages for Debian / Ubuntu attached to every release.
- `mcpal ui inspect` — classifies mcp-ui (`ui://`) and OpenAI Apps (`application/vnd.openai.app+json`) payloads in tool results.
- Trace events for elicitation + sampling in the notification stream.
- `--help` Examples blocks for `server add`, `tool call`, `auth login`, `raw`.
- Book chapters: Install, TUI, UI-rich MCP servers, Changelog.

### Changed
- README + book quickstarts collapsed: `server add` + `auth login` → single command.
- README hero reworked semble-style; tagline + badges + nav pills.
- Book sidebar reordered — Concepts moved ahead of How-to guides.
- Dropped "AWS-CLI" framing from doc strings + book prose; `--query` is documented as a JMESPath filter.
- Server import promotes `Authorization: Bearer …` headers to keyring or `bearer_env` automatically.

### Fixed
- TUI rendering corruption against servers that bleed installer progress to the controlling terminal (uv / fastmcp). stdio children launch via `setsid` and have stderr nulled.
- Control bytes in server-supplied strings sanitised before render.
- Esc inside the TUI preserves detail context; `h` / Left navigates to the previous tab.

## [0.1.1]

### Fixed
- Homebrew tap formula naming. Renamed crate `mcpal-cli` → `mcpal` so cargo-dist publishes `Formula/mcpal.rb` and `brew install pawelb0/tap/mcpal` works.

## [0.1.0]

### Added
- Initial release. CLI client for the Model Context Protocol: stdio + Streamable HTTP transports; OAuth 2.1 (PKCE + DCR); discovery from Claude Desktop / Cursor / opencode `mcp.json`; tool, resource, prompt commands; JSON-RPC `raw` escape hatch; `watch` for notifications; JMESPath `--query`; OS-keyring credentials.
