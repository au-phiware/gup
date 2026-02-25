// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared render pipeline cache for [`Selection`](crate::selection::Selection)s.
//!
//! When many Selections of the same [`Mark`](crate::mark::Mark) type coexist
//! (e.g. a dashboard with multiple scatter plots), each would normally create
//! its own render pipeline.  `PipelineCache` deduplicates them by keying on
//! [`TypeId`] so that all Selections of the same mark type share a single
//! `Arc<RenderPipeline>`.
//!
//! # Cache invalidation
//!
//! The cache should be cleared when:
//! - The GPU device is lost (call [`PipelineCache::clear`]).
//! - The surface texture format changes (call
//!   [`PipelineCache::invalidate_for_format`]).
//!
//! # Example
//!
//! ```rust,ignore
//! use gup::pipeline_cache::PipelineCache;
//!
//! let mut cache = PipelineCache::new();
//!
//! // First selection creates the pipeline …
//! sel_a.prepare_render(&device, &queue, mapper_a, Some(&mut cache))?;
//! // … second selection reuses it.
//! sel_b.prepare_render(&device, &queue, mapper_b, Some(&mut cache))?;
//!
//! assert_eq!(cache.stats().hits, 1);
//! ```

use crate::GupResult;
use crate::mark::{Mark, MarkInfo, MarkInfoImpl};
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::{Device, RenderPipeline, TextureFormat};

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

/// Statistics about pipeline cache usage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PipelineCacheStats {
    /// Number of times a cached pipeline was returned.
    pub hits: usize,
    /// Number of times a new pipeline was created.
    pub misses: usize,
    /// Number of times the cache was fully cleared (e.g. device loss).
    pub invalidations: usize,
}

impl PipelineCacheStats {
    /// Cache hit rate as a percentage (0–100).  Returns 0.0 when no lookups
    /// have been performed.
    pub fn hit_rate(&self) -> f64 {
        let total = (self.hits + self.misses) as f64;
        if total == 0.0 {
            0.0
        } else {
            (self.hits as f64 / total) * 100.0
        }
    }
}

// ---------------------------------------------------------------------------
// PipelineCache
// ---------------------------------------------------------------------------

/// A shared cache of `wgpu::RenderPipeline`s keyed by mark [`TypeId`].
///
/// Pass a `&mut PipelineCache` to
/// [`Selection::prepare_render`](crate::selection::Selection::prepare_render)
/// so that Selections of the same mark type share one pipeline.
pub struct PipelineCache {
    /// Cached pipelines keyed by mark `TypeId`.
    pipelines: HashMap<TypeId, Arc<RenderPipeline>>,
    /// The surface format the cached pipelines were compiled for.
    surface_format: Option<TextureFormat>,
    /// Usage statistics.
    stats: PipelineCacheStats,
}

impl PipelineCache {
    /// Create a new, empty pipeline cache.
    pub fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
            surface_format: None,
            stats: PipelineCacheStats::default(),
        }
    }

    /// Return an existing pipeline for `M`, or create, cache and return one.
    ///
    /// The first call for a given mark type calls
    /// [`MarkInfoImpl::create_render_pipeline`] (cache miss).  Subsequent calls
    /// for the same type return an `Arc` clone (cache hit).
    pub fn get_or_create<M: Mark>(&mut self, device: &Device) -> GupResult<Arc<RenderPipeline>> {
        let type_id = TypeId::of::<M>();

        if let Some(pipeline) = self.pipelines.get(&type_id) {
            self.stats.hits += 1;
            return Ok(Arc::clone(pipeline));
        }

        // Cache miss — create.
        self.stats.misses += 1;
        let mark_info = MarkInfoImpl::<M>::new();
        let pipeline = Arc::new(mark_info.create_render_pipeline(device)?);
        self.pipelines.insert(type_id, Arc::clone(&pipeline));
        Ok(pipeline)
    }

    /// Remove all cached pipelines.
    ///
    /// Call this on GPU device loss so that pipelines are rebuilt on the
    /// replacement device.
    pub fn clear(&mut self) {
        self.pipelines.clear();
        self.surface_format = None;
        self.stats.invalidations += 1;
    }

    /// Clear the cache if the surface format has changed since the last call.
    ///
    /// Pipelines are compiled for a specific texture format; a format change
    /// (e.g. switching from sRGB to linear) requires new pipelines.
    ///
    /// Returns `true` if the cache was actually invalidated.
    pub fn invalidate_for_format(&mut self, format: TextureFormat) -> bool {
        match self.surface_format {
            Some(prev) if prev == format => false,
            _ => {
                self.pipelines.clear();
                self.surface_format = Some(format);
                self.stats.invalidations += 1;
                true
            }
        }
    }

    /// Current usage statistics.
    pub fn stats(&self) -> &PipelineCacheStats {
        &self.stats
    }

    /// Number of pipelines currently cached.
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Returns `true` when no pipelines are cached.
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    /// Check whether a pipeline for mark type `M` is cached.
    pub fn contains<M: Mark>(&self) -> bool {
        self.pipelines.contains_key(&TypeId::of::<M>())
    }
}

impl Default for PipelineCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for PipelineCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipelineCache")
            .field("cached_pipelines", &self.pipelines.len())
            .field("surface_format", &self.surface_format)
            .field("stats", &self.stats)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Unit tests (non-GPU)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cache_is_empty() {
        let cache = PipelineCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 0);
        assert_eq!(cache.stats().invalidations, 0);
    }

    #[test]
    fn test_clear_increments_invalidations() {
        let mut cache = PipelineCache::new();
        cache.clear();
        assert_eq!(cache.stats().invalidations, 1);
        cache.clear();
        assert_eq!(cache.stats().invalidations, 2);
    }

    #[test]
    fn test_invalidate_for_format_first_call() {
        let mut cache = PipelineCache::new();
        // First call with a format always "invalidates" (sets the format).
        let invalidated = cache.invalidate_for_format(TextureFormat::Bgra8UnormSrgb);
        assert!(invalidated);
        assert_eq!(cache.stats().invalidations, 1);
    }

    #[test]
    fn test_invalidate_for_format_same_format() {
        let mut cache = PipelineCache::new();
        cache.invalidate_for_format(TextureFormat::Bgra8UnormSrgb);
        // Same format → no invalidation.
        let invalidated = cache.invalidate_for_format(TextureFormat::Bgra8UnormSrgb);
        assert!(!invalidated);
        assert_eq!(cache.stats().invalidations, 1);
    }

    #[test]
    fn test_invalidate_for_format_change() {
        let mut cache = PipelineCache::new();
        cache.invalidate_for_format(TextureFormat::Bgra8UnormSrgb);
        // Different format → invalidation.
        let invalidated = cache.invalidate_for_format(TextureFormat::Rgba8Unorm);
        assert!(invalidated);
        assert_eq!(cache.stats().invalidations, 2);
    }

    #[test]
    fn test_hit_rate_no_lookups() {
        let stats = PipelineCacheStats::default();
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_hit_rate_all_misses() {
        let stats = PipelineCacheStats {
            hits: 0,
            misses: 5,
            invalidations: 0,
        };
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_hit_rate_mixed() {
        let stats = PipelineCacheStats {
            hits: 3,
            misses: 1,
            invalidations: 0,
        };
        assert_eq!(stats.hit_rate(), 75.0);
    }

    #[test]
    fn test_debug_format() {
        let cache = PipelineCache::new();
        let debug = format!("{:?}", cache);
        assert!(debug.contains("PipelineCache"));
        assert!(debug.contains("cached_pipelines"));
    }

    #[test]
    fn test_default_trait() {
        let cache = PipelineCache::default();
        assert!(cache.is_empty());
    }
}

// ---------------------------------------------------------------------------
// GPU integration tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod gpu_tests {
    use super::*;
    use crate::mark::circle::{Circle, CircleAttributes, CircleInstance};
    use crate::mark::rectangle::{Rectangle, RectangleAttributes, RectangleInstance};
    use crate::selection::Selection;
    use crate::shader_function::{Vec2, Vec4};

    fn circle_attrs(x: f32, y: f32) -> CircleAttributes {
        CircleAttributes {
            center: Vec2 { x, y },
            radius: 0.1,
            fill_color: Vec4 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
        }
    }

    fn rect_attrs(x: f32, y: f32) -> RectangleAttributes {
        RectangleAttributes {
            center: Vec2 { x, y },
            size: Vec2 { x: 0.2, y: 0.1 },
            fill_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
                w: 1.0,
            },
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            corner_radius: 0.0,
        }
    }

    /// 10 Selections of the same Circle mark type share one pipeline.
    #[test]
    fn gpu_ten_selections_share_one_pipeline() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut cache = PipelineCache::new();

            let mut selections: Vec<Selection<CircleAttributes, Circle>> = Vec::new();
            for i in 0..10 {
                let data = vec![circle_attrs(i as f32 * 0.1, 0.0)];
                let mut sel = Selection::from_data(data);
                sel.prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    Some(&mut cache),
                )
                .expect("prepare_render");
                selections.push(sel);
            }

            // 1 miss, 9 hits
            assert_eq!(cache.stats().misses, 1);
            assert_eq!(cache.stats().hits, 9);
            assert_eq!(cache.len(), 1);
            assert!(cache.contains::<Circle>());

            // All selections should render without error.
            let mut ctx = std::sync::Arc::try_unwrap(context).expect("single owner");
            let mut frame = ctx.begin_frame().expect("begin_frame");
            {
                let mut rp = frame.render_pass(Some(wgpu::Color::BLACK));
                for sel in &selections {
                    sel.render(&mut rp).expect("render");
                }
            }
            frame.finish().expect("finish frame");
        });
    }

    /// Different mark types get separate cache entries.
    #[test]
    fn gpu_cache_distinguishes_mark_types() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut cache = PipelineCache::new();

            let mut circle_sel: Selection<CircleAttributes, Circle> =
                Selection::from_data(vec![circle_attrs(0.0, 0.0)]);
            circle_sel
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    Some(&mut cache),
                )
                .expect("circle prepare");

            let mut rect_sel: Selection<RectangleAttributes, Rectangle> =
                Selection::from_data(vec![rect_attrs(0.0, 0.0)]);
            rect_sel
                .prepare_render(
                    &context.device,
                    &context.queue,
                    |a| RectangleInstance::from(a),
                    Some(&mut cache),
                )
                .expect("rect prepare");

            assert_eq!(cache.len(), 2);
            assert_eq!(cache.stats().misses, 2);
            assert_eq!(cache.stats().hits, 0);
            assert!(cache.contains::<Circle>());
            assert!(cache.contains::<Rectangle>());
        });
    }

    /// After clear() the next prepare_render creates a fresh pipeline.
    #[test]
    fn gpu_cache_clear_forces_rebuild() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut cache = PipelineCache::new();

            // Build once.
            let mut sel: Selection<CircleAttributes, Circle> =
                Selection::from_data(vec![circle_attrs(0.0, 0.0)]);
            sel.prepare_render(
                &context.device,
                &context.queue,
                |a| CircleInstance::from(a),
                Some(&mut cache),
            )
            .expect("first prepare");

            assert_eq!(cache.stats().misses, 1);

            // Simulate device loss.
            cache.clear();
            assert!(cache.is_empty());
            assert_eq!(cache.stats().invalidations, 1);

            // Rebuild after loss (a new Selection would need a pipeline).
            let mut sel2: Selection<CircleAttributes, Circle> =
                Selection::from_data(vec![circle_attrs(0.5, 0.5)]);
            sel2.prepare_render(
                &context.device,
                &context.queue,
                |a| CircleInstance::from(a),
                Some(&mut cache),
            )
            .expect("rebuild after clear");

            assert_eq!(cache.stats().misses, 2);
            assert_eq!(cache.len(), 1);
        });
    }

    /// get_or_create::<M>() on the cache works independently.
    #[test]
    fn gpu_get_or_create_basic() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut cache = PipelineCache::new();

            let p1 = cache
                .get_or_create::<Circle>(&context.device)
                .expect("create circle pipeline");
            let p2 = cache
                .get_or_create::<Circle>(&context.device)
                .expect("get cached circle pipeline");

            assert!(std::sync::Arc::ptr_eq(&p1, &p2));
            assert_eq!(cache.stats().misses, 1);
            assert_eq!(cache.stats().hits, 1);
        });
    }

    /// Benchmark-style test: 100 Selections with a cache vs without.
    /// Verifies the cache path creates only 1 pipeline for 100 Selections.
    #[test]
    fn gpu_100_selections_pipeline_reuse() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut cache = PipelineCache::new();

            for i in 0..100 {
                let data = vec![circle_attrs(i as f32 * 0.01, 0.0)];
                let mut sel: Selection<CircleAttributes, Circle> = Selection::from_data(data);
                sel.prepare_render(
                    &context.device,
                    &context.queue,
                    |a| CircleInstance::from(a),
                    Some(&mut cache),
                )
                .expect("prepare");
            }

            // Only 1 pipeline was created, 99 cache hits.
            assert_eq!(cache.stats().misses, 1);
            assert_eq!(cache.stats().hits, 99);
            assert_eq!(cache.len(), 1);
        });
    }
}
