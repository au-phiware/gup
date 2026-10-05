# GUP-399: RFC-001 S1 — One `gup::Context`

## Story Overview

**Initiative**: RFC-001 Migration **Status**: ✅ Complete (2026-10-05)
**Created**: 2026-10-05

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

- [x] `gup_core::Context::shared() -> Result<Context>` exists (native only,
      `#[cfg(not(target_arch = "wasm32"))]`, per RFC-001 §2), backed by a
      `OnceLock`, returning a cheap clone of one process-wide `Context` built
      with `new_blocking()` on first call.
- [x] A unit test calls `Context::shared()` twice and asserts both calls return
      a `Context` with the same `ContextId` (same underlying device), and that
      the library-preload cost (RFC-001 "S0a findings": 6.6 ms release) is paid
      once, not per call — e.g. by timing the second call and asserting it is at
      least an order of magnitude faster than the first.
- [x] Doc comment on `Context::shared()` states why it matters: every `Context`
      preloads the naga_oil shader library (6.6 ms release per the S0a
      measurement), so `save_png`-style one-shot calls must share one process
      default rather than paying that cost per call.

### AC2: Lock order is documented and checkable

- [x] `Context`'s internal `Mutex` fields (`pipelines`, `shaders`, `text`) carry
      a doc comment stating the required acquisition order — pipelines before
      shaders — and that no code path holds the `text` lock while holding either
      of the other two.
- [x] A test (or, if a `Mutex` ordering debug-assertion already exists in the
      crate's dependency tree, e.g. via `parking_lot`'s deadlock detector —
      check before adding a new dependency) exercises the one call path that
      acquires both `pipelines` and `shaders` (pipeline-cache miss triggering
      shader composition) and does not deadlock or assert under
      `cargo test -p gup-core -- --test-threads=1`.

### AC3: `RenderContext` and `GupContext` stop creating their own device

- [x] `RenderContext::new()` (`src/render.rs`) and `GupContext::new()`
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
      _Pending CI (2026-10-05)._ A full debug test build does not fit in /tmp on
      the dev machine, so GitHub CI verifies the full suite. Run locally, all
      green: - `cargo test --lib`: 2502 passed. - `cargo test --doc`: 216
      passed, 0 failed (29 failed before this story). - The 37 integration
      binaries without required features that construct `RenderContext`,
      `GupContext` or `VisualTestUtils`: 421 passed. - `cargo test -p gup-core`,
      `mask visual-regression` and `mask smoke-examples`.

      No test was deleted or weakened. Signatures are unchanged; the device
      source is the only behaviour change.

- [x] **AC (user-visible, required)**: a new integration test constructs a
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

- [x] `crates/gup-core/src/lib.rs`'s existing `pub use wgpu;` is verified to
      re-export the exact same `wgpu` version the root `gup` crate and
      `RenderContext`/`GupContext` depend on (check `Cargo.lock`, not just
      `Cargo.toml` version ranges). If `gup-core` and the shared `Context` this
      story wires into the old path end up on different `wgpu` versions due to
      how the old path depends on `gup-core`, that is a blocking problem to fix
      in this story, not a follow-up.

## Technical Tasks

- [x] Add `Context::shared()` (native-only, `OnceLock`-backed) to
      `crates/gup-core/src/context.rs`.
- [x] Document and verify the `pipelines` → `shaders` lock order; add the
      ordering test described in AC2.
- [x] Decide and implement how the old `gup` crate depends on `gup-core` for
      this story (path dependency, feature-gated, or direct — `gup-core` is
      currently a non-default workspace member; this story makes it a real
      dependency of the frozen root crate for the first time, so record the
      `Cargo.toml` change and its effect on build times).
- [x] Rewire `RenderContext::new()` (`src/render.rs`) to obtain its device and
      queue from a shared `gup_core::Context` instead of requesting its own
      adapter/device.
- [x] Rewire `GupContext::new()` (`src/context.rs`) the same way.
- [x] Write the device-identity integration test (AC3).
- [x] Re-check `composite_*` examples under `GUP_SMOKE_FILTER=composite`; update
      `tests/visual_regression/expected_failures.toml` only if one now passes.
- [x] Verify `cargo tree` shows one resolved `wgpu` version across `gup`,
      `gup-core` and their dependents (AC4).
- [ ] Run the full existing root-crate test suite
      (`cargo test -- --test-threads=1`) to confirm no regression from the
      device-source change. Partly run locally; the full suite runs on CI (see
      the AC3 note).

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

- [x] `Context::shared()` exists, is `OnceLock`-backed, and is documented.
- [x] `RenderContext` and `GupContext` share one device when constructed in the
      same process; this is proven by an automated test, not inspection.
- [ ] The root `gup` crate's full test suite passes unchanged in behaviour.
      Partly run locally; the full suite runs on CI (see the AC3 note).
- [x] `tests/visual_regression/expected_failures.toml`'s `composite_*` entry is
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

- [ ] All Acceptance Criteria are satisfied and checked. Everything except the
      full-suite run, which is left to CI.
- [ ] All tests pass: `cargo test -- --test-threads=1`. Partly run locally; the
      full suite runs on CI (see the AC3 note).
- [x] Lint and format clean: `mask all-fix`
- [x] All examples compile: `cargo check --examples`
- [x] The device-identity test and the `composite_*` re-check are verified and
      their outcomes recorded in the retrospective
- [x] Story status updated to ✅ Complete in story file and INDEX.md
- [x] Retrospective added to story document

## Implementation Summary

**Completed**: 2026-10-05. Full root suite pending on CI; see the AC3 note.

### What was built

- **`gup_core::Context::shared()`** (`crates/gup-core/src/context.rs`) is the
  native process default.
  - It is backed by a `OnceLock`. First-time creation is serialised by a mutex,
    so concurrent first calls make one device.
  - Later calls are an `Arc` clone. A failed creation is retried on the next
    call.
- **`ContextOptions`** gains `backends`, `power_preference`, `required_features`
  and `required_limits`.
  - The default backends are `PRIMARY`; GL is tried only as a fallback.
  - `TIMESTAMP_QUERY` is optional by default.
  - Device limits start from the WebGPU defaults where the adapter supports
    them.
- **Public `Context::instance()`/`adapter()`** replace the crate-private
  `instance_and_adapter()`.
- **Lock order.** The `pipelines` → `shaders` order, with `text` alone, is
  documented on the fields. Debug builds check it on every acquisition, using
  the `Ordered` guard and `LockRank::check`.
- **Old-path shims** (`src/context.rs`: `core_context`, `core_handles`).
  - `RenderContext::new`/`with_viewport`, `GupContext::new`/`headless`/
    `with_options` and `VisualTestUtils::new` take their device from gup-core.
  - They use the shared context when it meets the options, and a dedicated
    gup-core context otherwise.
- **The root `gup` crate depends on `gup-core`** (path dependency, not
  re-exported). This adds 8 crates (gup-core, naga_oil, encase, encase_derive,
  encase_derive_impl, const_panic, data-encoding, typewit) and the `naga-ir`
  feature on the shared wgpu.
- **gup-core builds for wasm32.** `Layer` is now bounded by
  `wgpu::WasmNotSendSync`, and the native-only readback items are cfg-gated.
- **gup-core crate docs** state its contracts: one device per process by
  default, every GPU write counted, and the checked lock order.
- **Tracking.** The `examples/composite_*` expected-failure entry is removed
  (all four XPASS). The skip-list reasons now describe what each window shows.
- **RFC-001** gains an "S1 findings" section.

### Tests

- gup-core, 7 new:
  - `shared_is_one_context_created_once`
  - `device_limits_cover_the_webgpu_defaults_and_adapter_buffers`
  - `lock_order_allows_shaders_inside_pipelines`
  - three `lock_order_refuses_*` tests (`should_panic`, debug only)
  - `pipeline_cache_miss_composes_under_the_lock_order`, where two threads miss
    at once
- `tests/one_device.rs`, 3 new:
  - same device across `RenderContext`, `GupContext` and gup-core
  - a compute pass built on one context's device runs through the other's queue
  - a LowPower dedicated device as the negative control

### Verification (local, 2026-10-05)

| Suite                                                   | Result                                                                       |
| ------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `cargo test -p gup-core -- --test-threads=1`            | lib 51 passed, 1 ignored; integration and compile-fail suites green          |
| `cargo test --test one_device`                          | 3 passed                                                                     |
| `cargo test --lib -- --test-threads=1`                  | 2502 passed, 4 ignored                                                       |
| `cargo test --doc`                                      | 216 passed, 0 failed (baseline: 188 passed, 29 failed)                       |
| 37 integration binaries that construct the old contexts | 421 passed, 0 failed, 2 ignored                                              |
| `mask visual-regression`                                | 16 passed; no golden changes                                                 |
| `mask smoke-examples` (headless)                        | passed                                                                       |
| `GUP_SMOKE_WINDOWED=1 GUP_SMOKE_FILTER=composite`       | 4 × XPASS → entry removed; then 4 PASS                                       |
| `cargo build --target wasm32-unknown-unknown --lib`     | builds (gup-core: `cargo check -p gup-core --target wasm32-unknown-unknown`) |
| Full root `cargo test -- --test-threads=1`              | **left to GitHub CI** (debug build needs > 8.6 GB; it filled /tmp twice)     |

- **Baseline before the change** (`0b45d87`, full suite, `--no-fail-fast`): 106
  binaries, 3733 passed, 29 failed (all doctests), 152 ignored.
- **Old-path LOC**: 28882 → **28876** (−6). `render.rs` lost 23 lines of adapter
  and device request; `context.rs` gained 17 for the shim.

## Retrospective

**Completed**: 2026-10-05

**This was not a freeze violation.** The orchestrator review froze the old path
from S0 onward. This story changes `RenderContext` and `GupContext` only because
RFC-001 §11's S1 row asks for it: they become shims over `gup_core::Context`. No
feature was added to the old path. The only behaviour change is where the device
comes from, and the old path's LOC went down (−6).

### Key Technical Learnings

#### `wgpu::Device ==` is not device identity

- **Challenge**: AC3 suggested comparing `global_id()`s, which wgpu 27 no longer
  has. `Device: PartialEq` exists, but it compares wgpu-core ids, and each
  `Instance` (each wgpu-core `Global`) numbers its devices separately. A
  low-power device from a second instance printed as `Id(0,1)` and compared
  equal to the shared device.
- **Solution**: ask wgpu instead. Create a bind group on device B from a layout
  made on device A.
  - On one device this succeeds.
  - Across instances it panics with "BindGroupLayout[Id(0,1)] does not exist",
    the `composite_*` panic.
  - Across devices of one instance it is a validation error.
  - The probe catches both. A LowPower `GupContext` with its own device is the
    negative control.
- **Pattern**: an identity test needs a negative control that would fail.
  Without one, the naive `==` assertion would have passed before the fix.

#### Feature unification across a newly-joined crate

- **Challenge**: gup-core had not built for wasm32 since S0a
  (`Layer: Send + Sync`; wgpu's web types are `!Sync`). Enabling wgpu's
  `fragile-send-sync-non-atomic-wasm` fixed gup-core but, through feature
  unification, made wgpu's `WindowHandle` require `Send + Sync` on wasm for the
  root crate too, which broke its `MaybeSend` surfaces.
- **Solution**: bound the internal trait by `wgpu::WasmNotSendSync` (RFC §2's
  "MaybeSend on wasm") and leave wgpu's features alone.
- **Pattern**: once crate A depends on crate B, B's dependency features apply to
  A as well. Prefer trait bounds over feature flags for platform differences.

#### The 29 failing doctests were two wgpu builds in one target directory

- **Challenge**: the baseline's doctests failed with "multiple different
  versions of crate `wgpu_types`". `Cargo.lock` had only one version.
- **Solution**: with `gup-core` (`naga-ir`) and the root (without it) sharing
  `CARGO_TARGET_DIR`, there were two wgpu artifacts, and rustdoc's
  `-L dependency=` search picked inconsistently. Now that the root depends on
  gup-core, both resolve to the same feature set: 216 passed, 0 failed. The
  coordinator also cleared parts of the target directory meanwhile, so this
  attribution is likely but not isolated.

#### Limits had to be raised for the shared device to serve the old path

- **Challenge**: S0a's `Context` started from `downlevel_defaults()` (16 KB
  uniform bindings, 4 storage buffers per stage). The old path asks for
  `Limits::default()`.
- **Solution**: start from the WebGPU defaults when
  `Limits::default().check_limits(&adapter)` holds (downlevel otherwise), then
  raise to the adapter's buffer sizes and resolution, then to `required_limits`
  via `or_better_values_from`.

### Architectural Decisions

#### Primary backends by default, GL only as a fallback

- **Decision**: `ContextOptions::default().backends = PRIMARY` on native
  (`BROWSER_WEBGPU | GL` on wasm). GL is retried only if the requested backends
  have no adapter and `WGPU_BACKEND` is unset.
- **Reasoning**: on machines with Vulkan, Metal or DX12 this means no EGL
  instance exists, so S0b's `eglTerminate` teardown crash cannot happen. It also
  matches what the old path already requested.
- **Trade-off**: a machine whose only adapter is GL pays one failed adapter
  request first. An explicit `backends: GL` skips that.
- **Future**: S8's `GupApp` keeps S0b's teardown order regardless; this removes
  the trap, not the rule.

#### Shared device only when it meets the options

- **Decision**: `core_context(&GupOptions)` uses `Context::shared()` when the
  power preference is the default, the backends allow the shared adapter's, and
  its features and limits cover the request. Anything else gets a dedicated
  `gup_core::Context::with_options`.
- **Reasoning**: silently ignoring a LowPower request would be a quiet behaviour
  change. A dedicated context is still created by gup-core, the one place
  devices are made.
- **Trade-off**: custom options mean a second device again, as before S1.
  Nothing in the examples relies on mixing such a context with the default one.

#### Debug-checked lock order instead of a deadlock detector

- **Decision**: a thread-local bit set records which `Context` locks a thread
  holds. `LockRank::check` panics on any nesting other than `shaders` inside
  `pipelines`. It costs nothing in release builds.
- **Reasoning**: `parking_lot`'s deadlock detector is a feature flag on the
  `parking_lot` that wgpu uses, which is too broad. The rank check catches order
  inversions deterministically, on a single thread, in every test.
- **Future**: S2 adds `gup-text`. Any new lock gets a `LockRank`.

### Development Workflow Insights

- **Disk is the constraint.**
  - A full root debug test build needs more than 8.6 GB. It filled /tmp twice in
    this story, the second time mid-session, which blocked every tool, including
    Write (it uses temp files).
  - The session was handed back and resumed after the coordinator freed space.
  - Following the coordinator's direction, the full suite is CI's job. Locally I
    ran lib, doc and the 37 integration binaries that touch the changed
    constructors.
- **Look at windowed output before removing an expected failure.** The composite
  examples stopped panicking, but the screenshots showed overflowing bars, black
  elliptical points, no text and a zig-zag trend line. The skip list now says so
  instead of implying they're fixed.
- **`git stash` with a clean tree is a no-op.** It was harmless here, but check
  `git status` before relying on it for an A/B comparison.
- **zsh doesn't word-split `$args`.** Use `${=args}` when building
  `cargo test --test …` lists in a variable.
- **An offline `cargo tree` on `dogfood/` downgraded unrelated crates** to what
  happened to be cached. I reverted it; the dogfood build (not `--locked`) will
  add `gup-core` to its lockfile online.

### Follow-up Stories

None. Everything found is already covered by an RFC-001 step or is old-path code
that S14 deletes. Each is recorded in RFC-001 "S1 findings":

- `src/wasm_api.rs` still requests its own device; WebGL needs a
  `compatible_surface` adapter request (S8).
- `GupContext::attempt_recovery` still requests a device (deleted at S14).
- The composite examples' visual bugs (S11, already tracked through
  `chart_builders/composite`), including `composite_scatter_regression`'s
  zig-zag trend line (S11 mixed-`T` layers).
- Test and bench helpers that create raw devices (old path, S14).
