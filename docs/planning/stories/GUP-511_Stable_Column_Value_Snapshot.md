---
implementer: sonnet
implementer-reason: a small spike with a rule
touches:
  [
    crates/gup-core/src/encoding.rs,
    crates/gup-core/tests/compile_fail/borrowing_accessor.rs,
    crates/gup-core/tests/compile_fail/borrowing_accessor.stderr,
  ]
---

# GUP-511: Stable `ColumnValue` Snapshot

## Story Overview

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: ✅ Complete
(2026-10-24) **Created**: 2026-10-24

## Context

GUP-500 stopped new scales and WGSL modules from changing compile-fail
snapshots, but not new `ColumnValue` types: `borrowing_accessor.stderr` lists
`ColumnValue`'s impls twice. `#[diagnostic::do_not_recommend]` on the four impls
removes the first list, but rustc replaces the second with an unrelated hint
("the trait `IntoEncoding<T, V, ColumnMarker<D>>` is implemented for
`Encoded<A, S>`"), so GUP-500 left it off.

## User Story

> "As a worker adding a public column type, I want `borrowing_accessor.stderr`
> to stay as it is, so that my story does not churn it."

## Acceptance Criteria

- [x] Find a way to keep the `ColumnValue` candidate list out of
      `borrowing_accessor.stderr` without adding the `IntoEncoding` hint (for
      example `do_not_recommend` on the generic `IntoEncoding` impl for
      `Encoded<A, S>`, or a case that fails the bound once), and re-bless.
- [x] Seeded proof as GUP-500 AC3, with a temporary `ColumnValue` type.
- [x] `compile_fail.rs`'s header is updated.

## Decisions

- Concrete impls only, as in GUP-500, unless the spike shows the generic one is
  needed; a generic impl changes which bound rustc reports, so check
  `borrowing_accessor`'s second error.

## Escalation Triggers

- Any `.stderr` line outside the `ColumnValue` lists changes for the worse.

## Out of Scope

- Other traits' snapshots; the toolchain pin.

## Technical Tasks

- [x] Try the options; re-bless; seed; update the header.

## Dependencies

### Prerequisite Stories

- GUP-500 (complete).

## Testing Strategy

- `cargo test -p gup-core --test compile_fail`; the seed by hand.

## Success Metrics

- [x] A new `ColumnValue` type leaves every snapshot unchanged.

## Risk Assessment

- **Low**: rustc may offer no way; then record it and close.

## Definition of Done

- [x] Acceptance Criteria satisfied
- [x] `mask all-fix`; `mask all-check` pass
- [x] Story status updated in the file and `INDEX.md`
- [x] Retrospective added

## Implementation Summary

- `#[diagnostic::do_not_recommend]` on the four `ColumnValue` impls
  (`encoding.rs`); `borrowing_accessor.rs` now calls `encode` without passing
  the result to `attr`, so the bound fails once. Snapshot shrank from two errors
  to one with no candidate list and no `IntoEncoding` hint. No other snapshot
  changed. `compile_fail.rs`'s header and the note above `ColumnValue` are
  updated.
- Seed (AC2): a temporary `pub struct SeedCol` with a `ColumnValue` impl
  carrying the attribute left every snapshot unchanged
  (`cargo test -p gup-core --test compile_fail`, 3 passed); the seed was
  removed.

## Retrospective

**Completed**: 2026-10-24

### Key Technical Learnings

#### Hiding impls works only when the bound fails once

- **Challenge**: with the attribute on `ColumnValue`'s impls, the second error
  (from `attr`) swapped its list for an `IntoEncoding` hint.
- **Attempt 1**: also `do_not_recommend` on the generic
  `IntoEncoding for Encoded<A, S>`: rejected. The second error became "cannot
  drive a channel of visual type", and `wrong_value_type.stderr` degraded (E0271
  became a "but trait ... is implemented for it" block).
- **Attempt 2 (kept)**: a case that fails the bound once
  (`let _ = ...encode(..)`).
- **Pattern**: the hint appears only when a second error carries the
  `IntoEncoding` obligation. Users who pass such an accessor to `attr` still see
  both errors, the second with the unrelated hint; the first error is the
  actionable one.

### Architectural Decisions

#### Test case changed, not the library

- **Decision**: shrink the compile-fail case rather than make the generic impl
  opaque. **Trade-off**: the snapshot no longer covers the `attr` path.

### Follow-up Stories

None.
