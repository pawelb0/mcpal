# Phase 1 docs polish + release hygiene — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lift mcpal's project surface to slumber-tier polish — dedicated install chapter, sidebar reordered so concepts come before how-tos, CHANGELOG + CONTRIBUTING at repo root, README install section slimmed.

**Architecture:** Seven additive tasks, all documentation + repo-meta. No CLI / library / behaviour changes. Each task ships independently with one focused commit.

**Tech Stack:** mdbook (`{{#include …}}` directive), Keep-a-Changelog format, GitHub Flavoured Markdown.

**Spec:** `docs/superpowers/specs/2026-05-22-phase1-docs-polish-design.md`.

---

## File Structure

| File | Role |
|---|---|
| `CHANGELOG.md` (new, repo root) | Keep-a-Changelog. Backfilled `[0.2.0]` + `[0.1.1]` + `[0.1.0]`. `[Unreleased]` stays empty post-release. |
| `CONTRIBUTING.md` (new, repo root) | Release process + versioning rules. One page. |
| `book/src/install.md` (new) | Install matrix — per-OS one-liners, completions, verify steps. Single source of truth. |
| `book/src/changelog.md` (new) | One-line stub: `{{#include ../../CHANGELOG.md}}` so book mirrors repo CHANGELOG. |
| `book/src/SUMMARY.md` | Reordered — Concepts promoted, Install + Changelog added. |
| `book/src/intro.md` | Add reading-path line at end of pitch. |
| `README.md` | `## Install` shrunk to 4 lines + book link. Changelog badge added. |
| `Cargo.toml` (workspace) | Version bump 0.1.1 → 0.2.0 (Task 7 only). |

---

### Task 1: Add `CHANGELOG.md` with backfilled history

**Files:**
- Create: `CHANGELOG.md` (repo root)

- [ ] **Step 1: Create `CHANGELOG.md`**

```markdown
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
```

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "add CHANGELOG with backfilled history"
```

---

### Task 2: Add `CONTRIBUTING.md` with release process

**Files:**
- Create: `CONTRIBUTING.md` (repo root)

- [ ] **Step 1: Create `CONTRIBUTING.md`**

```markdown
# Contributing to mcpal

Issues + PRs welcome. Two notes before you open one.

## Versioning

mcpal follows [SemVer](https://semver.org/).

- **Patch** (`0.x.Y`): bug fixes; doc-only changes; non-breaking polish. CHANGELOG entry under `Fixed` or `Changed`.
- **Minor** (`0.X.0`): new commands, new flags, new error codes, new transports. CHANGELOG entry under `Added`.
- **Major** (`X.0.0`): breaking changes to the config schema, removed CLI flags, renamed verbs. Held off so far.

## Release ritual

1. Move `## [Unreleased]` block in `CHANGELOG.md` to a new `## [X.Y.Z] — YYYY-MM-DD` heading; leave a fresh empty `## [Unreleased]` on top.
2. Bump the workspace version in `Cargo.toml` (one line under `[workspace.package]`).
3. `cargo fmt --all && cargo clippy -p mcpal --all-targets -- -D warnings && cargo test -p mcpal --bin mcpal` — all clean.
4. `MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration` — passes.
5. Commit (`release vX.Y.Z`), tag (`git tag vX.Y.Z`), push (`git push && git push --tags`).

cargo-dist + `.github/workflows/release.yml` build artifacts. `.github/workflows/deb.yml` ships the .deb.

## Commit messages

Plain English, ≤50 chars. No `feat:` / `fix:` / `chore:` prefix. Body only when the *why* isn't obvious from the diff. No machine-generated co-author trailers.

## Tests

Unit + integration both gate the release ritual. Adding a new flag means an integration assertion. Adding a new error code means an `exit.rs` pattern + a `book/src/error-codes.md` entry.
```

- [ ] **Step 2: Commit**

```bash
git add CONTRIBUTING.md
git commit -m "add CONTRIBUTING with release process"
```

---

### Task 3: Add `book/src/install.md` chapter

**Files:**
- Create: `book/src/install.md`

- [ ] **Step 1: Create `book/src/install.md`**

```markdown
# Install

Pick whichever package manager is already on your machine. Every method drops the same binary; the difference is who's curating the metadata.

## macOS / Linux — Homebrew

```bash
brew tap pawelb0/tap
brew install pawelb0/tap/mcpal
```

## Debian / Ubuntu — `.deb`

```bash
curl -fsSLO https://github.com/pawelb0/mcpal/releases/latest/download/mcpal_amd64.deb
sudo dpkg -i mcpal_amd64.deb
```

## Any platform — `cargo`

```bash
cargo install --git https://github.com/pawelb0/mcpal --path crates/mcpal
```

Needs a Rust toolchain (rustup recommended). Builds against the current `main`.

## Prebuilt binary — `curl | sh`

```bash
curl -fsSL https://raw.githubusercontent.com/pawelb0/mcpal/main/dist/install.sh | sh
```

Drops the binary into `$HOME/.local/bin`. Read the script first if you're cautious — it's short.

## Windows

```powershell
cargo install --git https://github.com/pawelb0/mcpal --path crates/mcpal
```

A prebuilt MSI is on the roadmap. Until then, `cargo install` is the supported path.

## Shell completions

```bash
mcpal completion bash > ~/.local/share/bash-completion/completions/mcpal
mcpal completion zsh  > ~/.zsh/completions/_mcpal       # ensure dir is on $fpath
mcpal completion fish > ~/.config/fish/completions/mcpal.fish
mcpal completion powershell >> $PROFILE
```

Re-source your shell after writing the completion file.

## Verify

```bash
mcpal --version
mcpal debug doctor
```

`debug doctor` runs a quick local sanity check — config path, keyring backend, presence of `npx` for stdio servers.

Curious what shipped in the version you got? See the [Changelog](./changelog.md).
```

- [ ] **Step 2: Commit**

```bash
git add book/src/install.md
git commit -m "book: dedicated install chapter"
```

---

### Task 4: Add `book/src/changelog.md` page

**Files:**
- Create: `book/src/changelog.md`

- [ ] **Step 1: Create `book/src/changelog.md`**

```markdown
{{#include ../../CHANGELOG.md}}
```

Just the mdbook include directive. The book renders the repo CHANGELOG verbatim so the two never drift.

- [ ] **Step 2: Commit**

```bash
git add book/src/changelog.md
git commit -m "book: changelog page mirrors repo file"
```

---

### Task 5: Reorder `SUMMARY.md` + tighten intro

**Files:**
- Modify: `book/src/SUMMARY.md`
- Modify: `book/src/intro.md`

- [ ] **Step 1: Rewrite `book/src/SUMMARY.md`**

```markdown
# Summary

[Introduction](./intro.md)
[Install](./install.md)
[Your first MCP call](./getting-started.md)

# Concepts

- [Concepts](./concepts.md)

# How-to guides

- [Recipes](./recipes.md)
- [Authenticate to an HTTP server](./auth.md)
- [Interactive TUI](./tui.md)
- [UI-rich MCP servers](./ui.md)
- [Script around mcpal](./scripting.md)
- [Troubleshoot](./troubleshooting.md)

# Reference

- [Protocol compliance matrix](./protocol-matrix.md)
- [Error codes](./error-codes.md)
- [Changelog](./changelog.md)
```

Changes from prior `SUMMARY.md`:
- `Install` added as a prefix entry (unnumbered, above `getting-started`).
- `Your first MCP call` promoted to a prefix entry (it was the lone "Tutorial" section — now it sits in the linear reading path).
- `Concepts` is its own section, above `How-to guides` (Diátaxis allows Explanation anywhere — slumber reads better with it before recipes).
- `Changelog` added under `Reference`.

- [ ] **Step 2: Add reading-path line to `book/src/intro.md`**

Find the existing pitch blockquote in `book/src/intro.md`. Immediately after the closing `>` of the blockquote (and before the code block that starts with `mcpal server list --all`), insert:

```markdown
New here? **[Install](./install.md) → [Your first MCP call](./getting-started.md) → [Concepts](./concepts.md)**.
```

- [ ] **Step 3: (optional) `mdbook build book`**

If `mdbook` is on PATH, run `mdbook build book 2>&1 | tail -20` and confirm zero broken-link warnings. If not, skip — CI's `book.yml` validates on push.

- [ ] **Step 4: Commit**

```bash
git add book/src/SUMMARY.md book/src/intro.md
git commit -m "book: reorder sidebar, surface concepts earlier"
```

---

### Task 6: Shrink README install + add changelog badge

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Replace `## Install` block**

Find the existing `## Install` heading in `README.md` and replace its body (from `macOS / Linux via Homebrew:` down to the curl|sh block — the four current install sections) with:

```markdown
## Install

| Platform | One-liner |
|---|---|
| macOS / Linux | `brew install pawelb0/tap/mcpal` |
| Debian / Ubuntu | `curl -fsSLO …/releases/latest/download/mcpal_amd64.deb && sudo dpkg -i mcpal_amd64.deb` |
| Any (Rust) | `cargo install --git https://github.com/pawelb0/mcpal --path crates/mcpal` |
| Windows | `cargo install --git https://github.com/pawelb0/mcpal --path crates/mcpal` |

Full guide — `.deb` URL, `curl | sh` installer, shell completions, verify — in the [Install chapter](https://pawelb0.github.io/mcpal/install.html).
```

(The single `…` keeps the table cell readable; the Install chapter has the full URL.)

- [ ] **Step 2: Add `changelog` badge to the existing row**

In the `<p align="center"> … </p>` block holding the four badges, insert this `<a>` between the Latest-release badge and the License badge:

```html
<a href="CHANGELOG.md"><img src="https://img.shields.io/badge/changelog-keep--a--changelog-blue" alt="Changelog"></a>
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "shrink README install, badge the changelog"
```

---

### Task 7: Cut v0.2.0

**Files:**
- Modify: `Cargo.toml` (workspace) — `version = "0.1.1"` → `"0.2.0"`
- Modify: `CHANGELOG.md` (no edit — `[0.2.0]` block already dated 2026-05-22 from Task 1)

- [ ] **Step 1: Bump workspace version**

In `/Users/pawelb/workspace/mcpal/Cargo.toml`, find the `[workspace.package]` section. Change:

```toml
version = "0.1.1"
```

to:

```toml
version = "0.2.0"
```

- [ ] **Step 2: Verify the bump propagated**

```bash
cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | select(.name=="mcpal") | .version'
```

Expected: `0.2.0`.

- [ ] **Step 3: Run the full release-gate checks**

```bash
cargo fmt --all -- --check
cargo clippy -p mcpal --all-targets -- -D warnings
cargo test -p mcpal --bin mcpal
MCPAL_INTEGRATION_TESTS=1 cargo test -p mcpal --test integration -- --nocapture
```

All four exit 0.

- [ ] **Step 4: Commit and tag**

```bash
git add Cargo.toml
git commit -m "release v0.2.0"
git tag v0.2.0
```

Do NOT push yet — pushing is a human decision once CI on `main` is green. The plan stops at "tag created locally".

---

## Verification

After Tasks 1–7:

- `git log --oneline -10` shows 6 polish commits + the `release v0.2.0` commit, all plain English, no scoped prefixes.
- Sidebar in the rendered book (or in `SUMMARY.md`) reads
  Introduction → Install → Your first MCP call → Concepts → How-to → Reference → Changelog.
- `book/src/changelog.md` is one line (`{{#include …}}`); rendered page contains the full CHANGELOG.
- README `## Install` section is four lines + a link.
- README badge row has 5 badges including changelog.
- `cargo metadata` reports `mcpal 0.2.0`.
- `CHANGELOG.md`'s `[Unreleased]` block is empty.
- `git tag` includes `v0.2.0`.

End-to-end smoke (human, post-push):

```bash
git push origin main
git push origin v0.2.0
gh run watch                  # release.yml + deb.yml build artifacts
gh release view v0.2.0        # confirm the body picks up the CHANGELOG block
```

---

## Self-Review

**1. Spec coverage**

| Spec deliverable | Task |
|---|---|
| Sidebar reorder + new chapters | 5 |
| `book/src/install.md` | 3 |
| `CHANGELOG.md` (repo root) | 1 |
| `book/src/changelog.md` (include) | 4 |
| `CONTRIBUTING.md` (repo root) | 2 |
| README delta | 6 |
| Intro reading-path line | 5 |
| Release ritual gets exercised | 7 (added to verify CHANGELOG is honest) |

**2. Placeholder scan** — no TBDs. Every step is concrete content or an exact command.

**3. Type consistency** — N/A (docs only).

**4. Tasks order** — Tasks 1–6 can ship independently; Task 7 (release) must wait for 1 because it needs the `[0.2.0]` block to exist. Tasks 3 + 4 are pre-req for Task 5 (SUMMARY references both files). Tasks 1, 2, 6 are independent.
