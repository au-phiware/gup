# GUP-026: Data Source Merge Implementation

**Status**: 🚧 In Progress  
**Started**: 2025-01-27

## Story Overview

**Title**: Implement Data Source Combination for Merge Composition Mode
**Epic**: Phase 1 Initiative 1 - Core GPU Primitives and Selection API
**Priority**: Medium **Story Points**: 5

## Context

Currently, the Merge composition mode in GUP-021 uses a placeholder
implementation that simply renders both components sequentially. This story
implements actual data source combination for compatible visualization types,
enabling true data merging semantics.

## User Story

**As a** data visualization developer **I want** the merge composition mode to
actually combine data sources **So that** I can create unified visualizations
from multiple datasets with proper data integration

## Acceptance Criteria

### AC1: Core Data Merging

- [ ] **Type Compatibility**: System detects when two visualizations can be
      merged based on data types
- [ ] **Data Extraction**: Framework for extracting data from Mixable components
- [ ] **Data Combination**: Algorithms for combining compatible datasets (union,
      intersection, etc.)
- [ ] **Unified Rendering**: Create single visualization from merged data

### AC2: Technical Requirements

- [ ] **Data Type Registry**: System for registering and matching compatible
      data types
- [ ] **Merge Strategies**: Multiple merge strategies (append, deduplicate,
      interpolate)
- [ ] **Memory Efficiency**: Avoid unnecessary data duplication during merge
- [ ] **Error Handling**: Clear errors when components cannot be merged

### AC3: API Design

- [ ] **Mergeable Trait**: Trait for components that can expose their data for
      merging
- [ ] **Merge Strategy Config**: Configuration for different merge behaviors
- [ ] **Type Safety**: Compile-time validation where possible
- [ ] **Performance**: Merging adds <5% overhead compared to individual
      rendering

## Technical Design

### Mergeable Trait

```rust
/// Trait for visualizations that can expose their data for merging
pub trait Mergeable<T> {
    /// Get the underlying data for merging
    fn extract_data(&self) -> &[T];

    /// Create a new visualization from merged data
    fn from_merged_data(data: Vec<T>) -> Self;

    /// Check if this visualization can merge with another data type
    fn can_merge_with<U>(&self, _other_type: std::marker::PhantomData<U>) -> bool {
        std::any::TypeId::of::<T>() == std::any::TypeId::of::<U>()
    }
}
```

### Merge Strategies

```rust
#[derive(Debug, Clone)]
pub enum MergeStrategy {
    /// Append all data points
    Append,
    /// Remove duplicate data points
    Deduplicate,
    /// Interpolate between datasets
    Interpolate { steps: u32 },
    /// Custom merge function
    Custom(fn(&[DataPoint], &[DataPoint]) -> Vec<DataPoint>),
}
```

### Enhanced Merge Implementation

```rust
impl<A: Mixable + Mergeable<T>, B: Mixable + Mergeable<T>> ComposedVisualization<A, B> {
    fn render_merge(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Check type compatibility
        if !self.first.can_merge_with(std::marker::PhantomData::<T>) {
            return Err(GupError::CompositionError(
                "Components have incompatible data types for merging".to_string()
            ));
        }

        // Extract data from both components
        let data1 = self.first.extract_data();
        let data2 = self.second.extract_data();

        // Apply merge strategy
        let merged_data = self.merge_strategy.apply(data1, data2)?;

        // Create and render unified visualization
        let unified_viz = A::from_merged_data(merged_data);
        unified_viz.render(context)?;

        Ok(())
    }
}
```

## Dependencies

### Prerequisite Stories

- GUP-021: Advanced Composition Mode Implementation (provides merge framework)
- Data visualization types need to implement `Mergeable` trait

### Enables Stories

- Advanced data analysis workflows with merged datasets
- Cross-dataset correlation and comparison visualizations

## Testing Strategy

### Data Merging Tests

```rust
#[tokio::test]
async fn test_compatible_data_merge() {
    let data1 = vec![(1.0, 2.0), (3.0, 4.0)];
    let data2 = vec![(5.0, 6.0), (7.0, 8.0)];

    let plot1 = ScatterPlot::new(data1);
    let plot2 = ScatterPlot::new(data2);

    let mut merged = plot1.merge(plot2);
    let mut context = RenderContext::new().await.unwrap();

    assert!(merged.render(&mut context).is_ok());
    // Verify merged visualization contains all 4 data points
}

#[tokio::test]
async fn test_incompatible_merge_error() {
    let scatter_data = vec![(1.0, 2.0)];
    let heatmap_data = Grid::new(10, 10);

    let scatter = ScatterPlot::new(scatter_data);
    let heatmap = HeatMap::new(heatmap_data);

    let mut merged = scatter.merge(heatmap);
    let mut context = RenderContext::new().await.unwrap();

    assert!(merged.render(&mut context).is_err());
}
```

### Performance Tests

```rust
#[bench]
fn bench_merge_vs_individual_rendering(b: &mut Bencher) {
    let plot1 = create_large_scatter_plot(10000);
    let plot2 = create_large_scatter_plot(10000);

    b.iter(|| {
        let merged = plot1.clone().merge(plot2.clone());
        black_box(merged.render(&mut context)).unwrap();
    });
}
```

## Implementation Notes

### Phase 1: Basic Merge Support

- Implement for basic data types (Vec<(f32, f32)> scatter plots)
- Simple append merge strategy
- Type-based compatibility checking

### Phase 2: Advanced Strategies

- Deduplication algorithms
- Interpolation between datasets
- Custom merge functions

### Phase 3: Complex Data Types

- Support for multidimensional data
- Time series merging
- Categorical data combination

## Success Metrics

- [ ] **Functionality**: Compatible visualizations can be merged successfully
- [ ] **Performance**: Merge overhead <5% compared to individual components
- [ ] **Type Safety**: Incompatible merges detected at compile time where
      possible
- [ ] **Memory Usage**: No significant memory overhead from merge operations
- [ ] **Developer Experience**: Clear API and helpful error messages

## Definition of Done

- [ ] `Mergeable` trait implemented and documented
- [ ] Basic merge strategies (Append, Deduplicate) implemented
- [ ] Type compatibility system working
- [ ] Comprehensive tests for merge scenarios
- [ ] Performance benchmarks showing acceptable overhead
- [ ] Integration with existing composition system
- [ ] Documentation with examples of mergeable visualizations
- [ ] Error handling provides clear guidance for incompatible merges
