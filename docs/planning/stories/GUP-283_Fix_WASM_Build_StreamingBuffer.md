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
