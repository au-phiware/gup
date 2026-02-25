// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU integration tests for Dynamic Attribute GPU Upload Pipeline (GUP-186).
//!
//! These tests validate:
//! - Automatic GPU buffer creation when attributes are first set
//! - Dirty-only upload behaviour
//! - Per-instance data in storage buffers
//! - Static data in uniform buffers
//! - Integration with MarkRenderer
//! - Performance of attribute updates + GPU upload

use gup::context::GupContext;
use gup::error::GupResult;
use gup::mark::advanced_rendering::DynamicAttributeBufferManager;
use gup::mark::{Circle, DynamicAttributeMap, DynamicAttributeValue, Mark, MarkRenderer};
use std::sync::Arc;

/// Helper to create a headless GPU context.
async fn create_test_context() -> GupResult<Arc<GupContext>> {
    GupContext::headless().await
}

// ---------------------------------------------------------------------------
// Buffer creation tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_buffer_creation_on_first_static_attribute() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    // No buffers initially
    assert!(!manager.has_buffers());

    // Set a static attribute
    attrs.set(
        "color",
        DynamicAttributeValue::from_color(1.0, 0.0, 0.0, 1.0),
    );

    // Upload dirty attributes - should create a uniform buffer
    let uploaded = manager.upload_dirty(device, queue, &mut attrs)?;
    assert!(uploaded);
    assert!(manager.has_buffers());
    assert!(manager.uniform_buffer().is_some());
    assert_eq!(manager.buffer_count(), 1);

    Ok(())
}

#[tokio::test]
async fn test_buffer_creation_on_first_per_instance_attribute() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    let instance_data = vec![
        [1.0, 0.0, 0.0, 1.0],
        [0.0, 1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0, 1.0],
    ];
    attrs.set(
        "colors",
        DynamicAttributeValue::from_instances(instance_data),
    );

    let uploaded = manager.upload_dirty(device, queue, &mut attrs)?;
    assert!(uploaded);
    assert!(manager.storage_buffer("colors").is_some());
    assert_eq!(manager.buffer_count(), 1); // 1 storage, no uniform

    Ok(())
}

#[tokio::test]
async fn test_mixed_static_and_per_instance_buffers() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    attrs.set("opacity", DynamicAttributeValue::from_scalar(0.8));
    attrs.set(
        "sizes",
        DynamicAttributeValue::from_instances(vec![[5.0, 0.0, 0.0, 0.0]; 10]),
    );
    attrs.set(
        "colors",
        DynamicAttributeValue::from_instances(vec![[1.0, 0.0, 0.0, 1.0]; 10]),
    );

    manager.upload_dirty(device, queue, &mut attrs)?;

    // 1 uniform + 2 storage
    assert_eq!(manager.buffer_count(), 3);
    assert!(manager.uniform_buffer().is_some());
    assert!(manager.storage_buffer("sizes").is_some());
    assert!(manager.storage_buffer("colors").is_some());

    Ok(())
}

// ---------------------------------------------------------------------------
// Dirty-only upload tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dirty_only_upload_skips_clean() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    attrs.set(
        "color",
        DynamicAttributeValue::from_color(1.0, 0.0, 0.0, 1.0),
    );
    manager.upload_dirty(device, queue, &mut attrs)?;

    // Attrs are now clean — uploading again should be a no-op
    let uploaded = manager.upload_dirty(device, queue, &mut attrs)?;
    assert!(!uploaded);

    Ok(())
}

#[tokio::test]
async fn test_dirty_only_upload_partial_static() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    // Initial upload of two static attrs
    attrs.set("alpha", DynamicAttributeValue::from_scalar(1.0));
    attrs.set(
        "color",
        DynamicAttributeValue::from_color(1.0, 0.0, 0.0, 1.0),
    );
    manager.upload_dirty(device, queue, &mut attrs)?;
    manager.reset_stats();

    // Only change one attribute
    attrs.set("alpha", DynamicAttributeValue::from_scalar(0.5));
    let uploaded = manager.upload_dirty(device, queue, &mut attrs)?;
    assert!(uploaded);

    let stats = manager.stats();
    assert_eq!(stats.partial_uploads, 1);
    assert!(
        stats.bytes_saved > 0,
        "Should save bytes with partial upload"
    );

    Ok(())
}

#[tokio::test]
async fn test_dirty_only_upload_per_instance() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    attrs.set(
        "sizes",
        DynamicAttributeValue::from_instances(vec![[5.0; 4]; 10]),
    );
    attrs.set(
        "colors",
        DynamicAttributeValue::from_instances(vec![[1.0, 0.0, 0.0, 1.0]; 10]),
    );
    manager.upload_dirty(device, queue, &mut attrs)?;
    manager.reset_stats();

    // Only update "sizes"
    attrs.set(
        "sizes",
        DynamicAttributeValue::from_instances(vec![[8.0; 4]; 10]),
    );
    let uploaded = manager.upload_dirty(device, queue, &mut attrs)?;
    assert!(uploaded);

    let stats = manager.stats();
    assert_eq!(
        stats.storage_uploads, 1,
        "Only one storage buffer should be updated"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Buffer resize tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_buffer_grows_when_data_exceeds_capacity() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    // Start with small data
    attrs.set(
        "sizes",
        DynamicAttributeValue::from_instances(vec![[1.0; 4]; 5]),
    );
    manager.upload_dirty(device, queue, &mut attrs)?;
    manager.reset_stats();

    // Grow data significantly
    attrs.set(
        "sizes",
        DynamicAttributeValue::from_instances(vec![[1.0; 4]; 1000]),
    );
    manager.upload_dirty(device, queue, &mut attrs)?;

    let stats = manager.stats();
    assert!(stats.buffer_resizes > 0, "Buffer should have been resized");

    Ok(())
}

#[tokio::test]
async fn test_uniform_buffer_grows_with_more_attributes() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    // Start with 2 attributes
    attrs.set("a", DynamicAttributeValue::from_scalar(1.0));
    attrs.set("b", DynamicAttributeValue::from_scalar(2.0));
    manager.upload_dirty(device, queue, &mut attrs)?;

    // Add many more attributes (should trigger a full upload after count changes)
    for i in 0..50 {
        attrs.set(
            &format!("attr_{i:03}"),
            DynamicAttributeValue::from_scalar(i as f32),
        );
    }
    manager.upload_dirty(device, queue, &mut attrs)?;
    assert!(manager.uniform_buffer().is_some());

    Ok(())
}

// ---------------------------------------------------------------------------
// Attribute removal tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_storage_buffer_removed_when_attribute_removed() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    attrs.set(
        "colors",
        DynamicAttributeValue::from_instances(vec![[1.0; 4]; 5]),
    );
    attrs.set(
        "sizes",
        DynamicAttributeValue::from_instances(vec![[2.0; 4]; 5]),
    );
    manager.upload_dirty(device, queue, &mut attrs)?;
    assert_eq!(manager.buffer_count(), 2);

    // Remove one attribute
    attrs.remove("colors");
    manager.upload_dirty(device, queue, &mut attrs)?;

    assert!(manager.storage_buffer("colors").is_none());
    assert!(manager.storage_buffer("sizes").is_some());
    assert_eq!(manager.buffer_count(), 1);

    Ok(())
}

// ---------------------------------------------------------------------------
// Bind group creation tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_bind_group_creation_uniform_only() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    attrs.set(
        "color",
        DynamicAttributeValue::from_color(1.0, 0.0, 0.0, 1.0),
    );
    attrs.set("alpha", DynamicAttributeValue::from_scalar(0.5));
    manager.upload_dirty(device, queue, &mut attrs)?;

    let layout = manager.create_bind_group_layout(device);
    let bind_group = manager.create_bind_group(device, &layout);
    assert!(bind_group.is_some());

    Ok(())
}

#[tokio::test]
async fn test_bind_group_creation_mixed() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    attrs.set("opacity", DynamicAttributeValue::from_scalar(1.0));
    attrs.set(
        "sizes",
        DynamicAttributeValue::from_instances(vec![[5.0; 4]; 10]),
    );
    manager.upload_dirty(device, queue, &mut attrs)?;

    let layout = manager.create_bind_group_layout(device);
    let bind_group = manager.create_bind_group(device, &layout);
    assert!(bind_group.is_some());

    Ok(())
}

#[tokio::test]
async fn test_bind_group_returns_none_when_empty() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let manager = DynamicAttributeBufferManager::new();
    let layout = manager.create_bind_group_layout(device);
    let bind_group = manager.create_bind_group(device, &layout);
    assert!(bind_group.is_none());

    Ok(())
}

// ---------------------------------------------------------------------------
// Generation tracking tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_generation_tracking() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    attrs.set("x", DynamicAttributeValue::from_scalar(1.0));
    manager.upload_dirty(device, queue, &mut attrs)?;
    let gen1 = manager.last_upload_generation();
    assert!(gen1 > 0);

    attrs.set("y", DynamicAttributeValue::from_scalar(2.0));
    manager.upload_dirty(device, queue, &mut attrs)?;
    let gen2 = manager.last_upload_generation();
    assert!(gen2 > gen1);

    Ok(())
}

// ---------------------------------------------------------------------------
// upload_all tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_upload_all_ignores_dirty_state() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    attrs.set(
        "color",
        DynamicAttributeValue::from_color(1.0, 0.0, 0.0, 1.0),
    );
    attrs.set(
        "sizes",
        DynamicAttributeValue::from_instances(vec![[5.0; 4]; 10]),
    );
    // First upload clears dirty
    manager.upload_dirty(device, queue, &mut attrs)?;
    assert!(!attrs.is_dirty());

    // upload_all should still work even though nothing is dirty
    manager.upload_all(device, queue, &mut attrs)?;
    assert!(manager.has_buffers());

    Ok(())
}

// ---------------------------------------------------------------------------
// Performance test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_attribute_update_upload_performance() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    // Set up 100 static attributes
    for i in 0..100 {
        attrs.set(
            &format!("attr_{i:03}"),
            DynamicAttributeValue::from_scalar(i as f32),
        );
    }

    // Initial upload
    manager.upload_dirty(device, queue, &mut attrs)?;

    // Time 100 update+upload cycles (updating 10 attributes each cycle)
    let start = std::time::Instant::now();
    for cycle in 0..100 {
        for j in 0..10 {
            let idx = (cycle * 10 + j) % 100;
            attrs.set(
                &format!("attr_{idx:03}"),
                DynamicAttributeValue::from_scalar(cycle as f32 + j as f32),
            );
        }
        manager.upload_dirty(device, queue, &mut attrs)?;
    }
    let elapsed = start.elapsed();

    // Success criterion: 100 update+upload cycles should complete in < 100ms
    // (i.e., < 1ms per cycle on average)
    assert!(
        elapsed.as_millis() < 100,
        "100 update+upload cycles took {}ms (target: < 100ms)",
        elapsed.as_millis()
    );

    Ok(())
}

#[tokio::test]
async fn test_bandwidth_savings_with_dirty_only() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    // Set up 20 static attributes
    for i in 0..20 {
        attrs.set(
            &format!("attr_{i:02}"),
            DynamicAttributeValue::from_scalar(i as f32),
        );
    }
    manager.upload_dirty(device, queue, &mut attrs)?;
    manager.reset_stats();

    // Update only 2 out of 20 attributes, 10 times
    for cycle in 0..10 {
        attrs.set(
            "attr_05",
            DynamicAttributeValue::from_scalar(100.0 + cycle as f32),
        );
        attrs.set(
            "attr_15",
            DynamicAttributeValue::from_scalar(200.0 + cycle as f32),
        );
        manager.upload_dirty(device, queue, &mut attrs)?;
    }

    let stats = manager.stats();
    // With dirty-only: 2/20 = 10% of data uploaded each time
    // bytes_saved should be > 50% of total possible bytes
    let total_possible = (20 * std::mem::size_of::<[f32; 4]>() * 10) as u64;
    let savings_pct = (stats.bytes_saved as f64 / total_possible as f64) * 100.0;
    assert!(
        savings_pct > 50.0,
        "Dirty-only should save >50% bandwidth, got {savings_pct:.1}%"
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// MarkRenderer integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_mark_renderer_upload_and_render_setup() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut renderer = MarkRenderer::new(device);
    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    // Upload mark geometry
    let vertices = Circle::generate_vertices();
    renderer.upload_vertices(device, queue, &vertices)?;

    // Set dynamic attributes
    attrs.set("opacity", DynamicAttributeValue::from_scalar(0.8));
    attrs.set(
        "colors",
        DynamicAttributeValue::from_instances(vec![[1.0, 0.0, 0.0, 1.0]; 5]),
    );

    // Upload dynamic attributes to GPU
    let uploaded = manager.upload_dirty(device, queue, &mut attrs)?;
    assert!(uploaded);

    // Verify bind group can be created
    let layout = manager.create_bind_group_layout(device);
    let bind_group = manager.create_bind_group(device, &layout);
    assert!(bind_group.is_some());

    Ok(())
}

#[tokio::test]
async fn test_storage_buffer_names_sorted() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut manager = DynamicAttributeBufferManager::new();
    let mut attrs = DynamicAttributeMap::new();

    // Insert in reverse order
    attrs.set(
        "zebra",
        DynamicAttributeValue::from_instances(vec![[1.0; 4]]),
    );
    attrs.set(
        "alpha",
        DynamicAttributeValue::from_instances(vec![[2.0; 4]]),
    );
    attrs.set(
        "middle",
        DynamicAttributeValue::from_instances(vec![[3.0; 4]]),
    );
    manager.upload_dirty(device, queue, &mut attrs)?;

    let names = manager.storage_buffer_names();
    assert_eq!(names, vec!["alpha", "middle", "zebra"]);

    Ok(())
}
