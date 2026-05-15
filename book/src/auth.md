# Auth deep dive

Three auth modes: inline bearer, `BearerEnv`, OAuth 2.1. Plus the
`MCPAL_BEARER` env one-shot. Tokens live in the OS keyring, not in
`config.toml`.

## Bearer tokens

Store the token in the keyring:

```bash
mcpal auth login github --bearer ghp_xxx
mcpal tool list github
```

Lookups walk this precedence on each call:

1. `AuthSpec::Bearer { token }` set explicitly on the config entry
   (rare; tokens shouldn't be written to TOML).
2. `AuthSpec::BearerEnv { env }` — reads the env var named.
3. OAuth blob (see below).
4. Bearer keyring entry under `bearer:<ref>`.
5. `MCPAL_BEARER`.

Removing:

```bash
mcpal auth logout github     # wipes bearer + oauth for this ref
mcpal auth status github     # shows {bearer: true|false, oauth: …}
```

Via env (CI):

```bash
MCPAL_BEARER=ghp_xxx mcpal tool list github
```

Read from stdin:

```bash
secret-tool get … | mcpal auth login github --bearer -
```

## OAuth 2.1 + PKCE + DCR

For servers that authenticate end users. mcpal runs the flow itself.

```bash
mcpal server add notion --http https://mcp.notion.com/v1
mcpal auth login notion --oauth
mcpal tool list notion
```

Steps:

1. Metadata discovery. `GET /.well-known/oauth-protected-resource`
   (RFC 9728) on first 401, then the AS metadata at the linked URL.
2. Dynamic Client Registration (RFC 7591) if no `client_id` is cached:
   POST `registration_endpoint` with
   `redirect_uris=["http://127.0.0.1:<random>/callback"]` and
   `token_endpoint_auth_method="none"`. The issued `client_id` (and
   `client_secret`, if any) goes to the keyring under `client:<ref>`.
3. PKCE: S256 code challenge.
4. Loopback callback: mcpal binds `127.0.0.1:0`, captures the port,
   builds the `redirect_uri`, opens the browser to the authorize URL.
5. Token exchange with the captured code + verifier. Tokens go to
   `oauth:<ref>` as a JSON `StoredCredentials`.

Refresh:

```bash
mcpal auth refresh notion
```

Reads the stored OAuth blob and exchanges the refresh token. If the
refresh token has also expired, you get E0004; re-run
`mcpal auth login notion --oauth`.

No-browser mode:

```bash
mcpal auth login notion --oauth --no-browser
```

## Where each token lives

All entries are under keyring service `mcpal`.

| Key | Contents |
|---|---|
| `bearer:<ref>` | raw bearer string |
| `oauth:<ref>` | JSON `StoredCredentials` (rmcp) |
| `client:<ref>` | DCR `{client_id, client_secret?}` |

`mcpal doctor` round-trips a canary entry to verify the keyring is
accessible. On Linux, mcpal uses `linux-native-sync-persistent`, which
talks to `org.freedesktop.secrets`; if no Secret Service daemon is
running the round-trip fails.

## Bearer or OAuth?

Bearer is one moving part fewer. OAuth is required when the server
gates on user identity, and its refresh tokens save re-pasting on
expiry. If the server supports OAuth, use it.
