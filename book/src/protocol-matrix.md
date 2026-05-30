# Protocol compliance matrix

Every MCP method, server-initiated request, transport, and auth mode,
and the mcpal verb (or flag) that drives it. Unmarked rows are wired
in. `pending` rows are not.

## Methods

| Method | mcpal verb | Status |
|---|---|---|
| `initialize` | every command (handshake) | implicit |
| `ping` | `server ping <ref>` | via initialize handshake |
| `tools/list` | `tool list <ref>` |  |
| `tools/call` | `tool call <ref> <name> --key value` |  |
| `resources/list` | `resource list <ref>` |  |
| `resources/read` | `resource read <ref> <uri>` |  |
| `resources/templates/list` | `resource template list <ref>` |  |
| `resources/subscribe` | `resource subscribe <ref> <uri>` |  |
| `resources/unsubscribe` | `resource unsubscribe <ref> <uri>` |  |
| `prompts/list` | `prompt list <ref>` |  |
| `prompts/get` | `prompt get <ref> <name> --key value` |  |
| `completion/complete` | `prompt complete <ref> <name> --arg F=P` / `resource complete <ref> --template <uri> --arg F=P` |  |
| `logging/setLevel` | `logging set-level <ref> <level>` |  |
| any other / future method | `raw <ref> <method> --params …` | passthrough |

## Server-initiated requests

| Method | Default handler | Override |
|---|---|---|
| `roots/list` | returns `--root <path>` paths | `--root` flag |
| `elicitation/create` (form) | TTY prompt → Accept; non-TTY → Decline | `--no-interactive` to always decline |
| `elicitation/create` (url) | prints URL → Accept | n/a |
| `sampling/createMessage` | `MethodNotFound` | `--sampling-handler <CMD>` |
| `logging/message` | routed via `tracing`; emitted from `mcpal watch` | `RUST_LOG=…` |
| `notifications/progress` | emitted by `mcpal watch` | n/a |
| `notifications/resources/updated` | emitted by `mcpal watch` | n/a |
| `notifications/{tools,prompts,resources}/list_changed` | emitted by `mcpal watch` | n/a |
| `notifications/cancelled` | emitted by `mcpal watch` | n/a |

## Transports

| Transport | Status |
|---|---|
| stdio (child process) |  |
| Streamable HTTP (rustls) |  |
| SSE (legacy 2024-11-05) | not enabled |
| WebSocket | not enabled |

## Auth

| Mode | Status |
|---|---|
| Bearer (inline / env / keyring) |  |
| OAuth 2.1 + PKCE + DCR (RFC 7591) |  |
| OAuth 2.1 + CIMD (client metadata URL, SEP-991) | pending |
| Custom HTTP headers | via `ServerSpec::Http { headers }` |
| mTLS | pending |

## Spec 2026-07-28 release candidate

The next protocol revision changes enough on the wire that adopting
it is a planned upgrade, not a free ride. mcpal tracks `rmcp`'s
support for it; the column below records where each delta lands on
the mcpal side.

| Change (SEP) | Where it lands in mcpal |
|---|---|
| Handshake removed: no more `initialize` / `initialized` (SEP-2575) | `client.rs` connect path; rmcp upgrade required |
| Session header `Mcp-Session-Id` deprecated (SEP-2567) | HTTP transport; rmcp upgrade required |
| Required routing headers `Mcp-Method`, `Mcp-Name`, `MCP-Protocol-Version` (SEP-2243) | HTTP transport; rmcp upgrade required |
| Client info, version, capabilities move to per-request `_meta` | `client.rs`; rmcp upgrade required |
| W3C Trace Context in `_meta` (`traceparent`, `tracestate`, `baggage`, SEP-414) | follow-up after rmcp upgrade |
| `server/discover` method | new verb under `mcpal server …`, post-upgrade |
| `ttlMs` / `cacheScope` on list and resource read results (SEP-2549) | enables local catalogue cache (see [Why a CLI for MCP](./why-cli.md)) |
| Full JSON Schema 2020-12 in `inputSchema` (SEP-2106) | `jsonschema` crate already on 2020-12; verify `--skip-validation` path |
| `outputSchema` unrestricted; `structuredContent` any JSON | output rendering; small change in `output.rs` |
| Resource-missing error: `-32002` → `-32602` (SEP-2164) | `exit.rs` classifier table |
| OAuth `iss` validation per RFC 9207 (SEP-2468) | rmcp `AuthorizationManager`; verify on upgrade |
| OIDC `application_type` in DCR (SEP-837) | rmcp DCR call; verify on upgrade |
| Credentials bound to issuer (SEP-2352) | keyring key naming under review |
| Roots / Sampling / Logging deprecated (SEP-2577) | matrix above will mark deprecated when 2026-07-28 lands |
| Tasks extension (`tasks/get`, `tasks/update`, `tasks/cancel`) | new verb group, scoped after rmcp upgrade |
| Extensions framework (reverse-DNS IDs, `extensions` map) | `raw` already covers ad-hoc calls |
