# Auth deep dive

mcpal handles authentication in three flavors. The secret always lives in
the OS keyring — never in `config.toml`.

## Bearer tokens

The simplest case. Store the token in the keyring:

```bash
mcpal auth login github --bearer ghp_xxx
mcpal tool list github
```

Lookups walk this precedence on each call:

1. `AuthSpec::Bearer { token }` set explicitly on the config entry (rare; you
   don't usually write tokens to TOML).
2. `AuthSpec::BearerEnv { env }` — reads the env var named.
3. OAuth blob (see below).
4. Bearer keyring entry under `bearer:<ref>`.
5. `MCPAL_BEARER` env var (last-resort one-shot).

Removing:

```bash
mcpal auth logout github     # wipes bearer + oauth for this ref
mcpal auth status github     # shows {bearer: true|false, oauth: …}
```

Bearer via env (good for CI):

```bash
MCPAL_BEARER=ghp_xxx mcpal tool list github
```

Read from stdin (good for vault → mcpal pipelines):

```bash
secret-tool get … | mcpal auth login github --bearer -
```

## OAuth 2.1 + PKCE + DCR

For servers that gate on user identity. mcpal runs the full flow itself
— no browser plugin, no Inspector tab needed.

```bash
mcpal server add notion --http https://mcp.notion.com/v1 --auth oauth
mcpal auth login notion --oauth
# → mcpal opens your default browser to the authorize URL
# → after consent, the browser redirects to http://127.0.0.1:<port>/callback
# → mcpal exchanges the code for access + refresh tokens
# → tokens persist in the keyring under oauth:<ref>
mcpal tool list notion
```

The flow internally:

1. **Metadata discovery.** `GET /.well-known/oauth-protected-resource`
   (RFC 9728) on first 401, then the AS metadata at the linked URL.
2. **Dynamic Client Registration** (RFC 7591) if no `client_id` cached:
   POST `registration_endpoint` with `redirect_uris=["http://127.0.0.1:<random>/callback"]`
   and `token_endpoint_auth_method="none"`. mcpal stores the issued
   `client_id` (and `client_secret`, if any) in the keyring under
   `client:<ref>`.
3. **PKCE.** S256 code challenge.
4. **Loopback callback.** mcpal binds `127.0.0.1:0`, captures the port,
   builds the `redirect_uri`, opens your browser to the authorize URL.
5. **Token exchange** with the captured code + verifier. Stored in
   keyring under `oauth:<ref>` as a JSON `StoredCredentials`.

Refresh, when the access token expires:

```bash
mcpal auth refresh notion
```

This rebuilds the `AuthorizationManager` from the keyring blob and calls
`refresh_token`. If the refresh token has also expired, you'll see an
E0004 — re-run `mcpal auth login --oauth notion`.

Headless / no-browser mode (prints the URL for you to open manually):

```bash
mcpal auth login notion --oauth --no-browser
```

## Where each token lives

| Account | Keyring service | Contents |
|---|---|---|
| `bearer:<ref>` | `mcpal` | raw bearer string |
| `oauth:<ref>` | `mcpal` | JSON-encoded `StoredCredentials` (rmcp) |

`mcpal doctor` round-trips a canary entry to verify the keyring is
accessible. If it fails on Linux, your session may have no Secret Service
bus running; mcpal uses `linux-native-sync-persistent`, which talks to
`org.freedesktop.secrets`.

## Choosing between bearer and OAuth

| | Bearer | OAuth |
|---|---|---|
| Setup steps | 1 (paste the token) | 1 (run `--oauth`, click consent) |
| Token rotation | manual | `mcpal auth refresh` |
| Identity | static service token | per-user, scoped |
| When to use | personal access token, lab/CI tokens | user-facing apps; servers that require DCR |

If the server supports OAuth, prefer it — refresh tokens save you from
re-pasting credentials on every expiry.
