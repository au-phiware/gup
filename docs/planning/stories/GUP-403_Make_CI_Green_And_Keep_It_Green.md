# GUP-403: Make CI Green and Keep It Green

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: 🚧 In
Progress **Created**: 2026-10-05

## Context

The first push of 2026-10-05 (commit `a0560b9`) ran CI for the first time in
days. All five workflows that run on push to `main` failed, and CI had been red
on every earlier push too (the Gallery workflow has never passed: 52 of 52 runs
failed). Nothing in the story process consulted CI, so T0's guardrails (visual
regression, dogfood) were running on GitHub without anyone seeing their results.

Failures on `a0560b9`:

| Workflow (run)                | Failing step                               | Diagnosis                                                                                                                                                                                                                                 |
| ----------------------------- | ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| WASM Compilation Check (…837) | `wasm-pack build -- --features wasm-start` | `#![deny(missing_docs)]` fires on `pub fn main()` in `src/lib.rs`, which only exists with `wasm-start`. Pre-existing.                                                                                                                     |
| Performance Testing (…850)    | Comprehensive Benchmarking                 | `cargo bench -- --save-baseline main` passes the flag to the lib's default libtest bench harness, which rejects it. Pre-existing.                                                                                                         |
| Performance Testing (…850)    | Performance CI test suite                  | `tests/performance_ci_tests.rs:99` panics: no GPU adapter. The job runs in the Nix dev shell, whose `VK_ICD_FILENAMES` lists only radeon and intel ICDs, so software Vulkan (lavapipe) is invisible on the GPU-less runner. Pre-existing. |
| Gallery (…824)                | Check gallery config sync                  | `observable_plot_showcase` (deleted by GUP-389) is still in `scripts/gallery_config.toml`; `composite_layer_order` is missing from the config and `examples/INDEX.md`. The sync check is not in the local hook.                           |
| Visual regression (…803)      | gup-core clippy                            | CI's `dtolnay/rust-toolchain@stable` (rustc 1.99) has `clippy::chunks_exact_to_as_chunks`; the flake's rustc 1.93.1 does not. Local and CI toolchains drift.                                                                              |
| Dogfood (…777)                | Run dogfood suite                          | Task 1's workaround SVG is rasterised by shelling out to ImageMagick. On the runner (ImageMagick 6) the text renders with 0 ink px; locally (ImageMagick 7) it passes. The result depends on the environment.                             |

## User Story

> "As a maintainer, I want every workflow that runs on push to `main` to pass,
> and to fail only for real problems, so that a red CI badge means something and
> agents can rely on CI as a guardrail."

## Acceptance Criteria

### AC1: Each failure fixed at its root cause

- [ ] WASM: the `wasm-start` entry point is documented; the exact CI build
      (`wasm-pack build --target web -- --features wasm-start`) succeeds.
- [ ] Benchmarks: `cargo bench --all-features -- --save-baseline main` reaches
      Criterion for every bench target (no libtest harness rejects the flag).
- [ ] Performance CI tests get a software GPU adapter in CI and pass.
- [ ] Gallery: config and `examples/INDEX.md` match the Cargo examples;
      `scripts/check_gallery_sync.sh` exits 0.
- [ ] Visual regression: the `chunks_exact` lint is fixed in
      `crates/gup-visual-regression`.
- [ ] Dogfood: task 1 rasterises its SVG in-process with a Rust rasteriser and a
      bundled font, so its text checks do not depend on the runner's ImageMagick
      or fonts. No check is loosened.

### AC2: Local and CI toolchains cannot diverge

- [ ] `rust-toolchain.toml` pins the toolchain the flake used (1.93.1).
- [ ] `flake.nix` reads it (`rust-bin.fromRustupToolchainFile`).
- [ ] Every workflow, including the manual-dispatch mobile ones, installs the
      toolchain from the file (or runs in the Nix dev shell, which reads it).
- [ ] The gup-core trybuild suite runs on the pinned toolchain without a
      separate hard-coded version.

### AC3: Drift is caught before push

- [ ] `mask all-check` runs the gallery sync check.
- [ ] A `mask ci` task runs the push-to-main workflows' checks locally, so an
      agent can reproduce CI before pushing.

### AC4: Evidence

- [ ] Each fix is proven locally by running the exact CI command (or the closest
      local equivalent, documented).
- [ ] Anything that can only be verified on GitHub is listed for the
      orchestrator to confirm after push.

## Technical Tasks

- [ ] Write this story; verify each diagnosis with `gh run view --log-failed`.
- [ ] Pin the toolchain; wire flake and workflows to the pin.
- [ ] Fix each failure (AC1), one commit per workflow.
- [ ] Add the gallery sync check to `all-check`; add `mask ci`.
- [ ] Run each workflow's commands locally.

## Dependencies

### Prerequisite Stories

- GUP-388 ✅ (visual regression workflow), GUP-394 ✅ (dogfood workflow),
  GUP-390 ✅ (workspace layout).

### Related

- GUP-398 📋 (Honest Clippy Gate): the local clippy gate. This story only fixes
  the lints CI reports today.

## Testing Strategy

- Run each failing workflow's command locally, forcing software Vulkan where CI
  has no GPU.
- `cargo test -- --test-threads=1`, `mask all-check`.

## Success Metrics

- [ ] All push-to-main workflows pass on GitHub after the orchestrator pushes.

## Risk Assessment

- **Medium**: some steps never ran on GitHub because earlier steps failed (e.g.
  the visual-regression examples smoke test, gallery thumbnails). They may fail
  next. Mitigation: run them locally on lavapipe before pushing.
- **Low**: pinning the toolchain delays new lints. Moving the pin is a
  deliberate one-line change that updates local and CI together.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`.
- [ ] `mask all-check` passes.
- [ ] Every fix has local evidence; GitHub-only items are listed.
