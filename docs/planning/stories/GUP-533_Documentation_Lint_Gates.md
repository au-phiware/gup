---
implementer: haiku
implementer-reason:
  one crate attribute per crate and one mask command wired into all-check; every
  crate already passes (measured), so each AC is a command
touches: [maskfile.md, crates/gup-core/src/lib.rs, src/lib.rs]
---

# GUP-533: Documentation Lint Gates: `missing_docs` and `rustdoc -D warnings`

## Story Overview

**Initiative**: Strategic Review 2026-11 (N1 Docs and Tutorials) **Status**: ✅
Complete **Created**: 2026-10-24 **Completed**: 2026-10-24

## Context

The strategic review's track N1 asks for `#![warn(missing_docs)]` on
`gup-core`'s public items and a CI step that builds the docs with
`RUSTDOCFLAGS="-D warnings"`, so docs.rs never shows an undocumented item or a
broken link at release 0.1.

Measured on 2026-10-24 (this story's scribe):
`cargo rustc --lib -- -W missing_docs` reports **no** missing docs in
`gup-core`, `gup-core-macros`, `gup-text`, `gup-wgsl`, `gup-egui`,
`gup-visual-regression` or `gup`, and
`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` over those seven crates passes.
Nothing enforces either today: no crate sets the lint (`crates/gup-culling-lod`,
parked, has `#![deny(missing_docs)]`), and `mask doc` documents only the root
crate and is not part of `mask all-check`. The work is to lock in the current
state.

## Read First

- `maskfile.md` `## all-check` (the `concurrently` line) and `## doc`.
- The first 10 lines of each crate root: `src/lib.rs`,
  `crates/gup-core/src/lib.rs`, `crates/gup-core-macros/src/lib.rs`,
  `crates/gup-text/src/lib.rs`, `crates/gup-wgsl/src/lib.rs`,
  `gup-egui/src/lib.rs`, `gup-bevy/src/lib.rs`,
  `crates/gup-visual-regression/src/lib.rs`.

## User Story

> "As a user reading docs.rs, I want every public item documented and every link
> working, so that the reference never has holes."

## Acceptance Criteria

### AC1: The lint in every crate

- [x] Each of the eight crate roots above has `#![warn(missing_docs)]` right
      after its crate-level doc comment (before any `use` or `mod`).
- [x] `mask all-check` passes (clippy's `-D warnings` turns a missing doc into
      an error, so the attribute is a gate for every crate clippy lints).
- [x] Seeded proof, recorded and not committed: removing the doc comment of
      `gup_core::Plot::title` makes `mask all-check` fail with
      `missing documentation for a method`.

### AC2: `mask doc` builds every published crate's docs strictly

- [x] `mask doc` runs `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` with `-p`
      for each of `gup`, `gup-core`, `gup-core-macros`, `gup-text`, `gup-wgsl`,
      `gup-egui` and `gup-visual-regression`, and its description says so.
- [x] `mask all-check` runs `mask doc` as a new `concurrently` entry named
      `doc`, and the `all-check` description gains one sentence naming it.
- [x] Seeded proof, recorded and not committed: a broken intra-doc link
      (``[`Plot::nonexistent`]``) in `crates/gup-core/src/plot.rs` makes
      `mask all-check` fail in the `doc` entry.

## Decisions

- **`warn`, not `deny`**: clippy's `-D warnings` already fails the gate, and
  `warn` keeps `cargo build` usable while a contributor writes docs.
- **`gup-bevy` gets the attribute** but is not in `mask doc` or clippy (it is
  excluded from `all-check` for build cost; its own CI job builds it).
- **Default features** for `cargo doc`. docs.rs metadata and `--all-features`
  are track N3's release work.

## Escalation Triggers

- Any crate reports more than 10 missing docs once the attribute is added (the
  measurement said 0; `gup-bevy` was not measured): stop and report the list
  instead of writing docs for them.
- The `doc` entry adds more than 60 s to a warm `mask all-check`: record the
  time and report.

## Out of Scope

- Writing new documentation beyond what the lint demands.
- docs.rs configuration and a docs.rs build in CI (track N3).
- `gup-culling-lod` (parked).

## Technical Tasks

- [x] Start commit within 30 minutes.
- [x] Add the eight attributes; run `mask all-check`; commit.
- [x] Change `mask doc`; add it to `all-check`; run both seeded proofs; commit.

## Dependencies

### Prerequisite Stories

- None.

### Enables Stories

- Track N3's docs.rs build starts from a crate set that already documents
  cleanly.

## Testing Strategy

- **Gate**: `mask all-check` passes; the two seeded proofs fail it.

## Success Metrics

- [x] An undocumented public item or a broken doc link fails `mask all-check`.

## Risk Assessment

- **Low**: `#![warn(missing_docs)]` in the proc-macro crate also covers its
  exported macros. _Mitigation_: they are documented today (measured).

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output: not applicable, no visual change
- [x] `mask wasm-browser`: not needed, no layout, input or wasm change
- [ ] WASM size: unchanged, no library code change (attributes only)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `#![warn(missing_docs)]` after the crate-level doc comment in all eight crate
  roots: `gup`, `gup-core`, `gup-core-macros`, `gup-text`, `gup-wgsl`,
  `gup-egui`, `gup-bevy` and `gup-visual-regression`.
- `mask doc` now runs `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` with `-p`
  for the seven published-crate packages (default features).
- `mask all-check` gained a `doc` entry (`mask doc`) in its `concurrently` block
  and one sentence in its description.
- Files changed: `maskfile.md`, and the seven crate roots listed above.
- Verification: `mask all-check` exit 0 (warm, 63 s wall; `mask doc` alone takes
  about 1 s warm, about 27 s cold).
  `cargo test --workspace --exclude gup-bevy -- --test-threads=1`: 607 passed, 0
  failed, 9 ignored (pre-existing `#[ignore = "reason"]`).
  `cargo check --examples` exit 0. `cargo doc -p gup-bevy` exit 0 with no
  warnings.
- Seeded proofs (recorded, not committed; working tree restored):
  1. Removing the doc line of `Plot::title` in `crates/gup-core/src/plot.rs`:
     `error: missing documentation for a method`, `mask all-check` exit 1.
  2. A `[`Plot::nonexistent`]` link on that doc line: "error: unresolved link to
     Plot::nonexistent", from the `doc` entry, exit 1.

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### Measured zero did hold

- **Challenge**: The story rested on a scribe measurement; the real test is the
  gate itself.
- **Solution**: Adding the attribute and running `mask all-check` confirmed zero
  missing docs in all eight roots, including `gup-bevy` (which the hook's scoped
  clippy also lints, so it was checked on the commit).
- **Pattern**: Attribute-only locks are cheap. Measure, then gate.

#### `mask doc` in `all-check` costs almost nothing warm

- **Challenge**: The `doc` entry could push `all-check` past the 60 s budget.
- **Solution**: It runs concurrently with the other entries. Warm, `mask doc`
  finishes in about 1 s; cold, about 27 s. The warm `all-check` took 63 s wall,
  within the story's budget.

#### Prettier re-wraps maskfile.md

- **Challenge**: The pinned prettier re-wrapped the all-check paragraph and
  failed the `md` entry on the first run.
- **Solution**: `prettier --write maskfile.md` with the pinned binary (found via
  `which prettier`), then rerun `all-check`.

### Architectural Decisions

#### `warn`, not `deny`

- **Decision**: `#![warn(missing_docs)]` (as the story decided).
- **Reasoning**: Clippy's `-D warnings` already turns the warning into a gate.
- **Trade-off**: A bare `cargo build` only warns.
- **Future**: `cargo doc` with `-D warnings` remains the strict docs gate.

### Development Workflow Insights

- Other workers share the build directory, so `mask doc` often waited on the
  file lock; timing numbers above exclude that wait.
- Seeded violations were applied to `crates/gup-core/src/plot.rs` in the working
  tree only (never committed) and reverted with `git checkout --` before each
  commit. GUP-540 owns that file on another branch.

### Scope Notes

- **Touches deviation**: the story's `touches:` list names only `maskfile.md`,
  `crates/gup-core/src/lib.rs` and `src/lib.rs`. AC1 needs the attribute in all
  eight crate roots, so five more files were changed (`crates/gup-core-macros`,
  `crates/gup-text`, `crates/gup-wgsl`, `gup-egui`, `gup-bevy`,
  `crates/gup-visual-regression` lib.rs). Each is a two-line attribute addition
  and none is touched by GUP-540.
- **Not measured**: the DoD "WASM size unchanged" and the "checkpoint every
  20-30 minutes" items are not ticked. Attributes do not change library code,
  and I did not run `scripts/checkpoint.sh` between commits.

### Follow-up Stories

None. The reserved IDs GUP-524 and GUP-525 were not needed. `gup-culling-lod`
(parked) keeps its own `#![deny(missing_docs)]`, and `docs.rs` with
`--all-features` is track N3.
