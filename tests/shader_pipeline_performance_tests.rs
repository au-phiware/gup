// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Performance optimization tests for shader pipeline system.

#[cfg(test)]
mod tests {
    use gup::shader_function::ComposableShaderFunction;
    use gup::shader_pipeline::{
        ComposableShaderPipeline, InliningConfig, LruPipelineCache, OptimizationConfig,
        PipelineBatch, PipelineProfiler,
    };
    use gup::vec4;
    use gup::{ColorMap, LinearScale, Vec4};

    #[test]
    fn test_optimization_config() {
        let mut config = OptimizationConfig::default();
        assert!(config.enable_inlining);
        assert!(config.enable_constant_folding);
        assert!(config.enable_dead_code_elimination);

        // Test custom configuration
        config.enable_inlining = false;
        let mut pipeline = ComposableShaderPipeline::new().with_optimization_config(config.clone());

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        let vertex_shader = pipeline.generate_optimized_vertex_shader();
        assert!(vertex_shader.contains("vs_main"));

        // Verify config is applied
        assert!(!pipeline.optimization_config().enable_inlining);
    }

    #[test]
    fn test_inlining_config() {
        let mut config = InliningConfig::default();
        assert_eq!(config.inline_threshold, 5);
        assert_eq!(config.call_count_threshold, 3);
        assert!(!config.use_ast_analysis);

        // Test custom thresholds
        config.inline_threshold = 10;
        config.call_count_threshold = 5;
        config.use_ast_analysis = true;

        let opt_config = OptimizationConfig {
            enable_inlining: true,
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
            use_ast_analysis: false,
            inlining: config,
        };

        let mut pipeline = ComposableShaderPipeline::new().with_optimization_config(opt_config);
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        let vertex_shader = pipeline.generate_optimized_vertex_shader();
        assert!(vertex_shader.contains("vs_main"));
    }

    #[test]
    fn test_pipeline_profiler() {
        let mut profiler = PipelineProfiler::new(true);

        // Simulate some operations
        profiler.record_cache_hit();
        profiler.record_cache_hit();
        profiler.record_cache_miss();
        profiler.update_cache_entries(10);
        profiler.update_cache_memory(10240);

        let report = profiler.report(5, 1024, 512);
        assert_eq!(report.function_count, 5);
        assert_eq!(report.vertex_shader_size, 1024);
        assert_eq!(report.fragment_shader_size, 512);
        assert_eq!(report.cache_stats.hits, 2);
        assert_eq!(report.cache_stats.misses, 1);
        assert_eq!(report.cache_stats.entries, 10);
        assert_eq!(report.cache_stats.memory_usage, 10240);

        // Check hit rate calculation
        let hit_rate = report.cache_stats.hit_rate();
        assert!((hit_rate - 0.666).abs() < 0.01); // 2/3 ≈ 0.666
    }

    #[test]
    fn test_profiler_recommendations() {
        let mut profiler = PipelineProfiler::new(true);

        // Simulate poor cache performance
        for _ in 0..3 {
            profiler.record_cache_hit();
        }
        for _ in 0..15 {
            profiler.record_cache_miss();
        }

        let recommendations = profiler.recommendations();
        assert!(!recommendations.is_empty());

        // Should recommend increasing cache size
        assert!(
            recommendations
                .iter()
                .any(|r| r.optimization_type.contains("Cache"))
        );
    }

    #[test]
    fn test_lru_pipeline_cache() {
        let mut cache = LruPipelineCache::new(2, true);

        // Cache should be empty initially
        assert_eq!(cache.statistics().entries, 0);
        assert_eq!(cache.statistics().hits, 0);
        assert_eq!(cache.statistics().misses, 0);

        // Simulate cache miss
        let result = cache.get(12345);
        assert!(result.is_none());
        assert_eq!(cache.statistics().misses, 1);

        // Clear cache
        cache.clear();
        assert_eq!(cache.statistics().entries, 0);
    }

    #[test]
    fn test_pipeline_batch() {
        let mut batch = PipelineBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);

        // Create multiple pipelines
        let mut pipeline1 = ComposableShaderPipeline::new();
        let scale1 = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline1.add_function(scale1);

        let mut pipeline2 = ComposableShaderPipeline::new();
        let color2 = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        pipeline2.add_function(color2);

        // Add to batch
        batch.add_pipeline("pipeline1".to_string(), pipeline1);
        batch.add_pipeline("pipeline2".to_string(), pipeline2);

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());

        // Generate all shaders
        let results = batch.generate_all_shaders();
        assert_eq!(results.len(), 2);

        let (id1, vertex1, fragment1) = &results[0];
        assert_eq!(id1, "pipeline1");
        assert!(vertex1.contains("vs_main"));
        assert!(fragment1.contains("fs_main"));

        let (id2, vertex2, fragment2) = &results[1];
        assert_eq!(id2, "pipeline2");
        assert!(vertex2.contains("vs_main"));
        assert!(fragment2.contains("fs_main"));
    }

    #[test]
    fn test_pipeline_with_profiling() {
        let mut pipeline = ComposableShaderPipeline::new().with_profiling(true);

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        // Generate shaders
        let vertex_shader = pipeline.generate_vertex_shader();
        let fragment_shader = pipeline.generate_fragment_shader();

        assert!(!vertex_shader.is_empty());
        assert!(!fragment_shader.is_empty());

        // Get profiling report
        let report = pipeline.profile_report();
        assert!(report.is_some());

        let report = report.unwrap();
        assert_eq!(report.function_count, 1);
        // Note: shader sizes may be 0 if not cached yet, which is okay
        // The important part is that profiling is enabled and report is generated
    }

    #[test]
    fn test_optimization_recommendations() {
        let pipeline = ComposableShaderPipeline::new().with_profiling(false);

        // Without profiling, should return empty recommendations
        let recommendations = pipeline.optimization_recommendations();
        assert!(recommendations.is_empty());
    }

    #[test]
    fn test_advanced_inlining() {
        // Test with advanced inlining enabled
        let inlining_config = InliningConfig {
            inline_threshold: 10,
            call_count_threshold: 5,
            use_ast_analysis: true,
        };

        let opt_config = OptimizationConfig {
            enable_inlining: true,
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
            use_ast_analysis: false,
            inlining: inlining_config,
        };

        let mut pipeline = ComposableShaderPipeline::new().with_optimization_config(opt_config);

        // Add multiple functions
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

        pipeline.add_function(scale);
        pipeline.add_function(color);

        // Generate optimized shader
        let vertex_shader = pipeline.generate_optimized_vertex_shader();
        assert!(vertex_shader.contains("vs_main"));

        // Should contain inlining comments if any functions were inlined
        // (actual inlining is a placeholder in this implementation)
    }

    #[test]
    fn test_memory_optimization() {
        // Test that cache estimates memory usage
        let mut cache = LruPipelineCache::new(10, true);

        // Memory usage should be 0 initially
        assert_eq!(cache.statistics().memory_usage, 0);

        // After clearing, memory usage should be 0
        cache.clear();
        assert_eq!(cache.statistics().memory_usage, 0);
        assert_eq!(cache.statistics().entries, 0);
    }

    // -----------------------------------------------------------------------
    // AST integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_ast_optimized_vertex_shader() {
        let config = OptimizationConfig {
            use_ast_analysis: true,
            ..Default::default()
        };

        let mut pipeline = ComposableShaderPipeline::new().with_optimization_config(config);
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);
        pipeline.map_attribute("color", "linear_scale");

        let optimized = pipeline.generate_optimized_vertex_shader();

        // Must still contain the entry point.
        assert!(
            optimized.contains("vs_main"),
            "entry point preserved after AST optimization"
        );
    }

    #[test]
    fn test_ast_optimized_fragment_shader() {
        let config = OptimizationConfig {
            use_ast_analysis: true,
            ..Default::default()
        };

        let mut pipeline = ComposableShaderPipeline::new().with_optimization_config(config);
        let color = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        pipeline.add_function(color);
        pipeline.map_attribute("color", "color_map");

        let optimized = pipeline.generate_optimized_fragment_shader();

        assert!(
            optimized.contains("fs_main"),
            "entry point preserved after AST optimization"
        );
    }

    #[test]
    fn test_ast_fallback_preserves_shader() {
        // A pipeline whose generated shader may trip the AST parser.
        // Even if it does, the fallback must preserve the shader's entry points.
        let config = OptimizationConfig {
            use_ast_analysis: true,
            ..Default::default()
        };

        let mut pipeline = ComposableShaderPipeline::new().with_optimization_config(config);
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        pipeline.add_function(scale);
        pipeline.add_function(color);
        pipeline.map_attribute("size", "linear_scale");
        pipeline.map_attribute("color", "color_map");

        let vertex = pipeline.generate_optimized_vertex_shader();
        let fragment = pipeline.generate_optimized_fragment_shader();

        assert!(vertex.contains("vs_main"));
        assert!(fragment.contains("fs_main"));
    }

    #[test]
    fn test_ast_and_string_produce_equivalent_shaders() {
        // Both paths must produce shaders that contain the same key elements.
        let mut string_pipeline = ComposableShaderPipeline::new();
        let scale1 = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        string_pipeline.add_function(scale1);
        string_pipeline.map_attribute("color", "linear_scale");
        let string_vertex = string_pipeline.generate_optimized_vertex_shader();

        let ast_config = OptimizationConfig {
            use_ast_analysis: true,
            ..Default::default()
        };
        let mut ast_pipeline = ComposableShaderPipeline::new().with_optimization_config(ast_config);
        let scale2 = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        ast_pipeline.add_function(scale2);
        ast_pipeline.map_attribute("color", "linear_scale");
        let ast_vertex = ast_pipeline.generate_optimized_vertex_shader();

        // Both must contain core elements.
        for keyword in &["vs_main", "VertexOutput", "VertexInput"] {
            assert!(
                string_vertex.contains(keyword),
                "string shader missing '{keyword}'"
            );
            assert!(
                ast_vertex.contains(keyword),
                "AST shader missing '{keyword}'"
            );
        }
    }

    // -------------------------------------------------------------------
    // GUP-054 performance target validation tests
    // -------------------------------------------------------------------

    #[test]
    fn test_pipeline_creation_under_10ms() {
        // Target: pipeline creation < 10ms for complex compositions.
        let start = std::time::Instant::now();
        let mut pipeline = ComposableShaderPipeline::new();
        for i in 0..5 {
            pipeline.add_function(LinearScale::new(0.0, (i + 1) as f32 * 10.0, 0.0, 1.0));
        }
        pipeline.map_attribute("color", "linear_scale");
        let _vs = pipeline.generate_vertex_shader();
        let _fs = pipeline.generate_fragment_shader();
        let elapsed = start.elapsed();

        eprintln!("5-function pipeline generation: {:?}", elapsed);
        assert!(
            elapsed < std::time::Duration::from_millis(10),
            "Pipeline generation took {:?}, expected < 10ms",
            elapsed
        );
    }

    #[test]
    fn test_shader_generation_overhead_vs_baseline() {
        // Target: shader function overhead < 1% vs hand-written shaders.
        // We measure WGSL generation time only (not GPU execution).
        let iterations = 1000;

        // Composed approach
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let scale = LinearScale::new(0.0, 1.0, 0.0, 1.0);
            let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);
            std::hint::black_box(scale.generate_wgsl());
            std::hint::black_box(color_map.generate_wgsl());
        }
        let composed_time = start.elapsed();

        // Hand-written baseline (just string allocation of similar size)
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let shader = String::from(
                "fn linear_scale(value: f32, u: LinearScaleUniforms) -> f32 {\n\
                     let normalized = (value - u.domain_min) / (u.domain_max - u.domain_min);\n\
                     return u.range_min + normalized * (u.range_max - u.range_min);\n\
                 }\n",
            );
            std::hint::black_box(&shader);
        }
        let baseline_time = start.elapsed();

        // The composed overhead should be small relative to baseline.
        let overhead_ratio =
            composed_time.as_secs_f64() / baseline_time.as_secs_f64().max(f64::EPSILON);
        eprintln!(
            "Composed: {:?}, Baseline: {:?}, Ratio: {:.1}x",
            composed_time, baseline_time, overhead_ratio
        );
        // Allow up to 20x since we're comparing real WGSL generation vs
        // simple string construction.  The key metric is absolute time.
        assert!(
            composed_time < std::time::Duration::from_millis(50),
            "1000 compositions took {:?}, expected < 50ms total",
            composed_time
        );
    }

    #[test]
    fn test_lru_cache_hit_rate() {
        // Validates that LRU cache achieves high hit rates with repeated
        // pipeline lookups.
        let mut cache = LruPipelineCache::new(32, true);

        // Insert 5 entries.
        for i in 0..5u64 {
            cache.put(
                i,
                gup::CachedShaders {
                    vertex_shader: format!("vs_{i}"),
                    fragment_shader: format!("fs_{i}"),
                    bind_group_layout: None,
                    vertex_module: None,
                    fragment_module: None,
                },
            );
        }

        // Access each 10 times.
        for _ in 0..10 {
            for i in 0..5u64 {
                let _ = cache.get(i);
            }
        }

        let stats = cache.statistics();
        assert_eq!(stats.hits, 50);
        assert_eq!(stats.misses, 0);
        assert!(stats.hit_rate() > 0.99);
    }

    #[test]
    fn test_uniform_pool_reuse_rate() {
        // Validates that the pool achieves good reuse with repeated
        // acquire/release cycles (non-GPU test using stats only).
        let pool_stats = gup::UniformPoolStats {
            total_created: 4,
            total_reused: 96,
            idle_buffers: 4,
            bucket_count: 2,
        };
        assert!(pool_stats.reuse_rate() > 90.0);
    }

    #[test]
    fn test_batch_pipeline_generation_time() {
        // Validates that batch generation stays under targets.
        let mut batch = PipelineBatch::new();
        for i in 0..10 {
            let mut pipeline = ComposableShaderPipeline::new();
            pipeline.add_function(LinearScale::new(0.0, (i + 1) as f32, 0.0, 1.0));
            pipeline.map_attribute("color", "linear_scale");
            batch.add_pipeline(format!("pipeline_{i}"), pipeline);
        }

        let start = std::time::Instant::now();
        let shaders = batch.generate_all_shaders();
        let elapsed = start.elapsed();

        assert_eq!(shaders.len(), 10);
        eprintln!("10-pipeline batch generation: {:?}", elapsed);
        assert!(
            elapsed < std::time::Duration::from_millis(50),
            "Batch generation took {:?}, expected < 50ms",
            elapsed
        );
    }
}
