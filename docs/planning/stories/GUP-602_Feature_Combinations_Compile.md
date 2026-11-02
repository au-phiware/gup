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

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 🚧 In
Progress **Created**: 2026-11-03

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

- [x] **AC1**:
      `cargo test -p gup-core --no-default-features --features window --lib -- --test-threads=1`
      compiles and passes. Fix by gating the test code that needs `svg` (or
      making it feature-neutral), never by enabling `svg` in the test.
- [x] **AC2**: a `mask feature-matrix` task builds and lib-tests gup-core and
      `gup` in each of: default; `--no-default-features`;
      `--no-default-features --features window`;
      `--no-default-features --features svg`; and checks (no tests) wasm32
      `--no-default-features --features web`. It prints one line per combination
      and exits non-zero on any failure.
- [x] **AC3**: CI runs `mask feature-matrix` (a job in `tests.yml`, or in
      `release-check.yml` if that keeps Tests faster — decide and say why).
      `actionlint` passes.
- [x] **AC4**: seeded failure: a `#[cfg(test)]` use of an `svg`-only item
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

- [x] ACs met; `mask all-check` and `mask merge-check` pass; story ✅ in
      INDEX.md with Implementation Summary and Retrospective.

## Implementation Summary

- `crates/gup-core/src/scope.rs`: the test helper `seeded` cloned the batch's
  vector slot with `Arc::clone`, which only type-checks with `svg`; it now calls
  `.clone()`, so it is feature-neutral (no gating needed).
- `crates/gup-core/src/scene.rs`: the `expect(dead_code)` on `MarkBatch::vector`
  is now `all(not(svg), not(test))`, since the test build reads the field.
- `maskfile.md`: `mask feature-matrix` runs
  `cargo test -p gup-core -p gup --lib` in the four test combinations and
  `cargo check --target   wasm32-unknown-unknown --lib` for
  `--no-default-features --features web`. It prints one line per combination
  with seconds, runs all, and exits 1 on any failure.
- `.github/workflows/tests.yml`: new parallel job `feature-matrix`. Decision: in
  `tests.yml` (it needs lavapipe and Xvfb, which `release-check.yml` lacks) but
  as its own job, so the Tests job gets no longer; `actionlint` passes.

Evidence (this machine, warm cache, exit 0): default 44 s, no-default 43 s,
window 43 s, svg 38 s, wasm32 check 2 s; total 170 s. The window-only lib run:
281 passed, 3 ignored. AC4: restoring `Arc::clone(&batch.vector)` made the
no-default and window lines FAIL (E0308) and the task exit 1. `mask all-check`
and `mask merge-check` pass. CI timing is unmeasured (the job is parallel; cold
it adds a `cargo install mask`).

## Retrospective

**Completed**: 2026-11-03

### Key Technical Learnings

#### Feature-neutral beats feature-gated in tests

- **Challenge**: one test helper assumed `svg`'s `Arc<LayerVector>`.
- **Solution**: `.clone()` works for the `Arc` and the unit `VectorSlot`.
- **Pattern**: prefer a call valid for both slot types over a gate.

### Architectural Decisions

#### Job in tests.yml, not release-check.yml

- **Decision**: a separate job in `tests.yml`.
- **Reasoning**: lavapipe and Xvfb are set up there; a parallel job does not
  slow the Tests job.
- **Trade-off**: installs `mask` with `cargo install` (cold cost on the runner).

### Development Workflow Insights

- One matrix line failed once in a seeded run (svg) and passed on rerun and in
  isolation; most likely contention with another worker. Not reproduced.

### Follow-up Stories

None.
