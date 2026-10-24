---
implementer: haiku
implementer-reason:
  one crate attribute per crate and one mask command wired into all-check; every
  crate already passes (measured), so each AC is a command
touches: [maskfile.md, crates/gup-core/src/lib.rs, src/lib.rs]
---

# GUP-533: Documentation Lint Gates: `missing_docs` and `rustdoc -D warnings`

## Story Overview

**Initiative**: Strategic Review 2026-11 (N1 Docs and Tutorials) **Status**: 📋
Planned **Created**: 2026-10-24

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

- [ ] Each of the eight crate roots above has `#![warn(missing_docs)]` right
      after its crate-level doc comment (before any `use` or `mod`).
- [ ] `mask all-check` passes (clippy's `-D warnings` turns a missing doc into
      an error, so the attribute is a gate for every crate clippy lints).
- [ ] Seeded proof, recorded and not committed: removing the doc comment of
      `gup_core::Plot::title` makes `mask all-check` fail with
      `missing documentation for a method`.

### AC2: `mask doc` builds every published crate's docs strictly

- [ ] `mask doc` runs `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` with `-p`
      for each of `gup`, `gup-core`, `gup-core-macros`, `gup-text`, `gup-wgsl`,
      `gup-egui` and `gup-visual-regression`, and its description says so.
- [ ] `mask all-check` runs `mask doc` as a new `concurrently` entry named
      `doc`, and the `all-check` description gains one sentence naming it.
- [ ] Seeded proof, recorded and not committed: a broken intra-doc link
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

- [ ] Start commit within 30 minutes.
- [ ] Add the eight attributes; run `mask all-check`; commit.
- [ ] Change `mask doc`; add it to `all-check`; run both seeded proofs; commit.

## Dependencies

### Prerequisite Stories

- None.

### Enables Stories

- Track N3's docs.rs build starts from a crate set that already documents
  cleanly.

## Testing Strategy

- **Gate**: `mask all-check` passes; the two seeded proofs fail it.

## Success Metrics

- [ ] An undocumented public item or a broken doc link fails `mask all-check`.

## Risk Assessment

- **Low**: `#![warn(missing_docs)]` in the proc-macro crate also covers its
  exported macros. _Mitigation_: they are documented today (measured).

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output: not applicable, no visual change
- [ ] `mask wasm-browser`: not needed, no layout, input or wasm change
- [ ] WASM size: unchanged, no library code change (attributes only)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
