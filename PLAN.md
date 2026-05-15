# mcpal — positioning + competitor map + M7 plan

## Positioning

**mcpal is curl for MCP.** Single static Rust binary. Scripting-first
(stdout = data, stderr = diagnostics, exit codes are stable). Full protocol
coverage. The thing you reach for when you want to drive a Model Context
Protocol server from a terminal, a CI pipeline, or a Makefile.

Not in scope:
- Browser UI. Inspector owns that and should keep owning it.
- LLM-bundled chat. chrishayuk/mcp-cli + goose + mcphost own that lane.
- Server framework. FastMCP owns that.

In scope:
- Every protocol method as a first-class verb.
- OAuth 2.1 + bearer + OS keyring without a browser tab.
- Discovery across every MCP client config we can read.
- Scriptable output (YAML default, JSON when asked, `--query` for one-liners).
- mdBook user manual + per-command examples + a `doctor` subcommand for new users.

## Competitor map

### Browser-shaped
| Tool | Stars | Active | Killer feature | Why mcpal wins |
|---|---|---|---|---|
| `@modelcontextprotocol/inspector` | 9.8k | ✓ | Canonical, anthropic-blessed, form UI | mcpal is no-browser, no-Node, no proxy CVE class, OAuth in CLI |
| Claude Desktop / Code | – | ✓ | End-user enablement | mcpal is a debugger, not a host |

### CLI-shaped
| Tool | Stars | Active | Lane | Sharpest weakness |
|---|---|---|---|---|
| `f/mcptools` (Go) | 1.6k | stalled | proxy/mock/guard; macOS-only config scan | abandoned 6mo; no OAuth; no notifications |
| `wong2/mcp-cli` (Node) | 432 | slow | only competitor with clean OAuth | GPL-3.0 (commercial-unfriendly); interactive-first |
| `philschmid/mcp-cli` (Bun) | 1.1k | stalled | daemon connection pool, `grep` across tools | tools-only; no resources/prompts; stalled |
| `chrishayuk/mcp-cli` (Python) | 2.0k | ✓ | full LLM chat, agent-shaped | huge dep tree; agent-shaped not curl-shaped |
| FastMCP CLI (Python) | 25k (framework) | ✓ | best cross-IDE config scan | CLI is side-car to framework; slow Python startup |
| Inspector `--cli` | 9.8k | ✓ | reference protocol fidelity | header-only auth; verbose flag syntax; CLI is afterthought |
| `mcpal` (this) | – | ✓ | OAuth + keyring + discovery + YAML + Rust binary | gaps: completion, watch, raw, doctor, mdBook |

### Empty whitespace nobody owns
1. Full method coverage including `completion/complete`, `resources/subscribe`, `notifications/*`, `logging/setLevel`, raw passthrough.
2. OAuth 2.1 + DCR + PKCE under a permissive license.
3. Cross-IDE discovery in a single 5MB binary (FastMCP's discovery story without Python).
4. `curl`-grade verbosity + exit codes + man pages.

mcpal already occupies (2) and most of (3). M7 closes (1) and (4).

## What to copy / not copy (curl, aws, gh, grpcurl, kubectl, fly, stripe)

| From | Copy | Skip |
|---|---|---|
| curl | `--config <file>`, stable exit codes per failure class | flag soup; `curl --help` is 270+ options |
| httpie | `key:=value` for nested JSON alongside `--key value` for strings | eager colorization without `NO_COLOR` |
| grpcurl | `tool template <ref> <name>` printing a populated JSON skeleton | live-vs-descriptor mode split |
| websocat | streaming default (`mcpal watch`); `--autoreconnect` for HTTP | specifier soup |
| aws | `--cli-input-json`, `--query <jmespath>`, `--output {yaml,json}`, `aws help <topic>` | docs auto-generated from Botocore — keep ours hand-written |
| gh | verb-consistency, `gh api` escape hatch, `gh auth status`/`login` shape | hidden TTY wizards |
| fly | `doctor` subcommand, `logs` streaming tail | 14-command top level |
| stripe | `listen` tunnel pattern (analog: `mcpal proxy <ref>`) | sticky env modes |
| kubectl | `--watch` on every list verb, `explain` paths | plugin `$PATH` model |

## M7 roadmap (ease-of-use + docs)

### M7a — typed errors + exit codes (1 day)
- CLI-side `Error` enum with variants: usage(2), not_found(3), auth_required(4), auth_expired(5), transport(6), server(7), timeout(8).
- Each variant renders a rustc-style message with the next-step hint (`run \`mcpal auth refresh notion\``).
- `mcpal help exit-codes` documents the table.

### M7b — `mcpal doctor` (1 day)
- Config readable + path
- Keyring access works
- For each known ref: transport reachable + auth valid + token expiry
- Output: YAML by default, `--json` for paste-into-issue

### M7c — `mcpal tool template` + `--query <jmespath>` (1 day)
- `tool template <ref> <name>` → emit an example body matching `inputSchema` with typed placeholders.
- Add `--query` global flag that pipes the final response through a JMESPath expression before emitting.

### M7d — `mcpal watch <ref>` + raw passthrough (2 days)
- Extend `Handler` with `on_progress`, `on_resource_updated`, `on_*_list_changed`.
- New `watch` command streams every server notification as one YAML doc per event.
- Finish `mcpal raw <ref> <method> --params @file|-|inline` via `ClientRequest::CustomRequest`.
- `mcpal tool call --watch` overlays progress on a long call before the result.

### M7e — mdBook user manual (1–2 days)
Site layout (`book/`):
1. Concepts — refs, transports, discovery, auth
2. Getting Started — 60-second tour
3. Recipes — cookbook, one h2 per task, copy-pasteable
4. Auth & OAuth deep dive
5. Scripting & exit codes
6. Troubleshooting (`mcpal doctor` output reference)
7. MCP spec compliance matrix
8. Appendix A — auto-generated command reference (clap-markdown, committed)
9. Appendix B — protocol method ↔ mcpal verb cross-reference

Deploy via `peaceiris/actions-mdbook` + `peaceiris/actions-gh-pages` on tag. README shortens to a pointer + first 60 seconds.

Precedents (mdBook for CLI manuals): The Rust Book, The Cargo Book, The rustup Book, The rustc Book, Helix, Nushell, mdBook itself.

### M7f — `--mcp-json` import (0.5 day)
Read Claude Desktop / Cursor / VS Code-style `mcpServers` blobs directly. `mcpal --mcp-json ./mcp.json tool list everything` works without `server add`.

### M7g — `completion/complete` + `tool template` integration (1 day)
Wire `completion/complete` for prompt args and resource template args. Use the result to populate `tool template` placeholders when the server supports it.

## Ship order

The cheap-then-flashy path:
1. M7a typed errors (foundation)
2. M7c `--query` (1-line wins everywhere)
3. M7c `tool template` (sells the curl pitch)
4. M7d `raw` (closes Inspector's last edge)
5. M7b `doctor` (new-user trust)
6. M7d `watch` (closes the live-notification gap)
7. M7e mdBook (sells the project externally)
8. M7f mcp-json import (ergonomic for migrators)
9. M7g completion-complete (last protocol gap)

Each step is ≤ a day. Total ≈ 1 week of focused work.

## Test discipline

Every M7 piece ships with:
- `cargo fmt --all --check` + `cargo clippy --workspace --all-targets -- -D warnings`
- Integration test gated on `which::which("npx").is_ok()`
- README + book updated in the same commit when behavior changes

## Out of scope for M7

- LLM agent loops
- Browser UI
- Proxy/tunnel (defer to M8)
- Multi-session REPL (the `repl` command was removed by user request; revisit only if mdBook user feedback asks)
- Windows pre-built binaries beyond what the release workflow already produces
