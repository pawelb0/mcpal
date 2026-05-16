# Protocol compliance matrix

Unmarked rows are wired. `pending` rows are not.

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
| `completion/complete` | — | pending |
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
