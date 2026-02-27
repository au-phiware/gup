# GUP-231: WASM Build Platform Gating

**Priority**: Medium **Complexity**: Medium **Created**: 2025-08-07 **Status**:
✅ Complete

## Overview

Gate platform-specific accessibility backends and DOM integration code behind
`cfg` attributes so the full `gup` library compiles for
`wasm32-unknown-unknown`. Currently, several modules reference Linux, Windows,
and macOS native APIs that prevent WASM compilation.

## Context

GUP-172 (WebAssembly Performance Benchmarks) exposed that the full library does
not compile for `wasm32-unknown-unknown` due to platform-specific code in
accessibility and DOM integration modules. While the benchmark modules
themselves compile correctly, the library as a whole cannot be built with
`wasm-pack` until these issues are resolved.

## User Story

As a web developer, I want to build the Gup library for WebAssembly so that I
can use GPU-accelerated visualisations in the browser, including running
cross-platform benchmarks.

## Acceptance Criteria

- [x] `cargo build --target wasm32-unknown-unknown --lib` succeeds
- [x] `wasm-pack build --target web` succeeds and produces a loadable package
- [x] All native tests continue to pass
- [x] Platform-specific accessibility backends gated behind `cfg(target_os)`
- [x] DOM integration code properly gated for `wasm32` vs native targets
- [x] `Send`/`Sync` bounds resolved for WASM DOM callback types

## Technical Tasks

- [x] Audit all modules for `wasm32` compilation errors
- [x] Gate `LinuxAccessibility` behind `cfg(target_os = "linux")`
- [x] Gate Windows/macOS accessibility behind respective `cfg(target_os)`
- [x] Add missing `web-sys` features (e.g. `TouchEvent`) for WASM
- [x] Resolve `Send`/`Sync` bound issues on DOM callback types
- [x] Verify wasm-pack produces a working package
- [x] Update CI to include WASM compilation check

## Dependencies

- **Requires**: None (existing modules need cfg gating)
- **Enables**: GUP-172 full browser execution, GUP-226 WebAssembly axis
  performance validation

## Testing Strategy

- WASM target compilation check in CI
- Native test regression: all existing tests must continue passing
- Smoke test: load wasm-pack output in a browser page

## Risk Assessment

- **Low**: Most changes are adding `cfg` attributes, unlikely to break logic
- **Medium**: DOM callback `Send`/`Sync` may require architectural changes

## Definition of Done

- [x] `wasm-pack build --target web` produces a loadable package
- [x] Native test suite passes (same count as before)
- [x] CI includes WASM compilation check

## Implementation Summary

### What Was Implemented

Resolved all 56 WASM compilation errors to enable full library compilation for
`wasm32-unknown-unknown` target.

### Key Changes

1. **MaybeSend/MaybeSync marker traits** (`src/lib.rs`): Conditional marker
   traits that equal `Send`/`Sync` on native but are auto-implemented for all
   types on WASM (single-threaded). This avoids duplicating trait definitions.

2. **Trait bound relaxation**: Updated `Mixable`, `AsyncMixable`, `Mergeable`,
   `Renderable`, `StreamingDataSource`, and `ProgressiveDataLoader` traits to
   use `MaybeSend + MaybeSync` instead of `Send + Sync`.

3. **PlatformAccessibility trait split**: Dual `cfg`-gated trait definitions —
   `Send + Sync` on native, no bounds on WASM.

4. **async_trait gating**: All `#[async_trait]` attributes replaced with
   `cfg_attr` that selects `async_trait` (native) vs `async_trait(?Send)` (WASM).

5. **Background task spawning**: `tokio::spawn` calls in progressive and
   streaming modules gated to use `wasm_bindgen_futures::spawn_local` on WASM.

6. **Web-sys feature additions**: `TouchEvent`, `Touch`, `TouchList`,
   `HtmlHeadElement` features added to Cargo.toml.

7. **Bug fixes**: Non-exhaustive `AriaRole` matches (missing `Tooltip`,
   `Control`), borrow checker issue in `WebDomOverlay::update_from_aria_tree`,
   `FocusChanged` destructuring fix, explicit `Touch` type annotations.

8. **CI workflow**: New `.github/workflows/wasm.yml` verifies both `cargo build`
   and `wasm-pack build` for the WASM target.

### Files Changed

- `Cargo.toml` — web-sys features
- `src/lib.rs` — MaybeSend/MaybeSync traits
- `src/mixable.rs` — Mixable trait bounds
- `src/mixable/merge.rs` — Mergeable trait bounds
- `src/mixable/composition_recovery.rs` — Future Send bound cfg
- `src/interaction.rs` — Renderable trait bounds
- `src/accessibility/platform.rs` — PlatformAccessibility cfg split, LinuxAccessibility cfg, AriaRole match
- `src/accessibility/web_overlay.rs` — Bug fixes, AriaRole matches, type annotations
- `src/async_mixable.rs` — AsyncMixable trait bounds, async_trait cfg
- `src/async_mixable/progressive.rs` — spawn_local, trait bounds
- `src/async_mixable/streaming.rs` — spawn_local, trait bounds
- `src/async_mixable/utils.rs` — async_trait cfg
- `.github/workflows/wasm.yml` — New CI workflow

### Test Results

- All 320+ native tests pass (0 failures)
- `cargo build --target wasm32-unknown-unknown --lib` succeeds
- `wasm-pack build --target web` produces a 259KB loadable WASM package
- All examples compile

## Retrospective

**Completed**: 2026-02-27

### Key Technical Learnings

#### MaybeSend/MaybeSync Pattern for Cross-Platform Traits

- **Challenge**: wgpu's WebGPU backend wraps JS objects (`JsValue`,
  `GpuRenderPipeline`, etc.) that are inherently `!Send`/`!Sync` because they
  contain `*mut u8` and `RefCell` internally. However, the native Vulkan/Metal
  backends _are_ `Send`/`Sync`. Many core traits (`Mixable`, `AsyncMixable`,
  `Renderable`) required `Send + Sync` bounds.
- **Solution**: Introduced `MaybeSend` and `MaybeSync` marker traits in
  `lib.rs`. On native (`cfg(not(target_arch = "wasm32"))`), they are aliases for
  `Send`/`Sync` via blanket impls. On WASM, they're auto-implemented for all
  types. This lets the rest of the crate say `T: MaybeSend + MaybeSync` without
  duplicating every trait definition.
- **Pattern**: This is a well-known pattern in the Rust WASM ecosystem (e.g.
  `send_wrapper`, `fragile` crates). Defining it in-crate avoids an extra
  dependency and keeps the semantics clear.

#### async_trait Conditional Send Bounds

- **Challenge**: `#[async_trait]` by default desugars async methods into `Pin<Box<dyn Future + Send>>`. On WASM, the `Send` bound is impossible to satisfy
  because wgpu types aren't `Send`.
- **Solution**: Used `cfg_attr` to switch between `#[async_trait]` (native) and
  `#[async_trait(?Send)]` (WASM) on every trait definition and impl block. This
  was a mechanical but widespread change (13 occurrences across 4 files).
- **Pattern**: Any async trait in a cross-platform Rust project that touches
  WASM needs this pattern.

#### tokio::spawn vs wasm_bindgen_futures::spawn_local

- **Challenge**: `tokio::spawn` requires `Send` futures. On WASM, the runtime is
  single-threaded and `spawn_local` is the correct API.
- **Solution**: Used `#[cfg]` to dispatch to `tokio::spawn` on native and
  `wasm_bindgen_futures::spawn_local` on WASM. The WASM path uses a dummy
  `tokio::spawn(async {})` as the join handle since `spawn_local` doesn't return
  one.
- **Pattern**: For any background task spawning in cross-platform code,
  conditionally use `spawn_local` on WASM.

### Architectural Decisions

#### Marker Traits vs Duplicated Trait Definitions

- **Decision**: Used `MaybeSend`/`MaybeSync` marker traits rather than
  duplicating each trait with `#[cfg]` attributes.
- **Reasoning**: Duplicating would require maintaining two copies of every trait
  body (7+ traits), which is error-prone and verbose. The marker trait approach
  is a single point of change.
- **Trade-off**: The marker traits add a layer of indirection that might confuse
  contributors unfamiliar with the pattern.
- **Future**: If Rust stabilises trait aliases or conditional supertraits, these
  markers could be simplified.

#### PlatformAccessibility Trait Split

- **Decision**: Used two separate `#[cfg]`-gated trait definitions for
  `PlatformAccessibility` rather than the marker trait pattern.
- **Reasoning**: This trait had only 2 definitions needed and the `Send + Sync`
  bounds are part of the trait's public contract for platform bridge types.
  Keeping them explicit makes the platform-specific semantics clearer.
- **Trade-off**: Two trait blocks to maintain, but the trait is small and stable.
- **Future**: Could consolidate to use `MaybeSend`/`MaybeSync` if preferred.

### Development Workflow Insights

- The `wasm-pack build` uses `--release` mode which is significantly stricter
  than `cargo build`. Always test with `wasm-pack build --target web` not just
  `cargo build --target wasm32-unknown-unknown`.
- The initial `cargo build --target wasm32-unknown-unknown --lib` (dev profile)
  passed with 0 errors, but the `wasm-pack build` (release profile) had 56
  errors. This was because wasm-pack was already enabling the `wasm-bindgen`
  dependency which pulled in the WebGPU backend.
- Non-exhaustive pattern matches on `AriaRole` were pre-existing bugs that only
  surfaced during WASM compilation because the platform-specific code paths were
  newly compiled.
- The borrow checker issue in `WebDomOverlay::update_from_aria_tree` was also a
  pre-existing bug — `container` held an immutable borrow on `self` while
  methods needed mutable access. Cloning the `Element` (cheap JS object clone)
  resolved it cleanly.

### Follow-up Stories

1. **GUP-237: WASM Integration Test Suite** — Create a basic HTML page that
   loads the wasm-pack output and verifies core functionality (GPU context
   creation, mark rendering) in a headless browser. Currently we only verify
   compilation, not runtime behaviour.

2. **GUP-238: Remaining Send+Sync Audit** — Several traits still use direct
   `Send + Sync` bounds (e.g. `Axis`, `LabelFormatter`, `ErrorSink`,
   `RecoveryHandler`, `SurfaceEventHandler`, `IntoAttrValue`,
   `InteractionData`). While these don't currently cause WASM compilation errors
   (their concrete types happen to be `Send + Sync` even on WASM), they could
   break as more WASM-specific types are introduced. A systematic audit and
   migration to `MaybeSend`/`MaybeSync` would future-proof the codebase.
