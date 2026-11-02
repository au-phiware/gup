---
implementer: sonnet
implementer-reason:
  a compile fix plus a CI feature matrix; the failing command and file are known
touches:
  [
    crates/gup-core/src/scope.rs,
    crates/gup-core/src/,
    .github/workflows/tests.yml,
    maskfile.md,
  ]
---

# GUP-602: Every Supported Feature Combination Compiles and Tests

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 📋 Planned
**Created**: 2026-11-03

## Context

GUP-557 found that
`cargo test -p gup-core --no-default-features --features window --lib` does not
compile: a type mismatch at `scope.rs:338` (`MarkBatch::new` / `batch.vector`)
when the `svg` feature is off. The same error exists on main before GUP-557. The
library itself compiles in that combination (clippy passes), so only `cfg(test)`
code assumes `svg`. GUP-589 gave `gup` the features `window`, `svg` and `web`;
before 0.1, every combination a user can pick must build and test.

## User Story

> "As a user who turns off `svg` to save size, I want the crate's own tests and
> examples to prove that build works."

## Read First

- `crates/gup-core/Cargo.toml` and root `Cargo.toml` `[features]`.
- `crates/gup-core/src/scope.rs` around line 338 and every
  `#[cfg(feature = "svg")]` in `crates/gup-core/src/`.
- `.github/workflows/tests.yml` and `release-check.yml` (GUP-589).

## Acceptance Criteria

- [ ] **AC1**:
      `cargo test -p gup-core --no-default-features --features window     --lib -- --test-threads=1`
      compiles and passes. Fix by gating the test code that needs `svg` (or
      making it feature-neutral), never by enabling `svg` in the test.
- [ ] **AC2**: a `mask feature-matrix` task builds and lib-tests gup-core and
      `gup` in each of: default; `--no-default-features`;
      `--no-default-features     --features window`;
      `--no-default-features --features svg`; and checks (no tests) wasm32
      `--no-default-features --features web`. It prints one line per combination
      and exits non-zero on any failure.
- [ ] **AC3**: CI runs `mask feature-matrix` (a job in `tests.yml`, or in
      `release-check.yml` if that keeps Tests faster — decide and say why).
      `actionlint` passes.
- [ ] **AC4**: seeded failure: a `#[cfg(test)]` use of an `svg`-only item
      without a gate makes `mask feature-matrix` fail on the window-only line.

## Decisions

- Combinations are the five in AC2; `web` with `window` is not a target.
- Gate tests with `#[cfg(feature = "svg")]` at the test function, not the
  module, unless a whole module is SVG-only.

## Escalation Triggers

- A combination needs a library (not test) change that alters public API.
- The matrix adds more than 6 minutes to the CI job it lands in.

## Out of Scope

- Bevy and egui feature combinations.

## Definition of Done

- [ ] ACs met; `mask all-check` and `mask merge-check` pass; story ✅ in
      INDEX.md with Implementation Summary and Retrospective.
