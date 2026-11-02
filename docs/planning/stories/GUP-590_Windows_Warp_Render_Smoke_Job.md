---
implementer: sonnet
implementer-reason:
  one new workflow job around existing golden tests, with a spike step and fixed
  rules for what to do with each outcome
touches: [.github/workflows/]
---

# GUP-590: Windows (WARP) Render Smoke Job

## Story Overview

**Initiative**: Strategic Review 2026-11 (N3 Release 0.1) **Status**: 📋 Planned
**Created**: 2026-11-02

## Context

CI renders on Linux only, through Mesa's lavapipe (Vulkan) and SwiftShader (the
browser job). The README's "Not yet" list says "CI renders on Linux only;
Windows comes in N3". A crates.io user on Windows would be the first to run
Gup's DX12 path. Nothing has checked that the shaders compile under DX12 (naga
to HLSL), that the sRGB handling matches, or that the tests' paths work with `\`
separators.

GitHub's `windows-latest` runners have no GPU. Windows always provides WARP, the
"Microsoft Basic Render Driver": a software D3D12 rasteriser that DXGI lists as
a software adapter. wgpu's DX12 backend enumerates it, and `Context::new`
honours `WGPU_BACKEND` (`crates/gup-core/src/context.rs`, around line 277).
`WGPU_BACKEND=dx12` therefore selects it. wgpu's own CI tests on WARP.

The root crate's integration tests are the right smoke set. They are
`tests/tutorials.rs` (every tutorial image as a golden, with the harness's
structural checks), `tests/gallery.rs` once GUP-594 lands, and
`tests/examples_smoke.rs` (its GPU-free skip-list test). Goldens were blessed on
Linux, and they already pass on two different rasterisers (an Intel HD 630
locally and lavapipe in CI) within the default tolerance: ΔE 3, at most 0.25% of
pixels. That makes it plausible, not certain, that WARP passes too.

## Read First

- `.github/workflows/tests.yml` lines 34–95: the `tests` job to model the new
  one on.
- `crates/gup-core/src/context.rs` lines 210–300 (`ContextOptions`,
  `WGPU_BACKEND`) and line 399 (`adapter_info`).
- `tests/tutorials.rs` lines 1–75: the harness and how failures report (artifact
  paths in the panic message).
- `crates/gup-visual-regression/src/golden.rs` lines 245–270
  (`default_artifact_dir`: where diff images go).

## User Story

> "As a Windows developer, I want CI to draw Gup's charts on Windows, so that
> `cargo add gup` works for me as it does on Linux."

## Acceptance Criteria

### AC1: Spike — the adapter

- [ ] A first commit adds `.github/workflows/platforms.yml` (name "Platforms")
      with a `windows` job on `windows-latest`, run on push to `main`, on pull
      requests and on `workflow_dispatch`. It sets `WGPU_BACKEND: dx12` and
      `CARGO_INCREMENTAL: "0"`, installs the toolchain with
      `rustup toolchain install`, and caches the cargo registry. Its first test
      step is a new test, `tests/platform_adapter.rs`, which prints
      `Context::shared()?.adapter_info()` (name, backend, device type) and
      asserts that the backend is `Dx12`.
- [ ] Record the printed adapter in the Implementation Summary. Apply the rule
      in Decisions to the result.

### AC2: The smoke set

- [ ] The job's second step is `cargo test -p gup --tests -- --test-threads=1`.
      That covers every root integration test, so `tests/gallery.rs` joins
      automatically when GUP-594 lands.
- [ ] On failure, the job uploads the harness's artifact directory (actual
      images and diff images) with `actions/upload-artifact@v4`, under the name
      `windows-golden-diffs`.
- [ ] The job passes on GitHub. Link the run in the Implementation Summary, with
      its wall time. If the branch cannot be pushed from the worker, say so; the
      orchestrator links it after the merge.

### AC3: Paths and line endings

- [ ] Any failure caused by a path separator, a drive letter or CRLF in a test's
      own code is fixed in the test (for example, `Path::join` instead of a `/`
      in a string). Each fix is listed in the Implementation Summary. Goldens
      are binary. If git converts any checked-in text fixture the tests compare,
      add the file to `.gitattributes` with `-text`.

### AC4: Docs

- [ ] `actionlint .github/workflows/*.y*ml` passes. `maskfile.md` is not
      changed: `mask ci` runs Linux workflows locally, and this job cannot run
      locally. Say so in a comment at the top of `platforms.yml`.

## Decisions

- **Its own workflow file**, `platforms.yml`, not a job in `tests.yml`. Windows
  builds are slow, and GUP-591 adds macOS to the same file. Neither may hold up
  the Linux Tests workflow.
- **The smoke set is `-p gup --tests`**, not `gup-core`'s suite. gup-core's
  tests include timing-sensitive and lavapipe-specific cases (the GPU pick
  counts, the trybuild snapshots, whose rustc output differs on Windows). The
  facade's integration tests exercise every builder through the public API, and
  their goldens are exactly the user-visible images.
- **Doctests are not run on Windows**: about 100 doctests, each linked
  separately, would take most of the job's time and add nothing that the
  integration tests do not draw.
- **Spike rule (AC1)**:
  - **The adapter is WARP on DX12**: continue with AC2.
  - **There is a DX12 adapter that is not WARP** (a runner image with a GPU):
    continue, and record the name.
  - **No DX12 adapter, or device creation fails**: stop and report the error. Do
    not switch to GL or Vulkan; that would test a path Windows users do not get
    by default.
- **Golden failures are not tolerated away.** If a golden fails on WARP beyond
  the default tolerance, stop and report. Give the case names, the printed
  `DiffStats`, and the uploaded diff images, read by eye, with what differs (for
  example, antialiasing at circle edges, or sRGB colours). Do not add a
  per-platform tolerance or per-platform goldens; that is the orchestrator's
  decision.
- **Timeout**: 90 minutes. Caching the whole `target/` is not used: it is
  several GB on Windows and evicts the other caches.

## Escalation Triggers

- AC1's rule says stop.
- A golden fails beyond tolerance (see Decisions).
- A test fails on Windows for a reason in library code, not test code. For
  example, a WGSL shader fails naga's HLSL translation, or a texture format is
  unsupported. Report the error. Do not change `gup-core` in this story.

## Out of Scope

- macOS: GUP-591.
- Running gup-core's, gup-egui's or gup-bevy's suites on Windows.
- Windows windowed examples (`gup::show` on a runner desktop).

## Technical Tasks

- [ ] Start commit within 30 minutes; `scripts/checkpoint.sh` every 20–30
      minutes.
- [ ] `platforms.yml` with the adapter test; push; read the adapter (AC1).
      Commit.
- [ ] The smoke step and artifact upload; iterate on test-code fixes (AC2, AC3).
      Commit after each fix.
- [ ] actionlint; DoD commands.

## Dependencies

### Prerequisite Stories

- None blocking. GUP-594 (gallery goldens) 📋 is soft: its tests join the job
  automatically.

### Enables Stories

- GUP-591: macOS smoke spike — adds a job to `platforms.yml`.
- GUP-597: README for release 0.1 — the "CI renders on Linux only" line.

## Testing Strategy

- **CI**: the `windows` job green on GitHub.
- **Local**: `cargo test -p gup --test platform_adapter` passes on Linux. On
  Linux the test asserts the backend only when `WGPU_BACKEND=dx12` is set, and
  prints the adapter otherwise.
- **Visual validation**: on a failure, the uploaded diff images, read by eye.

## Success Metrics

- [ ] Every push draws every tutorial image on Windows and compares it with its
      golden.

## Risk Assessment

- **Medium**: WARP rasterises analytic antialiasing or MSAA resolves differently
  enough to fail goldens. _Mitigation_: the spike step reports the adapter
  early. A golden failure is escalated with its numbers, not worked around.
- **Low**: the runner image's WARP is too old for a feature wgpu 27 asks for.
  _Mitigation_: the escalation reports the error. wgpu's CI fetches a newer WARP
  package, which is a known follow-up if needed.

## Definition of Done

- [ ] First commit within 30 minutes, then `scripts/checkpoint.sh` every 20-30
      minutes between commits
- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] Rendered output verified by eye: the CI job's goldens pass; diff images
      read if any failed
- [ ] `mask wasm-browser`: not needed; no layout, input or wasm code changes
- [ ] WASM size: unchanged, no library code change
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
