# Protocol compliance matrix

Every MCP method, and how mcpal exposes it.

| Method | mcpal verb | Status |
|---|---|---|
| `initialize` | every command (handshake) | implicit |
| `ping` | `server test <ref>` | covered via initialize handshake |
| `tools/list` | `tool list <ref>` | first-class |
| `tools/call` | `tool call <ref> <name> --key value` | first-class |
| `resources/list` | `resource list <ref>` | first-class |
| `resources/read` | `resource read <ref> <uri>` | first-class |
| `resources/templates/list` | `resource template list <ref>` | first-class |
| `resources/subscribe` | `resource subscribe <ref> <uri>` | first-class |
| `resources/unsubscribe` | `resource unsubscribe <ref> <uri>` | first-class |
| `prompts/list` | `prompt list <ref>` | first-class |
| `prompts/get` | `prompt get <ref> <name> --key value` | first-class |
| `completion/complete` | — | pending |
| `logging/setLevel` | `logging set-level <ref> <level>` | first-class |
| any other / future method | `raw <ref> <method> --params …` | passthrough |

## Server-initiated requests (client → server response)

| Method | Default handler | Override |
|---|---|---|
| `roots/list` | returns `--root <path>` paths | `--root` flag |
| `elicitation/create` (form) | TTY prompt → Accept; non-TTY → Decline | `--no-interactive` to always decline |
| `elicitation/create` (url) | prints URL → Accept | n/a |
| `sampling/createMessage` | method-not-found | `--sampling-handler <CMD>` plugin |
| `logging/message` | routed via `tracing` and emitted from `mcpal watch` | `RUST_LOG=…` |
| `notifications/progress` | emitted by `mcpal watch` | n/a |
| `notifications/resources/updated` | emitted by `mcpal watch` | n/a |
| `notifications/{tools,prompts,resources}/list_changed` | emitted by `mcpal watch` | n/a |
| `notifications/cancelled` | emitted by `mcpal watch` | n/a |

## Transports

| Transport | Status |
|---|---|
| stdio (child process) | first-class |
| Streamable HTTP (rustls) | first-class |
| SSE (legacy 2024-11-05) | not enabled |
| WebSocket | not enabled |

## Auth

| Mode | Status |
|---|---|
| Bearer (inline / env / keyring) | first-class |
| OAuth 2.1 + PKCE + DCR (RFC 7591) | first-class |
| OAuth 2.1 + CIMD (client metadata URL, SEP-991) | pending |
| Custom HTTP headers | first-class via `ServerSpec::Http { headers }` |
| mTLS | pending |
