# Authenticate to an HTTP server

How to attach credentials to a remote MCP server, by mode. Stdio
servers inherit the parent shell's env and don't take credentials;
this page is HTTP only.

mcpal supports three auth modes, plus a one-shot env override:

| Mode | Where the secret lives | Best for |
|---|---|---|
| Inline bearer | OS keyring under `bearer:<ref>` | personal access tokens, CI service tokens |
| `BearerEnv` | environment variable named in `config.toml` | secrets injected by another tool (sops, vault, GitHub Actions) |
| OAuth 2.1 + PKCE + DCR | OS keyring under `oauth:<ref>` | user-facing services that authenticate humans (Notion, Linear, Atlassian) |
| `MCPAL_BEARER` | environment variable | one-shot scripts that don't want to touch the keyring |

The OS keyring is Keychain on macOS, Secret Service on Linux,
Credential Manager on Windows. mcpal never writes secrets to
`config.toml`.

## Bearer tokens

Store the token in the keyring:

```bash
mcpal auth login github --bearer ghp_xxx
mcpal tool list github
```

Read from stdin (good when the token comes from another tool):

```bash
secret-tool get service mcp-github | mcpal auth login github --bearer -
```

Use a different env var per call:

```bash
MCPAL_BEARER=ghp_xxx mcpal tool list github
```

Reference an env var from `config.toml` (lets one config travel
between machines without baking the secret in):

```toml
[server.github]
transport = "http"
url = "https://api.githubcopilot.com/mcp/"
auth = { type = "bearer_env", env = "GITHUB_MCP_TOKEN" }
```

Credentials are resolved per call in this order:

1. `AuthSpec::Bearer { token }` (rare; avoid writing tokens to TOML).
2. `AuthSpec::BearerEnv { env }`.
3. OAuth blob in the keyring under `oauth:<ref>`.
4. Bearer keyring entry under `bearer:<ref>`.
5. `MCPAL_BEARER` env var.

Inspect what's stored:

```bash
mcpal auth status github
# {ref: github, bearer: true, oauth: false}
```

Remove credentials:

```bash
mcpal auth logout github
```

## OAuth 2.1 + PKCE + DCR

For services that authenticate human users — Notion, Linear, Atlassian,
and so on. mcpal runs the full OAuth 2.1 authorization-code flow with
PKCE and Dynamic Client Registration; you do not need to pre-register
mcpal in a developer console.

### The shortest path

```bash
mcpal server add notion --http https://mcp.notion.com/v1
mcpal auth login notion --oauth
# → browser opens, you click "allow", terminal prints "authorized"
mcpal tool list notion
```

That's it for happy path. The rest of this section explains what
actually happened so you can debug it when it doesn't.

### What `mcpal auth login --oauth` does

Five RFCs interlock here: **OAuth 2.1** (the framework), **RFC 7636
PKCE** (protects the auth code in transit), **RFC 7591 Dynamic Client
Registration** (lets mcpal register itself without a developer console
trip), **RFC 8414 AS metadata** (tells mcpal where the endpoints are),
and **RFC 9728 Protected Resource Metadata** (the MCP server points at
its authorization server).

The flow, step by step:

**1. Resource metadata probe.** mcpal sends a single GET to the MCP
server's URL. If the server responds with a `401` and a
`WWW-Authenticate: Bearer resource_metadata="<url>"` header, mcpal
follows the link. Otherwise it falls back to the well-known path:

```
GET /.well-known/oauth-protected-resource
→ {
    "resource": "https://mcp.notion.com/v1",
    "authorization_servers": ["https://mcp.notion.com"]
  }
```

The response tells mcpal which authorization server (AS) gates this
resource.

**2. AS metadata discovery.** mcpal asks that AS for its endpoints:

```
GET https://mcp.notion.com/.well-known/oauth-authorization-server
→ {
    "issuer": "https://mcp.notion.com",
    "authorization_endpoint": ".../authorize",
    "token_endpoint":         ".../token",
    "registration_endpoint":  ".../register",
    "response_types_supported": ["code"],
    "code_challenge_methods_supported": ["S256"],
    "token_endpoint_auth_methods_supported": ["none"]
  }
```

If the server doesn't ship `oauth-authorization-server`, mcpal also
tries `/.well-known/openid-configuration` (OpenID Connect-style).

**3. Dynamic Client Registration.** Most public MCP servers don't
want you to pre-register a client; they accept RFC 7591 DCR. mcpal
POSTs:

```
POST .../register
{
  "client_name": "mcpal",
  "redirect_uris": ["http://127.0.0.1:<random>/callback"],
  "grant_types": ["authorization_code", "refresh_token"],
  "response_types": ["code"],
  "token_endpoint_auth_method": "none"
}
```

The AS replies with a `client_id` (and optionally `client_secret`).
mcpal stores both in the keyring under `client:<ref>` so the next
login on the same machine reuses the same client.

**4. PKCE setup.** mcpal generates a random `code_verifier` and
derives `code_challenge = base64url(sha256(code_verifier))`. The
challenge goes into the authorize URL; the verifier stays on disk
until the token exchange.

**5. Loopback callback.** mcpal binds a TCP listener on
`127.0.0.1:0`, captures the assigned port, and prints the authorize
URL:

```
.../authorize?
  response_type=code&
  client_id=<from step 3>&
  redirect_uri=http://127.0.0.1:54321/callback&
  state=<csrf token>&
  code_challenge=<from step 4>&
  code_challenge_method=S256&
  resource=https%3A%2F%2Fmcp.notion.com%2Fv1
```

mcpal opens that URL in the default browser unless you pass
`--no-browser`. The user clicks "allow" (or rejects). The AS
redirects the browser to `http://127.0.0.1:54321/callback?code=…&state=…`.
mcpal's listener catches the request, validates `state` against the
CSRF token it generated, and reads the `code`.

**6. Token exchange.** mcpal POSTs the code plus the PKCE verifier:

```
POST .../token
grant_type=authorization_code&
code=<from step 5>&
redirect_uri=http://127.0.0.1:54321/callback&
client_id=<from step 3>&
code_verifier=<from step 4>
```

The AS validates `code_verifier` matches `code_challenge`, replies
with an access token and (usually) a refresh token:

```
{
  "access_token": "...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "..."
}
```

**7. Store.** mcpal writes the response plus `token_received_at` into
the keyring under `oauth:<ref>`. Every subsequent `mcpal <verb>
<ref>` reads that blob and sends `Authorization: Bearer
<access_token>`.

### Refresh

Access tokens are short-lived (an hour is typical). Before each call,
mcpal checks `now() + 30s >= token_received_at + expires_in`. If yes
it sends the refresh token to the AS:

```
POST .../token
grant_type=refresh_token&
refresh_token=<stored>
```

…and replaces the stored blob. You can also refresh by hand:

```bash
mcpal auth refresh notion
```

If the refresh token has also expired or been revoked, mcpal surfaces
`E0004`; re-run `mcpal auth login notion --oauth`.

### Variations

**No browser.** Useful over SSH:

```bash
mcpal auth login notion --oauth --no-browser
# mcpal prints the URL; you open it on another machine
```

The loopback callback still has to reach mcpal. If the auth happens
on a different machine, that's harder; either tunnel the port back
(`ssh -L 54321:127.0.0.1:54321 ...` then visit
`http://127.0.0.1:54321/callback?...` locally) or run the whole flow
on the workstation and copy `~/Library/Application
Support/mcpal/config.toml` plus the keyring entry.

**Pre-registered client.** Some services don't support DCR. Add the
client_id and secret directly to the keyring (advanced; see
`mcpal/oauth` source for the JSON shape).

**Self-signed AS.** Not currently supported; rmcp uses rustls with
the system trust store.

### What `mcpal debug doctor` checks for auth

`mcpal debug doctor` reports per-server:

- `bearer_stored` — is there a `bearer:<ref>` keyring entry?
- `oauth_stored` — is there an `oauth:<ref>` keyring entry?
- `oauth_access_token_present` — does the blob contain an
  `access_token` field? (False after a failed refresh.)

It also round-trips a canary key to confirm the keyring is alive.

### Where each token lives

All entries are under keyring service `mcpal`.

| Key | Contents |
|---|---|
| `bearer:<ref>` | raw bearer string |
| `oauth:<ref>` | JSON `StoredCredentials` (rmcp): `{ client_id, token_response, granted_scopes, token_received_at }` |
| `client:<ref>` | DCR result `{ client_id, client_secret? }` |

On Linux the keyring lives in Secret Service
(`org.freedesktop.secrets`); the `linux-native-sync-persistent`
feature talks to it directly. Headless Linux without a Secret Service
daemon needs `MCPAL_BEARER` or `gnome-keyring-daemon --start`.

## Choosing a mode

- **Personal access token for one service** → bearer in the keyring.
- **Token rotated by another tool / available as `$VAR`** →
  `BearerEnv`.
- **Server authenticates end users (Notion, Linear, etc.)** → OAuth.
- **One-shot in a script that shouldn't touch the keyring** →
  `MCPAL_BEARER=… mcpal …`.
