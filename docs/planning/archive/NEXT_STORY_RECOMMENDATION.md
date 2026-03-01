# Next Story Recommendation

**Date**: 2025-01-10  
**After**: GUP-032 (Advanced Mark System with Custom Shapes)

## Recommendation: GUP-135 - Fix Example Compilation Errors

### Priority: HIGH

### Story Points: 3

### Estimated Time: 2-3 hours

## Reasoning

### 1. **Technical Debt Must Be Addressed**

The compilation errors in examples are technical debt that will:

- Block new users from learning Gup
- Make it harder to validate future changes
- Degrade confidence in the project
- Grow worse over time as more API changes happen

### 2. **Quick Win with High Impact**

- Only 3 story points (small investment)
- High value - all examples working again
- Unblocks documentation and tutorial work
- Sets up CI to prevent future breakage

### 3. **Momentum Preservation**

GUP-032 just completed the Mark system. Examples use marks extensively. Fixing
them now:

- Validates the new marks work correctly
- Provides immediate usage examples for Path, CompositeMark, Text
- Tests the Custom Mark Guide patterns in real code

### 4. **Natural Continuation**

The ShaderFunction API updates that broke examples are related to the work we
just did:

- GUP-032 added new marks with shader functions
- Examples show how to use ShaderFunctions
- Fixing them validates our architecture choices

## Alternative Considerations

**GUP-131 (Shader Type Constructors)** - Also quick (1 point) but:

- Lower impact than broken examples
- Can wait until next batch of mark work
- GUP-032 retrospective showed workaround is acceptable

**GUP-031 (GPU Interaction Event System)** - Partial, High priority but:

- Large story (13 points)
- Better to clear debt first
- Would benefit from working examples

**GUP-135 first enables:**

- Clean slate for next major feature
- Working examples for documentation
- CI protection against future breakage
- Validation of GUP-032 marks in real usage

## Dependencies Unblocked

Fixing examples will unblock:

- GUP-086: Observable Plot Migration Guide (needs working examples)
- Documentation work (needs runnable code samples)
- New user onboarding (first thing they try is examples)
- Integration testing of mark system

## Phase 1 Context

We're deep in Phase 1 foundation work. Before moving to Phase 2 high-level APIs:

- Examples must work (they're our dogfooding proof)
- Technical debt should be minimal
- Mark system should be proven in practice

GUP-135 completes the validation loop: ✅ GUP-032: Built advanced marks →
GUP-135: Prove they work in examples → Ready for next Phase 1 initiative

## Recommended Sequence

1. **Now**: GUP-135 (Fix Examples) - 3 points
2. **Next**: GUP-131 (Shader Constructors) - 1 point
3. **Then**: GUP-031 (GPU Interaction) or GUP-033 (Shader Composition) - Major
   features

This clears the deck for focused work on remaining Phase 1 goals.

## Summary

**Work on GUP-135 next** because:

- High priority technical debt
- Quick win (3 points)
- Validates GUP-032 work
- Unblocks documentation and onboarding
- Sets up CI protection
- Natural continuation of mark system work
- Clears the path for Phase 1 completion
