// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pipeline caching system for reusing compiled shader pipelines.
//!
//! This module implements an efficient cache for GPU render pipelines,
//! enabling pipeline reuse across multiple charts with identical configurations.

use super::shader_specialization::ShaderSpecialization;
use std::collections::HashMap;
use std::sync::Arc;

/// Statistics about pipeline cache performance.
#[derive(Debug, Clone, Default)]
pub struct PipelineCacheStats {
    /// Number of cache hits
    pub hits: usize,
    /// Number of cache misses (new pipeline compilations)
    pub misses: usize,
    /// Number of pipelines currently cached
    pub cached_pipelines: usize,
    /// Number of pipelines that have been pruned
    pub pruned_count: usize,
}

impl PipelineCacheStats {
    /// Calculate the cache hit rate as a percentage.
    pub fn hit_rate(&self) -> f64 {
        let total = (self.hits + self.misses) as f64;
        if total == 0.0 {
            0.0
        } else {
            (self.hits as f64 / total) * 100.0
        }
    }

    /// Reset statistics.
    pub fn reset(&mut self) {
        self.hits = 0;
        self.misses = 0;
        self.pruned_count = 0;
    }
}

/// Cache entry for a compiled pipeline.
#[derive(Clone)]
struct PipelineCacheEntry {
    /// The cached pipeline (in real implementation, would be wgpu::RenderPipeline)
    pipeline: Arc<String>, // Placeholder - would be Arc<wgpu::RenderPipeline>
    /// Number of times this pipeline has been used
    hit_count: usize,
}

/// Pipeline cache for reusing compiled GPU pipelines.
///
/// This cache maps shader specialization configurations to compiled pipelines,
/// eliminating redundant shader compilation for common chart patterns.
pub struct PipelineCache {
    /// Map from cache key to pipeline
    pipelines: HashMap<u64, PipelineCacheEntry>,
    /// Overall cache statistics
    stats: PipelineCacheStats,
    /// Maximum number of pipelines to cache
    max_cache_size: usize,
}

impl PipelineCache {
    /// Create a new pipeline cache with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(100)
    }

    /// Create a new pipeline cache with specified capacity.
    pub fn with_capacity(max_cache_size: usize) -> Self {
        Self {
            pipelines: HashMap::with_capacity(max_cache_size),
            stats: PipelineCacheStats::default(),
            max_cache_size,
        }
    }

    /// Get a pipeline from the cache or create a new one.
    ///
    /// Returns the cached pipeline if available (cache hit),
    /// or compiles and caches a new pipeline (cache miss).
    ///
    /// # Arguments
    ///
    /// * `specialization` - The shader specialization configuration
    ///
    /// # Returns
    ///
    /// A shared reference to the compiled pipeline.
    pub fn get_or_create(&mut self, specialization: &ShaderSpecialization) -> Arc<String> {
        let key = specialization.cache_key();

        if let Some(entry) = self.pipelines.get_mut(&key) {
            // Cache hit
            entry.hit_count += 1;
            self.stats.hits += 1;
            entry.pipeline.clone()
        } else {
            // Cache miss - compile new pipeline
            self.stats.misses += 1;

            let shader_source = specialization.generate_specialized_shader();
            let pipeline = Arc::new(shader_source);

            // Prune cache if at capacity
            if self.pipelines.len() >= self.max_cache_size {
                self.prune_least_used();
            }

            // Insert new entry
            let entry = PipelineCacheEntry {
                pipeline: pipeline.clone(),
                hit_count: 1,
            };
            self.pipelines.insert(key, entry);
            self.stats.cached_pipelines = self.pipelines.len();

            pipeline
        }
    }

    /// Prune pipelines with fewer than the specified minimum hits.
    ///
    /// This removes rarely-used pipelines to free up cache space.
    ///
    /// # Arguments
    ///
    /// * `min_hits` - Minimum number of hits required to keep a pipeline
    pub fn prune_cold_entries(&mut self, min_hits: usize) {
        let initial_count = self.pipelines.len();

        self.pipelines.retain(|_, entry| entry.hit_count >= min_hits);

        let pruned = initial_count - self.pipelines.len();
        self.stats.pruned_count += pruned;
        self.stats.cached_pipelines = self.pipelines.len();
    }

    /// Prune the least-used pipeline from the cache.
    fn prune_least_used(&mut self) {
        if let Some((&key_to_remove, _)) = self
            .pipelines
            .iter()
            .min_by_key(|(_, entry)| entry.hit_count)
        {
            self.pipelines.remove(&key_to_remove);
            self.stats.pruned_count += 1;
            self.stats.cached_pipelines = self.pipelines.len();
        }
    }

    /// Get the current cache statistics.
    pub fn stats(&self) -> &PipelineCacheStats {
        &self.stats
    }

    /// Get a mutable reference to cache statistics.
    pub fn stats_mut(&mut self) -> &mut PipelineCacheStats {
        &mut self.stats
    }

    /// Clear all cached pipelines and reset statistics.
    pub fn clear(&mut self) {
        self.pipelines.clear();
        self.stats.cached_pipelines = 0;
    }

    /// Get the number of cached pipelines.
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    /// Check if a pipeline for this specialization is cached.
    pub fn contains(&self, specialization: &ShaderSpecialization) -> bool {
        self.pipelines.contains_key(&specialization.cache_key())
    }

    /// Get the hit count for a specific pipeline.
    pub fn hit_count(&self, specialization: &ShaderSpecialization) -> Option<usize> {
        self.pipelines
            .get(&specialization.cache_key())
            .map(|entry| entry.hit_count)
    }
}

impl Default for PipelineCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::shader_specialization::*;
    use super::*;

    fn create_test_specialization(mark: MarkType) -> ShaderSpecialization {
        ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![AccessorType::DirectField, AccessorType::DirectField],
            mark,
        )
    }

    #[test]
    fn test_pipeline_cache_basic() {
        let mut cache = PipelineCache::new();
        let spec = create_test_specialization(MarkType::Circle);

        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());

        // First access should be a miss
        let pipeline1 = cache.get_or_create(&spec);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 1);

        // Second access should be a hit
        let pipeline2 = cache.get_or_create(&spec);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);

        // Should return same pipeline
        assert!(Arc::ptr_eq(&pipeline1, &pipeline2));
    }

    #[test]
    fn test_cache_hit_rate() {
        let mut cache = PipelineCache::new();
        let spec = create_test_specialization(MarkType::Circle);

        // 1 miss, 0 hits
        cache.get_or_create(&spec);
        assert_eq!(cache.stats().hit_rate(), 0.0);

        // 1 miss, 1 hit
        cache.get_or_create(&spec);
        assert_eq!(cache.stats().hit_rate(), 50.0);

        // 1 miss, 2 hits
        cache.get_or_create(&spec);
        assert!((cache.stats().hit_rate() - 66.666).abs() < 0.01);

        // 1 miss, 3 hits
        cache.get_or_create(&spec);
        assert_eq!(cache.stats().hit_rate(), 75.0);
    }

    #[test]
    fn test_multiple_specializations() {
        let mut cache = PipelineCache::new();
        let spec_circle = create_test_specialization(MarkType::Circle);
        let spec_rect = create_test_specialization(MarkType::Rectangle);
        let spec_line = create_test_specialization(MarkType::Line);

        let pipeline1 = cache.get_or_create(&spec_circle);
        let pipeline2 = cache.get_or_create(&spec_rect);
        let pipeline3 = cache.get_or_create(&spec_line);

        assert_eq!(cache.len(), 3);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 3);

        // All pipelines should be different
        assert!(!Arc::ptr_eq(&pipeline1, &pipeline2));
        assert!(!Arc::ptr_eq(&pipeline1, &pipeline3));
        assert!(!Arc::ptr_eq(&pipeline2, &pipeline3));
    }

    #[test]
    fn test_prune_cold_entries() {
        let mut cache = PipelineCache::new();
        let spec_hot = create_test_specialization(MarkType::Circle);
        let spec_cold = create_test_specialization(MarkType::Rectangle);

        // Create two pipelines
        cache.get_or_create(&spec_hot);
        cache.get_or_create(&spec_cold);

        // Use hot pipeline multiple times
        for _ in 0..5 {
            cache.get_or_create(&spec_hot);
        }

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.hit_count(&spec_hot), Some(6)); // 1 create + 5 hits
        assert_eq!(cache.hit_count(&spec_cold), Some(1)); // 1 create only

        // Prune entries with fewer than 3 hits
        cache.prune_cold_entries(3);

        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&spec_hot));
        assert!(!cache.contains(&spec_cold));
        assert_eq!(cache.stats().pruned_count, 1);
    }

    #[test]
    fn test_max_cache_size() {
        let mut cache = PipelineCache::with_capacity(3);

        let spec1 = ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![AccessorType::DirectField],
            MarkType::Circle,
        );
        let spec2 = ShaderSpecialization::new(
            DataLayout::Float2WithColor,
            vec![AccessorType::DirectField],
            MarkType::Circle,
        );
        let spec3 = ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![AccessorType::Computed],
            MarkType::Circle,
        );
        let spec4 = ShaderSpecialization::new(
            DataLayout::SimpleFloat2,
            vec![AccessorType::DirectField],
            MarkType::Rectangle,
        );

        // Fill cache to capacity
        cache.get_or_create(&spec1);
        cache.get_or_create(&spec2);
        cache.get_or_create(&spec3);
        assert_eq!(cache.len(), 3);

        // Adding 4th should prune least-used
        cache.get_or_create(&spec4);
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.stats().pruned_count, 1);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = PipelineCache::new();
        let spec = create_test_specialization(MarkType::Circle);

        cache.get_or_create(&spec);
        cache.get_or_create(&spec);

        assert_eq!(cache.len(), 1);
        assert!(cache.stats().hits > 0);

        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_stats_reset() {
        let mut cache = PipelineCache::new();
        let spec = create_test_specialization(MarkType::Circle);

        cache.get_or_create(&spec);
        cache.get_or_create(&spec);

        assert!(cache.stats().hits > 0);
        assert!(cache.stats().misses > 0);

        cache.stats_mut().reset();
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 0);
        // Note: reset doesn't clear pruned_count or cached_pipelines
    }

    #[test]
    fn test_contains() {
        let mut cache = PipelineCache::new();
        let spec_cached = create_test_specialization(MarkType::Circle);
        let spec_not_cached = create_test_specialization(MarkType::Rectangle);

        assert!(!cache.contains(&spec_cached));
        assert!(!cache.contains(&spec_not_cached));

        cache.get_or_create(&spec_cached);

        assert!(cache.contains(&spec_cached));
        assert!(!cache.contains(&spec_not_cached));
    }
}
