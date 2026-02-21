# GUP-087: Chart Builder Performance Optimization

**Status**: 🚧 In Progress  
**Priority**: Medium  
**Story Points**: 3  
**Epic**: Phase 2 Initiative 1 - Observable Plot-Style Chart Builders

## Overview

Optimize the chart builder system (GUP-018) to eliminate remaining performance
overhead through compile-time accessor resolution and GPU shader specialization.
While GUP-018 achieved zero-cost abstractions for basic operations, there are
opportunities to optimize accessor function compilation and shader generation
for better performance with complex chart compositions.

## Context

GUP-018 delivered a high-level Observable Plot-compatible API with zero-cost
abstractions over the Selection system. However, profiling has revealed
opportunities for optimization:

1. **Accessor Function Resolution**: Currently uses dynamic dispatch via
   `Box<dyn Fn>`, which adds runtime overhead
2. **Shader Generation**: Generated shaders are not specialized per chart type,
   leading to redundant operations
3. **Type Conversion**: Multiple type conversions in the accessor chain create
   unnecessary allocations
4. **Build-Time Compilation**: Repeated compilation of identical accessor
   patterns

These optimizations will make the high-level API truly zero-cost while
maintaining API ergonomics.

## User Story

**As a** developer using Gup's chart builders  
**I want** compile-time accessor resolution and optimized shader generation  
**So that** the high-level Observable Plot-style API has identical performance
to hand-written low-level code

## Acceptance Criteria

### AC1: Compile-Time Accessor Resolution

- [ ] Accessor functions resolve at compile time where possible
- [ ] Generic accessor types eliminate dynamic dispatch overhead
- [ ] Type conversions optimized away by compiler
- [ ] Benchmark: Zero overhead vs direct field access

### AC2: GPU Shader Specialization

- [ ] Generate specialized shaders per chart type and data layout
- [ ] Eliminate redundant WGSL operations in generated shaders
- [ ] Cache compiled shader pipelines based on accessor patterns
- [ ] Benchmark: <1ms shader compilation for common patterns

### AC3: Build-Time Optimization

- [ ] Identical accessor patterns reuse compiled pipelines
- [ ] Build-time macro expansion for common field accessors
- [ ] Compile-time validation of accessor type compatibility
- [ ] Benchmark: <5ms chart build time for 100K points

### AC4: Performance Validation

- [ ] High-level API matches low-level Selection performance exactly
- [ ] Complex chart compositions (3+ accessor functions) show no overhead
- [ ] Memory allocations identical to hand-written Selection code
- [ ] 100K point charts render at 60 FPS with complex accessors

## Technical Tasks

### 1. Implement Compile-Time Accessor Resolution

Create generic accessor types that resolve at compile time:

```rust
// Replace Box<dyn Fn> with generic types
pub struct FieldAccessor<T, F>
where
    F: Fn(&T) -> f32
{
    accessor: F,
    _phantom: PhantomData<T>,
}

// Zero-cost field access
impl<T, F> FieldAccessor<T, F>
where
    F: Fn(&T) -> f32
{
    pub fn new(accessor: F) -> Self {
        Self {
            accessor,
            _phantom: PhantomData,
        }
    }

    #[inline(always)]
    pub fn get(&self, data: &T) -> f32 {
        (self.accessor)(data)
    }
}

// Compile-time macro for common field access patterns
#[macro_export]
macro_rules! field_accessor {
    ($type:ty, $field:ident) => {
        |data: &$type| data.$field
    };
}
```

### 2. GPU Shader Specialization System

Generate specialized shaders per chart configuration:

```rust
pub struct ShaderSpecialization {
    data_layout: DataLayout,
    accessor_types: Vec<AccessorType>,
    mark_type: MarkType,
}

impl ShaderSpecialization {
    /// Generate specialized WGSL shader for this configuration
    pub fn generate_specialized_shader(&self) -> String {
        let mut shader = String::new();

        // Specialized vertex input based on data layout
        shader.push_str(&self.generate_vertex_input());

        // Optimized accessor functions (inline field access)
        shader.push_str(&self.generate_accessor_functions());

        // Mark-specific rendering logic
        shader.push_str(&self.generate_mark_shader());

        shader
    }

    /// Generate cache key for shader pipeline reuse
    pub fn cache_key(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.data_layout.hash(&mut hasher);
        self.accessor_types.hash(&mut hasher);
        self.mark_type.hash(&mut hasher);
        hasher.finish()
    }
}
```

### 3. Pipeline Caching System

Cache compiled pipelines based on accessor patterns:

```rust
pub struct PipelineCache {
    pipelines: HashMap<u64, Arc<wgpu::RenderPipeline>>,
    hit_count: HashMap<u64, usize>,
}

impl PipelineCache {
    pub fn get_or_create(
        &mut self,
        specialization: &ShaderSpecialization,
        device: &wgpu::Device,
    ) -> Arc<wgpu::RenderPipeline> {
        let key = specialization.cache_key();

        if let Some(pipeline) = self.pipelines.get(&key) {
            *self.hit_count.entry(key).or_insert(0) += 1;
            return pipeline.clone();
        }

        // Create new specialized pipeline
        let shader_source = specialization.generate_specialized_shader();
        let pipeline = self.compile_pipeline(device, &shader_source);
        let pipeline = Arc::new(pipeline);

        self.pipelines.insert(key, pipeline.clone());
        self.hit_count.insert(key, 1);

        pipeline
    }

    /// Prune rarely-used pipelines to manage memory
    pub fn prune_cold_entries(&mut self, min_hits: usize) {
        self.pipelines.retain(|key, _| {
            self.hit_count.get(key).copied().unwrap_or(0) >= min_hits
        });
    }
}
```

### 4. Optimize Chart Builder Implementation

Update chart builders to use optimized accessor system:

```rust
impl<T> ScatterPlotBuilder<T> {
    /// Build with compile-time accessor resolution
    pub fn build_optimized<FX, FY>(
        self,
        data: Vec<T>,
        x_accessor: FX,
        y_accessor: FY,
        context: Arc<GupContext>,
    ) -> Result<ScatterPlot<T>, ChartBuilderError>
    where
        FX: Fn(&T) -> f32 + Send + Sync + 'static,
        FY: Fn(&T) -> f32 + Send + Sync + 'static,
    {
        // Create specialization for this chart configuration
        let specialization = ShaderSpecialization {
            data_layout: DataLayout::infer::<T>(),
            accessor_types: vec![
                AccessorType::Float,
                AccessorType::Float,
            ],
            mark_type: MarkType::Circle,
        };

        // Get or create specialized pipeline
        let pipeline = context.pipeline_cache
            .lock()
            .unwrap()
            .get_or_create(&specialization, &context.device);

        // Build selection with specialized pipeline
        let mut selection = Selection::new_with_pipeline(
            data,
            context.clone(),
            pipeline,
        );

        // Set accessor functions (now zero-cost)
        selection.set_accessor("x", x_accessor);
        selection.set_accessor("y", y_accessor);

        Ok(ScatterPlot { selection })
    }
}
```

### 5. Performance Benchmarking

Create comprehensive benchmarks to validate optimizations:

```rust
#[bench]
fn bench_accessor_resolution_overhead(b: &mut Bencher) {
    let data = create_test_data(100_000);

    // Direct field access baseline
    b.iter(|| {
        data.iter().map(|d| d.x + d.y).sum::<f32>()
    });
}

#[bench]
fn bench_optimized_accessor_overhead(b: &mut Bencher) {
    let data = create_test_data(100_000);
    let x_accessor = field_accessor!(TestData, x);
    let y_accessor = field_accessor!(TestData, y);

    // Optimized accessor (should match baseline)
    b.iter(|| {
        data.iter().map(|d| {
            x_accessor.get(d) + y_accessor.get(d)
        }).sum::<f32>()
    });
}

#[bench]
fn bench_chart_build_time(b: &mut Bencher) {
    let data = create_test_data(100_000);
    let context = create_test_context();

    b.iter(|| {
        scatter()
            .build_optimized(
                data.clone(),
                |d| d.x,
                |d| d.y,
                context.clone(),
            )
            .expect("Chart build should succeed")
    });
}

#[bench]
fn bench_specialized_shader_generation(b: &mut Bencher) {
    let specialization = ShaderSpecialization {
        data_layout: DataLayout::simple_float2(),
        accessor_types: vec![AccessorType::Float, AccessorType::Float],
        mark_type: MarkType::Circle,
    };

    b.iter(|| {
        specialization.generate_specialized_shader()
    });
}
```

## Dependencies

### Prerequisite Stories

- **GUP-018**: Observable Plot-Style Chart Builders (completed) - provides the
  foundation to optimize

### Enables Stories

- Enhanced performance enables larger dataset handling in all chart types
- Improved compile times benefit all future chart builder additions
- Shader specialization patterns apply to future visualization types

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_compile_time_accessor_resolution() {
    let data = TestData { x: 1.0, y: 2.0 };
    let accessor = field_accessor!(TestData, x);

    // Should compile to direct field access
    assert_eq!(accessor(&data), 1.0);
}

#[test]
fn test_shader_specialization_cache_keys() {
    let spec1 = ShaderSpecialization {
        data_layout: DataLayout::simple_float2(),
        accessor_types: vec![AccessorType::Float, AccessorType::Float],
        mark_type: MarkType::Circle,
    };

    let spec2 = spec1.clone();

    // Identical configurations should have same cache key
    assert_eq!(spec1.cache_key(), spec2.cache_key());
}

#[test]
fn test_pipeline_cache_reuse() {
    let mut cache = PipelineCache::new();
    let context = create_test_context();
    let spec = create_test_specialization();

    let pipeline1 = cache.get_or_create(&spec, &context.device);
    let pipeline2 = cache.get_or_create(&spec, &context.device);

    // Should return same pipeline instance
    assert!(Arc::ptr_eq(&pipeline1, &pipeline2));
    assert_eq!(cache.hit_count[&spec.cache_key()], 2);
}
```

### Performance Tests

```rust
#[test]
fn test_zero_overhead_accessor() {
    let data = create_test_data(100_000);

    // Direct field access
    let start = Instant::now();
    let sum1: f32 = data.iter().map(|d| d.x).sum();
    let direct_time = start.elapsed();

    // Optimized accessor
    let accessor = field_accessor!(TestData, x);
    let start = Instant::now();
    let sum2: f32 = data.iter().map(|d| accessor(d)).sum();
    let accessor_time = start.elapsed();

    assert_eq!(sum1, sum2);

    // Accessor should have <5% overhead
    let overhead = (accessor_time.as_nanos() as f64 / direct_time.as_nanos() as f64) - 1.0;
    assert!(overhead < 0.05, "Accessor overhead: {:.1}%", overhead * 100.0);
}

#[test]
fn test_chart_build_performance() {
    let data = create_test_data(100_000);
    let context = create_test_context();

    let start = Instant::now();
    let chart = scatter()
        .build_optimized(
            data,
            |d| d.x,
            |d| d.y,
            context,
        )
        .expect("Chart build should succeed");
    let build_time = start.elapsed();

    // Should build in <5ms
    assert!(build_time < Duration::from_millis(5),
            "Build time: {:?}", build_time);
}
```

### Integration Tests

```rust
#[test]
fn test_optimized_chart_render_performance() {
    let data = create_test_data(100_000);
    let context = create_test_context();

    let chart = scatter()
        .build_optimized(
            data,
            |d| d.x,
            |d| d.y,
            context.clone(),
        )
        .expect("Chart build should succeed");

    // Render multiple frames
    for _ in 0..60 {
        let start = Instant::now();
        chart.render_frame(&context).expect("Render should succeed");
        let frame_time = start.elapsed();

        // Should maintain 60 FPS (16.67ms per frame)
        assert!(frame_time < Duration::from_millis(16),
                "Frame time: {:?}", frame_time);
    }
}
```

## Success Metrics

### Performance Metrics

- **Accessor Resolution**: <5% overhead vs direct field access
- **Shader Compilation**: <1ms for cached pipelines, <50ms for new pipelines
- **Chart Build Time**: <5ms for 100K point datasets
- **Memory Allocations**: Identical to hand-written Selection code

### Validation Metrics

- **Pipeline Cache Hit Rate**: >95% for typical usage patterns
- **Render Performance**: 100K points at 60 FPS maintained
- **Compile Time Impact**: <100ms increase in debug builds
- **Binary Size Impact**: <50KB increase in release builds

## Risk Assessment

### Technical Risks

- **Medium**: Complex generic types may increase compile times
- **Low**: Pipeline caching may use excessive memory for diverse charts
- **Low**: Macro-based accessors may have cryptic error messages

### Mitigation Strategies

- Monitor compile time impact with CI benchmarks
- Implement cache pruning for rarely-used pipelines
- Provide clear documentation and error messages for macro usage
- Profile memory usage with large chart collections

## Definition of Done

- [ ] Compile-time accessor resolution implemented and tested
- [ ] GPU shader specialization system working
- [ ] Pipeline caching system implemented with hit rate tracking
- [ ] All benchmarks showing <5% overhead vs hand-written code
- [ ] Performance tests passing with 100K point datasets at 60 FPS
- [ ] Memory profiling confirms zero allocation overhead
- [ ] Documentation updated with optimization patterns
- [ ] `mask all-fix` passes
- [ ] All tests pass with `cargo test -- --test-threads=1`
- [ ] Code review completed and approved
