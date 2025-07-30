# Gup Development Conventions

This document captures key learnings, patterns, and conventions discovered
during the development of the Gup visualization library.

## Rust Design Patterns

### Prefer Enums Over Trait Objects for Known Sets

**Learning**: When implementing extensible behavior with a finite, known set of
variants, prefer enums over trait objects (`Box<dyn Trait>`).

**Example**: In GUP-021, we initially tried:

```rust
// ❌ Problematic - trait not object-safe due to generic methods
trait CustomCompositionBehavior {
    fn compose<A: Mixable, B: Mixable>(...) -> GupResult<()>;
}
custom_behavior: Option<Box<dyn CustomCompositionBehavior>>,
```

**Solution**:

```rust
// ✅ Better - enum-based approach
#[derive(Debug, Clone)]
enum CustomCompositionBehavior {
    CrossFade(CrossFadeComposition),
    GridLayout(GridLayoutComposition),
}
```

**Benefits**:

- Compile-time type safety
- Better performance (no vtable indirection)
- Easier to serialize/deserialize
- Pattern matching exhaustiveness checking

**When to use trait objects**: When you need true open extensibility where
external crates can add implementations.

### Generic Method Limitations

**Learning**: Traits with generic methods cannot be made into trait objects due
to Rust's object safety rules.

**Guideline**: If you need both trait objects and generic methods, consider:

1. Separate the generic methods into a different trait
2. Use an enum-based approach for known variants
3. Use associated types instead of generic parameters where possible

## API Design Patterns

### Fluent APIs with Backward Compatibility

**Learning**: When extending existing APIs, maintain backward compatibility
while providing new convenience methods.

**Pattern**:

```rust
// Existing API continues to work
let composed = chart1.mix(chart2);

// New convenience methods added via extension traits
let overlay = chart1.overlay(chart2);
let beside = chart1.beside_with_config(chart2, config);
```

**Guidelines**:

- Use extension traits for new convenience methods
- Keep core trait minimal and stable
- Provide both simple defaults and configurable variants

### Configuration Structs with Defaults

**Learning**: Complex configuration is best handled with dedicated structs that
implement `Default`.

**Pattern**:

```rust
#[derive(Debug, Clone)]
pub struct SideBySideConfig {
    pub direction: LayoutDirection,
    pub split_ratio: f32,
    pub padding: f32,
}

impl Default for SideBySideConfig {
    fn default() -> Self {
        Self {
            direction: LayoutDirection::Horizontal,
            split_ratio: 0.5,
            padding: 10.0,
        }
    }
}
```

**Benefits**:

- Easy to extend without breaking changes
- Clear documentation of options
- Sensible defaults reduce API complexity

## Graphics Programming Patterns

### Viewport State Management

**Learning**: Rendering operations that modify global state (like viewport) must
restore original state.

**Pattern**:

```rust
fn render_with_viewport(&mut self, context: &mut RenderContext) -> GupResult<()> {
    let original_viewport = context.viewport();

    // Modify viewport for this operation
    context.set_viewport(new_viewport)?;

    // Do rendering work
    self.component.render(context)?;

    // Always restore original state
    context.set_viewport(original_viewport)?;

    Ok(())
}
```

**Guidelines**:

- Always capture original state before modifications
- Use RAII patterns where possible (consider viewport guards)
- Document state modifications clearly

### Coordinate System Conventions

**Learning**: Be explicit about coordinate system conventions and document them
clearly.

**Convention**: Grid layouts use `(row, col)` indexing where:

- Rows: 0 to `num_rows - 1` (top to bottom)
- Columns: 0 to `num_cols - 1` (left to right)

**Pattern**:

```rust
pub struct GridPosition {
    pub row: u32,    // 0-based row index
    pub col: u32,    // 0-based column index
}

// Always validate bounds
fn is_valid_position(&self, pos: GridPosition) -> bool {
    pos.row < self.rows && pos.col < self.cols
}
```

## Testing Patterns

### Comprehensive Composition Testing

**Learning**: When implementing composition systems, test all combinations and
edge cases.

**Test Categories**:

1. **Basic functionality** - Each mode works independently
2. **Configuration validation** - Invalid configs are rejected
3. **State management** - Viewport/state restoration works
4. **Error propagation** - Failures are handled correctly
5. **Integration** - Different modes work together
6. **Edge cases** - Boundary conditions and limits

**Pattern**:

```rust
#[tokio::test]
async fn test_viewport_restoration() {
    let mut context = RenderContext::new().await.unwrap();
    let original_viewport = context.viewport();

    let mut composition = create_side_by_side_composition();
    composition.render(&mut context).unwrap();

    // Verify viewport was restored
    assert_eq!(context.viewport(), original_viewport);
}
```

### Performance Regression Testing

**Learning**: When adding abstraction layers, benchmark to ensure performance
overhead stays minimal.

**Guidelines**:

- Benchmark direct operations vs. composed operations
- Target <1% overhead for composition layers
- Use `cargo bench` with consistent test data
- Monitor for performance regressions in CI

## Error Handling Patterns

### Context-Rich Error Messages

**Learning**: Composition errors should provide context about which component
failed and why.

**Pattern**:

```rust
// ❌ Not helpful
Err(GupError::RenderError("Component invalid".to_string()))

// ✅ Better - includes context
Err(GupError::CompositionError(format!(
    "First component is invalid: {}",
    self.first.description()
)))
```

**Guidelines**:

- Include component descriptions in error messages
- Specify which part of a composition failed
- Provide actionable information where possible

## Documentation Standards

### Code Examples in Documentation

**Learning**: Complex APIs benefit from comprehensive examples showing common
usage patterns.

**Pattern**:

````rust
/// Example: Creating compositions with different modes
///
/// ```rust
/// use gup::*;
///
/// // Basic overlay
/// let overlay = chart1.overlay(chart2);
///
/// // Configured side-by-side
/// let config = SideBySideConfig {
///     direction: LayoutDirection::Horizontal,
///     split_ratio: 0.6,
///     padding: 20.0,
/// };
/// let beside = chart1.beside_with_config(chart2, config);
/// ```
````

**Guidelines**:

- Provide runnable examples in doc comments
- Show both simple and advanced usage
- Include common configuration patterns
- Test examples with `cargo test --doc`

## Performance Considerations

### Lazy Evaluation Patterns

**Learning**: Composition systems benefit from lazy evaluation - defer expensive
operations until render time.

**Pattern**:

```rust
// ✅ Composition is cheap - just stores components
let composition = chart1.mix(chart2).mix(chart3);

// ✅ Expensive work happens only at render time
composition.render(&mut context)?;
```

### Viewport Calculation Caching

**Learning**: Repeated viewport calculations can be expensive - consider caching
when viewport doesn't change.

**Future Optimization**:

```rust
struct CachedViewportCalculation {
    original_viewport: Viewport,
    split_viewports: Option<(Viewport, Viewport)>,
    config_hash: u64,
}
```

## Architecture Decisions

### Composition Over Inheritance

**Learning**: Rust's trait system encourages composition patterns over
inheritance hierarchies.

**Pattern**: The `Mixable` trait enables universal composability:

```rust
// Any two Mixable types can be composed
let result = anything.mix(anything_else);

// Compositions are themselves Mixable
let complex = a.mix(b).mix(c.mix(d));
```

### Type System as Documentation

**Learning**: Well-designed types serve as documentation and prevent errors.

**Example**:

```rust
// ✅ Intent is clear from types
pub fn beside_with_config(
    self,
    other: T,
    config: SideBySideConfig
) -> ComposedVisualization<Self, T>

// ❌ Less clear
pub fn beside_with_config(
    self,
    other: T,
    direction: u8,
    ratio: f32,
    padding: f32
) -> ComposedVisualization<Self, T>
```

---

_This document is a living record of learnings. Update it as new patterns and
conventions are discovered._
