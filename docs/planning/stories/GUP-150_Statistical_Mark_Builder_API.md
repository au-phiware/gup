# GUP-150: Statistical Mark Builder API

**Status**: 📋 Planned

## Story Overview

**Title**: Observable Plot-style Builder for Statistical Marks  
**Epic**: Phase 2 Initiative 1 - High-Level Convenience APIs  
**Priority**: Low  
**Story Points**: 5

## Context

Per the implementation strategy, high-level APIs are Phase 2 work. Box plots and other statistical marks currently require manual attribute construction. An Observable Plot-style builder would provide ergonomic, declarative syntax for creating statistical visualizations.

## User Story

**As a** data visualization developer  
**I want** to create box plots using a simple builder API  
**So that** I can quickly visualize distributions without manual attribute setup

## Acceptance Criteria

### AC1: Box Plot Builder

- [ ] `box_plot()` builder function
- [ ] Fluent API for configuration
- [ ] Automatic statistical computation from data
- [ ] Sensible defaults for all visual properties

### AC2: Statistical Mark Patterns

- [ ] Generic pattern for statistical marks (reusable for violin, histogram, etc.)
- [ ] Support for grouped data (by category)
- [ ] Support for custom statistical functions
- [ ] Integration with scale system

### AC3: API Ergonomics

- [ ] Minimal code for common cases
- [ ] Clear error messages
- [ ] Type-safe attribute specification
- [ ] Natural composition with other marks

## Technical Requirements

- Follow Observable Plot conventions where sensible
- Build on proven low-level API (dog-fooding requirement)
- Use builder pattern with type-state for compile-time safety
- Support both statistical computation and pre-computed statistics

## Dependencies

- **Requires**: GUP-147 (Box Plot Visualization) - ✅ Complete
- **Requires**: GUP-149 (Box Plot GPU Rendering) - 📋 Planned
- **Part of**: Phase 2 High-Level APIs

## Testing Strategy

- API ergonomics testing with real use cases
- Compare code verbosity to D3/Observable Plot equivalents
- Test with various data shapes and groupings
- Benchmark convenience vs. manual attribute construction

## Success Metrics

- Box plot creation requires <10 lines of code
- API feels natural to D3/Observable Plot users
- Zero runtime overhead vs. manual construction
- Users prefer builder API in 80%+ of cases

## Risk Assessment

**Medium Risk**: High-level APIs are harder to get right. Need user feedback.

**Mitigation**: Start with Phase 1 patterns, iterate based on real usage.

## Definition of Done

- [ ] BoxPlot builder API implemented
- [ ] Pattern documented for other statistical marks
- [ ] Examples comparing manual vs. builder approaches
- [ ] User testing shows positive feedback
- [ ] All tests pass

---

_Identified during GUP-147 implementation. Aligns with Phase 2 strategy._
