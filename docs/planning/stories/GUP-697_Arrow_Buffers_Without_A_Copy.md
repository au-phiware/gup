---
implementer: opus
implementer-reason:
  adds a `Column` variant over a foreign shared buffer, which every reader of
  `Column` (the slice gather, `shared()`, `read_value`, the length check) must
  accept
touches:
  [
    crates/gup-core/src/columns.rs,
    crates/gup-core/src/columns_arrow.rs,
    crates/gup-core/src/column.rs,
    crates/gup-core/src/selection.rs,
  ]
---

# GUP-697: Arrow Buffers Without a Copy

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 💡 New (2026-11-09)

## Context

GUP-630 imports a `Float64` or `Float32` Arrow array by copying it once into an
`Arc<[f64]>` or `Arc<[f32]>` (no nulls: a copy; nulls: a copy and a patch).
`Arc<[T]>` cannot adopt an Arrow buffer, because the `Arc` header lives in front
of the values. Measured (release, 10M rows, 80 MB, Intel HD 630 machine):
`from_arrow` of a Float64 column takes 31 ms (41 ms with 1 in 7 null; Int64 32
ms), against 93 ms for the columnar first resolve (GUP-628 findings). It is a
third of the resolve and one more copy of the data in memory (80 MB at 10M).

## User Story

> "As a developer charting 100M rows from a Parquet reader, I want Gup to read
> my Arrow buffers where they are, so that the import costs neither the time nor
> the memory of a copy."

## Acceptance Criteria

- [ ] `Columns::from_arrow` shares a `Float64` or `Float32` array's values
      (sliced arrays included) without copying, when it has no nulls; with nulls
      it either copies (documented) or the validity bits are applied where the
      store writes the chunk.
- [ ] A `Column` variant over a foreign shared buffer (an owner trait object
      plus a slice; `Column` is `#[non_exhaustive]`) is read by every path that
      reads `Column::F64`: the slice gather, `shared()`, `read_value` and the
      row-count check; a test per path.
- [ ] A test shows the pointer of the chunk source equals the Arrow buffer's,
      and the image equals the copied path's byte for byte.
- [ ] `scale_bench` has an Arrow path; the first-resolve and RSS rows at 10M are
      recorded in the story.

## Decisions

- Decide between a generic owner variant and an Arrow-specific one (it must not
  put Arrow in a default build's public types).
- Integers, dates and timestamps convert, so they still copy.

## Out of Scope

- Key columns (GUP-674 covers sharing them).

## Dependencies

- GUP-630 ✅, GUP-664 ✅.

## Testing Strategy

- Per-path unit tests; one byte-for-byte render comparison.

## Success Metrics

- Import time of a 10M-row Float64 batch under 1 ms.

## Risk Assessment

- **Medium**: a new `Column` variant touches every reader. _Mitigation_: the
  match arms are exhaustive inside the crate, so the compiler finds each one.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] `mask all-fix`, `mask feature-matrix`, `mask wasm-size` unchanged
- [ ] Retrospective added to the story document
