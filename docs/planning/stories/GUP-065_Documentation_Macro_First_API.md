# GUP-065: Documentation for Macro-First Type Construction API

## Story Overview

**Title**: Update Documentation for Macro-First Type Construction API **Epic**:
Phase 1 Initiative 2 - Unified Shader Function System **Priority**: Medium
**Story Points**: 3

## Context

Following GUP-008's implementation of the macro-based type construction system,
the documentation needs to be updated to reflect the new macro-first approach.
The old constructor-based examples are no longer valid and may confuse
developers.

## User Story

**As a** developer using Gup's type system **I want** clear documentation
showing the macro-first construction approach **So that** I can quickly learn
and adopt the ergonomic type construction patterns

## Acceptance Criteria

### AC1: Updated Code Examples

- [ ] All documentation examples use macro construction (`vec3![x, y, z]`)
- [ ] Remove references to old constructor patterns (`Vec3::new(x, y, z)`)
- [ ] Update README examples with macro patterns
- [ ] Include macro import requirements in examples

### AC2: Macro Usage Documentation

- [ ] Document all available macros: `vec2!`, `vec3!`, `vec4!`, `mat2!`,
      `mat3!`, `mat4!`
- [ ] Show correct bracket syntax for each macro
- [ ] Explain benefits of macro approach over constructors
- [ ] Include performance notes (compile-time validation)

### AC3: Migration Guide

- [ ] Provide migration guide from old constructor syntax
- [ ] Include find-and-replace patterns for upgrading existing code
- [ ] Document import requirements for macros
- [ ] Explain compatibility breaking changes

## Technical Tasks

### 1. README Updates

- [ ] Update main README examples to use macro construction
- [ ] Add macro import examples with `use gup::*;`
- [ ] Replace all constructor-based code snippets
- [ ] Update performance claims to include macro benefits

### 2. API Documentation

- [ ] Update module-level documentation in `shader_function.rs`
- [ ] Add comprehensive macro usage examples
- [ ] Document GPU memory layout benefits
- [ ] Include type safety explanations

### 3. Tutorial Content

- [ ] Update getting started examples
- [ ] Create macro-specific tutorial section
- [ ] Include common usage patterns
- [ ] Add troubleshooting for import issues

## Dependencies

### Prerequisite Stories

- GUP-008: Type System Integration (completed)

### Enables Stories

- Future API consistency initiatives
- Developer onboarding improvements

## Testing Strategy

### Documentation Tests

```rust
// Ensure all doc examples compile and run
#[test]
fn test_documentation_examples() {
    // Test examples from README
    let position = vec3![1.0, 2.0, 3.0];
    let transform = mat4![/* ... 16 values ... */];

    // Verify they work as documented
    assert_eq!(position.x, 1.0);
}
```

### Content Validation

- [ ] All code examples compile successfully
- [ ] Links to macro documentation work correctly
- [ ] Import examples are complete and accurate
- [ ] Migration guide examples are tested

## Success Metrics

### Documentation Quality

- [ ] **Example Accuracy**: 100% of code examples compile and run
- [ ] **Migration Coverage**: All old patterns have documented replacements
- [ ] **Import Clarity**: Clear guidance on macro imports
- [ ] **Performance Claims**: Accurate statements about macro benefits

### Developer Experience

- [ ] **Quick Start**: Developers can use macros immediately from examples
- [ ] **Error Recovery**: Clear guidance when import issues occur
- [ ] **Migration Path**: Existing code can be updated systematically
- [ ] **Performance Understanding**: Benefits of macro approach are clear

## Implementation Notes

### Documentation Structure

````markdown
# Type Construction

## Quick Start

```rust
use gup::*;

let position = vec3![0.0, 1.0, 0.0];
let transform = mat4![1.0, 0.0, 0.0, 0.0, /* ... */];
```
````

## Available Macros

- `vec2![x, y]` - 2D vector construction
- `vec3![x, y, z]` - 3D vector with GPU padding
- `vec4![x, y, z, w]` - 4D vector construction
- `mat2![m00, m01, m10, m11]` - 2x2 matrix
- `mat3![...]` - 3x3 matrix (9 parameters)
- `mat4![...]` - 4x4 matrix (16 parameters)

## Migration from Constructors

```rust
// ❌ Old constructor syntax
let v = Vec3::new(1.0, 2.0, 3.0);

// ✅ New macro syntax
let v = vec3![1.0, 2.0, 3.0];
```

### Key Messages

- **Ergonomic**: Cleaner syntax for complex types
- **Performance**: Compile-time validation, zero runtime cost
- **GPU-Compatible**: Proper memory layout handled automatically
- **Consistent**: Uniform interface across all vector/matrix types

## Definition of Done

- [ ] All documentation uses macro-first examples
- [ ] Migration guide tested with real code
- [ ] Import requirements clearly documented
- [ ] Performance benefits accurately stated
- [ ] Doc tests pass for all examples
- [ ] Code review completed and approved
