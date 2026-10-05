# GUP-408: Run gup-core's Browser Smoke Test in CI

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-06

## Context

[GUP-401](GUP-401_RFC_001_S3_Scene_Renderer_RenderTarget.md) ran gup-core in a
real browser for the first time (headless Chromium, WebGPU). That run found two
bugs no native test could:

- `std::time::Instant::now` panics on wasm32;
- the glue's uniform layout differed because naga's WGSL writer drops `@align`.

Both are fixed. A native unit test now guards the second, but the general class,
behaviour that differs only in a browser, is caught only by `mask wasm-browser`,
which needs a local GPU and runs only by hand.

## User Story

> "As a Gup maintainer, I want every push to render a chart in a browser, so
> that browser-only regressions fail CI instead of reaching users."

## Acceptance Criteria

- [ ] A CI job builds the `wasm-size/scatter` harness and runs the
      `mask wasm-browser` page in headless Chromium on a GitHub runner, using a
      software WebGPU adapter (SwiftShader/Dawn) if no GPU is available. The job
      records which adapter ran.
- [ ] The job fails on `GUP FAIL`, on a timeout, or on any console error. It
      uploads the browser PNG as an artifact.
- [ ] A seeded regression (for example reintroducing `std::time::Instant` in a
      render path) makes the job fail. This is shown once and noted in the
      retrospective.
- [ ] The job takes under 10 minutes and caches the wasm build.

## Technical Tasks

- [ ] Find Chromium flags for software WebGPU on Linux runners
      (`--enable-unsafe-swiftshader` or Dawn's fallback adapter).
- [ ] Add the job to the visual-regression workflow, with Nix or a pinned
      Chromium.
- [ ] Make `mask wasm-browser` exit non-zero on console errors.

## Dependencies

### Prerequisite Stories

- GUP-401 ✅ (`mask wasm-browser`)

### Enables Stories

- RFC-001 S8's wasm entry point, tested on every push.

## Testing Strategy

- Run the job on a branch with a seeded failure, then without it.

## Success Metrics

- Browser regressions fail CI.

## Risk Assessment

- **Medium**: software WebGPU in headless Chromium may be unavailable or flaky
  on GitHub runners. Mitigation: record the outcome. If it is infeasible,
  document why and fall back to a nightly self-hosted run.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] Story status is updated in the story file and INDEX.md
- [ ] A retrospective is added
