# Next Story Recommendation

**Date**: 2025-01-10  
**After**: GUP-135 (Fix Example Compilation Errors)

## Recommendation: GUP-131 - Add Constructor Methods to Shader Types

### Priority: LOW (but high value)

### Story Points: 1

### Estimated Time: 1-2 hours

## Reasoning

### 1. **Natural Follow-up to Example Fixes**

GUP-135 just cleaned up all the examples and exposed a recurring pattern:

- Examples create `Vec2`, `Vec3`, `Vec4` with struct literal syntax
- The code is verbose and error-prone
- Constructor methods would make examples cleaner

### 2. **Low Effort, High Ergonomics**

- Only 1 story point (very small)
- Simple implementation (just add `new()` methods)
- Immediately improves all example code
- Makes the shader types more intuitive for new users

### 3. **Momentum with Small Wins**

After GUP-032 (8 points) and GUP-135 (3 points), a 1-point story keeps momentum:

- Quick completion feels good
- Lets us test the new constructors in examples immediately
- Builds confidence before tackling larger stories

### 4. **Identified During GUP-032**

The GUP-032 retrospective specifically called out the need for constructors.
It's fresh in mind and a logical cleanup task.

## Alternative Considerations

**GUP-031 (GPU Interaction Event System)** - Partial, High priority but:

- Large story (13 points)
- Partially complete means we need to understand what's done
- Better to finish small wins first
- Benefits from having cleaner example code

**GUP-132 (GPU Path Tessellation)** - New, Medium priority but:

- Larger story (8 points)
- Builds on Path mark from GUP-032
- Would benefit from constructor methods being in place

**GUP-033 (Shader Function Composition Engine)** - New, Medium priority but:

- Large story (10 points)
- Core Phase 1 work
- Should be done after clearing small technical debt

## Dependencies Unblocked

GUP-131 is a dependency/enabler for:

- Cleaner example code in all demos
- Better developer experience for mark creation
- More intuitive API for new users
- Foundation for future mark tutorials

## Phase 1 Context

Phase 1 is about building rock-solid GPU primitives. We've completed:

- ✅ GUP-001 through GUP-015: Core foundations
- ✅ GUP-016 through GUP-032: Advanced features including mark system
- ✅ GUP-135: Example validation

Before moving to larger Phase 1 features (GUP-031, GUP-033), cleaning up the API
with constructors makes sense:

- Examples become teaching tools
- API feels more complete
- Small polish before big features

## Recommended Sequence

1. **Now**: GUP-131 (Shader Constructors) - 1 point ← _Start here_
2. **Next**: GUP-031 (GPU Interaction) - 13 points (complete the partial work)
3. **Then**: GUP-033 (Shader Composition) - 10 points (core Phase 1 feature)

## Alternative Path: Jump to Major Features

If you want to tackle significant functionality immediately:

1. **GUP-031 (GPU Interaction Event System)** - High priority, partially done
   - Would complete a critical Phase 1 component
   - Builds on GUP-012 work
   - 13 points but some work already exists

2. **GUP-033 (Shader Function Composition Engine)** - Core Phase 1 architecture
   - Enables dynamic shader composition
   - Medium priority but high architectural value
   - 10 points of greenfield work

## Summary

**Work on GUP-131 next** because:

- Natural follow-up to GUP-135 example fixes
- Very small effort (1 point)
- High ergonomic value for all mark usage
- Quick win to maintain momentum
- Identified during GUP-032 retrospective
- Makes examples and API more intuitive
- Good warm-up before tackling GUP-031 (13 points)

**Alternative: Jump to GUP-031** if:

- You want to tackle the largest remaining Phase 1 gap
- You're ready for a multi-day story
- You want to complete partially-done work
