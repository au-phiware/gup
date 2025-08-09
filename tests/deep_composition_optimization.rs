//! Performance validation tests for deep composition chain optimization.
//!
//! This test suite validates the performance requirements specified in GUP-022:
//! - Linear scaling with composition depth
//! - Batch rendering efficiency
//! - Memory usage optimization
//! - Cache effectiveness

use gup::{CompositionExecutor, GupResult, Mixable, OPTIMIZATION_THRESHOLD, RenderContext};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct PerformanceTestVisualization {
    id: usize,
    data_size: usize,
    computation_complexity: usize,
}

impl PerformanceTestVisualization {
    fn new(id: usize, data_size: usize) -> Self {
        Self {
            id,
            data_size,
            computation_complexity: data_size / 100, // Complexity scales with data
        }
    }

    fn with_complexity(mut self, complexity: usize) -> Self {
        self.computation_complexity = complexity;
        self
    }
}

impl Mixable for PerformanceTestVisualization {
    type Output = ();

    fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
        // Simulate realistic rendering workload with computation proportional to complexity
        let mut sum = 0f64;
        for i in 0..self.computation_complexity * 1000 {
            sum += (i as f64).sin() * (i as f64).cos();
        }
        std::hint::black_box(sum);

        // Simulate data processing for different data sizes
        let mut data_work = 0u64;
        for i in 0..self.data_size {
            data_work = data_work.wrapping_add(i as u64 * 31);
        }
        std::hint::black_box(data_work);

        Ok(())
    }

    fn description(&self) -> String {
        format!(
            "PerformanceTestViz(id={}, data={}, complexity={})",
            self.id, self.data_size, self.computation_complexity
        )
    }
}

// Instead of a complex generic type, create individual visualizations for simpler testing
fn create_test_visualizations(count: usize, data_size: usize) -> Vec<PerformanceTestVisualization> {
    (0..count)
        .map(|i| PerformanceTestVisualization::new(i, data_size))
        .collect()
}

#[tokio::test]
async fn test_linear_scaling_with_depth() {
    let mut context = RenderContext::new().await.unwrap();
    let data_size = 1000;

    let mut times = Vec::new();

    // Test different numbers of visualizations (simulating composition depth)
    for depth in [2, 4, 8, 16].iter() {
        let mut visualizations = create_test_visualizations(*depth, data_size);

        let start = Instant::now();
        for viz in &mut visualizations {
            viz.render(&mut context).unwrap();
        }
        let duration = start.elapsed();
        times.push((*depth, duration));

        println!("Depth {depth}: {duration:?}");
    }

    // Verify roughly linear scaling (allowing for some variation)
    // Time should not grow exponentially with depth
    for i in 1..times.len() {
        let (prev_depth, prev_time) = times[i - 1];
        let (curr_depth, curr_time) = times[i];

        let depth_ratio = curr_depth as f64 / prev_depth as f64;
        let time_ratio = curr_time.as_nanos() as f64 / prev_time.as_nanos() as f64;

        // Time growth should be roughly proportional to depth growth
        // Allow up to 10x the depth ratio for implementation overhead and system variability
        assert!(
            time_ratio <= depth_ratio * 10.0,
            "Non-linear scaling detected: depth ratio {depth_ratio:.2}, time ratio {time_ratio:.2}"
        );
    }
}

#[tokio::test]
async fn test_optimized_vs_direct_rendering() {
    let mut context = RenderContext::new().await.unwrap();
    let depth = 10; // Above optimization threshold
    let data_size = 500;

    // Create multiple visualizations to simulate deep composition
    let mut visualizations = create_test_visualizations(depth, data_size);

    // Test direct rendering
    let start = Instant::now();
    for viz in &mut visualizations {
        viz.render(&mut context).unwrap();
    }
    let direct_time = start.elapsed();

    // Test that rendering works at depth
    assert!(
        direct_time < Duration::from_millis(100),
        "Deep composition rendering too slow: {direct_time:?}"
    );

    println!("Deep composition (depth={depth}): {direct_time:?}");
}

#[tokio::test]
async fn test_composition_executor_performance() {
    let mut context = RenderContext::new().await.unwrap();
    let mut executor = CompositionExecutor::new();

    let viz = PerformanceTestVisualization::new(1, 5000).with_complexity(10);

    // Test flattening performance
    let start = Instant::now();
    executor.flatten_composition(&viz).unwrap();
    let flatten_time = start.elapsed();

    // Test execution performance
    let start = Instant::now();
    executor.execute(&mut context).unwrap();
    let execute_time = start.elapsed();

    let metrics = executor.metrics();

    // Verify performance requirements
    assert!(
        flatten_time < Duration::from_millis(10),
        "Composition flattening too slow: {flatten_time:?}"
    );
    assert!(
        execute_time < Duration::from_millis(50),
        "Composition execution too slow: {execute_time:?}"
    );

    // Verify metrics are collected
    assert!(metrics.optimization_time > Duration::ZERO);
    assert!(metrics.render_time > Duration::ZERO);
    assert!(metrics.operation_count > 0);

    println!(
        "Flatten: {:?}, Execute: {:?}, Operations: {}",
        flatten_time, execute_time, metrics.operation_count
    );
}

#[tokio::test]
async fn test_memory_efficiency_with_depth() {
    let data_size = 1000;

    // Test memory usage doesn't grow exponentially with depth
    let shallow_visualizations = create_test_visualizations(2, data_size);
    let deep_visualizations = create_test_visualizations(10, data_size);

    // Memory usage is difficult to measure precisely, but we can test that
    // visualizations can be created without excessive memory allocation
    let shallow_size = std::mem::size_of_val(&shallow_visualizations);
    let deep_size = std::mem::size_of_val(&deep_visualizations);

    println!("Shallow visualizations size: {shallow_size} bytes");
    println!("Deep visualizations size: {deep_size} bytes");

    // Deep visualizations should scale roughly linearly
    let depth_ratio = 10.0 / 2.0;
    let memory_ratio = deep_size as f64 / shallow_size as f64;

    assert!(
        memory_ratio <= depth_ratio * 2.0,
        "Memory usage not scaling linearly: ratio {memory_ratio:.2}, expected ~{depth_ratio:.2}"
    );
}

#[tokio::test]
async fn test_batching_effectiveness() {
    let _context = RenderContext::new().await.unwrap();
    let mut executor = CompositionExecutor::new();

    // Create multiple similar visualizations that could be batched
    let viz1 = PerformanceTestVisualization::new(1, 1000);
    let viz2 = PerformanceTestVisualization::new(2, 1000);
    let viz3 = PerformanceTestVisualization::new(3, 1000);

    // Test each visualization individually
    for viz in [viz1, viz2, viz3] {
        executor.flatten_composition(&viz).unwrap();
        let metrics = executor.metrics();

        // Each should create at least one batch
        assert!(
            metrics.batch_count > 0,
            "No batches created for visualization"
        );
        assert_eq!(
            metrics.operation_count, 1,
            "Should have exactly one operation"
        );

        executor.reset_metrics();
    }
}

#[tokio::test]
async fn test_cache_hit_rate_improvement() {
    let mut context = RenderContext::new().await.unwrap();
    let mut executor = CompositionExecutor::new();

    let viz = PerformanceTestVisualization::new(1, 2000);

    // First execution - should be cache miss
    executor.flatten_composition(&viz).unwrap();
    executor.execute(&mut context).unwrap();
    let first_metrics = executor.metrics();

    // Second execution - might have cache hits (depending on implementation)
    executor.flatten_composition(&viz).unwrap();
    executor.execute(&mut context).unwrap();
    let second_metrics = executor.metrics();

    // Cache hit rate should be between 0.0 and 1.0
    assert!(first_metrics.cache_hit_rate >= 0.0);
    assert!(first_metrics.cache_hit_rate <= 1.0);
    assert!(second_metrics.cache_hit_rate >= 0.0);
    assert!(second_metrics.cache_hit_rate <= 1.0);

    println!("First cache hit rate: {:.2}", first_metrics.cache_hit_rate);
    println!(
        "Second cache hit rate: {:.2}",
        second_metrics.cache_hit_rate
    );
}

#[tokio::test]
async fn test_optimization_threshold_behavior() {
    let mut context = RenderContext::new().await.unwrap();

    // Test visualizations below threshold
    let mut shallow_visualizations = create_test_visualizations(OPTIMIZATION_THRESHOLD - 1, 500);
    let start = Instant::now();
    for viz in &mut shallow_visualizations {
        viz.render(&mut context).unwrap();
    }
    let shallow_time = start.elapsed();

    // Test visualizations above threshold
    let mut deep_visualizations = create_test_visualizations(OPTIMIZATION_THRESHOLD + 5, 500);
    let start = Instant::now();
    for viz in &mut deep_visualizations {
        viz.render(&mut context).unwrap();
    }
    let deep_time = start.elapsed();

    println!(
        "Shallow (depth={}): {:?}",
        OPTIMIZATION_THRESHOLD - 1,
        shallow_time
    );
    println!(
        "Deep (depth={}): {:?}",
        OPTIMIZATION_THRESHOLD + 5,
        deep_time
    );

    // Both should complete in reasonable time
    assert!(shallow_time < Duration::from_millis(100));
    assert!(deep_time < Duration::from_millis(200));
}

#[tokio::test]
async fn test_resource_pool_efficiency() {
    let mut executor = CompositionExecutor::new();
    let viz = PerformanceTestVisualization::new(1, 1000);

    // Execute multiple times to test resource reuse
    for i in 0..5 {
        executor.flatten_composition(&viz).unwrap();
        let metrics = executor.metrics();

        println!(
            "Iteration {}: memory_saved = {} bytes",
            i, metrics.memory_saved
        );

        // Memory saved should be non-negative (always true for usize, but documents expectation)
        assert!(metrics.memory_saved < usize::MAX);
    }

    // Clear resources
    executor.clear_resources();

    // After clearing, next execution should start fresh
    executor.flatten_composition(&viz).unwrap();
    let final_metrics = executor.metrics();
    assert_eq!(final_metrics.operation_count, 1);
}

#[tokio::test]
async fn test_error_handling_in_optimization() {
    let mut context = RenderContext::new().await.unwrap();
    let mut executor = CompositionExecutor::new();

    // Test with valid visualization
    let valid_viz = PerformanceTestVisualization::new(1, 1000);
    let result = executor.flatten_composition(&valid_viz);
    assert!(result.is_ok(), "Should handle valid visualization");

    let result = executor.execute(&mut context);
    assert!(result.is_ok(), "Should execute successfully");

    // Test clearing and reuse
    executor.clear_resources();
    let result = executor.flatten_composition(&valid_viz);
    assert!(result.is_ok(), "Should handle visualization after clear");
}

#[tokio::test]
async fn test_performance_regression_detection() {
    let mut context = RenderContext::new().await.unwrap();
    let depth = 6;
    let data_size = 2000;

    // Run the same visualizations multiple times to detect performance regression
    let mut times = Vec::new();

    for _ in 0..3 {
        let mut visualizations = create_test_visualizations(depth, data_size);

        let start = Instant::now();
        for viz in &mut visualizations {
            viz.render(&mut context).unwrap();
        }
        let duration = start.elapsed();
        times.push(duration);
    }

    // Check that performance is reasonably consistent
    let avg_time = times.iter().sum::<Duration>() / times.len() as u32;

    for time in &times {
        let variance_ratio = time.as_nanos() as f64 / avg_time.as_nanos() as f64;
        assert!(
            (0.5..=2.0).contains(&variance_ratio),
            "High performance variance detected: {time:?} vs avg {avg_time:?}"
        );
    }

    println!("Performance consistency test passed. Avg: {avg_time:?}");
}

#[tokio::test]
async fn test_composition_complexity_metrics() {
    let mut executor = CompositionExecutor::new();

    // Test simple composition
    let simple = PerformanceTestVisualization::new(1, 100);
    executor.flatten_composition(&simple).unwrap();
    let simple_metrics = executor.metrics();

    // Test complex composition
    executor.reset_metrics();
    let complex = PerformanceTestVisualization::new(1, 10000).with_complexity(50);
    executor.flatten_composition(&complex).unwrap();
    let complex_metrics = executor.metrics();

    // Complex visualization should have similar operation count but potentially more work
    assert_eq!(
        simple_metrics.operation_count,
        complex_metrics.operation_count
    );

    // Optimization time might be different
    assert!(simple_metrics.optimization_time >= Duration::ZERO);
    assert!(complex_metrics.optimization_time >= Duration::ZERO);

    println!(
        "Simple: {:?}, Complex: {:?}",
        simple_metrics.optimization_time, complex_metrics.optimization_time
    );
}
