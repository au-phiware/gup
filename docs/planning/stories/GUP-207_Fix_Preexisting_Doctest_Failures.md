# GUP-207: Fix Pre-existing Doctest Failures

## Story Overview

**Epic**: Phase 1 - Foundation  
**Theme**: Code Quality and Maintenance  
**Priority**: Medium  
**Story Points**: 2  
**Status**: ✅ Complete (2025-07-17)

## Problem Statement

Six doctests across three modules fail due to API drift. The code examples in
Rustdoc comments reference outdated constructors and missing imports, causing
`cargo test --doc` to report failures. This erodes documentation trust and makes
it harder to detect new doctest regressions.

## Acceptance Criteria

- [x] All doctests in `src/context.rs` pass (fix `new_headless()` → `headless()`
      API change)
- [x] All doctests in `src/chart_builder/optimized_accessor.rs` pass (fix
      missing macro import)
- [x] All doctests in `src/mixable/merge.rs` pass (fix missing
      `#[derive(Debug)]`)
- [x] `cargo test --doc` reports 0 failures
- [x] No regressions in existing tests

## Technical Tasks

1. Update `GupContext::add_surface_with_config` doctest to use `headless()`
2. Update `GupContext::query_surface_capabilities` doctest to use `headless()`
3. Fix `field_accessor` doctest import
4. Fix `Mergeable` doctest `#[derive(Debug)]` annotation
5. Run full doctest suite to verify zero failures
6. Update `ComposedChart::generate_axis_geometry` doctests

## Dependencies

None — these are independent fixes.

## Testing Strategy

- `cargo test --doc` must report 0 failures
- `cargo test -- --test-threads=1` must still pass

## Definition of Done

- [x] All 6 failing doctests fixed
- [x] `cargo test --doc` passes with 0 failures
- [x] No regressions

## Implementation Summary

### What was implemented

Fixed all 6 pre-existing doctest compilation failures across 4 source files:

1. **`src/context.rs`** (2 doctests): Updated `new_headless()` → `headless()`
   with proper async wrapper (`async fn example()`) and dummy window type
   implementing `HasWindowHandle`/`HasDisplayHandle` traits (replacing
   `Arc::new(())` which doesn't implement these).
2. **`src/mixable/merge.rs`** (1 doctest): Added missing `#[derive(Debug)]` to
   `MyChart` struct in the `Mergeable` trait doctest.
3. **`src/chart_builder/optimized_accessor.rs`** (1 doctest): Added missing
   `use gup::field_accessor;` import for the `field_accessor!` macro.
4. **`src/chart_builder.rs`** (2 doctests): Fixed `Circle` import path from
   `gup::selection::Circle` to `gup::Circle` in both `generate_axis_geometry`
   and `generate_axis_geometry_resolved` doctests.

### Key files changed

- `src/context.rs`
- `src/mixable/merge.rs`
- `src/chart_builder/optimized_accessor.rs`
- `src/chart_builder.rs`

### Test results

- 106 doctests pass (105 ok + 1 compile-fail), 49 ignored, 0 failures
- 1330+ unit/integration tests pass with 0 failures
