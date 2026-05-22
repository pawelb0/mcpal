# Phase 1 — docs polish + release hygiene

Status: approved · 2026-05-22

## Context

Slumber (https://slumber.lucaspickering.me/) reads as a mature
project — one-shot install path, dedicated install chapter, key
concepts taught before how-to chapters, CHANGELOG visible from the
landing page, 63 numbered releases. mcpal already has the Diátaxis
chapters but the reading path is suboptimal (Explanation comes last)
and the project lacks a CHANGELOG / contributing guide.

This phase brings the surface up to slumber's bar without touching
behaviour. Two-day scope. Phase 2 — the `mcpal.yml` collection file —
gets its own spec.

## Goals

1. New reader lands on the book and follows a clean
   Intro → Install → First call → Concepts → How-to → Reference path.
2. Install matrix lives in one place (book), README links to it.
3. Project carries a CHANGELOG, `[Unreleased]` block, and a
   one-page CONTRIBUTING with the release process.
4. README + book sidebars surface the changelog.

Non-goals:
- New CLI surface, new commands, new error codes.
- Slumber's themes, telemetry framing, or Python SDK.
- Phase 2 collection file (`mcpal.yml`) — separate spec.

## Deliverables

### 1. Sidebar reorder + new chapters

`book/src/SUMMARY.md` reorganised:

```
[Introduction](./intro.md)
[Install](./install.md)                  # NEW
[Your first MCP call](./getting-started.md)

# Concepts
- [Concepts](./concepts.md)              # PROMOTED from Explanation

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
- [Changelog](./changelog.md)            # NEW
```

`Introduction` and `Install` are unnumbered prefix entries; everything
else stays grouped per Diátaxis.

### 2. `book/src/install.md`

Single source of truth for install. Contents:

- Per-OS one-liner (Homebrew, .deb, cargo, curl|sh, Windows).
- Shell completion install for bash / zsh / fish / PowerShell.
- `mcpal --version` + `mcpal debug doctor` verify section.
- Link to CHANGELOG for what's in the version they got.

README shrinks: keep the Quickstart code blocks, replace the current
`## Install` body with a 4-line table + "Full guide: book/install.md".

### 3. `CHANGELOG.md` (repo root)

Keep-a-Changelog format. Backfill from existing tags:

- `[Unreleased]` — every commit since v0.1.1 (server add one-liner,
  E0013, --help examples, README rework, AWS-CLI drop).
- `[0.1.1]` — TUI, .deb, mcp-ui inspect, elicitation/sampling traces,
  Homebrew tap fix.
- `[0.1.0]` — initial release: stdio + HTTP, OAuth 2.1, discovery,
  tool/resource/prompt, JMESPath, raw, watch.

### 4. `book/src/changelog.md`

Stub that pulls in the repo CHANGELOG via mdbook's `{{#include
../../CHANGELOG.md}}` so book + repo never drift.

### 5. `CONTRIBUTING.md` (repo root)

Short — release process only (no contributor agreement, no
maintainer list yet). Three sections:

- Versioning: patch / minor / major rules with concrete CLI-change
  examples.
- Release ritual: bump `Cargo.toml`, move `[Unreleased]`, tag,
  push — five steps.
- Tests: `cargo fmt --check`, `clippy -D warnings`, unit + integration
  must pass before tagging.

### 6. README delta

- Drop `## Install` body, replace with one-line per OS + book link.
- Add a `changelog` badge to the existing row (between release +
  license).

### 7. Intro page link to install

`book/src/intro.md` already opens with the pitch. After the
blockquote, add: `New here? → [Install](./install.md) → [Your first
MCP call](./getting-started.md).` Three-step reading path right in
the intro.

## Files

| File | Change |
|---|---|
| `book/src/SUMMARY.md` | Reorder + 2 new entries |
| `book/src/install.md` | NEW |
| `book/src/changelog.md` | NEW (one-line include) |
| `book/src/intro.md` | Add reading-path line |
| `CHANGELOG.md` | NEW |
| `CONTRIBUTING.md` | NEW |
| `README.md` | Shrink Install section + changelog badge |

## Verification

- `mdbook build book` succeeds (skip if mdbook not on PATH — CI
  `book.yml` validates). Confirm `{{#include}}` resolves: the rendered
  `changelog.md` page contains the CHANGELOG body.
- Every SUMMARY entry resolves to an existing file (no broken links).
- README badge link `href="CHANGELOG.md"` points at the new file.
- `mcpal --version` instructions in install.md run and exit 0.
- Spot-check the rendered sidebar order matches the spec.

## Rollout (small commits per project rule)

1. `add CHANGELOG with backfilled entries`
2. `add CONTRIBUTING with release process`
3. `book: dedicated install chapter`
4. `book: reorder sidebar, surface concepts earlier`
5. `book: changelog page (mdbook include)`
6. `README: shrink install, badge changelog`

Six commits, ~2 days. Each commit individually shippable.
