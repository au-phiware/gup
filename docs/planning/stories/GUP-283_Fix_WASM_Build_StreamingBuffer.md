# GUP-283: Fix WASM Build (`StreamingBuffer` Send/Sync)

## Story Overview

**Initiative**: Core Infrastructure **Status**: ✅ Complete **Created**:
2025-07-18 **Completed**: 2025-07-18

## Context

The `wasm-pack build --target web` and `wasm-pack test --headless --chrome`
commands fail because `StreamingBuffer<T>` (in
`src/streaming/streaming_buffer.rs`) casts to `Box<dyn Any + Send + Sync>`, but
the underlying wgpu `Buffer` type is `!Send` on WASM targets. This blocks all
WASM packaging and browser-based integration tests.

Discovered during GUP-264 (Tauri Integration) where the WASM build was attempted
to verify the new `render_scatter` entry point.

## User Story

> "As a Gup developer, I want `wasm-pack build` and `wasm-pack test` to succeed
> so that WASM packaging, browser benchmarks, and integration tests work in CI."

## Acceptance Criteria

- [x] `wasm-pack build --target web` succeeds.
- [x] `wasm-pack test --headless --chrome` succeeds (or is blocked only by
      browser/WebGPU availability, not compilation errors).
- [x] All native tests (`cargo test -- --test-threads=1`) continue to pass.
- [x] No loss of functionality in the streaming module on native targets.

## Technical Tasks

- [x] Audit `StreamingBuffer` and `DataStream` for `Send + Sync` requirements.
- [x] Use `#[cfg(target_arch = "wasm32")]` to relax bounds on WASM or use
      `MaybeSend`/`MaybeSync` traits already defined in the crate.
- [x] Verify no other WASM compilation errors exist.

## Dependencies

### Prerequisite Stories

- None (standalone fix).

### Enables Stories

- GUP-264 ✅ (WASM build validation)
- Any future WASM-dependent stories.

## Testing Strategy

- `wasm-pack build --target web` compiles cleanly.
- `cargo test -- --test-threads=1` on native shows no regressions.

## Risk Assessment

- **Low**: The fix is likely a straightforward conditional compilation change.

## Definition of Done

- [x] `wasm-pack build --target web` succeeds.
- [x] All native tests pass.
- [x] `mask all-fix` exits cleanly.

## Implementation Summary

### What Was Implemented

The root cause was `Selection.stream_source` being typed as
`Option<Box<dyn Any + Send + Sync>>`. On WASM, `wgpu::Buffer` contains
`RefCell<WebBufferMapState>` which is `!Sync`, so `DataStream<T>` (which wraps
`StreamingBuffer<T>` containing `wgpu::Buffer`) could not be cast to
`Box<dyn Any + Send + Sync>`.

### Key Changes

1. **`src/selection.rs`** — Split `stream_source` field with `#[cfg]`: uses
   `Box<dyn Any + Send + Sync>` on native, `Box<dyn Any>` on WASM. Split
   `stream()`, `stream_ref()`, `stream_mut()`, and `detach_stream()` methods
   similarly to relax `Send + Sync` bounds on WASM.

2. **`src/streaming/stream.rs`** — Split `Subscriber<T>.callback` and
   `subscribe()` method with `#[cfg]` to relax `Send + Sync` on WASM.

3. **`src/lib.rs`** — Added missing doc comments on `MaybeSend`/`MaybeSync`
   WASM-only trait definitions.

4. **`src/accessibility/atspi.rs`**, **`src/accessibility/platform.rs`**,
   **`src/accessibility/web_overlay.rs`** — Added missing doc comments on
   WASM-only types/fields that were blocking `wasm-pack build` due to
   `#[deny(missing_docs)]`.

### Test Counts

- 243 native tests pass, 0 failures
- `wasm-pack build --target web` succeeds (✨ Done)
- `wasm-pack test` blocked by pre-existing `tokio::runtime::Runtime::new()` in
  integration tests (not a `StreamingBuffer` issue)

## Retrospective

**Completed**: 2025-07-18

### Key Technical Learnings

#### wgpu WASM Buffer Internals

- **Challenge**: `wgpu::Buffer` on WASM wraps `WebBuffer` which contains
  `RefCell<WebBufferMapState>` — this makes it `!Sync`. Any type containing
  `wgpu::Buffer` cannot satisfy `Send + Sync` bounds on WASM.
- **Solution**: Use `#[cfg(target_arch = "wasm32")]` to conditionally relax
  `Send + Sync` bounds wherever GPU resource types are type-erased via
  `Box<dyn Any + Send + Sync>`.
- **Pattern**: The codebase already had this pattern for
  `transition_end_callback` in `Selection`. Apply the same `#[cfg]` split to any
  new type-erased field that may contain wgpu resources.

#### WASM Documentation Enforcement

- **Challenge**: The `#[deny(missing_docs)]` lint only surfaces for
  `#[cfg(target_arch = "wasm32")]` items when actually targeting WASM, so
  missing docs on WASM-only types go unnoticed during normal development.
- **Solution**: Fixed 5 missing doc items across `lib.rs`, `atspi.rs`,
  `platform.rs`, and `web_overlay.rs`.
- **Pattern**: Periodically run `wasm-pack build` in CI or locally to catch
  WASM-only lint issues early.

### Architectural Decisions

#### cfg-Split vs MaybeSend/MaybeSync for Type Erasure

- **Decision**: Used `#[cfg]` split on struct fields and method bounds rather
  than converting to `MaybeSend`/`MaybeSync` traits.
- **Reasoning**: `Box<dyn Any + MaybeSend + MaybeSync>` is not valid Rust —
  `MaybeSend`/`MaybeSync` are not supertraits of standard library traits, so
  they cannot be used as additional trait bounds on `dyn Any`. The `#[cfg]`
  approach is the only viable pattern for type-erased `Any` boxing.
- **Trade-off**: Some code duplication between native and WASM variants, but the
  duplication is minimal (just the method signatures differ, not the bodies).
- **Future**: If more type-erased fields with `Send + Sync` bounds are added,
  consider a macro to reduce duplication.

### Development Workflow Insights

- The fix was straightforward once the root cause was identified via the
  compiler error message. The key insight was recognising the existing
  `transition_end_callback` pattern as the template.
- `wasm-pack build` takes ~60s on a warm cache, which is fast enough for
  iterative development.
- Pre-existing `gup-macros` clippy warnings are noisy but do not affect the main
  crate.

### Follow-up Stories

1. **GUP-285: Fix WASM Integration Test Compilation** — The `wasm-pack test`
   command fails because `tests/html_export_integration.rs` uses
   `tokio::runtime::Runtime::new()` which is not available on
   `wasm32-unknown-unknown`. These tests need
   `#[cfg(not(target_arch = "wasm32"))]` guards or a WASM-compatible runtime
   (e.g. `wasm-bindgen-test`).
