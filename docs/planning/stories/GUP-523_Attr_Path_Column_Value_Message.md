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

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 📋 Planned
**Created**: 2026-10-24

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

- [ ] `impl ColumnValue for f64` has no `#[diagnostic::do_not_recommend]`; the
      `f32`, `Option<f64>` and `Option<f32>` impls keep theirs.
- [ ] The comment above `ColumnValue` (lines 648–653 today) says: the library's
      impls carry `#[diagnostic::do_not_recommend]` except `f64`'s, which rustc
      then names in every "not a column value" error, in `encode` and in `attr`;
      a new column type's impl carries the attribute, so the snapshots do not
      change (GUP-511, GUP-523).

### AC2: The `attr` path has a snapshot

- [ ] `crates/gup-core/tests/compile_fail/borrowing_accessor.rs` is exactly:

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

- [ ] After
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

- [ ] `git diff --stat crates/gup-core/tests/` lists only
      `borrowing_accessor.rs`, `borrowing_accessor.stderr` and
      `compile_fail.rs`, and
      `git grep -n "implemented for .Encoded"     crates/gup-core/tests` prints
      nothing.
- [ ] `scripts/with_cargo_env.sh cargo test -p gup-core --test compile_fail`
      passes (all three tests) with `TRYBUILD` unset.

### AC3: Seeded proof of stability

Each seed is temporary: add it to `encoding.rs`, run
`scripts/with_cargo_env.sh cargo test -p gup-core --test compile_fail` without
`TRYBUILD`, remove it, and record the result in the retrospective.

- [ ] Seed 1: `#[derive(Clone, Copy)] pub struct SeedCol(f64);` with
      `#[diagnostic::do_not_recommend] impl ColumnValue for SeedCol`
      (`type Gpu = f32;`, `to_f64` returns `self.0`). Every snapshot passes.
- [ ] Seed 2: `pub struct SeedEnc;` and `pub enum SeedMarker {}` with
      `impl<T, V: Visual> IntoEncoding<T, V, SeedMarker> for SeedEnc`
      (`into_encoding` is `unimplemented!()`). `borrowing_accessor` passes;
      record whether `bare_number` changed (it may: `compile_fail.rs`'s header
      already says a new `IntoEncoding` impl changes it).

### AC4: Header updated

- [ ] The "What changes a snapshot" paragraph of `compile_fail.rs`'s header
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

- [ ] Remove the attribute from the `f64` impl; rewrite the comment.
- [ ] Restore `borrowing_accessor.rs`; overwrite the snapshot and compare it
      with AC2.
- [ ] Run seeds 1 and 2; remove them.
- [ ] Update `compile_fail.rs`'s header.

## Dependencies

### Prerequisite Stories

- GUP-511: Stable `ColumnValue` Snapshot ✅ — the attributes this story relaxes.

### Enables Stories

- None.

## Testing Strategy

- **Integration tests**: `cargo test -p gup-core --test compile_fail`.
- **Seeds**: AC3, by hand.

## Success Metrics

- [ ] Neither error for a string accessor passed to `attr` says "`IntoEncoding`
      … is implemented for".
- [ ] A new `ColumnValue` type leaves every snapshot unchanged.

## Risk Assessment

- **Low**: the probe used a scratch file, so only the line numbers were
  translated into AC2's text. _Mitigation_: the first escalation trigger.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not applicable (compile errors only)
- [ ] `mask wasm-browser`: not needed (no layout, input or wasm code changes)
- [ ] WASM size: unchanged: a diagnostic attribute only
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
