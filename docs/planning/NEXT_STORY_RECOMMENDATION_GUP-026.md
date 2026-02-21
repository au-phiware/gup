# Next Story Recommendation After GUP-026

**Date**: 2025-01-27  
**Completed Story**: GUP-026 - Data Source Merge Implementation

## Recommendation: GUP-028 - Composition Performance Optimization

### Reasoning

**Priority**: Medium  
**Story Points**: 4  
**Dependencies**: GUP-021 (Complete), GUP-027 (Complete), GUP-026 (Complete)

GUP-028 is the natural next step because:

1. **Dependency Chain Complete**: All prerequisite composition stories (GUP-021,
   GUP-026, GUP-027) are now complete. GUP-028 represents the optimization and
   refinement phase of the composition system.

2. **Phase 1 Completion**: Phase 1's core composition system is functionally
   complete. GUP-028 would validate that we meet the "100K+ points at 60 FPS"
   performance target that's a key Phase 1 success metric.

3. **Natural Follow-up**: Having implemented:
   - Advanced composition modes (GUP-021)
   - Data source merging (GUP-026)
   - GPU blend state integration (GUP-027)

   We now need to ensure all these features perform well together. GUP-028 would
   profile and optimize the composition system as a whole.

4. **Foundation for Phase 2**: Performance validation of Phase 1 primitives
   ensures we're building Phase 2's high-level APIs on a solid, performant
   foundation—exactly the "dog-fooding" philosophy from
   IMPLEMENTATION_STRATEGY.md.

### Alternative: GUP-033 - Shader Function Composition Engine

If performance optimization feels premature (since we haven't identified
bottlenecks), GUP-033 would be an excellent alternative:

**Why GUP-033**:

- Addresses a Phase 1 core goal: "unified shader function system"
- Medium priority, 10 story points (substantial but achievable)
- Enables more advanced shader composition patterns
- No blocking dependencies

**Trade-off**: Adds new features rather than validating existing ones

### Alternative: Documentation and Polish

Given Phase 1 functional completion, we could also focus on:

1. **GUP-086** - Observable Plot Migration Guide (Low priority, 2 points)
   - Helps users understand Gup's API
   - Lightweight story, good for momentum

2. **Phase 1 Validation** - Create a comprehensive example/benchmark suite that:
   - Tests all composition modes together
   - Validates 100K+ point performance
   - Documents Phase 1 capabilities
   - Doesn't exist as a story yet, would need to be created

## Recommendation Summary

**Primary**: GUP-028 (Composition Performance Optimization)

- Validates Phase 1 success metrics
- Natural conclusion to composition work
- Ensures we meet performance goals before Phase 2

**Secondary**: GUP-033 (Shader Function Composition Engine)

- Advances shader system capabilities
- Aligns with Phase 1 goals
- Builds on completed shader work (GUP-051, GUP-052, GUP-053, GUP-054)

**Tertiary**: Create a Phase 1 validation/benchmark story

- Comprehensive testing of all Phase 1 features
- Documentation of capabilities
- Performance baseline for future optimization
