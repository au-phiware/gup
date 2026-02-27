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
