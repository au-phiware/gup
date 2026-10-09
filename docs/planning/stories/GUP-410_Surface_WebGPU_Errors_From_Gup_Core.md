# GUP-410: Surface WebGPU Errors from gup-core on Every Target

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 🚧 In Progress **Created**:
2026-10-09

## Context

[GUP-408](GUP-408_Browser_Smoke_Test_In_CI.md) seeded a WGSL error that only
Chrome rejects: a derivative in non-uniform control flow in `rect.wgsl`. naga
accepts it, so every native test passed. In the browser, `ImageTarget::render`
returned `Ok` with a blank image. The only trace was four "rendering" warnings
in Chrome's console: the shader module, the pipeline, the command buffer and the
submit were each invalid.

Native and wasm differ here:

- **Native**: wgpu-core's default uncaptured-error handler panics, so a bad
  pipeline aborts the program with a validation message.
- **wasm32**: wgpu's WebGPU backend installs no handler (`on_uncaptured_error`
  is only set if the host calls it). Errors become browser console warnings, and
  gup-core reports success.

gup-core uses no error scopes. GUP-406 noted that invalid glue now surfaces as
an uncaptured validation error at pipeline creation instead of `Error::Compose`,
and deferred error scopes to S5. GUP-408 shows the consequence on wasm is worse
than a panic: a silent wrong result. The browser smoke test catches it in CI
only because it watches Chrome's console. A user's app cannot.

## User Story

> "As a developer embedding a Gup chart in a web page, I want a GPU validation
> failure to come back as an error from `render`, so that I can report it
> instead of showing a blank chart."

## Acceptance Criteria

- [ ] Shader module and pipeline creation in gup-core run inside a validation
      error scope (`Device::push_error_scope` / `pop_error_scope`). A failure
      becomes a `gup_core::Error` that names the pipeline (its label and glue
      signature) and carries wgpu's message, on native and on wasm32.
- [ ] Rendering through every target (`ImageTarget`, `TextureTarget`,
      `WindowTarget`) also scopes its encoding and submission. A failure is an
      `Err` from `render`, not a panic and not `Ok` with a blank image. Hosts
      that own the pass (`Prepared::draw`) get the error from preparation.
- [ ] Pipelines are cached, so a failed creation is not cached: the next render
      reports the same error again rather than reusing an invalid pipeline.
- [ ] Seeded proof (GUP-398 convention): GUP-408's `dpdx` seed in `rect.wgsl`
      makes `mask wasm-browser` report the gup-core error text through the
      page's `GUP FAIL` line (not only through Chrome's warnings). A seeded
      invalid pipeline on native (for example a bind group layout mismatch)
      returns `Err` from a unit test instead of panicking.
- [ ] The cost is measured: pipeline creation and one cached-pipeline render,
      before and after, on native (`selection.rs`'s timing test or a Criterion
      bench). Popping a scope is async on every backend; record whether it adds
      a round trip to the cached render path, and keep scopes off that path if
      it does.

## Technical Tasks

- [ ] Add an `Error::Gpu { what, message }` variant (or extend an existing one)
      with context naming the pipeline or target.
- [ ] Wrap `create_shader_module` + `create_render_pipeline` in `PipelineCache`
      (`render.rs`) in an error scope. Creation is synchronous today, so decide
      where the async pop happens: `Plot::resolve` / `Prepared` creation, or the
      first `render`.
- [ ] Scope encoding + `Context::submit` in each target's `render`.
- [ ] Do not cache a pipeline whose scope reported an error.
- [ ] Extend `wasm-size/scatter`'s page to print `render_scatter`'s `Err` text
      (it already does through `GUP FAIL`; check the message is useful).
- [ ] Document the behaviour in gup-core's crate docs (errors from `render`).

## Dependencies

### Prerequisite Stories

- GUP-406 ✅ (WGSL-only shader path; the glue errors now reach wgpu)
- GUP-408 ✅ (browser smoke test; the seed and the harness)

### Enables Stories

- RFC-001 S5's `#[wgsl_function]`: user WGSL can then fail pipeline creation,
  and needs this error path.
- RFC-001 S8's wasm entry point: hosts get an error instead of a blank canvas.

## Testing Strategy

- Native unit test: a deliberately invalid pipeline (seeded through a
  `#[cfg(test)]` hook or a test-only glue) returns `Err`, and the next render
  returns it again.
- `mask wasm-browser` with GUP-408's `dpdx` seed: the page's `GUP FAIL` line
  carries gup-core's error.
- `mask ci visual-regression` and the Tests workflow stay green.

## Success Metrics

- No GPU validation failure in gup-core ends as `Ok` with wrong pixels on any
  target.
- The cached render path's cost is unchanged within noise.

## Risk Assessment

- **Medium**: `pop_error_scope` is async. Pipeline creation is synchronous in
  `PipelineCache` today, so the API may need to change where creation is
  awaited. Mitigation: scope creation once per pipeline (cached), not per frame.
- **Low**: wgpu's error-scope semantics differ slightly between wgpu-core and
  the browser (scopes are per device, and the browser may report later).
  Mitigation: the browser smoke test is the check.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] Story status is updated in the story file and INDEX.md
- [ ] A retrospective is added
