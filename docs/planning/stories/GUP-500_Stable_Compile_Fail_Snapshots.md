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

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: ✅ Complete
(2026-10-24) **Created**: 2026-10-24

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

- [x] Every non-generic library impl of `CpuMirror` (`Linear`, `Pow`, `Log`,
      `Symlog`, `Time`, `Offset`, `Sequential`, `Diverging`, `Categorical`, and
      `Band`/`Point` through the macro in `scale/band.rs`) and of `ColumnValue`
      (`f64`, `f32`, `Option<f64>`, `Option<f32>`) carries
      `#[diagnostic::do_not_recommend]`. Generic impls (`ScaleRef<S>`,
      `Then<A, B>`) and every `IntoEncoding` impl do not.
- [x] Re-bless with
      `TRYBUILD=overwrite cargo test -p gup-core --test compile_fail`.
      **Decision rule**: for each of the two traits, keep the attributes if
      `git diff` of the `.stderr` files shows only removed lines of that trait's
      candidate list (the "the following other types implement trait" help and
      its entries). If any other line of any snapshot changes because of a
      trait's attributes, remove that trait's attributes, re-bless, and record
      the diff in the Implementation Summary.
- [x] The comment on each trait says why its library impls carry the attribute
      (keeps compile-fail snapshots stable; GUP-500).

### AC2: module lists leave the snapshots

- [x] `gup-wgsl` gains
      `pub fn closest<'a>(name: &str, known: impl IntoIterator<Item = &'a str>) -> Option<&'a str>`
      in `compose.rs` (edit distance ≤ 3, ties broken as today's
      `(distance, name)` minimum), used by the unknown-import error.
- [x] The unknown-import error names only the closest module when there is one:
      "`{import}` is not a module this one can import (did you mean `{k}`?)";
      with no close match it keeps "...; the library has: {list}".
- [x] `#[derive(Mark)]`'s unknown-module error does the same with `closest` over
      the library's `gup::marks::*` modules: "`{path}` is not a module of Gup's
      library (did you mean `{k}`?)"; with no close match it keeps "...; its
      marks are: {list}".
- [x] `derive_mark_fail/unknown_module.rs` uses `gup::marks::circel` (its
      comment stays "A module that is not in Gup's library"), and both
      `unknown_import.stderr` and `unknown_module.stderr` are re-blessed;
      neither contains "the library has" or "its marks are"
      (`grep -l 'the library has\|its marks are' crates/gup-core/tests/*/*.stderr`
      prints nothing).
- [x] Unit tests keep the list path covered: in `gup-wgsl`, an import with no
      close match produces a message containing every library module's import
      path; in `gup-core-macros`, the message helper with no close match
      contains every mark module.

### AC3: proof that a new impl or module no longer churns

- [x] Seeded proof, recorded in the Implementation Summary and not committed:
      add a temporary `pub struct SeedScale;` with a `CpuMirror` impl carrying
      the attribute (and the `ShaderFn` impl it needs, copied from `Linear`'s
      shape), run `cargo test -p gup-core --test compile_fail` without
      `TRYBUILD=overwrite`: it passes. Remove the attribute: it fails on
      `chain_without_mirror`.
- [x] `compile_fail.rs`'s header says which snapshots still change when (a new
      `IntoEncoding` impl changes `bare_number`; a toolchain bump changes many)
      and that library `CpuMirror`/`ColumnValue` impls take
      `#[diagnostic::do_not_recommend]`.

### AC4: the wgpu doctest spike

- [x] Run, in the worker's checkout through `scripts/with_cargo_env.sh` (shared
      build directory), each with `-- --test-threads=1`, and record pass/fail
      and any error lines:
  1. `cargo test -p gup` (plain: lib, tests, examples, doctests);
  2. `cargo build -p gup-bevy --tests` (puts Bevy's wgpu feature set into the
     shared build directory), then again `cargo test -p gup`;
  3. `cargo test --workspace --exclude gup-bevy --doc`.
- [x] **Decision rule**:
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

- [x] Start commit within 30 minutes.
- [x] Add the attributes; re-bless; apply AC1's rule; commit.
- [x] Add `closest`, change both messages, update `unknown_module.rs`, re-bless,
      add the two unit tests; commit.
- [x] Run the AC3 seed by hand; update `compile_fail.rs`'s header.
- [x] Run the AC4 recipe (long builds: use a 600000 ms timeout or the
      background; checkpoint before each).
- [x] `cargo test -p gup-core --test compile_fail`, `cargo test -p gup-wgsl`,
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

- [x] `grep -c 'implement trait .CpuMirror' crates/gup-core/tests/*/*.stderr`
      prints 0 per file, or the rule's recorded reason.
- [x] The two module-list snapshots contain no list.
- [x] The AC3 seed passes with the attribute and fails without it.

## Risk Assessment

- **Medium**: rustc 1.93 may treat `do_not_recommend` differently from the PR's
  description (for example still listing impls when every candidate is hidden).
  _Mitigation_: AC1 is a spike with a rule; the module-list half of the story
  stands alone.
- **Low**: a user loses the list of types implementing `CpuMirror` in the error.
  _Mitigation_: the `on_unimplemented` note already says what to implement; the
  list never helped fix the error.

## Definition of Done

- [x] First commit within 30 minutes of starting, then `scripts/checkpoint.sh`
      every 20–30 minutes between commits (story-worker "Checkpoint rule")
- [x] All Acceptance Criteria are satisfied and checked
- [x] `cargo test -p gup-core --all-features -- --test-threads=1` passes
- [x] Lint and format clean: `mask all-fix`; `mask all-check` passes
- [x] `mask wasm-browser`: not needed (diagnostic attributes and build-time
      error messages; no layout, input or wasm behaviour); say so
- [x] WASM size: run `mask wasm-size` and record the scatter gz bytes (expected
      unchanged)
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Commits**: `802ab701` (code, snapshots, unit tests) and the story commits.

- **AC1 (partly, by the rule)**: `#[diagnostic::do_not_recommend]` on all 11
  concrete `CpuMirror` impls (the 9 scales, plus `Band` and `Point` through the
  `ordinal_scale!` macro) and, as a trial, on the 4 `ColumnValue` impls.
  - `CpuMirror`: the re-bless changed `chain_without_mirror.stderr` and
    `monotonic_without_mirror.stderr` only inside the candidate list. The
    concrete entries and "and N others" are gone; the help block is now the two
    generic impls (`ScaleRef<S>`, `Then<A, B>`), printed as a spanned `help:`
    instead of a `= help:` list. Kept. I read "only removed lines of that
    trait's candidate list" as covering this reformat of the same help block
    (the added lines are all entries of it). Reviewer: confirm that reading.
  - `ColumnValue`: the re-bless changed `borrowing_accessor.stderr` outside the
    candidate list: the first `ColumnValue` help block was removed, but the
    second was replaced by an unrelated hint, "the trait
    `IntoEncoding<T, V, ColumnMarker<D>>` is implemented for `Encoded<A, S>`"
    with the whole generic impl. By the rule the `ColumnValue` attributes were
    removed (a comment on the trait records why) and `borrowing_accessor.stderr`
    is unchanged from before. A new `ColumnValue` type still churns it
    (follow-up GUP-511).
  - `grep -c 'implement trait .CpuMirror'` prints 1 for `chain_without_mirror`
    and `monotonic_without_mirror`: the generic impls remain listed.
- **AC2**: `gup_wgsl::compose::closest`; the unknown-import and unknown-module
  errors name only the closest module, else keep the list. `unknown_module.rs`
  uses `gup::marks::circel`. The `grep -l` over the snapshots prints nothing.
  Unit tests: `unknown_import_lists_the_library_only_without_a_close_match`
  (`gup-wgsl/tests/compose.rs`) and
  `unknown_module_message_lists_marks_only_without_a_close_match`
  (`gup-core-macros/src/mark.rs`). No other `wgsl_function_fail` snapshot
  changed.
- **AC3 (seed)**: a temporary `SeedScale` (a `ShaderFn` copied from `Offset`
  plus a `CpuMirror` impl) in `scale/offset.rs`, not committed. With the
  attribute: `cargo test -p gup-core --test compile_fail` 3 passed. Without it:
  1 failed, with mismatches on `chain_without_mirror` and
  `monotonic_without_mirror` (the new type in the list). The header of
  `compile_fail.rs` says what changes which snapshot.
- **AC4 (doctest spike), not reproduced on `70c64e81` plus this story**: (1)
  plain `cargo test -p gup -- --test-threads=1`: pass, 10 doctests; (2)
  `cargo build -p gup-bevy --tests` then again `cargo test -p gup`: pass; (3)
  `cargo test --workspace --exclude gup-bevy --doc`: pass (50 doctests in
  `gup-core`, among others). No `E0464`, "multiple candidates" or "multiple
  versions of crate". The doctests that failed in GUP-486 were deleted in
  GUP-488. No code change.
- **Gates**: `cargo test -p gup-core --all-features -- --test-threads=1`,
  `gup-wgsl` and `gup-core-macros` tests pass; `mask all-check` passes;
  `mask wasm-size`: scatter gz 334187 B (recorded 334149, +38 B, within the +2%
  limit), over wgpu 301827 B. `mask wasm-browser` not needed (diagnostic
  attributes and build-time error messages; no layout, input or wasm behaviour).

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### `do_not_recommend` on concrete impls

- **Challenge**: it was unclear how rustc 1.93 treats a trait whose concrete
  impls are all hidden.
- **Solution**: it drops them and prints the remaining (generic) impls as a
  spanned `help:` block; that block does not depend on the hidden types, so a
  new concrete impl leaves the snapshot alone (proved by the seed).
- **Pattern**: for `ColumnValue` the hint changed to the unrelated
  `IntoEncoding` impl, because rustc falls back to "the trait is implemented
  for" when the list is empty. Test the attribute against every snapshot that
  mentions the trait before adopting it.

### Architectural Decisions

#### `ColumnValue` keeps its candidate list

- **Decision**: removed the trial attributes by the story's rule.
- **Reasoning**: the replacement hint is noisier than the list.
- **Trade-off**: adding a `ColumnValue` type still re-blesses
  `borrowing_accessor.stderr`.
- **Future**: GUP-511.

### Development Workflow Insights

- The worktree sandbox refuses shell commands it cannot prove stay in the
  worktree (here, some heredocs and `cd ../..` chains); writing a script file
  with the Write tool and running it by absolute path worked around it.
- A trybuild overwrite run takes 1 to 2.5 minutes; run it with a 600 s timeout.

### Follow-up Stories

1. **GUP-511: Stable `ColumnValue` snapshot**: see the story file.
