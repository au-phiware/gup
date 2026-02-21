# GUP-130: Mark Type ID Proc Macro

**Status**: 💡 New

## Story Overview

**Title**: Create Proc Macro for Stable Mark Type IDs  
**Epic**: Phase 1 Initiative 4 - Interaction System  
**Priority**: Low  
**Story Points**: 5

## Context

GUP-128 fixed mark type ID generation by using type name string matching (e.g.,
checking if type name contains "Circle"). This works but is fragile to
refactoring (renaming a type breaks GPU interaction). A proc macro would provide
compile-time stable IDs with better error messages.

## User Story

**As a** Gup library developer  
**I want** mark types to automatically get stable GPU-compatible IDs at compile
time  
**So that** mark type IDs are correct by construction and resistant to
refactoring

## Acceptance Criteria

### AC1: Proc Macro Implementation

- [ ] Create `#[derive(MarkTypeId)]` proc macro in `gup-macros` crate
- [ ] Generates `const MARK_TYPE_ID: u32` for each mark type
- [ ] Assigns IDs based on annotation or default sequence
- [ ] Works with all existing mark types (Circle, Rectangle, Line)

### AC2: Compile-Time Validation

- [ ] Error if mark type ID exceeds valid range (0-255)
- [ ] Error if two marks get the same ID (conflict detection)
- [ ] Warning if mark type doesn't match shader enum
- [ ] Helpful error messages for common mistakes

### AC3: GPU Shader Integration

- [ ] Update `get_mark_type_id()` to use `M::MARK_TYPE_ID`
- [ ] Add test that validates IDs match shader expectations
- [ ] Document ID assignment in both Rust and WGSL

### AC4: Backward Compatibility

- [ ] Existing code works without modification
- [ ] Derive macro is optional (manual IDs still work)
- [ ] No breaking changes to public API

## Technical Tasks

### 1. Proc Macro Crate Setup

- [ ] Extend `gup-macros` with `MarkTypeId` derive
- [ ] Add `syn`, `quote`, `proc-macro2` dependencies
- [ ] Set up macro test infrastructure

### 2. ID Assignment Logic

- [ ] Parse `#[mark_type_id = N]` attribute if present
- [ ] Auto-assign sequential IDs if no attribute
- [ ] Validate IDs are in valid range (0-255)
- [ ] Detect and error on ID conflicts

### 3. Code Generation

- [ ] Generate `const MARK_TYPE_ID: u32` associated constant
- [ ] Implement `MarkTypeIdProvider` trait
- [ ] Add doc comments explaining the ID value

### 4. Integration

- [ ] Update `Mark` trait to include `MARK_TYPE_ID` constant
- [ ] Refactor `get_mark_type_id()` to use trait constant
- [ ] Update all built-in marks (Circle, Rectangle, Line)

### 5. Testing

- [ ] Unit tests for proc macro code generation
- [ ] Compile-fail tests for invalid IDs
- [ ] Integration test with GPU interaction system
- [ ] Test custom mark types with macro

## Dependencies

- **Requires**: GUP-128 (GPU Hit Test Debug) - ✅ Complete
- **Enables**: More robust mark type ID system
- **Blocks**: Nothing (enhancement, not required for functionality)

## Success Metrics

- [ ] All marks use proc macro for IDs
- [ ] No runtime overhead vs current implementation
- [ ] Compile errors caught before runtime
- [ ] Custom marks can easily add GPU interaction support

## Risk Assessment

**Low Risk**: Optional enhancement. Existing type name approach continues to
work. Can be adopted incrementally.

## Implementation Notes

### Example Usage

```rust
use gup::mark::Mark;

#[derive(Clone, Mark, MarkTypeId)]
#[mark_type_id = 0]  // Explicit ID (optional)
pub struct Circle {
    // ...
}

#[derive(Clone, Mark, MarkTypeId)]
// Auto-assigned ID = 1 (next available)
pub struct Rectangle {
    // ...
}

#[derive(Clone, Mark, MarkTypeId)]
#[mark_type_id = 2]
pub struct Line {
    // ...
}
```

### Generated Code

```rust
impl Circle {
    pub const MARK_TYPE_ID: u32 = 0;
}

impl MarkTypeIdProvider for Circle {
    fn mark_type_id() -> u32 {
        Self::MARK_TYPE_ID
    }
}
```

### Validation

```rust
// Compile-time validation in tests
#[test]
fn test_mark_ids_match_shader() {
    assert_eq!(Circle::MARK_TYPE_ID, 0, "Circle must be 0 in shader");
    assert_eq!(Rectangle::MARK_TYPE_ID, 1, "Rectangle must be 1 in shader");
    assert_eq!(Line::MARK_TYPE_ID, 2, "Line must be 2 in shader");
}
```

### Shader Documentation

Add to `hit_test.compute.wgsl`:

```wgsl
// Mark type IDs must match Rust MarkTypeId assignments:
// 0 = Circle (gup::mark::Circle)
// 1 = Rectangle (gup::mark::Rectangle)
// 2 = Line (gup::mark::Line)
// Update this comment if mark types change!
```

## Alternative: Const Trait Methods

Instead of a proc macro, could use const trait methods (when stabilized):

```rust
pub trait Mark {
    const MARK_TYPE_ID: u32;
}

impl Mark for Circle {
    const MARK_TYPE_ID: u32 = 0;
}
```

**Pros**: Simpler, no proc macro needed  
**Cons**: Manual assignment, no validation, requires remembering to add ID

**Decision**: Proc macro provides better error checking and automation, worth
the complexity.

---

_Created from GUP-128 retrospective - identified fragility in type name-based
mark type IDs._
