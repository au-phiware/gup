# Mark System Performance Guide

This guide covers performance characteristics, optimization strategies, and
benchmarking for the mark system.

## Performance Characteristics

### Pipeline Operations

| Operation                | Typical Time | Target | Notes                                     |
| ------------------------ | ------------ | ------ | ----------------------------------------- |
| Pipeline creation        | ~15ms        | <100ms | First call per mark type                  |
| Cached pipeline access   | ~0.015ms     | <1ms   | Subsequent calls (67× better than target) |
| Bind group creation      | ~2ms         | <5ms   | Per mark type                             |
| Buffer upload (5K inst.) | ~25ms        | <50ms  | Depends on instance size                  |
| End-to-end (1K inst.)    | ~45ms        | <100ms | Full frame including setup                |

### Instanced Rendering Throughput

The mark system uses GPU instancing: a single draw call renders all instances of
a mark type. Performance scales with instance count rather than mark complexity.

| Instance Count | Expected FPS | Notes                              |
| -------------- | ------------ | ---------------------------------- |
| 1–1,000        | 60+          | Trivial for modern GPUs            |
| 1,000–10,000   | 60+          | Sweet spot for most visualizations |
| 10,000–100,000 | 60+          | Target range for the project       |
| 100,000+       | 30–60        | May need LOD or culling            |

## Optimization Strategies

### 1. Pre-allocate Buffers

Avoid GPU buffer reallocation during rendering by pre-sizing buffers:

```rust
// Calculate expected data size
let instance_size = std::mem::size_of::<CircleInstance>();
let expected_instances = 10_000;

let renderer = MarkRenderer::with_capacity(
    &device,
    1024,                                // vertex data (small)
    instance_size * expected_instances,  // instance data
    Some(256),                           // index data
);
```

Buffer reallocation requires a new GPU allocation and data copy. The default
1.5× growth factor amortizes this cost, but pre-allocation avoids it entirely.

### 2. Minimize Pipeline Switches

Each call to `render_pass.set_pipeline()` has a GPU cost. Group instances by
mark type to minimize switches:

```rust
// Good: batch by type (2 pipeline switches)
renderer.render_marks::<Circle>(&mut pass, &circle_pipeline, &circle_bg, 5000)?;
renderer.render_marks::<Rectangle>(&mut pass, &rect_pipeline, &rect_bg, 3000)?;

// Avoid: interleaving types (more switches)
```

The `InstancedBatchRenderer` handles this automatically by sorting draw calls by
pipeline.

### 3. Use Hand-Optimized Shaders

Generated shaders are flexible but may miss optimization opportunities. For
performance-critical marks, provide hand-written WGSL:

```rust
impl Mark for PerformanceCriticalMark {
    const VERTEX_SHADER: Option<&'static str> =
        Some(include_str!("shaders/fast_mark.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> =
        Some(include_str!("shaders/fast_mark.frag.wgsl"));
    // ...
}
```

All seven built-in marks use hand-optimized shaders.

### 4. Compact Instance Data

GPU bandwidth is a common bottleneck. Keep per-instance data as small as
possible:

```rust
// Good: 32 bytes per instance
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy)]
struct CompactInstance {
    position: [f32; 2],   // 8 bytes
    color: [f32; 4],      // 16 bytes
    radius: f32,          // 4 bytes
    _padding: f32,        // 4 bytes (alignment)
}

// Costly: 400+ bytes per instance
struct BloatedInstance {
    data: [f32; 100],  // Avoid this
}
```

Remember WGSL alignment rules:

- `vec2<f32>` needs 8-byte alignment
- `vec4<f32>` needs 16-byte alignment
- Add explicit padding fields to match GPU expectations

### 5. Viewport Culling

Skip instances outside the visible area using the `CullingManager`:

```rust
use gup::mark::{CullingManager, Viewport2D, LodLevel};

let viewport = Viewport2D {
    min_x: -1.0, max_x: 1.0,
    min_y: -1.0, max_y: 1.0,
    width_pixels: 1920.0,
    height_pixels: 1080.0,
};

let mut culling = CullingManager::new();

for instance in &instances {
    match culling.determine_lod(&viewport, instance.position, instance.size) {
        LodLevel::Full => { /* render at full detail */ }
        LodLevel::Simplified => { /* use fewer vertices */ }
        LodLevel::Point => { /* render as single pixel */ }
        LodLevel::Culled => { /* skip entirely */ }
    }
}
```

### 6. Compute Shader Filtering

For large datasets, use the GPU to filter instances before rendering:

```rust
use gup::mark::{ComputeInstanceFilter, FilterConfig};

let filter = ComputeInstanceFilter::new(&device, FilterConfig {
    workgroup_size: 256,
    ..Default::default()
});

let result = filter.filter(&device, &queue, &instance_buffer, &filter_params)?;
// result.visible_indices contains only the instances to render
```

This moves culling and filtering work from the CPU to the GPU, which is
significantly faster for 10K+ instances.

### 7. Multi-Pass Efficiency

When using multi-pass rendering, all passes execute within a single render pass
(no additional render pass creation overhead):

```rust
// Single render pass, multiple draw calls
renderer.render_marks_multi_pass::<Circle>(
    &mut render_pass,
    &multi_pass_config,
    &pipelines,       // One pipeline per pass
    &bind_group,
    instance_count,
)?;
```

## Profiling

### Mark Profiler

Use the built-in `MarkProfiler` to measure vertex generation performance:

```rust
use gup::mark::validation::MarkProfiler;

let profile = MarkProfiler::<Circle>::profile();
println!("{}", profile.summary());
```

Performance classifications:

| Class      | Vertex Gen Time | Interpretation             |
| ---------- | --------------- | -------------------------- |
| Excellent  | < 1μs           | Optimal for GPU rendering  |
| Good       | < 100μs         | Suitable for most uses     |
| Acceptable | < 1ms           | Consider optimization      |
| Needs Work | ≥ 1ms           | Optimize vertex generation |

### Performance Metrics

Track rendering performance per frame with `MarkPerformanceMetrics`. The
simplest approach is to use the tracked render methods which automatically
accumulate counters:

```rust
// At the start of each frame, reset counters
renderer.reset_performance_counters();

// Use tracked variants — metrics are updated automatically
renderer.render_marks_tracked::<Circle>(&mut pass, &pipeline, &bind_group, 500)?;
renderer.render_marks_tracked::<Rectangle>(&mut pass, &pipeline2, &bind_group2, 200)?;

// At the end of the frame, read metrics
let metrics = renderer.get_performance_metrics();
println!("Draw calls:       {}", metrics.draw_calls);        // 2
println!("Total instances:  {}", metrics.total_instances);    // 700
println!("Pipeline switches: {}", metrics.pipeline_switches); // 0
```

You can also accumulate metrics manually via `metrics_mut()` when using the
non-tracked `render_marks()` variant.

### GPU Timing

For GPU-side timing, use the project's GPU timestamp integration (see
[GPU Timestamp Integration](../GPU_TIMESTAMP_INTEGRATION.md)):

```rust
// GPU timestamps measure actual shader execution time,
// not just CPU-side submission time.
```

## Scaling Considerations

### Memory Usage

Each mark type has two memory costs:

1. **Fixed cost** — Vertex and index buffers (shared across all instances).
   Typically small (32–128 bytes for a quad).

2. **Per-instance cost** — Instance buffer grows linearly with instance count.
   Size depends on the instance data struct (typically 32–64 bytes per
   instance).

**Example**: 100,000 circle instances at 32 bytes each = ~3.2 MB of GPU memory.

### Draw Call Limits

Modern GPUs handle thousands of draw calls per frame, but each draw call has CPU
overhead for command recording. The mark system's instanced rendering reduces
this to one draw call per mark type regardless of instance count.

### Pipeline Count

Shader compilation is the most expensive operation (~15ms per pipeline). In a
typical visualization:

- 3–5 mark types registered → 3–5 pipeline compilations on startup
- Subsequent access is cached → near-zero cost

Avoid creating unique pipelines per instance or per data point.

## Troubleshooting

### Common Performance Issues

| Symptom                      | Likely Cause          | Solution                          |
| ---------------------------- | --------------------- | --------------------------------- |
| Slow first frame             | Pipeline compilation  | Pre-warm cache at startup         |
| Frame rate drops with data   | Buffer reallocation   | Pre-allocate with `with_capacity` |
| High CPU usage during render | Too many draw calls   | Use instanced rendering (default) |
| GPU memory growing           | Buffer not reused     | Use `MarkBufferPool`              |
| Stutter on data updates      | Full buffer re-upload | Upload only changed regions       |

### Validation

Use `MarkValidator` to catch performance issues early:

```rust
use gup::mark::validation::MarkValidator;

let report = MarkValidator::<MyMark>::validate();
for issue in report.issues() {
    eprintln!("{}: {}", issue.severity, issue.message);
}
```

The validator checks:

- Vertex/index consistency
- Memory layout GPU compatibility
- Attribute type correctness
- Shader constant pairing
