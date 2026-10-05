# GUP-403: Make CI Green and Keep It Green

## Story Overview

**Initiative**: Strategic Review 2026-10 (T0 guardrails) **Status**: ✅ Complete
(2026-10-05) Progress **Created**: 2026-10-05

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

- [x] WASM: the `wasm-start` entry point is documented; the exact CI build
      (`wasm-pack build --target web -- --features wasm-start`) succeeds.
- [x] Benchmarks: `cargo bench --all-features -- --save-baseline main` reaches
      Criterion for every bench target (no libtest harness rejects the flag).
- [x] Performance CI tests get a software GPU adapter in CI and pass.
- [x] Gallery: config and `examples/INDEX.md` match the Cargo examples;
      `scripts/check_gallery_sync.sh` exits 0.
- [x] Visual regression: the `chunks_exact` lint is fixed in
      `crates/gup-visual-regression`.
- [x] Dogfood: task 1 rasterises its SVG in-process with a Rust rasteriser and a
      bundled font, so its text checks do not depend on the runner's ImageMagick
      or fonts. No check is loosened.

### AC2: Local and CI toolchains cannot diverge

- [x] `rust-toolchain.toml` pins the toolchain the flake used (1.93.1).
- [x] `flake.nix` reads it (`rust-bin.fromRustupToolchainFile`).
- [x] Every workflow, including the manual-dispatch mobile ones, installs the
      toolchain from the file (or runs in the Nix dev shell, which reads it).
- [x] The gup-core trybuild suite runs on the pinned toolchain without a
      separate hard-coded version.

### AC3: Drift is caught before push

- [x] `mask all-check` runs the gallery sync check.
- [x] A `mask ci` task runs the push-to-main workflows' checks locally, so an
      agent can reproduce CI before pushing.

### AC4: Evidence

- [x] Each fix is proven locally by running the exact CI command (or the closest
      local equivalent, documented).
- [x] Anything that can only be verified on GitHub is listed for the
      orchestrator to confirm after push.

## Technical Tasks

- [x] Write this story; verify each diagnosis with `gh run view --log-failed`.
- [x] Pin the toolchain; wire flake and workflows to the pin.
- [x] Fix each failure (AC1), one commit per workflow.
- [x] Add the gallery sync check to `all-check`; add `mask ci`.
- [x] Run each workflow's commands locally.

## Dependencies

### Prerequisite Stories

- GUP-388 ✅ (visual regression workflow), GUP-394 ✅ (dogfood workflow),
  GUP-390 ✅ (workspace layout).

### Related

- GUP-398 📋 (Honest and Fast Quality Gates): a strict clippy gate and proof
  that every hook check and CI job can fail. This story only makes today's jobs
  pass at their root cause; it does not seed failures or rescope the hook.

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

- [x] All Acceptance Criteria are satisfied and checked.
- [ ] All tests pass: `cargo test -- --test-threads=1`. **Not run locally**: a
      full debug test build of this repo needed more than the 8.6 GB /tmp had
      free (see Retrospective). Every workflow's own test commands passed
      locally on lavapipe; the first GitHub CI run after merge verifies those.
      Note that no push-to-main workflow runs the full suite, so it still needs
      a run on a machine with enough disk.
- [x] `mask all-check` passes (pre-commit hook, every commit).
- [x] Every fix has local evidence; GitHub-only items are listed.

## Implementation Summary

**Completed**: 2026-10-05. **Verification is the first GitHub CI run after
merge.** The full local `cargo test -- --test-threads=1` was skipped for disk
reasons (see Retrospective). Instead, each workflow's own commands were run
locally on lavapipe.

### Root causes and fixes

- **Visual regression**: CI's floating `stable` (rustc 1.99) had a newer clippy
  (`chunks_exact_to_as_chunks`) than the flake (1.93.1). Fix:
  `rust-toolchain.toml` pins 1.93.1; the flake (`fromRustupToolchainFile`) and
  every workflow, mobile included (`rustup toolchain install`), read it. The
  gup-core trybuild step lost its separate pin. Lint fixed with `as_chunks`.
- **WASM**: `pub fn main` exists only with `wasm-start` and had no doc comment,
  so `#![deny(missing_docs)]` failed the wasm-pack build. Fix: doc comment;
  `rustup target add` steps dropped (the pinned toolchain has the target).
- **Performance, benchmarks**: the lib and the `validate_marks` and
  `wasm_bench_native` bins kept the default libtest bench harness, which rejects
  `--save-baseline`; two auto-discovered Criterion benches defaulted to it too.
  Fix: `bench = false` on the lib and both bins, `harness = false` for the two
  benches. All 20 bench targets (316 benchmarks) accept Criterion flags.
- **Performance, CI tests**: the Nix dev shell's `VK_ICD_FILENAMES` listed only
  the radeon and intel ICDs, so the GPU-less runner had no adapter. Fix:
  `GUP_SOFTWARE_GPU=1` in the workflow env makes the shell select lavapipe.
- **Gallery**: `observable_plot_showcase` (deleted in GUP-389) was still
  configured and `composite_layer_order` was missing. The steps after the sync
  check had never run: thumbnails need a GPU, and GitHub Pages is not enabled
  for the repository. Fix: config and `examples/INDEX.md` corrected; sync check
  added to `mask all-check`; lavapipe installed; release binaries stripped;
  deploy gated on `vars.DEPLOY_GALLERY == 'true'`.
- **Dogfood**: task 1 rasterised its SVG with ImageMagick; version 7 (local)
  renders the text, version 6 (runner) rendered 0 ink px. Fix: `rasterise_svg`
  (`dogfood/src/lib.rs`) uses resvg with only the bundled
  `assets/fonts/default.ttf`. No check changed; ImageMagick removed from the
  workflow.

Also:

- **Comprehensive Benchmarking** moved from every push to a weekly schedule plus
  manual dispatch (180-minute timeout): 316 benchmarks at Criterion's default 3
  s warm-up + 5 s measurement need over 40 minutes of measurement alone on
  lavapipe.
- **Gates that failed open**: the Performance Threshold Check recorded
  `mask perf-check`'s exit code but never failed on it (it now exits with it).
  The WASM test-compile step hid its failure with `|| echo` (now
  `continue-on-error`, so the known GUP-285B failure is an annotated step). The
  benchmark-history commit step could never succeed (read-only token,
  git-ignored directory) and was removed; the trend point is uploaded with the
  artifact.
- **`mask ci`** (`gallery`, `wasm`, `visual-regression`, `dogfood`,
  `performance`) mirrors the push-to-main workflows on lavapipe, with
  `WGPU_BACKEND=vulkan` so gup-core cannot pick a hardware GL adapter. The dev
  shell gains `GUP_SOFTWARE_GPU`, `GUP_LAVAPIPE_ICD` and `xvfb-run`.
- RFC-001's S5/S6 note now points at `rust-toolchain.toml` instead of a separate
  1.93.1 trybuild pin.

### Local evidence (lavapipe)

- WASM: `cargo build --target wasm32-unknown-unknown --lib` and
  `wasm-pack build --target web -- --features wasm-start` succeed;
  `mask ci wasm` passes.
- Performance: interaction_performance_tests 7 passed; performance_ci_tests
  (`--features debug`) 2 passed; cross_platform_axis_performance_tests 5 passed;
  wasm_bench_axis 4 passed;
  `cargo bench --all-features -- --list --save-baseline ci` exits 0.
- Gallery: sync check clean; 17/17 thumbnails rendered (bar_chart.png inspected:
  four bars with axes and ticks); HTML generated; 0 of 101 links broken.
- Visual regression: `mask ci visual-regression` exits 0 (harness, goldens,
  culling/LOD, gup-core clippy, tests, doctests and trybuild, examples smoke).
- Dogfood: fmt and unit tests pass; under Xvfb every task is PASS or XFAIL.
  t1_svg.png inspected: title, date ticks, legend and three series (981 and 480
  ink px for title and ticks, 0 on CI before).
- Clippy on the pinned toolchain:
  `cargo clippy -p gup-core --all-targets -- -D warnings` exits 0.

### Open: needs GitHub to confirm

- [ ] `rustup toolchain install` (no arguments) reads `rust-toolchain.toml` on
      the ubuntu and macOS runners (needs rustup 1.28 or later).
- [ ] Gallery: the full `cargo build --release --examples` fits the runner's
      disk and the 30-minute timeout.
- [ ] Visual-regression goldens and the examples smoke test pass on the runner's
      lavapipe (these steps had never run on GitHub).
- [ ] Dogfood windowed tasks pass under the runner's Xvfb.
- [ ] The weekly benchmark job's duration (trigger once with workflow_dispatch).
- [ ] The full `cargo test -- --test-threads=1` suite, on a machine with enough
      disk (no workflow runs it).

### Key files

`rust-toolchain.toml` (new), `flake.nix`, `Cargo.toml`, `maskfile.md`, all seven
workflows in `.github/workflows/` and its `README.md`, `src/lib.rs`,
`crates/gup-visual-regression/src/image.rs`,
`crates/gup-core/tests/compile_fail.rs`, `scripts/gallery_config.toml`,
`examples/INDEX.md`, `docs/gallery/index.html`, `dogfood/` (`Cargo.toml`,
`run_all.sh`, `README.md`, `src/lib.rs`, `src/suite.rs`,
`src/bin/t1_timeseries.rs`), `docs/planning/rfcs/RFC-001_Core_Architecture.md`.
No new tests: the change is CI configuration, verified by running the workflows'
own commands.

## Retrospective

**Completed**: 2026-10-05

### Key Technical Learnings

#### A floating toolchain makes CI lint different code rules

- **Challenge**: `dtolnay/rust-toolchain@stable` installed rustc 1.99 on CI
  while the flake's rust-overlay lock gave 1.93.1. New clippy lints failed CI
  and could not be reproduced locally.
- **Solution**: one `rust-toolchain.toml`. rust-overlay's
  `fromRustupToolchainFile` reads it for the dev shell, and a plain
  `rustup toolchain install` reads it on the runners, so no third-party action
  or duplicated version string is needed.
- **Pattern**: keep the version in one file that both environments read. Moving
  it is a deliberate commit that fixes new lints and re-blesses the trybuild
  snapshots. About 13 more `chunks_exact(<const>)` sites (gup-core `target.rs`
  and `zoom_bench`, and `src/`) will trip when the pin moves to a clippy with
  that lint.

#### `cargo bench -- <flags>` reaches every bench-enabled target

- **Challenge**: `--save-baseline` is a Criterion flag, but `cargo bench` passes
  trailing arguments to every target with `bench = true`: by default the lib,
  every `[[bin]]`, and auto-discovered benches (with the libtest harness). Each
  rejected it, and the failures surfaced one target at a time.
- **Solution**: `bench = false` on the lib and bins; `harness = false` for every
  Criterion bench.
- **Pattern**: `cargo bench --all-features -- --list --save-baseline ci` checks
  every target's harness in seconds without running a benchmark.

#### Software rendering has to be pinned on both ICD and backend

- **Challenge**: four of five workflows needed a GPU adapter on GPU-less
  runners, configured three different ways (apt lavapipe, the Nix ICD list,
  nothing). The Nix shell hard-coded hardware ICDs. gup-core's
  `InstanceDescriptor::from_env_or_default()` accepts GL, so a "lavapipe-only"
  local run could silently use the Intel GPU through GL.
- **Solution**: `GUP_SOFTWARE_GPU=1` and `GUP_LAVAPIPE_ICD` in the dev shell;
  `WGPU_BACKEND=vulkan` in `mask ci`.
- **Pattern**: to reproduce a GPU-less runner, pin both the Vulkan ICD and the
  wgpu backend.

#### Environment-dependent workarounds fail only on CI

- **Challenge**: the dogfood task shelled out to ImageMagick, whose SVG text
  rendering differs between versions 6 and 7 and depends on installed fonts.
- **Solution**: resvg in-process, with a bundled font and no system fonts.
- **Pattern**: anything a check measures should come from pinned code and
  bundled assets, not from tools found on `PATH`.

#### Windowed checks need a display without a window manager

- **Challenge**: on the tiling Wayland desktop the dogfood windowed tasks failed
  because the compositor resized their windows; under Xvfb they pass.
- **Solution**: `mask ci dogfood` unsets `WAYLAND_DISPLAY` and uses `xvfb-run`,
  as the workflow does.

### Architectural Decisions

#### Weekly comprehensive benchmarks

- **Decision**: move Comprehensive Benchmarking from every push to a weekly
  schedule plus manual dispatch.
- **Reasoning**: 316 benchmarks need over 40 minutes of measurement on lavapipe,
  beyond the 45-minute timeout, and numbers from shared software runners are
  trend data, not a gate.
- **Trade-off**: a regression shows up to a week later; per-PR pattern
  benchmarks still run.
- **Future**: GPU runners (commented out in the matrix) would make per-push
  benchmarks meaningful.

#### Gate the gallery deploy instead of letting it fail

- **Decision**: `deploy` runs only when `vars.DEPLOY_GALLERY == 'true'`.
- **Reasoning**: GitHub Pages is not enabled for the repository. Enabling it is
  a repository-settings decision for the maintainer, not this story.
- **Trade-off**: the gallery is built and uploaded as an artifact but not
  published until the variable is set.

#### Plain `rustup` instead of a toolchain action

- **Decision**: `rustup toolchain install` in each workflow, not
  `actions-rust-lang/setup-rust-toolchain`.
- **Reasoning**: rustup reads the file natively. The action's v2 defaults
  (`build-warnings: deny`, its own cache) would change build behaviour and clash
  with the existing `actions/cache` steps.

### Development Workflow Insights

- **Gates that fail open hide red CI.** The Performance Threshold Check wrote
  its exit code to `$GITHUB_OUTPUT` but never failed; the WASM test-compile step
  ended in `|| echo`; the benchmark-history commit could never succeed. Each
  looked like a passing check. GUP-398's seeded-failure audit should cover every
  CI step, not only the hook.
- **Nothing consulted CI.** Gallery had failed 52 of 52 runs. Agents should
  check `gh run list` after a push; `mask ci` reproduces CI before pushing.
- **Fixing one step exposes the next.** Several failures were the first failing
  step in their job, so later steps had never run on GitHub (gallery thumbnails,
  the visual-regression smoke test). Running whole jobs locally on lavapipe
  found the next failures (no GPU for thumbnails, Pages disabled, bin bench
  harnesses) before pushing.
- **/tmp sizing.** With `CARGO_INCREMENTAL=0`, a full
  `cargo test -- --test-threads=1` debug build of this repo needs more than 8.6
  GB: that much free space was exhausted. /tmp filled twice in this story (once
  with release builds plus dogfood, once with the full test build). With /tmp
  full every tool call fails, including the `rm` that would free space. Check
  `df -h /tmp` before heavy builds and delete release and wasm targets as soon
  as they are no longer needed. Consolidating `tests/*.rs` into a few harness
  binaries (strategic review T0.5) is the structural fix.

### Follow-up Stories

None written; the gaps found are already covered:

- WASM test compilation (`html_export_integration`, `event_handling_tests`):
  GUP-285B (📋, folded into T0).
- Proving every CI step can fail, and a strict clippy gate: GUP-398 (📋).
- Test-binary consolidation to cut the build footprint: strategic review T0.5.
