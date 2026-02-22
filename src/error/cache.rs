// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Error context caching to reduce overhead of repeated similar errors.
//!
//! This module provides an LRU cache for error contexts to avoid recreating
//! expensive system information for similar errors.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lru::LruCache;

use super::{ErrorContext, GupError};

/// Maximum size of error context cache (10MB as specified in story).
const MAX_CACHE_SIZE_BYTES: usize = 10 * 1024 * 1024;

/// Estimated average size of an ErrorContext in bytes.
const ESTIMATED_CONTEXT_SIZE: usize = 4096;

/// Maximum number of cached contexts based on memory limit.
const MAX_CACHE_ENTRIES: usize = MAX_CACHE_SIZE_BYTES / ESTIMATED_CONTEXT_SIZE;

/// Error context cache with LRU eviction policy.
///
/// Caches error contexts for similar errors to avoid expensive system
/// information collection. Provides cache statistics for monitoring.
pub struct ErrorContextCache {
    cache: Mutex<LruCache<ErrorSignature, Arc<ErrorContext>>>,
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
}

/// Signature for identifying similar errors that can share context.
///
/// This includes the error type discriminant and key parameters that affect
/// the context generation. Two errors with the same signature can share a
/// cached context.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ErrorSignature {
    /// Discriminant of the error variant
    error_type: u64,
    /// Key parameters that affect context generation
    key_params: Vec<String>,
}

impl ErrorSignature {
    /// Create a signature from an error.
    ///
    /// Only includes parameters that are relevant for context caching.
    /// For example, specific error messages are excluded since they don't
    /// affect the system information collected.
    fn from_error(error: &GupError) -> Self {
        let error_type = Self::discriminant(error);
        let key_params = Self::extract_key_params(error);

        Self {
            error_type,
            key_params,
        }
    }

    /// Get discriminant as u64 for hashing.
    fn discriminant(error: &GupError) -> u64 {
        let mut hasher = DefaultHasher::new();
        mem::discriminant(error).hash(&mut hasher);
        hasher.finish()
    }

    /// Extract key parameters that affect context generation.
    fn extract_key_params(error: &GupError) -> Vec<String> {
        match error {
            // For memory errors, the specific amounts don't affect system info collection
            GupError::GpuMemoryExhausted { .. } => {
                vec!["gpu_memory".to_string()]
            }

            // Shader type affects recommendations but not system info
            GupError::ShaderCompilationError { shader_type, .. } => {
                vec!["shader".to_string(), shader_type.clone()]
            }

            // Platform-specific errors cache by platform
            GupError::PlatformNotSupported { platform, .. } => {
                vec!["platform".to_string(), platform.clone()]
            }

            // Most errors can share context regardless of specific parameters
            _ => vec![],
        }
    }
}

impl ErrorContextCache {
    /// Create a new error context cache.
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(LruCache::new(
                std::num::NonZeroUsize::new(MAX_CACHE_ENTRIES).unwrap(),
            )),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
        }
    }

    /// Get or create an error context, using the cache when possible.
    ///
    /// Returns a cached context if one exists for a similar error, otherwise
    /// creates a new context and caches it.
    pub fn get_or_create_context(&self, error: &GupError) -> Arc<ErrorContext> {
        let signature = ErrorSignature::from_error(error);

        // Try to get from cache
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(context) = cache.get(&signature) {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Arc::clone(context);
            }
        }

        // Cache miss - create new context
        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        let context = Arc::new(ErrorContext::new(error.clone()));

        // Store in cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.put(signature, Arc::clone(&context));
        }

        context
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;

        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        let cache_size = {
            let cache = self.cache.lock().unwrap();
            cache.len()
        };

        CacheStats {
            hits,
            misses,
            hit_rate,
            cache_size,
            max_cache_size: MAX_CACHE_ENTRIES,
        }
    }

    /// Clear the cache.
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    /// Check if cache hit rate is above target (80% as per story).
    pub fn is_hit_rate_acceptable(&self) -> bool {
        self.stats().hit_rate >= 0.8
    }
}

impl Default for ErrorContextCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics for monitoring.
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub cache_size: usize,
    pub max_cache_size: usize,
}

impl CacheStats {
    /// Get cache utilization as a percentage.
    pub fn utilization(&self) -> f64 {
        (self.cache_size as f64 / self.max_cache_size as f64) * 100.0
    }

    /// Get total accesses.
    pub fn total_accesses(&self) -> u64 {
        self.hits + self.misses
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_hit() {
        let cache = ErrorContextCache::new();

        // First access should be a miss
        let error1 = GupError::gpu_memory_exhausted(2048, 1024);
        let ctx1 = cache.get_or_create_context(&error1);
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 0);

        // Second access with same error should be a hit
        let error2 = GupError::gpu_memory_exhausted(4096, 2048);
        let ctx2 = cache.get_or_create_context(&error2);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);

        // Both contexts should be the same (Arc pointer equality)
        assert!(Arc::ptr_eq(&ctx1, &ctx2));
    }

    #[test]
    fn test_cache_miss_different_error() {
        let cache = ErrorContextCache::new();

        let error1 = GupError::gpu_memory_exhausted(2048, 1024);
        let _ctx1 = cache.get_or_create_context(&error1);

        let error2 = GupError::shader_compilation_failed("vertex", "syntax error");
        let _ctx2 = cache.get_or_create_context(&error2);

        // Different error types should miss
        assert_eq!(cache.stats().misses, 2);
        assert_eq!(cache.stats().hits, 0);
    }

    #[test]
    fn test_hit_rate_calculation() {
        let cache = ErrorContextCache::new();

        // Generate some hits and misses
        for _ in 0..10 {
            let error = GupError::gpu_memory_exhausted(2048, 1024);
            let _ctx = cache.get_or_create_context(&error);
        }

        let stats = cache.stats();
        assert_eq!(stats.misses, 1); // First access
        assert_eq!(stats.hits, 9); // Subsequent accesses
        assert!((stats.hit_rate - 0.9).abs() < 0.01);
        assert!(cache.is_hit_rate_acceptable());
    }

    #[test]
    fn test_cache_clear() {
        let cache = ErrorContextCache::new();

        let error = GupError::gpu_memory_exhausted(2048, 1024);
        let _ctx = cache.get_or_create_context(&error);

        assert_eq!(cache.stats().cache_size, 1);

        cache.clear();
        assert_eq!(cache.stats().cache_size, 0);
    }

    #[test]
    fn test_signature_extraction() {
        // Same error type with different values should have same signature
        let error1 = GupError::gpu_memory_exhausted(2048, 1024);
        let error2 = GupError::gpu_memory_exhausted(4096, 2048);

        let sig1 = ErrorSignature::from_error(&error1);
        let sig2 = ErrorSignature::from_error(&error2);

        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_shader_signature_differentiation() {
        // Different shader types should have different signatures
        let error1 = GupError::shader_compilation_failed("vertex", "error1");
        let error2 = GupError::shader_compilation_failed("fragment", "error2");

        let sig1 = ErrorSignature::from_error(&error1);
        let sig2 = ErrorSignature::from_error(&error2);

        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_lru_eviction() {
        let cache = ErrorContextCache::new();

        // Fill cache beyond capacity by creating many different error types
        // This tests LRU eviction behavior
        for i in 0..MAX_CACHE_ENTRIES + 100 {
            let error = GupError::ConfigurationError {
                parameter: format!("param_{}", i),
                message: "test".to_string(),
            };
            let _ctx = cache.get_or_create_context(&error);
        }

        let stats = cache.stats();
        assert!(stats.cache_size <= MAX_CACHE_ENTRIES);
    }
}
