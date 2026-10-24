---
implementer: sonnet
implementer-reason:
  attribute and message changes with decision rules for the one rustc behaviour
  to confirm; the doctest spike has a fixed recipe and rules
touches:
  [
    crates/gup-core/src/encoding.rs,
    crates/gup-core/src/scale/,
    crates/gup-wgsl/src/compose.rs,
    crates/gup-core-macros/src/mark.rs,
    crates/gup-core/tests/,
  ]
---

# GUP-500: Stable Compile-Fail Snapshots and the wgpu Doctest Spike

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 📋 Planned
**Created**: 2026-10-24

## Context

`gup-core`'s trybuild suites (`crates/gup-core/tests/compile_fail.rs`: 7 typed
channel cases, 15 `#[wgsl_function]` cases, 9 `#[derive(Mark)]` cases) compare
exact rustc output with `.stderr` snapshots. Snapshots change for two reasons
unrelated to the case under test, and every such change has to be re-blessed and
merged (STRATEGIC_REVIEW_2026-11 §3, "trybuild `.stderr` churn"):

1. **rustc's candidate lists.** When a bound fails, rustc adds "the following
   other types implement trait `X`" and lists up to eight impls. Adding a public
   type that implements `CpuMirror` (GUP-461's `Offset`) or a new `ColumnValue`
   changes `chain_without_mirror`, `monotonic_without_mirror`, `bare_number` and
   `borrowing_accessor`.
2. **Gup's own module lists.** `gup-wgsl`'s unknown-import error (`compose.rs`
   around line 425) and `#[derive(Mark)]`'s unknown-module error
   (`gup-core-macros/src/mark.rs` around line 359) print every library module,
   so a new WGSL module changes `unknown_import.stderr` (6 commits so far) and
   `unknown_module.stderr` (4).

Since Rust 1.85, `#[diagnostic::do_not_recommend]` on an impl keeps it out of
rustc's "other types implement" list (rust-lang/rust PR 127614). Our own
messages can name only the closest module when there is one.

The same review item lists a second gate problem: GUP-486's worker saw "multiple
versions of crate `wgpu`" in 17 doctests under a plain `cargo test` with the
shared build directory (GUP-411, GUP-427), while `cargo test --doc` alone
passed. The doctests it named (`streaming`, `brush`, `gpu_timer`, `proc_macros`)
were in the old `gup` crate, deleted in GUP-488, so the problem may be gone.
This story reproduces it with a fixed recipe and applies a decision rule.

## Read First

- `crates/gup-core/tests/compile_fail.rs` (35 lines).
- `crates/gup-core/tests/wgsl_function_fail/chain_without_mirror.stderr`,
  `crates/gup-core/tests/compile_fail/bare_number.stderr`,
  `crates/gup-core/tests/compile_fail/borrowing_accessor.stderr`,
  `crates/gup-core/tests/wgsl_function_fail/unknown_import.stderr`,
  `crates/gup-core/tests/derive_mark_fail/unknown_module.stderr`.
- `crates/gup-core/src/encoding.rs` lines 480–495 (`CpuMirror`'s
  `on_unimplemented`), 615–690 (the `Then` and `ColumnValue` impls).
- `crates/gup-wgsl/src/compose.rs` lines 400–435 and 775–800 (`edit_distance`).
- `crates/gup-core-macros/src/mark.rs` lines 335–365.
- `CLAUDE.md` "Sharing build output between checkouts" (already in context).

## User Story

> "As a worker adding a public scale, column type or WGSL module, I want the
> compile-fail snapshots to stay as they are, so that my story does not churn
> unrelated test files or conflict with a parallel story."

## Acceptance Criteria

### AC1: candidate lists leave the snapshots (spike, with a rule)

- [ ] Every non-generic library impl of `CpuMirror` (`Linear`, `Pow`, `Log`,
      `Symlog`, `Time`, `Offset`, `Sequential`, `Diverging`, `Categorical`, and
      `Band`/`Point` through the macro in `scale/band.rs`) and of `ColumnValue`
      (`f64`, `f32`, `Option<f64>`, `Option<f32>`) carries
      `#[diagnostic::do_not_recommend]`. Generic impls (`ScaleRef<S>`,
      `Then<A, B>`) and every `IntoEncoding` impl do not.
- [ ] Re-bless with
      `TRYBUILD=overwrite cargo test -p gup-core --test compile_fail`.
      **Decision rule**: for each of the two traits, keep the attributes if
      `git diff` of the `.stderr` files shows only removed lines of that trait's
      candidate list (the "the following other types implement trait" help and
      its entries). If any other line of any snapshot changes because of a
      trait's attributes, remove that trait's attributes, re-bless, and record
      the diff in the Implementation Summary.
- [ ] The comment on each trait says why its library impls carry the attribute
      (keeps compile-fail snapshots stable; GUP-500).

### AC2: module lists leave the snapshots

- [ ] `gup-wgsl` gains
      `pub fn closest<'a>(name: &str, known: impl IntoIterator<Item = &'a str>) -> Option<&'a str>`
      in `compose.rs` (edit distance ≤ 3, ties broken as today's
      `(distance, name)` minimum), used by the unknown-import error.
- [ ] The unknown-import error names only the closest module when there is one:
      "`{import}` is not a module this one can import (did you mean `{k}`?)";
      with no close match it keeps "...; the library has: {list}".
- [ ] `#[derive(Mark)]`'s unknown-module error does the same with `closest` over
      the library's `gup::marks::*` modules: "`{path}` is not a module of Gup's
      library (did you mean `{k}`?)"; with no close match it keeps "...; its
      marks are: {list}".
- [ ] `derive_mark_fail/unknown_module.rs` uses `gup::marks::circel` (its
      comment stays "A module that is not in Gup's library"), and both
      `unknown_import.stderr` and `unknown_module.stderr` are re-blessed;
      neither contains "the library has" or "its marks are"
      (`grep -l 'the library has\|its marks are' crates/gup-core/tests/*/*.stderr`
      prints nothing).
- [ ] Unit tests keep the list path covered: in `gup-wgsl`, an import with no
      close match produces a message containing every library module's import
      path; in `gup-core-macros`, the message helper with no close match
      contains every mark module.

### AC3: proof that a new impl or module no longer churns

- [ ] Seeded proof, recorded in the Implementation Summary and not committed:
      add a temporary `pub struct SeedScale;` with a `CpuMirror` impl carrying
      the attribute (and the `ShaderFn` impl it needs, copied from `Linear`'s
      shape), run `cargo test -p gup-core --test compile_fail` without
      `TRYBUILD=overwrite`: it passes. Remove the attribute: it fails on
      `chain_without_mirror`.
- [ ] `compile_fail.rs`'s header says which snapshots still change when (a new
      `IntoEncoding` impl changes `bare_number`; a toolchain bump changes many)
      and that library `CpuMirror`/`ColumnValue` impls take
      `#[diagnostic::do_not_recommend]`.

### AC4: the wgpu doctest spike

- [ ] Run, in the worker's checkout through `scripts/with_cargo_env.sh` (shared
      build directory), each with `-- --test-threads=1`, and record pass/fail
      and any error lines:
  1. `cargo test -p gup` (plain: lib, tests, examples, doctests);
  2. `cargo build -p gup-bevy --tests` (puts Bevy's wgpu feature set into the
     shared build directory), then again `cargo test -p gup`;
  3. `cargo test --workspace --exclude gup-bevy --doc`.
- [ ] **Decision rule**:
  - none of the runs prints `E0464`, "multiple candidates" or "multiple versions
    of crate": record "not reproduced on `<sha>`; the doctests that failed in
    GUP-486 were deleted in GUP-488" in the story; no code change;
  - it reproduces, the error's candidate paths have **different** hashes, and
    the failing doctest names a crate (such as `wgpu`) that its package does not
    depend on directly: change the doctest to use the re-export (`gup::wgpu`,
    `gup_core::wgpu`) and grep every doctest for the same pattern
    (`grep -rn --include='*.rs' -E '^\s*//[/!].*\bwgpu::'`, minus those in
    crates with a direct `wgpu` dependency);
  - any other cause (for example the **same** artifact reached under both the
    build directory and the target directory): stop and escalate with the
    `cargo test -v` rustdoc command line and the error.

## Decisions

- **`do_not_recommend` on concrete impls only**: on a generic impl with a
  where-clause it also changes which bound rustc reports (it reports the outer
  impl instead of the failing inner bound), which would make
  `borrowing_accessor`'s second error worse. Concrete impls have no
  where-clause, so only the list changes.
- **Our messages**: when a close match exists, the list adds nothing a user
  needs; when none does, the list is the help, and a unit test (not a snapshot)
  covers it.
- **`closest` lives in `gup-wgsl::compose`** (behind the `compose` feature that
  `gup-core-macros` already enables), so both messages share one edit distance.
- **No trybuild normaliser or harness change**: trybuild's normalisation is not
  configurable, and replacing the harness (for example with `ui_test`) is more
  than the problem needs.
- **The toolchain pin** (`rust-toolchain.toml`, 1.93.1) does not move.

## Escalation Triggers

- AC1's decision rule removes the attributes for `CpuMirror` (the churn it
  targets stays).
- Any `.stderr` line outside the candidate lists and the two module messages
  changes when re-blessed.
- The doctest spike hits the escalation branch of its rule.
- `closest` or the message change alters any `wgsl_function_fail` snapshot other
  than `unknown_import.stderr`.

## Out of Scope

- New compile-fail cases, or changes to the cases' intent.
- The doctests' content beyond the spike's re-export fix.
- `scripts/cargo_env.sh` changes, unless the spike's escalation is resolved by
  the orchestrator into a follow-up.

## Technical Tasks

- [ ] Start commit within 30 minutes.
- [ ] Add the attributes; re-bless; apply AC1's rule; commit.
- [ ] Add `closest`, change both messages, update `unknown_module.rs`, re-bless,
      add the two unit tests; commit.
- [ ] Run the AC3 seed by hand; update `compile_fail.rs`'s header.
- [ ] Run the AC4 recipe (long builds: use a 600000 ms timeout or the
      background; checkpoint before each).
- [ ] `cargo test -p gup-core --test compile_fail`, `cargo test -p gup-wgsl`,
      `cargo test -p gup-core-macros`, `mask all-check`, `mask wasm-size`.

## Dependencies

### Prerequisite Stories

- GUP-496: Checkpoint Script 📋 — soft, for the DoD's checkpoint line.

### Enables Stories

- Every later story that adds a scale, column type or WGSL module (N2, N5): no
  snapshot churn.

## Testing Strategy

- **Snapshots**: `cargo test -p gup-core --test compile_fail` passes on the
  re-blessed files.
- **Unit**: the two list-path tests (AC2).
- **Seeded proof**: AC3.
- **Spike**: AC4's three runs.
- **Size**: `mask wasm-size` (gup-core source changes; attributes should change
  nothing).

## Success Metrics

- [ ] `grep -c 'implement trait .CpuMirror' crates/gup-core/tests/*/*.stderr`
      prints 0 per file, or the rule's recorded reason.
- [ ] The two module-list snapshots contain no list.
- [ ] The AC3 seed passes with the attribute and fails without it.

## Risk Assessment

- **Medium**: rustc 1.93 may treat `do_not_recommend` differently from the PR's
  description (for example still listing impls when every candidate is hidden).
  _Mitigation_: AC1 is a spike with a rule; the module-list half of the story
  stands alone.
- **Low**: a user loses the list of types implementing `CpuMirror` in the error.
  _Mitigation_: the `on_unimplemented` note already says what to implement; the
  list never helped fix the error.

## Definition of Done

- [ ] First commit within 30 minutes of starting, then `scripts/checkpoint.sh`
      every 20–30 minutes between commits (story-worker "Checkpoint rule")
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `cargo test -p gup-core --all-features -- --test-threads=1` passes
- [ ] Lint and format clean: `mask all-fix`; `mask all-check` passes
- [ ] `mask wasm-browser`: not needed (diagnostic attributes and build-time
      error messages; no layout, input or wasm behaviour); say so
- [ ] WASM size: run `mask wasm-size` and record the scatter gz bytes (expected
      unchanged)
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
