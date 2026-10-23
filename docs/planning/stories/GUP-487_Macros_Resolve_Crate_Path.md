---
implementer: sonnet
implementer-reason:
  one resolution function with a fixed precedence table and seven call sites;
  unit-tested as a table
---

# GUP-487: `gup-core` Macros Resolve `gup` or `gup_core`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-23)
**Created**: 2026-10-19

## Context

`#[derive(Mark)]` and `#[wgsl_function]` (`crates/gup-core-macros`) emit paths
rooted at `::gup_core` in seven places. That works for crates that depend on
`gup-core` directly, and inside `gup-core` itself through its
`extern crate self as gup_core;` (`crates/gup-core/src/lib.rs` line 162).

At the flip (GUP-488), users depend on `gup`, which re-exports `gup-core`, and
not on `gup-core`. Their expansions would then name a crate they do not have.
The old path met the same problem in GUP-377 for its own macro ("made the
expansion use `::gup`"). The usual fix is to look the crate up in the calling
crate's `Cargo.toml` at expansion time with `proc-macro-crate`, which also
handles a renamed dependency (`charts = { package = "gup", .. }`).

This story lands before the flip so the flip only has to add the re-export.

## Read First

- `crates/gup-core-macros/src/lib.rs` lines 60–120 (the error-path impl, lines
  81, 109, 114) and 470–490 (`root`, `private`), and
  `crates/gup-core-macros/src/mark.rs` lines 190–200.
- `crates/gup-core-macros/Cargo.toml` (whole).
- `crates/gup-core/src/lib.rs` lines 155–165 (`extern crate self`) and the
  `__private` module (lines 304–340).
- The trybuild suites
  `crates/gup-core/tests/{compile_fail,derive_mark_fail, wgsl_function_fail}`
  and `crates/gup-core/tests/compile_fail.rs`.

## User Story

> "As a user who depends only on `gup`, I want `#[derive(Mark)]` and
> `#[wgsl_function]` to expand to paths my crate can name."

## Acceptance Criteria

### AC1: One resolution function

- [ ] `crates/gup-core-macros/src/root.rs` has
      `pub(crate) fn crate_root() -> proc_macro2::TokenStream`, built on a pure
      `fn resolve(lookup: impl Fn(&str) -> Option<FoundCrate>) -> TokenStream`
      implementing Decisions' table.
- [ ] All seven `::gup_core` sites use it (`root` and
      `private = #root::__private` in both macros; the three in the error-path
      impl). `git grep -n     "::gup_core" crates/gup-core-macros/src` finds
      only doc comments and the fallback in `resolve`.

### AC2: Tested

- [ ] A unit test of `resolve` covers every row of the table, comparing token
      strings.
- [ ] Every existing gup-core test passes, including the three trybuild suites
      and their `.stderr` snapshots unchanged (gup-core's own tests take the
      first row).
- [ ] `cargo test --manifest-path dogfood/Cargo.toml` and the dogfood suite pass
      (an external crate depending on `gup-core` by its real name).

## Decisions

- **Dependency**: `proc-macro-crate = "3"` in `gup-core-macros` (3.3 and 3.5 are
  in the local registry). Reason: it is the standard lookup, reads the caller's
  manifest, and handles renames.
- **Precedence table** (first match wins):
  1. `crate_name("gup-core")` is `Itself`: `::gup_core` (the
     `extern crate self`).
  2. `crate_name("gup-core")` is `Name(n)`: `::n`.
  3. `crate_name("gup")` is `Itself`: `::gup` (the facade's own examples and
     tests).
  4. `crate_name("gup")` is `Name(n)`: `::n`.
  5. Neither is found: `::gup_core` (today's behaviour).

  `gup-core` comes first so that a crate depending on both (dogfood until the
  flip) uses the direct path, and trybuild snapshots do not change.

- **No `#[gup(crate = ..)]` attribute**: the lookup covers renames; an override
  can come later if a user needs one.

## Escalation Triggers

- Any trybuild `.stderr` snapshot changes in message text (paths in messages
  must stay `gup_core`).
- `proc-macro-crate` cannot read the manifest under trybuild's generated project
  or in doctests.
- Pulling in `proc-macro-crate` changes another resolved dependency version in
  `Cargo.lock` (for example `toml_edit` versions already pinned elsewhere) in a
  way that breaks a build.

## Out of Scope

- The `gup` facade itself: GUP-488.
- The old path's `gup-macros` crate: deleted by GUP-488.

## Technical Tasks

- [ ] Add the dependency; `root.rs` with `resolve` and `crate_root`.
- [ ] Replace the seven sites; unit test the table.
- [ ] Run gup-core's tests (trybuild included) and dogfood.

## Dependencies

### Prerequisite Stories

- GUP-420, GUP-422: `#[wgsl_function]` v2 and `#[derive(Mark)]` v2 ✅.

### Enables Stories

- GUP-488: S14c — expansions work for crates that depend only on `gup`.

## Testing Strategy

- **Unit tests**: the precedence table.
- **Integration tests**: gup-core's full suite, trybuild included; dogfood.
- **End to end**: GUP-488's dogfood run on `gup` alone (t6 uses
  `#[wgsl_function]`) proves the `gup` row.

## Success Metrics

- [ ] No hard-coded `::gup_core` path left in macro output.

## Risk Assessment

- **Low**: `proc-macro-crate` reads `Cargo.toml` from `CARGO_MANIFEST_DIR` at
  every expansion. _Mitigation_: it caches per manifest; compile time is
  unchanged in practice (record `cargo build -p gup-core` time before and
  after).
- **Medium**: trybuild `.stderr` churn: expected none (escalation trigger).
  Line-number-only diffs in gup-core sources would not come from this story.

## Definition of Done

- [ ] First code commit within 30 minutes of starting; then commit at each
      checkpoint (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -p gup-core -- --test-threads=1`,
      `cargo test -p gup-core-macros`
- [ ] Lint and format clean: `mask all-fix`
- [ ] `mask wasm-browser` passes (the wasm harness uses `#[wgsl_function]`)
- [ ] `mask wasm-size` run and the figures reported in the Implementation
      Summary (expected unchanged; the orchestrator re-records)
- [ ] `mask old-path-loc` recorded (expected unchanged)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document

## Implementation Summary

`crates/gup-core-macros/src/root.rs`: `crate_root()` over a pure
`resolve(lookup)` (the five-row table, unit-tested). All seven sites use it, and
the error-path impl resolves the root once. New test
`crates/gup-core/tests/reexport.rs` checks a detached workspace
(`tests/reexport/`) in which `reexport-user` depends only on a stand-in `gup`
facade, renamed `charts`, and uses `#[wgsl_function]`. Trybuild snapshots
unchanged; gup-core-macros 4 tests, dogfood suite pass. `mask old-path-loc`:
26601 (unchanged). `mask wasm-size`: scatter 331601 B gz (+2% of baseline),
over_wgpu 299241, per_row_type 1730; budgets pass.

## Retrospective

**Completed**: 2026-10-23

- The `proc-macro-crate` lookup works under trybuild and in the nested detached
  workspace with no extra setup.
- Decision: the re-export test shells out to `cargo check --offline` on a
  detached workspace (lockfile and target gitignored), reusing
  `CARGO_TARGET_DIR/reexport` when set. It takes about 40 s cold.
- `#[derive(Mark)]` shares `crate_root`; only `#[wgsl_function]` is exercised
  through the facade, the derive path being the same call.
- Follow-ups: none.
