---
implementer: haiku
implementer-reason:
  one attribute removed and one test file restored, with the snapshot given
  verbatim; the approach was probed on 2026-10-24
touches:
  [
    crates/gup-core/src/encoding.rs,
    crates/gup-core/tests/compile_fail.rs,
    crates/gup-core/tests/compile_fail/borrowing_accessor.rs,
    crates/gup-core/tests/compile_fail/borrowing_accessor.stderr,
  ]
---

# GUP-523: Attr-Path Column Value Message

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: ✅ Complete
(2026-10-24) **Created**: 2026-10-24

## Context

A user who writes `sel.attr(Circle::FILL, scale.encode(|r| r.name.as_str()))`
gets two errors: one at `encode` (its `D: ColumnValue` bound) and one at `attr`
(the `IntoEncoding` impl for `Encoded<A, S>` repeats that bound). GUP-511 put
`#[diagnostic::do_not_recommend]` on all four `ColumnValue` impls so that a new
column type stops changing `borrowing_accessor.stderr`. With no impl left to
recommend, rustc fills the `attr` error's help with the root impl instead: "the
trait `IntoEncoding<T, V, ColumnMarker<D>>` is implemented for `Encoded<A, S>`"
and a seven-line impl header. That hint has nothing to do with the mistake.
GUP-511 then narrowed the compile-fail case to `encode` alone
(`let _ = …encode(..)`), so no snapshot covers the `attr` path that users
actually write.

Probed on 2026-10-24 (the toolchain in `rust-toolchain.toml`) with a scratch
case of the `attr` form:

| Change                                                                      | `attr` error's help                                | Other snapshots                    |
| --------------------------------------------------------------------------- | -------------------------------------------------- | ---------------------------------- |
| None (today)                                                                | `IntoEncoding … is implemented for Encoded<A, S>`  | —                                  |
| `do_not_recommend` on the `Encoded` `IntoEncoding` impl (GUP-511 attempt 1) | headline becomes "cannot drive a channel"          | `wrong_value_type` loses its E0271 |
| `Encoded<A, S, G>` with `G = D::Gpu`, so a failed bound poisons the type    | the same hint, on `Encoded<…, _>`                  | three snapshots change             |
| **Remove `do_not_recommend` from `impl ColumnValue for f64` only**          | "the trait `ColumnValue` is implemented for `f64`" | only `borrowing_accessor` (+5)     |

The last row gives both errors the same headline, note and help, all about the
real mistake, and names the type to return. A new column type still leaves every
snapshot unchanged as long as its impl carries the attribute.

## Read First

- `crates/gup-core/src/encoding.rs` lines 643–700: the comment above
  `ColumnValue`, its `#[diagnostic::on_unimplemented]` and the four impls.
- `crates/gup-core/tests/compile_fail.rs` lines 1–28 (header and
  `channel_mistakes_do_not_compile`).
- `crates/gup-core/tests/compile_fail/borrowing_accessor.rs` and `.stderr`.
- GUP-511's retrospective
  (`docs/planning/stories/GUP-511_Stable_Column_Value_Snapshot.md`, "Hiding
  impls works only when the bound fails once").

## User Story

> "As a user who passes a string accessor to `attr`, I want every error I get to
> say what a column value is and what to call instead, so that I can fix it on
> the first read."

## Acceptance Criteria

### AC1: `f64` is the recommended column value

- [x] `impl ColumnValue for f64` has no `#[diagnostic::do_not_recommend]`; the
      `f32`, `Option<f64>` and `Option<f32>` impls keep theirs.
- [x] The comment above `ColumnValue` (lines 648–653 today) says: the library's
      impls carry `#[diagnostic::do_not_recommend]` except `f64`'s, which rustc
      then names in every "not a column value" error, in `encode` and in `attr`;
      a new column type's impl carries the attribute, so the snapshots do not
      change (GUP-511, GUP-523).

### AC2: The `attr` path has a snapshot

- [x] `crates/gup-core/tests/compile_fail/borrowing_accessor.rs` is exactly:

```rust
// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// RFC-001 §12 risk 4: `encode`'s accessor is `Fn(&T) -> D`, whose `D`
// cannot borrow from the row, and a string is not a column value. The same
// closure compiles through `encode_key` (tests/compile_pass/key_accessor.rs).
// Both errors, in `encode` and in `attr`, say so and point at `f64`, the one
// `ColumnValue` impl without `#[diagnostic::do_not_recommend]` (GUP-523), so
// a new column type or `IntoEncoding` impl leaves this snapshot unchanged.
use gup_core::prelude::*;

struct Row {
    continent: String,
}

fn main() {
    let mut sel = Selection::<Row, Circle>::new(vec![Row {
        continent: "Asia".into(),
    }]);
    sel.attr(
        Circle::FILL,
        Categorical::okabe_ito().encode(|r: &Row| r.continent.as_str()),
    );
}
```

- [x] After
      `TRYBUILD=overwrite scripts/with_cargo_env.sh cargo test -p gup-core --test compile_fail channel_mistakes`,
      `borrowing_accessor.stderr` is exactly:

```text
error[E0277]: `&str` is not a column value; `encode` takes accessors returning numbers
  --> tests/compile_fail/borrowing_accessor.rs:22:34
   |
22 |         Categorical::okabe_ito().encode(|r: &Row| r.continent.as_str()),
   |                                  ^^^^^^ `encode`'s accessor returns `&str`
   |
   = help: the trait `ColumnValue` is not implemented for `&str`
   = note: for string keys (categories), use `encode_key(|row| row.field.as_str())`, whose accessor may return a borrow of the row; for integer or enum keys, `encode_owned_key(|row| row.field)`; both on a dictionary function such as `Categorical`, `Band` or `Point`
help: the trait `ColumnValue` is implemented for `f64`
  --> src/encoding.rs
   |
   | impl ColumnValue for f64 {
   | ^^^^^^^^^^^^^^^^^^^^^^^^
note: required by a bound in `encode`
  --> src/encoding.rs
   |
   |     fn encode<T, D, A>(&self, accessor: A) -> Encoded<A, Self>
   |        ------ required by a bound in this associated function
...
   |         D: ColumnValue,
   |            ^^^^^^^^^^^ required by this bound in `EncodeFn::encode`

error[E0277]: `&str` is not a column value; `encode` takes accessors returning numbers
  --> tests/compile_fail/borrowing_accessor.rs:22:9
   |
20 |     sel.attr(
   |         ---- required by a bound introduced by this call
21 |         Circle::FILL,
22 |         Categorical::okabe_ito().encode(|r: &Row| r.continent.as_str()),
   |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `encode`'s accessor returns `&str`
   |
   = help: the trait `ColumnValue` is not implemented for `&str`
   = note: for string keys (categories), use `encode_key(|row| row.field.as_str())`, whose accessor may return a borrow of the row; for integer or enum keys, `encode_owned_key(|row| row.field)`; both on a dictionary function such as `Categorical`, `Band` or `Point`
help: the trait `ColumnValue` is implemented for `f64`
  --> src/encoding.rs
   |
   | impl ColumnValue for f64 {
   | ^^^^^^^^^^^^^^^^^^^^^^^^
   = note: required for `Encoded<{closure@$DIR/tests/compile_fail/borrowing_accessor.rs:22:41: 22:50}, gup_core::Categorical>` to implement `IntoEncoding<Row, gup_core::Color, ColumnMarker<&str>>`
note: required by a bound in `gup_core::Selection::<T, M>::attr`
  --> src/selection.rs
   |
   |     pub fn attr<V: Visual, E, K>(&mut self, channel: Channel<M, V>, encoding: E) -> &mut Self
   |            ---- required by a bound in this associated function
   |     where
   |         E: IntoEncoding<T, V, K>,
   |            ^^^^^^^^^^^^^^^^^^^^^ required by this bound in `Selection::<T, M>::attr`
```

- [x] `git diff --stat crates/gup-core/tests/` lists only
      `borrowing_accessor.rs`, `borrowing_accessor.stderr` and
      `compile_fail.rs`, and
      `git grep -n "implemented for .Encoded"     crates/gup-core/tests` prints
      nothing.
- [x] `scripts/with_cargo_env.sh cargo test -p gup-core --test compile_fail`
      passes (all three tests) with `TRYBUILD` unset.

### AC3: Seeded proof of stability

Each seed is temporary: add it to `encoding.rs`, run
`scripts/with_cargo_env.sh cargo test -p gup-core --test compile_fail` without
`TRYBUILD`, remove it, and record the result in the retrospective.

- [x] Seed 1: `#[derive(Clone, Copy)] pub struct SeedCol(f64);` with
      `#[diagnostic::do_not_recommend] impl ColumnValue for SeedCol`
      (`type Gpu = f32;`, `to_f64` returns `self.0`). Every snapshot passes.
- [x] Seed 2: `pub struct SeedEnc;` and `pub enum SeedMarker {}` with
      `impl<T, V: Visual> IntoEncoding<T, V, SeedMarker> for SeedEnc`
      (`into_encoding` is `unimplemented!()`). `borrowing_accessor` passes;
      record whether `bare_number` changed (it may: `compile_fail.rs`'s header
      already says a new `IntoEncoding` impl changes it).

### AC4: Header updated

- [x] The "What changes a snapshot" paragraph of `compile_fail.rs`'s header
      says: a new `ColumnValue` type changes none if its impl carries
      `#[diagnostic::do_not_recommend]` (only `f64`'s does not, so rustc names
      it), and `borrowing_accessor` covers both the `encode` and the `attr`
      error. The clause about failing the bound once, in `encode`, is removed.

## Decisions

- **One recommended impl.** Remove the attribute from the `f64` impl only (the
  last row of the probe table). `f64` is the type to suggest: most accessors
  return it, and integers convert with `as f64`.
- **One case, in the `attr` form.** `borrowing_accessor.rs` returns to its
  pre-GUP-511 body, the form users write, which covers both errors; no second,
  `encode`-only case.
- **No other library change**: no new trait, and no change to `Encoded`,
  `IntoEncoding` or any `on_unimplemented` text.

## Escalation Triggers

- The re-blessed `borrowing_accessor.stderr` differs from AC2's text, or any
  other `.stderr` changes.
- Seed 1 changes any snapshot.

## Out of Scope

- The toolchain pin and other traits' snapshots (GUP-500, GUP-511).
- Messages for other `IntoEncoding` failures (`wrong_value_type`, `bare_number`,
  `closure_on_color`); they stay as they are.

## Technical Tasks

- [x] Remove the attribute from the `f64` impl; rewrite the comment.
- [x] Restore `borrowing_accessor.rs`; overwrite the snapshot and compare it
      with AC2.
- [x] Run seeds 1 and 2; remove them.
- [x] Update `compile_fail.rs`'s header.

## Dependencies

### Prerequisite Stories

- GUP-511: Stable `ColumnValue` Snapshot ✅ — the attributes this story relaxes.

### Enables Stories

- None.

## Testing Strategy

- **Integration tests**: `cargo test -p gup-core --test compile_fail`.
- **Seeds**: AC3, by hand.

## Success Metrics

- [x] Neither error for a string accessor passed to `attr` says "`IntoEncoding`
      … is implemented for".
- [x] A new `ColumnValue` type leaves every snapshot unchanged.

## Risk Assessment

- **Low**: the probe used a scratch file, so only the line numbers were
  translated into AC2's text. _Mitigation_: the first escalation trigger.

## Definition of Done

- [x] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [x] All Acceptance Criteria are satisfied and checked
- [x] All tests pass: `cargo test -- --test-threads=1`
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] Rendered output verified by eye: not applicable (compile errors only)
- [x] `mask wasm-browser`: not needed (no layout, input or wasm code changes)
- [x] WASM size: unchanged: a diagnostic attribute only
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

- `crates/gup-core/src/encoding.rs`: removed `#[diagnostic::do_not_recommend]`
  from `impl ColumnValue for f64` only; rewrote the comment above `ColumnValue`.
  No other code change.
- `crates/gup-core/tests/compile_fail/borrowing_accessor.rs`: restored to the
  `attr` form (AC2 text, verbatim).
- `crates/gup-core/tests/compile_fail/borrowing_accessor.stderr`: re-blessed
  with `TRYBUILD=overwrite`; compared with AC2's text by `diff`: identical (47
  lines). No other `.stderr` changed.
- `crates/gup-core/tests/compile_fail.rs`: header's "What changes a snapshot"
  paragraph updated (AC4).

Test evidence:

- `cargo test -p gup-core --test compile_fail` with `TRYBUILD` unset: 3 passed.
- `cargo test -p gup-core -- --test-threads=1`: all suites pass (195 lib unit
  tests; the 3 ignored ones were already ignored before this story).
- `cargo check --examples`: passes.
- The stale-hint grep over `crates/gup-core/tests` for "implemented for
  .Encoded" prints nothing.

Seeds (AC3, temporary, reverted; `encoding.rs` is back to the committed state):

- Seed 1 (`SeedCol` with `do_not_recommend`): every snapshot passes.
- Seed 2 (`SeedEnc` with a new `IntoEncoding` impl): `borrowing_accessor`
  passes; `bare_number` changes, as the header predicted. Its "the following
  other types implement trait `IntoEncoding`" list gains the new impl. This is
  not an escalation trigger, which covers only `ColumnValue` seeds.

Commits: `c6137e0b` (start), `6b21f67c` (change).

WASM size was not measured: the change removes a diagnostic attribute and adds
no generated code.

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### Hiding one impl changes the second error, not only the first

- **Challenge**: GUP-511 put `do_not_recommend` on all four `ColumnValue` impls.
  That removed the candidate list from the `encode` error, but the `attr` error
  then fell back to the root `IntoEncoding` impl and printed a seven-line
  header.
- **Solution**: Leave one impl without the attribute. Rustc then names that impl
  in every "not a column value" error. `f64` is the choice because most
  accessors return it, and integers convert with `as f64`.
- **Pattern**: When a diagnostic falls back to a candidate list, the attribute
  decides which impl gets suggested. Pick the one users should see.

#### The snapshot should use the form users write

- **Challenge**: GUP-511 narrowed the case to `let _ = ...encode(..)`, which has
  one error, so the `attr` message had no snapshot at all.
- **Solution**: Return `borrowing_accessor.rs` to the `attr` form. One snapshot
  now covers both errors.
- **Pattern**: Snapshot the call form a user writes, not the smallest form that
  reproduces the bound failure.

### Architectural Decisions

#### One recommended `ColumnValue` impl

- **Decision**: `f64` is the only impl without `do_not_recommend`.
- **Reasoning**: Rustc needs exactly one impl to name, and `f64` is the type
  accessors most often return.
- **Trade-off**: A column type that should be suggested must be the unmarked
  one.
- **Future**: A new `ColumnValue` type leaves every snapshot unchanged, as seed
  1 showed.

### Development Workflow Insights

- Blessing with `TRYBUILD=overwrite` rewrote only the failing case; the other
  `.stderr` files stayed untouched.
- Comparing the blessed file with the AC2 block by `diff` (after extracting the
  block with `awk`) is a quick exact check.
- Seed 2's full `bare_number` output is in the seed run's log only; the story
  records the outcome.

### Follow-up Stories

None. The `bare_number` list growing with each `IntoEncoding` impl is already
the known behaviour in `compile_fail.rs`'s header (GUP-500).
