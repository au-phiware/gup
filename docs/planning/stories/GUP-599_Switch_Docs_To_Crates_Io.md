---
implementer: haiku
implementer-reason:
  mechanical text replacements at listed lines, then one external build command
  that passes or fails
touches:
  [
    README.md,
    docs/tutorials/01_getting_started.md,
    docs/EGUI_INTEGRATION.md,
    docs/BEVY_INTEGRATION.md,
  ]
---

# GUP-599: Switch the Docs to the crates.io Release

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 📋 Planned
**Created**: 2026-11-02

## Context

Until 0.1.0 is on crates.io, the docs tell users to depend on the git
repository. This is the last story of track N3, and it runs only after the owner
has published all seven crates (GUP-598's checklist, step 6). On 2026-11-02 the
git lines were:

- `README.md` lines 52–58: "Gup is not on crates.io yet; release 0.1 will be.
  Add it from git to `Cargo.toml`:" and
  `gup = { git = "https://github.com/au-phiware/gup" }`.
- `docs/tutorials/01_getting_started.md` lines 33–40: the same dependency, and
  "Release 0.1 is not on crates.io yet, which is why the dependency names the
  repository."
- `docs/EGUI_INTEGRATION.md` lines 11–12 and `docs/BEVY_INTEGRATION.md` lines
  18–19: `gup-egui`, `gup-bevy` and `gup-core` as git dependencies.

GUP-597 may have moved the README lines; find them by their text.

## Read First

- The four places above.
- `docs/RELEASING.md` (GUP-598) step 8.

## User Story

> "As a new user, I want the install instructions to name the published crates,
> so that I get a released version and not whatever is on `main`."

## Acceptance Criteria

### AC1: The published version exists

- [ ] `curl -sf https://crates.io/api/v1/crates/gup/0.1.0` succeeds, and so do
      the same URLs for `gup-core`, `gup-egui` and `gup-bevy`. If any fails,
      stop: the owner has not published yet.

### AC2: The docs

- [ ] README: the sentence becomes "Add Gup to `Cargo.toml`:" and the line
      becomes `gup = "0.1"`. The browser paragraph (GUP-597) uses
      `gup = { version = "0.1", default-features = false, features = ["web", "svg"] }`.
- [ ] Tutorial 1: the line becomes `gup = "0.1"`. The "Release 0.1 is not on
      crates.io yet…" sentence is removed, and the paragraph keeps its other
      sentences ("There is nothing else to add…").
- [ ] The egui guide: `gup-egui = "0.1"` and `gup-core = "0.1"`. The Bevy guide:
      `gup-core = "0.1"` and `gup-bevy = "0.1"`.
- [ ] `git grep -n 'git = "https://github.com/au-phiware/gup"'` prints nothing
      outside `docs/planning/` and `docs/archive/`.
- [ ] `cargo test --doc -p gup` passes (the README and tutorials are doctests).
      `cargo test --doc -p gup-egui` and `cargo test --doc -p gup-bevy` pass
      (the guides are compiled, GUP-535).

### AC3: An external project builds from crates.io

- [ ] In a new directory outside the repository (under
      `~/.cache/gup/test-scratch/`, never `/tmp`), run
      `cargo new release-check`. Add `gup = "0.1"`. Paste the README's in-memory
      Quick Start block as `src/main.rs`. Then `cargo run --release` with
      `scripts/headless_env.sh --run`. It prints nothing and exits 0. Paste the
      `cargo tree -d` output (duplicate dependencies) and the build time into
      the Implementation Summary.

## Decisions

- **Caret requirements in docs** (`"0.1"`), not exact ones: users take patches.
  The exact `=` pins are only between Gup's own crates.
- **No version badge** in the README. A crates.io badge would load from a
  third-party image host; the plain dependency line says the same.

## Escalation Triggers

- AC1 fails (not published yet).
- AC3 fails to build or run. That is a release bug: report the error before
  changing anything.

## Out of Scope

- Anything else in the README (GUP-597).
- A 0.1.1.

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] AC1 check; the four edits (AC2). Commit.
- [ ] Doctests; the external build (AC3). DoD commands.

## Dependencies

### Prerequisite Stories

- GUP-598: Release checklist 📋, and the owner's publish (its checklist, step
  6).
- GUP-597: README for release 0.1 📋.

## Testing Strategy

- **Doctests**: `gup`, `gup-egui`, `gup-bevy`.
- **External build**: AC3.

## Success Metrics

- [ ] A user following the README gets the published 0.1 and a working chart.

## Risk Assessment

- **Low**: the external build resolves a newer dependency than the workspace
  lock (no lockfile in a new project) that breaks the build. _Mitigation_: AC3
  is exactly that check; report a failure as a release bug.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: the doctests of AC2
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: not affected
- [ ] Rendered output verified by eye: not applicable
- [ ] `mask wasm-browser`: not needed, docs only
- [ ] WASM size: unchanged, no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
