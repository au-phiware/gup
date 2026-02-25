# GUP-207: Fix Pre-existing Doctest Failures

## Story Overview

**Epic**: Phase 1 - Foundation  
**Theme**: Code Quality and Maintenance  
**Priority**: Medium  
**Story Points**: 2  
**Status**: 📋 Planned

## Problem Statement

Six doctests across three modules fail due to API drift. The code examples in
Rustdoc comments reference outdated constructors and missing imports, causing
`cargo test --doc` to report failures. This erodes documentation trust and makes
it harder to detect new doctest regressions.

## Acceptance Criteria

- [ ] All doctests in `src/context.rs` pass (fix `new_headless()` → `headless()`
      API change)
- [ ] All doctests in `src/chart_builder/optimized_accessor.rs` pass (fix
      missing macro import)
- [ ] All doctests in `src/mixable/merge.rs` pass (fix missing
      `#[derive(Debug)]`)
- [ ] `cargo test --doc` reports 0 failures
- [ ] No regressions in existing tests

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

- [ ] All 6 failing doctests fixed
- [ ] `cargo test --doc` passes with 0 failures
- [ ] No regressions
