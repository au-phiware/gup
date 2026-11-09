---
implementer: sonnet
implementer-reason:
  a feature-gated conversion from Arrow arrays into GUP-628/629's column types,
  with the type table and error rules settled in Decisions
touches:
  [
    Cargo.toml,
    crates/gup-core/Cargo.toml,
    crates/gup-core/src/columns.rs,
    crates/gup-core/src/lib.rs,
    src/lib.rs,
    maskfile.md,
    Cargo.lock,
  ]
---

# GUP-630: Arrow Record Batches

## Story Overview

**Initiative**: Track N5: Performance at Scale **Status**: 🚧 In Progress
**Created**: 2026-11-04

## Context

The November review names "Arrow behind a feature" as the second half of
columnar input. Arrow is how columnar data moves between Rust libraries (Polars,
DataFusion, Parquet readers, DuckDB), so one conversion from an Arrow
`RecordBatch` into `Columns` (GUP-628, GUP-629) lets all of them feed Gup
without a row type. Gup's features today are `window`, `svg` and `web`
(`crates/gup-core/Cargo.toml` and the root `Cargo.toml`, forwarded by `gup`),
and `mask feature-matrix` (GUP-602) builds every supported combination.

## Read First

- `crates/gup-core/src/columns.rs` after GUP-628 and GUP-629.
- `crates/gup-core/Cargo.toml` and `Cargo.toml`: `[features]` and
  `[package.metadata.docs.rs]` (`all-features = true`, wasm32 included).
- `maskfile.md` `## feature-matrix` (lines 28–73).
- `src/lib.rs` lines 1–30 (the features table in the crate docs).

## User Story

> "As a developer with data in Arrow (from Parquet, Polars or a database), I
> want to hand a record batch to Gup, so that I can chart it without copying it
> into structs."

## Acceptance Criteria

### AC1: Conversion

- [ ] With feature `arrow`,
      `Columns::from_arrow(batch: &RecordBatch) ->     Result<Columns>` converts
      every column per the table in Decisions, named by the schema's field
      names.
- [ ] An unsupported type is an error naming the column and its Arrow type, for
      example `column "when": Arrow type Interval(DayTime) is not supported`.
- [ ] Nulls: numeric nulls become NaN; key nulls become `NULL_CODE`.
- [ ] A test builds a batch with one column of each supported type (with a null
      in each) and checks every converted value; another renders a scatter from
      a two-column Float64 batch and compares it byte for byte with the same
      data through `Columns::new().with(..)`.

### AC2: Feature plumbing

- [ ] `gup-core` feature `arrow = ["dep:arrow-array", "dep:arrow-schema"]`;
      `gup` forwards it (`arrow = ["gup-core/arrow"]`); both crate docs' feature
      tables list it.
- [ ] `mask feature-matrix` adds `--no-default-features --features arrow` to its
      test combinations, and the wasm32 check also builds with `arrow` (docs.rs
      builds all features on wasm32).
- [ ] `mask wasm-size` is unchanged (the scatter does not enable `arrow`).
- [ ] `cargo deny`/`cargo audit` (whichever `mask all-check` runs) passes with
      the new dependencies, and their licences are compatible with
      GPL-3.0-or-later (Apache-2.0 is).

## Decisions

- **Dependencies**: `arrow-array` and `arrow-schema` only (not the `arrow`
  umbrella crate: smaller, no compute kernels), the latest release whose MSRV is
  at most the workspace's `rust-version` (1.93), `default-features = false`.
  Record the version chosen.
- **Type table**:
  - Float64: `F64` (one copy into `Arc<[f64]>`).
  - Float32: `F32` (one copy).
  - Int8 to Int64 and UInt8 to UInt64: `F64` (cast; values above 2^53 lose
    precision, documented).
  - Date32: `F64`, days × 86,400 (Unix seconds, for `Time`).
  - Date64 and Timestamp (any unit, any timezone): `F64`, Unix seconds (the
    timezone does not shift the instant).
  - Utf8, LargeUtf8 and Utf8View: `Keys`, through
    `KeyColumn::from_optional_labels`.
  - Dictionary (any integer key; Utf8 or LargeUtf8 values): `Keys`, with the
    dictionary's values as labels and its keys as codes (validated).
  - Boolean: `Keys` with labels `"false"` and `"true"`.
  - Anything else: an error.

- **Zero-copy**: not attempted. Arrow buffers are not `Arc<[T]>`, and a copy per
  numeric column is small next to the f32 conversion the store does anyway;
  revisit only if GUP-628's findings show the copy matters.
- **API home**: an inherent `Columns::from_arrow` under
  `#[cfg(feature = "arrow")]` (plus `impl TryFrom<&RecordBatch> for Columns`);
  no Arrow types in any other public signature.

## Escalation Triggers

- No `arrow-array` release supports Rust 1.93 (then report the MSRV gap).
- `arrow-array` does not build for `wasm32-unknown-unknown` with
  `default-features = false`.
- The dependency adds more than 60 s to a cold `mask all-check` (measure once).

## Out of Scope

- Polars or Parquet directly (they produce Arrow); builders taking `Columns`.
- Writing Arrow (export).

## Technical Tasks

- [ ] Dependencies and features; feature-matrix lines; crate docs tables.
- [ ] `Columns::from_arrow` and `TryFrom`; tests.
- [ ] `mask feature-matrix`, `mask wasm-size`, licence check.

## Dependencies

### Prerequisite Stories

- GUP-628: Columnar Layers 📋 — `Columns`, `Column`.
- GUP-629: Key Columns 📋 — `KeyColumn`.
- GUP-602: Feature Combinations Compile ✅ — `mask feature-matrix`.

## Testing Strategy

- **Unit tests**: per-type conversion with nulls; errors.
- **Integration tests**: identical image from an Arrow batch and from `Columns`.

## Success Metrics

- [ ] A Parquet-to-chart path needs only `Columns::from_arrow` and
      `add_columns`.

## Risk Assessment

- **Low**: an optional dependency behind a feature. _Mitigation_: the feature
  matrix and docs.rs builds catch breakage.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass:
      `cargo test -p gup-core --features arrow -- --test-threads=1` and
      `mask feature-matrix`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: not needed, identity is tested
- [ ] `mask wasm-browser`: not needed, no browser path changes
- [ ] WASM size: `mask wasm-size` scatter gz bytes, expected unchanged
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
