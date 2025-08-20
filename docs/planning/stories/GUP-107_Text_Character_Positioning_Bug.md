# GUP-107: Text Character Positioning Bug

**Status**: Open  
**Priority**: High  
**Component**: Text Rendering  
**Affects**: SDF Text Rendering System

## Summary

Text strings are rendering with the leftmost characters missing or positioned
outside the visible area. Only partial text content is displayed, with the
rightmost portion of each string visible at the correct screen coordinates.

## Problem Statement

When rendering text strings, only a portion of each text string is visible on
screen. The leftmost characters of text strings are not rendered, while the
rightmost characters appear at the expected screen positions. This affects all
text rendering regardless of position, font size, or styling.

## Steps to Reproduce

1. Run the text rendering demo example:
   `cargo run --example text_rendering_demo`
2. Observe the rendered text on screen
3. Compare expected vs actual text content

## Expected Behavior

Text strings should render completely with all characters visible:

- "HELLO WORLD TESTING 123" should display the complete string
- "Performance Test: Many Text Elements" should display the complete title
- "Large text for testing" should display the complete string

## Actual Behavior

Text strings render with leftmost characters missing:

- "HELLO WORLD TESTING 123" displays as "ORLD TESTING 123" (missing "HELLO W")
- "Performance Test: Many Text Elements" displays as "y Text Elements" (missing
  "Performance Test: Man")
- "Large text for testing" displays as "ext for testing" (missing "Large text
  for t")

## Additional Observations

- The number of missing characters varies between different text strings
- The visible portions appear at the correct screen coordinates
- Text color, size, and styling render correctly for the visible portions
- Both demo text and performance test text are affected
- The issue occurs consistently across application restarts
- Screen positioning (X, Y coordinates) appears to work correctly
- Only character positioning within each text string is affected

## Test Case

```rust
// Simple test case to reproduce the issue
TextDemo {
    position: Vec2 { x: 100.0, y: 100.0 },
    text: "HELLO WORLD TESTING 123".to_string(),
    style: TextStyle::new(72.0).with_rgba(1.0, 0.0, 0.0, 1.0),
}
// Expected: "HELLO WORLD TESTING 123" in red at (100, 100)
// Actual: "ORLD TESTING 123" in red at correct screen position
```

## Environment

- Platform: Linux 6.12.39
- GPU: WebGPU/wgpu 26.0
- Text System: SDF (Signed Distance Field) rendering
- Font: Embedded default font (Squada One)

## Impact

- **Severity**: High - Text content is partially missing
- **User Experience**: Text appears truncated and incomplete
- **Functionality**: Core text rendering feature is broken
- **Workaround**: None identified

## Acceptance Criteria

- [ ] Complete text strings render with all characters visible
- [ ] Text positioning works correctly for all string lengths
- [ ] No characters are clipped or positioned outside visible area
- [ ] Existing text styling and positioning features continue to work
- [ ] Performance is not significantly impacted by the fix

## Investigation Notes

- Issue manifests as character-level positioning within text strings
- Screen coordinate system appears to function correctly
- Font atlas and SDF rendering system loads and processes characters
- Debug output confirms text reaches GPU rendering pipeline
- Surface size and projection matrix appear correct (1200x800)
