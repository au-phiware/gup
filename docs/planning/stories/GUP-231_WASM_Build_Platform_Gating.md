# GUP-231: WASM Build Platform Gating

**Priority**: Medium **Complexity**: Medium **Created**: 2025-08-07 **Status**:
🚧 In Progress

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

- [ ] `cargo build --target wasm32-unknown-unknown --lib` succeeds
- [ ] `wasm-pack build --target web` succeeds and produces a loadable package
- [ ] All native tests continue to pass
- [ ] Platform-specific accessibility backends gated behind `cfg(target_os)`
- [ ] DOM integration code properly gated for `wasm32` vs native targets
- [ ] `Send`/`Sync` bounds resolved for WASM DOM callback types

## Technical Tasks

- [ ] Audit all modules for `wasm32` compilation errors
- [ ] Gate `LinuxAccessibility` behind `cfg(target_os = "linux")`
- [ ] Gate Windows/macOS accessibility behind respective `cfg(target_os)`
- [ ] Add missing `web-sys` features (e.g. `TouchEvent`) for WASM
- [ ] Resolve `Send`/`Sync` bound issues on DOM callback types
- [ ] Verify wasm-pack produces a working package
- [ ] Update CI to include WASM compilation check

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

- [ ] `wasm-pack build --target web` produces a loadable package
- [ ] Native test suite passes (same count as before)
- [ ] CI includes WASM compilation check
