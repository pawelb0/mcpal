# Contributing to mcpal

Issues + PRs welcome. Two notes before you open one.

## Versioning

mcpal follows [SemVer](https://semver.org/).

- **Patch** (`0.x.Y`): bug fixes; doc-only changes; non-breaking polish. CHANGELOG entry under `Fixed` or `Changed`.
- **Minor** (`0.X.0`): new commands, new flags, new error codes, new transports. CHANGELOG entry under `Added`.
- **Major** (`X.0.0`): breaking changes to the config schema, removed CLI flags, renamed verbs. Held off so far.

## Release ritual

1. Move `## [Unreleased]` block in `CHANGELOG.md` to a new `## [X.Y.Z]` heading; leave a fresh empty `## [Unreleased]` on top. (Git tag carries the date.)
2. Bump the workspace version in `Cargo.toml` (one line under `[workspace.package]`).
3. `cargo fmt --all && cargo clippy -p mcpal --all-targets -- -D warnings && cargo test -p mcpal --bin mcpal` — all clean.
4. `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration` — passes.
5. Commit (`release vX.Y.Z`), tag (`git tag vX.Y.Z`), push (`git push && git push --tags`).

cargo-dist + `.github/workflows/release.yml` build artifacts. `.github/workflows/deb.yml` ships the .deb.

## Commit messages

Plain English, ≤50 chars. No `feat:` / `fix:` / `chore:` prefix. Body only when the *why* isn't obvious from the diff. No machine-generated co-author trailers.

## Tests

Unit + integration both gate the release ritual. Adding a new flag means an integration assertion. Adding a new error code means an `exit.rs` pattern + a `book/src/error-codes.md` entry.
