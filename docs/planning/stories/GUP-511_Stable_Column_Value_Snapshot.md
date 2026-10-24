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

**Initiative**: Strategic Review 2026-11 (N0 Hygiene) **Status**: 💡 New
**Created**: 2026-10-24

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

- [ ] Find a way to keep the `ColumnValue` candidate list out of
      `borrowing_accessor.stderr` without adding the `IntoEncoding` hint (for
      example `do_not_recommend` on the generic `IntoEncoding` impl for
      `Encoded<A, S>`, or a case that fails the bound once), and re-bless.
- [ ] Seeded proof as GUP-500 AC3, with a temporary `ColumnValue` type.
- [ ] `compile_fail.rs`'s header is updated.

## Decisions

- Concrete impls only, as in GUP-500, unless the spike shows the generic one is
  needed; a generic impl changes which bound rustc reports, so check
  `borrowing_accessor`'s second error.

## Escalation Triggers

- Any `.stderr` line outside the `ColumnValue` lists changes for the worse.

## Out of Scope

- Other traits' snapshots; the toolchain pin.

## Technical Tasks

- [ ] Try the options; re-bless; seed; update the header.

## Dependencies

### Prerequisite Stories

- GUP-500 (complete).

## Testing Strategy

- `cargo test -p gup-core --test compile_fail`; the seed by hand.

## Success Metrics

- [ ] A new `ColumnValue` type leaves every snapshot unchanged.

## Risk Assessment

- **Low**: rustc may offer no way; then record it and close.

## Definition of Done

- [ ] Acceptance Criteria satisfied
- [ ] `mask all-fix`; `mask all-check` pass
- [ ] Story status updated in the file and `INDEX.md`
- [ ] Retrospective added
