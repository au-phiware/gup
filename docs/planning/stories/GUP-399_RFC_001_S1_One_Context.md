# GUP-399: RFC-001 S1 — One `gup::Context`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: 📋 Planned **Created**: 2026-10-05

## Context

[RFC-001](../rfcs/RFC-001_Core_Architecture.md) §2 diagnoses the root cause of
one of the strategic review's sharpest findings: a chart built with the old
builder path cannot be shown in a window, because `RenderContext`
(`src/render.rs:13`, used by charts and `Selection`) and `GupContext`
(`src/context.rs:1155`, used by `GupApp` and `RenderFrame`) are separate
device-owning types that each request their own `wgpu::Adapter`/`Device`
(`src/render.rs:291-302`, `src/context.rs:1339-1362`). `gup-core`'s own
`Context` (`crates/gup-core/src/context.rs`) was built in
[GUP-395](GUP-395_Gup_Core_Vertical_Slice_Headless.md) and extended by
[GUP-396](GUP-396_Gup_Core_Vertical_Slice_Window_Performance.md): it already has
`new`/`new_blocking`/`from_wgpu`, `Caps` read from device limits/features, and
`Mutex<ShaderLibrary>` / `Mutex<PipelineCache>` / `Mutex<TextSystem>` caches,
exactly matching RFC-001 §2's design. `pub use wgpu;` already exists in
`crates/gup-core/src/lib.rs`.

This is RFC-001's S1 story (§11 migration table): "`gup::Context` in `gup-core`;
old `gup` builds `RenderContext`/`GupContext` as thin shims over it;
`pub use wgpu`." Per the Orchestrator review's amendment ("No feature work on
the old path from S0 onward. Bug fixes only if they block T0 tooling."), the old
`gup` path is otherwise frozen until the S14 flip. **This story is the one
explicit exception**: RFC-001's own migration plan calls for `RenderContext` and
`GupContext` to be rewired onto `gup-core::Context` now, specifically so that
the two-context split stops blocking dogfooding before S14 deletes both types
outright. This is architectural unification mandated by the RFC, not new
old-path feature work, and the distinction must be stated explicitly in the
retrospective so it isn't read as a freeze violation.

`RenderContext` (1,360 lines) and `GupContext` (3,787 lines) carry substantial
functionality beyond device ownership — multi-surface management, buffer and
texture pools, device-loss recovery — all of which RFC-001 §11's "Delete" column
marks for removal at S14, not adaptation now. This story's scope is narrower
than "delete the pools": it only removes the duplicate adapter/device creation,
so both old-path types get their `wgpu::Device` and `wgpu::Queue` from one
shared `gup_core::Context` instead of each requesting their own. The pools,
multi-surface map and recovery logic keep working against whatever device
they're handed; deleting them is S14's job.

## User Story

> "As a Gup implementer continuing the RFC-001 migration, I want
> `gup-core::Context` to be the one place a `wgpu::Device` and `Queue` are
> created, with the old `gup` crate's `RenderContext` and `GupContext` wrapping
> it instead of each creating their own, so that the old path's
> two-incompatible- contexts bug stops blocking dogfooding before the S14 flip
> deletes both types."

## Acceptance Criteria

### AC1: `Context::shared()` — a lazily-initialised process default

- [ ] `gup_core::Context::shared() -> Result<Context>` exists (native only,
      `#[cfg(not(target_arch = "wasm32"))]`, per RFC-001 §2), backed by a
      `OnceLock`, returning a cheap clone of one process-wide `Context` built
      with `new_blocking()` on first call.
- [ ] A unit test calls `Context::shared()` twice and asserts both calls return
      a `Context` with the same `ContextId` (same underlying device), and that
      the library-preload cost (RFC-001 "S0a findings": 6.6 ms release) is paid
      once, not per call — e.g. by timing the second call and asserting it is at
      least an order of magnitude faster than the first.
- [ ] Doc comment on `Context::shared()` states why it matters: every `Context`
      preloads the naga_oil shader library (6.6 ms release per the S0a
      measurement), so `save_png`-style one-shot calls must share one process
      default rather than paying that cost per call.

### AC2: Lock order is documented and checkable

- [ ] `Context`'s internal `Mutex` fields (`pipelines`, `shaders`, `text`) carry
      a doc comment stating the required acquisition order — pipelines before
      shaders — and that no code path holds the `text` lock while holding either
      of the other two.
- [ ] A test (or, if a `Mutex` ordering debug-assertion already exists in the
      crate's dependency tree, e.g. via `parking_lot`'s deadlock detector —
      check before adding a new dependency) exercises the one call path that
      acquires both `pipelines` and `shaders` (pipeline-cache miss triggering
      shader composition) and does not deadlock or assert under
      `cargo test -p gup-core -- --test-threads=1`.

### AC3: `RenderContext` and `GupContext` stop creating their own device

- [ ] `RenderContext::new()` (`src/render.rs`) and `GupContext::new()`
      (`src/context.rs`) no longer each call `instance.request_adapter(..)` /
      `adapter.request_device(..)`. Both obtain their `wgpu::Device` and
      `wgpu::Queue` from a shared `gup_core::Context` (e.g.
      `gup_core::Context::shared()`, or a `gup_core::Context` passed in), so a
      `RenderContext` and a `GupContext` constructed in the same process use the
      same device.
- [ ] Every existing public method on `RenderContext` and `GupContext` keeps its
      current signature and behaviour (buffer pools, texture pools,
      multi-surface management, device-loss recovery are untouched — this story
      only changes where the device comes from).
      `cargo test -- --test-threads=1` passes for the root `gup` crate's
      existing suite with no test deleted or weakened to make this pass.
- [ ] **AC (user-visible, required)**: a new integration test constructs a
      `RenderContext` and a `GupContext` in the same process and asserts
      `render_context.device.global_id() == gup_context.device.global_id()` (or
      the equivalent `wgpu::Device` identity check) — the first automated proof
      that the "two contexts that can't work together" bug (RFC-001 §2 "Today")
      is fixed at the device level. Record in the retrospective whether this
      alone is sufficient to un-break the `composite_*` examples
      (`tests/visual_regression/expected_failures.toml`, tracked to "RFC-001 S1
      and S11") or whether S11 (composite on `Plot`) is still required — update
      the expected-failures entry only if a `composite_*` example actually now
      passes under `GUP_SMOKE_FILTER=composite`, never on assumption.

### AC4: `pub use wgpu` is correct and exact

- [ ] `crates/gup-core/src/lib.rs`'s existing `pub use wgpu;` is verified to
      re-export the exact same `wgpu` version the root `gup` crate and
      `RenderContext`/`GupContext` depend on (check `Cargo.lock`, not just
      `Cargo.toml` version ranges). If `gup-core` and the shared `Context` this
      story wires into the old path end up on different `wgpu` versions due to
      how the old path depends on `gup-core`, that is a blocking problem to fix
      in this story, not a follow-up.

## Technical Tasks

- [ ] Add `Context::shared()` (native-only, `OnceLock`-backed) to
      `crates/gup-core/src/context.rs`.
- [ ] Document and verify the `pipelines` → `shaders` lock order; add the
      ordering test described in AC2.
- [ ] Decide and implement how the old `gup` crate depends on `gup-core` for
      this story (path dependency, feature-gated, or direct — `gup-core` is
      currently a non-default workspace member; this story makes it a real
      dependency of the frozen root crate for the first time, so record the
      `Cargo.toml` change and its effect on build times).
- [ ] Rewire `RenderContext::new()` (`src/render.rs`) to obtain its device and
      queue from a shared `gup_core::Context` instead of requesting its own
      adapter/device.
- [ ] Rewire `GupContext::new()` (`src/context.rs`) the same way.
- [ ] Write the device-identity integration test (AC3).
- [ ] Re-check `composite_*` examples under `GUP_SMOKE_FILTER=composite`; update
      `tests/visual_regression/expected_failures.toml` only if one now passes.
- [ ] Verify `cargo tree` shows one resolved `wgpu` version across `gup`,
      `gup-core` and their dependents (AC4).
- [ ] Run the full existing root-crate test suite
      (`cargo test -- --test-threads=1`) to confirm no regression from the
      device-source change.

## Dependencies

### Prerequisite Stories

- GUP-396: gup-core Vertical Slice, Window and Performance 📋 — this story moves
  `gup-core`'s `Context` into place "as built" by S0a/S0b; it assumes the
  `Context` shape (including whatever S0b's window/performance work settles,
  such as non-sRGB surface views) is stable before the old path depends on it.

### Enables Stories

- RFC-001 S2 (extract `gup-text`), S3 (`Scene`/`Renderer`/`RenderTarget`) and
  later S-stories build on `Context::shared()` and the documented lock order.
- Potentially un-blocks the `composite_*` examples' expected-failure entries
  (see AC3) — confirm by measurement, not assumption.

## Testing Strategy

- **Unit tests**: `Context::shared()` returns the same `ContextId` across calls
  and pays the preload cost once (AC1); the lock-order exercise (AC2).
- **Integration tests**: `RenderContext` and `GupContext` constructed in the
  same process share one device (AC3).
- **Regression**: the full existing root-crate suite
  (`cargo test -- --test-threads=1`) must stay green — this story changes where
  two widely-used types get their device, which is a high-blast-radius change
  even though it adds no new behaviour.
- **Visual validation**: re-run the `composite_*` examples under
  `GUP_SMOKE_FILTER=composite` and record the actual outcome (still failing, or
  now passing) rather than assuming either.

## Success Metrics

- [ ] `Context::shared()` exists, is `OnceLock`-backed, and is documented.
- [ ] `RenderContext` and `GupContext` share one device when constructed in the
      same process; this is proven by an automated test, not inspection.
- [ ] The root `gup` crate's full test suite passes unchanged in behaviour.
- [ ] `tests/visual_regression/expected_failures.toml`'s `composite_*` entry is
      updated only if measurement shows it should be.

## Risk Assessment

- **High**: `RenderContext` and `GupContext` are large (1,360 and 3,787 lines)
  and used throughout the old path (chart builders, examples, `GupApp`). Making
  the root `gup` crate depend on `gup-core` for the first time, and changing
  where two heavily-used types source their device, is a high-blast-radius
  change for a "frozen" crate. _Mitigation_: change only device/queue sourcing,
  not any public API or pool/recovery logic; rely on the existing (large) test
  suite to catch regressions; do not attempt to also delete the pools or
  multi-surface code in this story even though RFC-001 §11 eventually deletes
  them — that is S14's job, not this one's.
- **Medium**: the `composite_*` panic's root cause (RFC-001 §2 "Today",
  `tests/visual_regression/expected_failures.toml`) is tracked to both S1 and
  S11. This story may unify the device without actually un-breaking those
  examples, if the panic also depends on pipeline/bind-group state that S11's
  `Plot`/`Selection` rework addresses. _Mitigation_: AC3 requires measuring the
  actual outcome and updating the tracked expected-failure only on evidence.
- **Low**: adding `gup-core` as a real dependency of the root crate (previously
  isolated, non-default) could surface a version or feature-flag conflict
  between `gup`'s and `gup-core`'s dependency trees (AC4). _Mitigation_: check
  `cargo tree` for duplicate `wgpu`/`naga` resolutions before and after.

## Definition of Done

- [ ] All Acceptance Criteria are satisfied and checked
- [ ] All tests pass: `cargo test -- --test-threads=1`
- [ ] Lint and format clean: `mask all-fix`
- [ ] All examples compile: `cargo check --examples`
- [ ] The device-identity test and the `composite_*` re-check are verified and
      their outcomes recorded in the retrospective
- [ ] Story status updated to ✅ Complete in story file and INDEX.md
- [ ] Retrospective added to story document
