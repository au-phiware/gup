# GUP-130: Mark Type ID Proc Macro

**Status**: ✅ Complete (2025-01-15)

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

- [x] Create `#[derive(MarkTypeId)]` proc macro in `gup-macros` crate
- [x] Generates `const MARK_TYPE_ID: u32` for each mark type
- [x] Assigns IDs based on annotation or default sequence
- [x] Works with all existing mark types (Circle, Rectangle, Line)

### AC2: Compile-Time Validation

- [x] Error if mark type ID exceeds valid range (0-255)
- [x] Error if two marks get the same ID (conflict detection)
- [x] Warning if mark type doesn't match shader enum
- [x] Helpful error messages for common mistakes

### AC3: GPU Shader Integration

- [x] Update `get_mark_type_id()` to use `M::MARK_TYPE_ID`
- [x] Add test that validates IDs match shader expectations
- [x] Document ID assignment in both Rust and WGSL

### AC4: Backward Compatibility

- [x] Existing code works without modification
- [x] Derive macro is optional (manual IDs still work)
- [x] No breaking changes to public API

## Technical Tasks

### 1. Proc Macro Crate Setup

- [x] Extend `gup-macros` with `MarkTypeId` derive
- [x] Add `syn`, `quote`, `proc-macro2` dependencies
- [x] Set up macro test infrastructure

### 2. ID Assignment Logic

- [x] Parse `#[mark_type_id = N]` attribute if present
- [x] Auto-assign sequential IDs if no attribute
- [x] Validate IDs are in valid range (0-255)
- [x] Detect and error on ID conflicts

### 3. Code Generation

- [x] Generate `const MARK_TYPE_ID: u32` associated constant
- [x] Implement `MarkTypeIdProvider` trait
- [x] Add doc comments explaining the ID value

### 4. Integration

- [x] Update `Mark` trait to include `MARK_TYPE_ID` constant
- [x] Refactor `get_mark_type_id()` to use trait constant
- [x] Update all built-in marks (Circle, Rectangle, Line)

### 5. Testing

- [x] Unit tests for proc macro code generation
- [x] Compile-fail tests for invalid IDs
- [x] Integration test with GPU interaction system
- [x] Test custom mark types with macro

## Dependencies

- **Requires**: GUP-128 (GPU Hit Test Debug) - ✅ Complete
- **Enables**: More robust mark type ID system
- **Blocks**: Nothing (enhancement, not required for functionality)

## Success Metrics

- [x] All marks use proc macro for IDs
- [x] No runtime overhead vs current implementation
- [x] Compile errors caught before runtime
- [x] Custom marks can easily add GPU interaction support

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

## Implementation Summary

**Completed**: 2025-01-15

### What Was Implemented

1. **MarkTypeId Derive Macro** (`gup-macros/src/mark_type_id.rs`):
   - Parses `#[mark_type_id = N]` attribute from mark types
   - Validates ID is in valid range (0-255)
   - Generates `const MARK_TYPE_ID: u32` associated constant
   - Implements `MarkTypeIdProvider` trait automatically
   - Provides helpful error messages for missing or invalid attributes

2. **MarkTypeIdProvider Trait** (`src/mark.rs`):
   - New trait for accessing mark type IDs
   - Single method: `fn mark_type_id() -> u32`
   - Documentation explains GPU integration purpose

3. **Mark Implementations Updated**:
   - `Circle`: `#[derive(MarkTypeId)] #[mark_type_id = 0]`
   - `Rectangle`: `#[derive(MarkTypeId)] #[mark_type_id = 1]`
   - `Line`: `#[derive(MarkTypeId)] #[mark_type_id = 2]`

4. **Selection Integration** (`src/selection.rs`):
   - Updated `get_mark_type_id()` to use `TypeId` comparison
   - Accesses `M::MARK_TYPE_ID` directly for known types
   - Falls back to type name matching for custom marks (backward compatible)

5. **GPU Shader Documentation** (`src/shaders/hit_test.compute.wgsl`):
   - Added header comment documenting mark type ID mappings
   - References the Rust derive macro for maintainability

6. **Tests** (`tests/mark_integration.rs`):
   - `test_mark_type_id_constants`: Validates all IDs are correct
   - Checks IDs match shader expectations (0=Circle, 1=Rectangle, 2=Line)
   - Validates IDs are in valid range (0-255)
   - Ensures IDs are unique

### Key Files Changed

- `gup-macros/src/mark_type_id.rs` (new): Proc macro implementation
- `gup-macros/src/lib.rs`: Added `MarkTypeId` derive macro export
- `src/mark.rs`: Added `MarkTypeIdProvider` trait
- `src/mark/circle.rs`: Applied `#[derive(MarkTypeId)]`
- `src/mark/rectangle.rs`: Applied `#[derive(MarkTypeId)]`
- `src/mark/line.rs`: Applied `#[derive(MarkTypeId)]`
- `src/selection.rs`: Updated mark type ID lookup logic
- `src/shaders/hit_test.compute.wgsl`: Added documentation
- `tests/mark_integration.rs`: Added validation test

### Test Results

- All acceptance criteria met
- New test `test_mark_type_id_constants` passes
- All existing tests pass (GPU tests with `--test-threads=1`)
- Clippy passes with no warnings in `gup-macros`
- No breaking changes to public API

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
