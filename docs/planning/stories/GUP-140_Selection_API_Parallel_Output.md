# GUP-140: Selection API Parallel Output Integration

**Status**: 💡 New

## Story Overview

**Title**: Enable Selection API to Work with Parallel Composed Functions
**Epic**: Phase 1 Initiative 4 - Advanced Data Mapping
**Priority**: Medium
**Story Points**: 8

## Context

GUP-136 implemented the core `ParallelComposition` functionality, enabling shader functions that compute multiple outputs from a single input. However, the Selection API doesn't yet know how to consume `ParallelOutput<A, B>` types and bind the individual outputs to separate visual attributes (e.g., position and color).

This story completes the parallel composition feature by integrating it with the rendering pipeline.

## User Story

**As a** data visualization developer
**I want** to use parallel composition with the Selection API
**So that** I can efficiently map data to multiple visual channels (position, color, size) in a single pass

## Acceptance Criteria

### AC1: ParallelOutput Buffer Management
- [ ] Create buffer extraction utilities for `ParallelOutput<A, B>`
- [ ] Support splitting ParallelOutput into separate GPU buffers for each attribute
- [ ] Maintain proper memory alignment and padding

### AC2: Selection API Multi-Attribute Binding
- [ ] Add `.attr_parallel()` method to Selection for binding parallel outputs
- [ ] Support binding position + color in single call
- [ ] Support binding position + color + size (nested ParallelOutput)
- [ ] Maintain type safety with compile-time checks

### AC3: Integration Examples
- [ ] Create example using parallel composition for scatter plot
- [ ] Demonstrate 3-attribute binding (position XY + color + size)
- [ ] Show performance comparison vs sequential attribute binding

## Technical Requirements

- Buffer extraction from `ParallelOutput<A, B>` 
- Selection API method: `.attr_parallel(parallel_function, ["attr1", "attr2"])`
- Attribute name mapping to output fields
- Support for nested parallel outputs

## Dependencies

- **Requires**: GUP-136 (Parallel Composition Implementation) - Complete
- **Requires**: GUP-002 (Core Selection Type) - Complete
- **Enables**: Full end-to-end parallel multi-attribute data mapping

## Testing Strategy

- Unit tests for buffer extraction
- Integration tests with Selection API
- Visual tests with example charts
- Performance benchmarks (coordinate with GUP-137)

## Success Metrics

- Parallel composition works in full rendering pipeline
- Examples demonstrate clear API usage
- Type errors caught at compile time
- Performance improvement measurable (GUP-137)

## Risk Assessment

**Medium Risk**: Buffer management complexity may require careful alignment handling. Mitigation: Leverage existing buffer infrastructure patterns.

## Definition of Done

- [ ] ParallelOutput buffer extraction implemented
- [ ] Selection API integration complete
- [ ] Multi-attribute binding working
- [ ] Examples demonstrating usage
- [ ] All tests pass
- [ ] Documentation updated

---

_Identified during GUP-136 implementation as necessary for full parallel composition support._
