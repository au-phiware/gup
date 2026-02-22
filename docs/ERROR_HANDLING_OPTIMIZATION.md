# Error Handling Performance Optimization Guide

This guide explains how to use Gup's optimized error handling features to minimize performance overhead while maintaining comprehensive error diagnostics.

## Overview

Gup provides three levels of error handling optimization:

1. **Fast-path classification** - Const fn methods for compile-time optimization
2. **Lazy context creation** - Deferred expensive diagnostics collection
3. **Context caching** - LRU cache for repeated similar errors

## Fast-Path Classification

Use `category_fast()` instead of `category()` in hot paths:

```rust
use gup::error::GupError;

// ❌ Slower - runtime match
let category = error.category();

// ✅ Faster - const fn, compiler-optimized
let category = error.category_fast();
```

Check if error needs full diagnostics:

```rust
// Skip expensive context creation for hot-path errors
if error.needs_full_context() {
    let context = ErrorContext::new(error);
    // ... handle with full diagnostics
} else {
    // ... handle lightweight error
}
```

Identify hot-path errors:

```rust
if error.is_hot_path_error() {
    // Use lazy context or skip context entirely
    let lazy = LazyErrorContext::new(error);
} else {
    // Create full context immediately
    let context = ErrorContext::new(error);
}
```

## Lazy Error Context

Use `LazyErrorContext` for errors that may not need full diagnostics:

```rust
use gup::error::{LazyErrorContext, GupError};

// Fast: only stores error, no system info collection
let lazy = LazyErrorContext::new(error);

// Access error without creating context
println!("Error: {}", lazy.error());

// Context created only when needed
if user_wants_details {
    let context = lazy.context(); // Expensive operation happens here
    println!("Details: {:?}", context.system_info);
}
```

### When to Use Lazy Context

- **Performance-critical paths** where errors are created frequently
- **Recoverable errors** that may be handled without diagnostics
- **Hot loops** where error creation happens in tight loops
- **Background tasks** where errors may be aggregated

### When to Use Full Context

- **Critical errors** that always need diagnostics (GPU init, memory exhaustion)
- **User-facing errors** that will be reported immediately
- **Non-hot-path errors** where performance doesn't matter

## Error Context Caching

Use `ErrorContextCache` to share context between similar errors:

```rust
use gup::error::{ErrorContextCache, GupError};
use std::sync::Arc;

// Create a cache (typically at application level)
let cache = Arc::new(ErrorContextCache::new());

// First access creates context (cache miss)
let error1 = GupError::gpu_memory_exhausted(2048, 1024);
let ctx1 = cache.get_or_create_context(&error1);

// Similar error reuses context (cache hit)
let error2 = GupError::gpu_memory_exhausted(4096, 2048);
let ctx2 = cache.get_or_create_context(&error2); // Fast! Uses cached context

// Check cache performance
let stats = cache.stats();
println!("Cache hit rate: {:.1}%", stats.hit_rate * 100.0);
assert!(cache.is_hit_rate_acceptable()); // Should be > 80%
```

### Cache Statistics

Monitor cache performance:

```rust
let stats = cache.stats();
println!("Hits: {}", stats.hits);
println!("Misses: {}", stats.misses);
println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);
println!("Cache size: {} / {}", stats.cache_size, stats.max_cache_size);
println!("Utilization: {:.1}%", stats.utilization());
```

### Cache Configuration

The cache is configured with:

- **Maximum size**: 10MB (~2560 contexts)
- **Eviction policy**: LRU (Least Recently Used)
- **Target hit rate**: >80%

## Performance Best Practices

### 1. Hot Path Optimization

```rust
// In performance-critical code
fn hot_path_function() -> Result<(), GupError> {
    // ✅ Fast error creation
    if some_condition {
        return Err(GupError::performance_target_missed(16.67, 20.0));
    }
    
    // ✅ Fast classification without context
    let result = might_fail()?;
    if result.category_fast() == ErrorCategory::Performance {
        // Handle performance errors quickly
    }
    
    Ok(())
}
```

### 2. Error Aggregation

```rust
use std::collections::HashMap;

// Aggregate hot-path errors with lazy context
let mut errors: HashMap<String, LazyErrorContext> = HashMap::new();

for item in large_dataset {
    if let Err(error) = process(item) {
        if error.is_hot_path_error() {
            // Store with lazy context
            errors.insert(item.id.clone(), LazyErrorContext::new(error));
        } else {
            // Handle critical errors immediately
            handle_critical_error(error);
        }
    }
}

// Create full context only for errors that need reporting
for (id, lazy) in errors {
    if should_report(&id) {
        let context = lazy.into_context();
        report_error(context);
    }
}
```

### 3. Shared Cache Pattern

```rust
use std::sync::Arc;

// Application-level cache
pub struct App {
    error_cache: Arc<ErrorContextCache>,
}

impl App {
    pub fn new() -> Self {
        Self {
            error_cache: Arc::new(ErrorContextCache::new()),
        }
    }
    
    pub fn handle_error(&self, error: GupError) {
        // All components share the same cache
        let context = self.error_cache.get_or_create_context(&error);
        self.report_error(context);
    }
}
```

## Performance Targets

The optimizations aim to achieve:

- **Error creation**: <50 nanoseconds for hot-path errors
- **Classification**: <10 nanoseconds (const fn)
- **Lazy context**: <100 nanoseconds (no context creation)
- **Cached context**: <1 microsecond (cache hit)
- **Memory**: <50MB under sustained load
- **Cache hit rate**: >80% in typical usage

## Benchmarks

Run benchmarks to measure performance:

```bash
cargo bench --bench error_handling_benchmarks
```

This measures:
- Error creation overhead
- Lazy context performance
- Cache hit/miss performance
- Fast-path classification
- Complete error workflows
- Memory allocation patterns

## Migration from GUP-017

If you're using the GUP-017 error handling:

### Before (GUP-017)
```rust
let error = GupError::performance_target_missed(16.67, 20.0);
let context = ErrorContext::new(error); // Always creates full context
```

### After (GUP-084 optimized)
```rust
let error = GupError::performance_target_missed(16.67, 20.0);

// Option 1: Use lazy context (hot path)
let lazy = LazyErrorContext::new(error);
if needs_details {
    let context = lazy.context();
}

// Option 2: Use cache (shared errors)
let context = cache.get_or_create_context(&error);
```

## Monitoring

Check cache effectiveness:

```rust
// Periodically check cache health
if !cache.is_hit_rate_acceptable() {
    log::warn!(
        "Error cache hit rate below target: {:.1}%",
        cache.stats().hit_rate * 100.0
    );
}

// Log cache statistics
log::info!(
    "Error cache stats: {} hits, {} misses, {:.1}% hit rate, {} entries",
    stats.hits,
    stats.misses,
    stats.hit_rate * 100.0,
    stats.cache_size
);
```

## Troubleshooting

### Low Cache Hit Rate

If cache hit rate is <80%:

1. **Check error patterns**: Are errors too diverse for caching?
2. **Increase cache size**: Modify `MAX_CACHE_SIZE_BYTES` if needed
3. **Review error signatures**: May need custom signature logic

### High Memory Usage

If error handling uses >50MB:

1. **Check cache size**: `cache.stats().cache_size`
2. **Clear cache periodically**: `cache.clear()`
3. **Reduce context retention**: Don't store LazyErrorContext long-term

### Performance Not Meeting Targets

1. **Run benchmarks**: Identify which operations are slow
2. **Profile hot paths**: Use `perf` or similar tools
3. **Check context creation**: May need to skip for more error types

## See Also

- [GUP-017 Error Handling Framework](../stories/GUP-017_Error_Handling_Framework.md)
- [GUP-084 Performance Optimization Story](../stories/GUP-084_Error_Handling_Performance_Optimization.md)
- Error handling benchmarks: `benches/error_handling_benchmarks.rs`
