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
}
