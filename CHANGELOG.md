# Changelog

All notable changes documented here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning: [SemVer](https://semver.org/).

## [Unreleased]

## [0.2.0] — 2026-05-22

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

## [0.1.1] — 2026-05-16

### Fixed
- Homebrew tap formula naming. Renamed crate `mcpal-cli` → `mcpal` so cargo-dist publishes `Formula/mcpal.rb` and `brew install pawelb0/tap/mcpal` works.

## [0.1.0] — 2026-05-16

### Added
- Initial release. CLI client for the Model Context Protocol: stdio + Streamable HTTP transports; OAuth 2.1 (PKCE + DCR); discovery from Claude Desktop / Cursor / opencode `mcp.json`; tool, resource, prompt commands; JSON-RPC `raw` escape hatch; `watch` for notifications; JMESPath `--query`; OS-keyring credentials.
