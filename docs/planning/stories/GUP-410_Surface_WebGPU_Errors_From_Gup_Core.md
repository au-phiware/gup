# GUP-410: Surface WebGPU Errors from gup-core on Every Target

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-10)
**Created**: 2026-10-09

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

- [x] Shader module and pipeline creation in gup-core run inside a validation
      error scope (`Device::push_error_scope` / `pop_error_scope`). A failure
      becomes a `gup_core::Error` that names the pipeline (its label and glue
      signature) and carries wgpu's message, on native and on wasm32.
- [x] Rendering through every target (`ImageTarget`, `TextureTarget`,
      `WindowTarget`) also scopes its encoding and submission. A failure is an
      `Err` from `render`, not a panic and not `Ok` with a blank image. Hosts
      that own the pass (`Prepared::draw`) get the error from preparation.
- [x] Pipelines are cached, so a failed creation is not cached: the next render
      reports the same error again rather than reusing an invalid pipeline.
- [x] Seeded proof (GUP-398 convention): GUP-408's `dpdx` seed in `rect.wgsl`
      makes `mask wasm-browser` report the gup-core error text through the
      page's `GUP FAIL` line (not only through Chrome's warnings). A seeded
      invalid pipeline on native (for example a bind group layout mismatch)
      returns `Err` from a unit test instead of panicking.
- [x] The cost is measured: pipeline creation and one cached-pipeline render,
      before and after, on native (`selection.rs`'s timing test or a Criterion
      bench). Popping a scope is async on every backend; record whether it adds
      a round trip to the cached render path, and keep scopes off that path if
      it does.

## Technical Tasks

- [x] Add an `Error::Gpu { what, message }` variant (or extend an existing one)
      with context naming the pipeline or target.
- [x] Wrap `create_shader_module` + `create_render_pipeline` in `PipelineCache`
      (`render.rs`) in an error scope. Creation is synchronous today, so decide
      where the async pop happens: `Plot::resolve` / `Prepared` creation, or the
      first `render`.
- [x] Scope encoding + `Context::submit` in each target's `render`.
- [x] Do not cache a pipeline whose scope reported an error.
- [x] Extend `wasm-size/scatter`'s page to print `render_scatter`'s `Err` text
      (it already does through `GUP FAIL`; check the message is useful).
- [x] Document the behaviour in gup-core's crate docs (errors from `render`).

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

- [x] All Acceptance Criteria are satisfied and checked
- [x] Story status is updated in the story file and INDEX.md
- [x] A retrospective is added

## Implementation Summary

**Completed**: 2026-10-10

GPU errors raised by gup-core's own work are now `Error::Gpu { what, message }`
on native and wasm32. The full design, costs and the S5/S8 adjustments are in
RFC-001's "GUP-410 findings".

- **`Context::scoped`** (`crates/gup-core/src/scope.rs`, new) pushes validation
  and out-of-memory scopes (and internal, natively), runs the work and pops
  them. Natively wgpu-core resolves the pop at once, so the error is returned
  from the same call. In a browser the popped scope is spawned into a oneshot.
  The next synchronous call on the context reports what has arrived, and
  `ImageTarget::read`/`render` and `WindowTarget::take_capture` await every
  pending scope.
- **Scoped steps**: glue program creation, mark and guide pipeline creation (a
  miss creates and inserts inside the scope, never under the cache lock), the
  layer step of `Plot::resolve`, `Renderer::prepare` (draw-in-pass hosts get the
  error here), `Renderer::render` (acquire, encode, present and submit), and the
  setup of `ImageTarget`, `TextureTarget` and `WindowTarget` (`resize` now
  returns `Result`).
- **Never cached**: any scope error makes the context forget its programs,
  pipelines and gup-text's glyph pipelines (`TextSystem::forget_pipelines`,
  new).
- **Threads**: wgpu-core's scope stack is per device, so the outermost scope on
  a thread holds a process-wide lock. It is taken before any context lock, which
  debug builds assert.
- **Browser backstop**: on a device Gup creates in a browser, uncaptured errors
  are recorded and reported by the next call.
- **Docs**: the crate docs' contract "GPU errors are `Err`, never a panic or a
  blank image", the `Error::Gpu` docs and `ImageTarget::read`.

### Evidence

- **Native seeds** (`scope::tests`, 3 tests):
  - A pipeline layout without the glue's group 2 makes `ImageTarget::render`
    return this (abridged):

    ```text
    GPU error in mark pipeline `Circle {x: f32rel→…}` (Rgba8Unorm, 4× MSAA):
    … Shader global ResourceBinding { group: 2, binding: 0 } is not available
    in the pipeline layout
    ```

    It does so twice, no mark pipeline is cached, and the valid scene then
    renders.

  - Broken glue WGSL is an `Err` from `Renderer::prepare` carrying naga's
    `expected identifier` diagnostic.
  - A frame without `RENDER_ATTACHMENT` is an `Err` from `Renderer::render`.

- **Browser seed**: GUP-408's seed 2 (`dpdx` in `rect.wgsl`) makes
  `mask wasm-browser` fail with the page's own line:

  ```text
  GUP FAIL GPU error in guide pipeline `gup rects` (Rgba8Unorm, 4× MSAA):
  Error while parsing WGSL: :43:19 error: 'dpdx' must only be called from
  uniform control flow
  ```

  Chrome logs no "rendering" warnings now, because the scopes capture the
  errors. Without the seed: `GUP PASS white=30884 grey=16219 coloured=16897`,
  the same counts as before. I looked at the PNG: title, log y ticks, viridis
  circles, the tinted background and the legend bar are all present.

- **Goldens unchanged**:
  - gup-core's golden tests pass (`scatter_png`, `scene_items`, `msaa`,
    `targets`, `svg`).
  - `window_parity` gives ΔE max 0.0000 against both `ImageTarget` and the
    golden PNG.
  - The legacy `mask visual-regression` suite passes.
- **Cost**:
  - About 250 ns per empty scope; a cached frame opens three.
  - `zoom_bench` Mailbox CPU work median: 0.787/0.794/0.798 ms before and
    0.782/0.800/0.776 ms after, in alternating runs.
  - Pipeline create median: 0.739 ms before, 0.719 ms after.
  - WASM: +11.2 KB gz (392,954 → 404,121 B).
- **Tests**:
  - `cargo test -p gup-core -p gup-text -- --test-threads=1`: all pass (57 lib
    tests in gup-core, 3 of them new, plus 2 new ignored measurements).
  - `mask all-check` passes.

### Limits

- **Late browser errors**: in a browser, a synchronous `Renderer::render` into a
  `TextureTarget` or `WindowTarget` (or a `prepare` for a host's pass) returns
  `Ok`. Its error is reported by the next call on the context. Only an awaited
  render or readback can return the error itself.
- **Internal errors in a browser**: wgpu's WebGPU backend panics on a
  `GPUInternalError` (`Error::from_js`), so browser scopes leave internal errors
  to the uncaptured handler, which hits the same panic.
- **`TextSystem::new`**: it creates its bind group and pipeline layouts outside
  any scope, in `Context::build`, which cannot fail. The layouts are static.

## Retrospective

**Completed**: 2026-10-10

### Key Technical Learnings

#### wgpu-core's error scopes are per device, not per thread

- **Challenge**: The design assumed scopes were thread-local. In wgpu 27,
  wgpu-core's `push_error_scope` pushes onto one `ErrorSinkRaw` stack per
  device. Every thread and every `Context` wrapping that device share it, and
  `Context::shared()` hands that device to everything, including the legacy
  crate.
- **Solution**: The outermost scope on a thread holds a process-wide mutex until
  it pops. A thread-local depth counter lets nested scopes skip it. The scope
  lock ranks above the context locks, and debug builds assert that no context
  lock is held when it is taken. That ordering forced cache misses to run
  outside the pipeline-cache lock. They now re-check and insert inside the
  scope.
- **Pattern**: Read the backend's implementation of a "scope" before relying on
  its isolation (`backend/wgpu_core.rs`, `pop_error_scope` is
  `ready(scope.error)`).

#### Natively, popping is synchronous in practice

- **Challenge**: `pop_error_scope` returns a future on every backend, and the
  story worried about a round trip per frame.
- **Solution**: wgpu-core returns `ready(...)`, so polling once with
  `Waker::noop()` resolves it. An empty scope costs about 250 ns. Only the
  browser needs real waiting. There the scope is spawned into a oneshot, which
  keeps `Context` `Send + Sync` (an earlier draft stored the `!Send` futures and
  made it lose both on wasm32). Awaited readbacks settle the pending scopes, and
  synchronous calls report what has arrived.
- **Pattern**: One code path that polls once, with a fallback for "not ready
  yet". Natively the fallback is `unreachable!`, and it is never hit.

#### Captured errors leave the browser console silent

- **Challenge**: GUP-408's smoke test caught seed 2 through Chrome's "rendering"
  warnings and the pixel check.
- **Solution**: With scopes, Chrome logs no warnings, because captured errors
  are not uncaptured. The run now fails through the page's own
  `GUP FAIL GPU error in guide pipeline 'gup rects' …` line, which carries the
  WGSL diagnostic. The smoke test still fails on the seed, through a better
  channel. Its warning rule now guards only errors outside every scope.

#### wgpu's WebGPU backend panics on `GPUInternalError`

- **Challenge**: `crate::Error::from_js` handles validation and out-of-memory
  errors and `panic!`s on anything else.
- **Solution**: Browser scopes filter only `Validation` and `OutOfMemory`.
  Natively `Internal` is scoped too. An internal error in a browser still
  reaches the uncaptured handler and the same panic; that is upstream's to fix.

### Architectural Decisions

#### Scope every GPU step, and forget every pipeline on an error

- **Decision**: `Context::scoped(what, f)` wraps the pipeline and program
  creation, layer resolution, prepare, render, and target and surface setup. Any
  reported error makes the context drop gup-core's programs and pipelines and
  gup-text's glyph pipelines.
- **Reasoning**: Nested scopes give precise names: the pipeline's own scope
  catches its error before the render scope sees it. Dropping every cached
  pipeline covers the browser case, where the failing pipeline is already cached
  by the time its error arrives, and gup-text's cache, which gup-core cannot
  key. Errors are rare, so recreating everything after one is cheap.
- **Trade-off**: After an error, all of a context's pipelines are recreated, not
  only the bad one.
- **Future**: S5's user WGSL fails as `Error::Gpu` naming the mark pipeline and
  glue signature, with no new machinery.

#### Browser errors are reported per context, and late for synchronous calls

- **Decision**: Keep `Renderer::render` and `prepare` synchronous. In a browser,
  their errors come from the next call on the context, or from an awaited
  `ImageTarget::read` or `WindowTarget::take_capture`.
- **Reasoning**: Making `render` async would ripple through every host
  integration for an error path. An awaited render already exists for the case
  that matters most: a headless or one-shot browser chart.
- **Trade-off**: A browser canvas loop sees a frame's error one frame late, and
  any chart on the same context may be the one that reports it.

#### Native keeps wgpu's panic for uncaptured errors

- **Decision**: Install the recording handler only in a browser, and only on a
  device Gup created.
- **Reasoning**: Every gup-core step is scoped, so a remaining native panic
  means a missed scope or another crate's error. The legacy crate shares the
  device, and recording its errors silently would hide its bugs.

### Development Workflow Insights

- **Shared target directories**: building the pre-story commit in a
  `git worktree` with the shared `CARGO_TARGET_DIR` overwrote the main
  checkout's gup-core release artifacts under the same unit hash. Cargo then
  called the main build "Fresh" and handed back the old binary, and the first
  before/after comparison compared one binary with itself. Two things caught it:
  `cmp` said the binaries were identical, and `strings` found none of the new
  format strings. `cargo clean -p gup-core -p gup-text -p gup-wgsl --release`
  fixed it. Concurrent worktrees that share a target directory (this story and
  GUP-409) can hit the same thing.
- **Noisy timings**: the cached-frame test's median moved between 188 and 324 µs
  on unchanged code, because of CPU frequency. Two things were stable: a
  micro-benchmark of the thing being added, and alternating A/B runs of
  `zoom_bench`.
- **Perl replacements**: `s{…}{…}` failed on Rust code with unbalanced braces in
  the replacement. The Edit tool and whole-block file splices were reliable.

### Follow-up Stories

None. Noted without stories:

- **wasm32 clippy**: `clippy --target wasm32-unknown-unknown` reports
  `arc_with_non_send_sync` at `selection.rs`'s `Arc::new(LayerUniforms … )`. The
  finding predates this story, and no gate lints wasm32. It belongs with the
  gate work (GUP-409's area) if wasm linting is added.
- **Late-error browser test**: the browser harness tests only the awaited path.
  S8's wasm entry point (canvas loop) should add a seeded check that a
  synchronous canvas render reports its error on the next frame.
- **Upstream**: wgpu's `Error::from_js` panics on `GPUInternalError`. Report it,
  or wrap it in a later wgpu bump.
