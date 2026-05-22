# Collections

Drop a `mcpal.yml` at your project root, define saved tool calls and
profile-scoped variables, then run them by name:

```bash
mcpal run get-issue --profile prod
```

The collection file is plain YAML — check it into git, share it with
teammates, switch environments with `--profile`. Secrets stay out of
the file (`{{env.X}}` reads them at runtime from your shell or
`.envrc`).

## Minimal example

```yaml
default-profile: dev

profiles:
  dev:
    issue_id: ENG-1
    workspace: my-team
  prod:
    issue_id: ENG-999
    workspace: my-team

calls:
  get-issue:
    server: cursor:linear
    tool: get-issue
    params:
      id: "{{profile.issue_id}}"
      workspace: "{{profile.workspace}}"

  echo-token:
    server: gh
    tool: list_repos
    params:
      owner: "{{env.GH_USER}}"
```

`server` accepts any `<ref>` mcpal already understands — an alias from
`mcpal server add`, a `<source>:<name>` pair from `mcpal server
discover`, or an `https://` URL.

## Lookup

`mcpal run` walks from the current directory up to the filesystem
root looking for `mcpal.yml`. First hit wins. Override with
`--collection PATH`:

```bash
mcpal --collection ./mcpal.staging.yml run get-issue
```

If no file is found, `E0015`.

## Profiles

Pick which one is active with (in precedence order):

1. `--profile NAME` on the command line
2. `MCPAL_PROFILE` env var
3. `default-profile:` key in `mcpal.yml`
4. literal `default`

If the active name isn't a profile in the file, `E0016`.

Naming caveat: don't name a profile `default`. The literal string
`default` is the fallback marker — if both your `--profile` resolves
to `default` *and* the file declares `default-profile: dev`, the
file's `dev` wins. Pick any other name for the dev/staging baseline.

## Templating

Two namespaces, nothing else:

- `{{profile.X}}` — reads from the active profile's key/value map.
- `{{env.X}}` — reads from your OS environment.

Substitution happens before the call is sent. Unresolved variables
fail loudly with `E0014` (all misses listed in one message); the
request never reaches the server.

Escape literal `{{` with `{{{{`.

## Dry-run

```bash
mcpal run echo --dry-run
```

Prints the resolved `(server, tool, params)` JSON and exits without
opening a connection. Useful for CI assertions on what a call *would*
do.

## One-off overrides

```bash
mcpal run echo --params-override message="custom value"
```

`--params-override` overlays raw `K=V` pairs onto the rendered params
*after* templating. Repeatable. Useful for tweaking a saved call
without editing the file.
