# GUP-238: Remaining Send+Sync Audit

**Priority**: Low **Complexity**: Low **Created**: 2026-02-27 **Status**: 🚧 In
Progress

## Overview

Audit and migrate remaining `Send + Sync` trait bounds throughout the codebase
to use the `MaybeSend`/`MaybeSync` marker traits introduced in GUP-231. While
the current bounds don't cause WASM compilation failures, they could break as
more WASM-specific concrete types are introduced.

## Context

GUP-231 resolved all blocking WASM compilation errors by introducing
`MaybeSend`/`MaybeSync` for the core traits (`Mixable`, `AsyncMixable`,
`Renderable`, etc.). However, several secondary traits still use direct
`Send + Sync` bounds:

- `Axis: Send + Sync`
- `LabelFormatter: Send + Sync`
- `ErrorSink: Send + Sync`
- `RecoveryHandler: Send + Sync`
- `SurfaceEventHandler: Send + Sync`
- `IntoAttrValue: Send + Sync`
- `InteractionData: Send + Sync`
- Various closure bounds (`Box<dyn Fn(...) + Send + Sync>`)

These currently work because their concrete implementors happen to be
`Send + Sync` even on WASM, but this is fragile.

## User Story

As a library developer, I want all trait bounds to consistently use conditional
Send/Sync so that adding new WASM-specific types never causes unexpected
compilation failures.

## Acceptance Criteria

- [ ] All trait definitions using `Send + Sync` migrated to
      `MaybeSend + MaybeSync`
- [ ] All `Box<dyn Fn(...) + Send + Sync>` type aliases updated
- [ ] Native tests continue to pass
- [ ] WASM build continues to succeed

## Technical Tasks

- [ ] Grep for `Send + Sync` in trait definitions and type aliases
- [ ] Replace with `MaybeSend + MaybeSync` where appropriate
- [ ] Add MaybeSend/MaybeSync imports to affected files
- [ ] Verify no regressions

## Dependencies

- **Requires**: GUP-231 (WASM Build Platform Gating) ✅

## Testing Strategy

- Native test regression
- WASM compilation check

## Risk Assessment

- **Low**: Purely mechanical replacement of bounds

## Definition of Done

- [ ] No direct `Send + Sync` bounds remain on public traits
- [ ] Native and WASM builds pass
- [ ] All tests pass
