// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU integration tests for the SelectionMaskBuffer.
//!
//! These tests verify that the compute shader correctly applies alpha
//! dimming to instance data based on the selection mask.

use gup::linked_selection::SharedSelectionState;
use gup::mark::circle::CircleInstance;
use gup::selection_mask::{AlphaOffsets, SelectionMaskBuffer};

// ---------------------------------------------------------------------------
// GPU context helper
// ---------------------------------------------------------------------------

async fn create_gpu_context() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .ok()?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("test_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: Default::default(),
        })
        .await
        .ok()?;
    Some((device, queue))
}

/// Read back GPU buffer contents as a Vec<f32>.
async fn read_buffer_f32(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    count: usize,
) -> Vec<f32> {
    let size = (count * std::mem::size_of::<f32>()) as u64;
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("staging_readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("readback_encoder"),
    });
    encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, size);
    queue.submit([encoder.finish()]);

    let slice = staging.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });
    let _ = device.poll(wgpu::PollType::Wait);
    rx.recv().unwrap().unwrap();

    let data = slice.get_mapped_range();
    let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
    drop(data);
    staging.unmap();
    result
}

// ---------------------------------------------------------------------------
// Helper to create test instances
// ---------------------------------------------------------------------------

fn make_circle_instances(count: usize) -> Vec<CircleInstance> {
    (0..count)
        .map(|i| {
            let v = i as f32 / count as f32;
            CircleInstance {
                center: [v, v],
                radius: 0.05,
                _pad0: 0.0,
                fill_color: [1.0, 0.0, 0.0, 1.0],
                stroke_width: 1.0,
                _pad1: [0.0; 3],
                stroke_color: [0.0, 0.0, 1.0, 0.8],
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_selection_mask_buffer_creation() {
    let Some((device, _queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let offsets = AlphaOffsets::for_circle();
    let mask_buf = SelectionMaskBuffer::new(&device, 1000, &offsets);
    assert!(mask_buf.is_ok());

    let mask_buf = mask_buf.unwrap();
    assert_eq!(mask_buf.capacity(), 1000);
    assert!(!mask_buf.has_active_selection());
    assert_eq!(mask_buf.last_generation(), 0);
}

#[tokio::test]
async fn test_selection_mask_buffer_zero_capacity_fails() {
    let Some((device, _queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let offsets = AlphaOffsets::for_circle();
    let result = SelectionMaskBuffer::new(&device, 0, &offsets);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mask_update_from_shared_state() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let offsets = AlphaOffsets::for_circle();
    let mut mask_buf = SelectionMaskBuffer::new(&device, 10, &offsets).unwrap();

    let state = SharedSelectionState::<usize>::new();
    let data: Vec<usize> = (0..10).collect();

    // No selection → should not report change (gen 0 == 0).
    let changed = mask_buf.update_mask(&queue, &data, |_item, idx| idx, &state);
    assert!(!changed);

    // Select some items → should detect change.
    state.select([2, 5, 7]);
    let changed = mask_buf.update_mask(&queue, &data, |_item, idx| idx, &state);
    assert!(changed);
    assert!(mask_buf.has_active_selection());

    // Same generation → should not report change.
    let changed = mask_buf.update_mask(&queue, &data, |_item, idx| idx, &state);
    assert!(!changed);
}

#[tokio::test]
async fn test_dimming_compute_shader_selected_items_keep_alpha() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let instances = make_circle_instances(4);
    let offsets = AlphaOffsets::for_circle();
    let mut mask_buf = SelectionMaskBuffer::new(&device, 4, &offsets).unwrap();

    // Upload source instances.
    let source_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("source_instances"),
        size: (4 * std::mem::size_of::<CircleInstance>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&source_buffer, 0, bytemuck::cast_slice(&instances));

    // Select items 0 and 2 only.
    let state = SharedSelectionState::<usize>::new();
    state.select([0, 2]);

    let data: Vec<usize> = (0..4).collect();
    mask_buf.update_mask(&queue, &data, |_item, idx| idx, &state);

    // Dispatch dimming with opacity 0.2.
    mask_buf.dispatch_dimming(&device, &queue, &source_buffer, 4, 0.2);

    // Read back output.
    let floats_per_instance = offsets.floats_per_instance() as usize;
    let total_floats = 4 * floats_per_instance;
    let output = read_buffer_f32(&device, &queue, mask_buf.output_buffer(), total_floats).await;

    // Instance 0 (selected): fill_color alpha at index 7 should be 1.0.
    assert!(
        (output[0 * floats_per_instance + 7] - 1.0).abs() < 1e-5,
        "Selected instance 0 fill alpha should be 1.0, got {}",
        output[0 * floats_per_instance + 7]
    );
    // Instance 0 (selected): stroke_color alpha at index 15 should be 0.8.
    assert!(
        (output[0 * floats_per_instance + 15] - 0.8).abs() < 1e-5,
        "Selected instance 0 stroke alpha should be 0.8, got {}",
        output[0 * floats_per_instance + 15]
    );

    // Instance 1 (unselected): fill alpha should be 1.0 * 0.2 = 0.2.
    assert!(
        (output[1 * floats_per_instance + 7] - 0.2).abs() < 1e-5,
        "Unselected instance 1 fill alpha should be 0.2, got {}",
        output[1 * floats_per_instance + 7]
    );
    // Instance 1 (unselected): stroke alpha should be 0.8 * 0.2 = 0.16.
    assert!(
        (output[1 * floats_per_instance + 15] - 0.16).abs() < 1e-5,
        "Unselected instance 1 stroke alpha should be 0.16, got {}",
        output[1 * floats_per_instance + 15]
    );

    // Instance 2 (selected): full alpha.
    assert!(
        (output[2 * floats_per_instance + 7] - 1.0).abs() < 1e-5,
        "Selected instance 2 fill alpha should be 1.0"
    );

    // Instance 3 (unselected): dimmed.
    assert!(
        (output[3 * floats_per_instance + 7] - 0.2).abs() < 1e-5,
        "Unselected instance 3 fill alpha should be 0.2"
    );
}

#[tokio::test]
async fn test_dimming_preserves_non_alpha_data() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let instances = make_circle_instances(2);
    let offsets = AlphaOffsets::for_circle();
    let mut mask_buf = SelectionMaskBuffer::new(&device, 2, &offsets).unwrap();

    let source_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("source_instances"),
        size: (2 * std::mem::size_of::<CircleInstance>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&source_buffer, 0, bytemuck::cast_slice(&instances));

    // Select only item 0 → item 1 gets dimmed.
    let state = SharedSelectionState::<usize>::new();
    state.select([0]);
    let data: Vec<usize> = (0..2).collect();
    mask_buf.update_mask(&queue, &data, |_item, idx| idx, &state);
    mask_buf.dispatch_dimming(&device, &queue, &source_buffer, 2, 0.2);

    let floats_per_instance = offsets.floats_per_instance() as usize;
    let output = read_buffer_f32(&device, &queue, mask_buf.output_buffer(), 2 * floats_per_instance).await;

    // Verify non-alpha fields are preserved for instance 1 (dimmed).
    let base = 1 * floats_per_instance;
    let expected_center_x = instances[1].center[0];
    let expected_center_y = instances[1].center[1];
    let expected_radius = instances[1].radius;
    let expected_fill_r = instances[1].fill_color[0];

    assert!(
        (output[base] - expected_center_x).abs() < 1e-6,
        "center.x preserved"
    );
    assert!(
        (output[base + 1] - expected_center_y).abs() < 1e-6,
        "center.y preserved"
    );
    assert!(
        (output[base + 2] - expected_radius).abs() < 1e-6,
        "radius preserved"
    );
    assert!(
        (output[base + 4] - expected_fill_r).abs() < 1e-6,
        "fill_color.r preserved"
    );
}

#[tokio::test]
async fn test_no_selection_means_full_opacity() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let instances = make_circle_instances(3);
    let offsets = AlphaOffsets::for_circle();
    let mut mask_buf = SelectionMaskBuffer::new(&device, 3, &offsets).unwrap();

    let source_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("source_instances"),
        size: (3 * std::mem::size_of::<CircleInstance>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&source_buffer, 0, bytemuck::cast_slice(&instances));

    // Empty selection → mask is all 1s → no dimming.
    let state = SharedSelectionState::<usize>::new();
    // Need to trigger a generation change so update_mask runs.
    state.select(Vec::<usize>::new());
    let data: Vec<usize> = (0..3).collect();
    mask_buf.update_mask(&queue, &data, |_item, idx| idx, &state);
    mask_buf.dispatch_dimming(&device, &queue, &source_buffer, 3, 0.2);

    let floats_per_instance = offsets.floats_per_instance() as usize;
    let output = read_buffer_f32(&device, &queue, mask_buf.output_buffer(), 3 * floats_per_instance).await;

    // All instances should have full alpha since selection is empty.
    for i in 0..3 {
        let fill_alpha = output[i * floats_per_instance + 7];
        assert!(
            (fill_alpha - 1.0).abs() < 1e-5,
            "Instance {i} should have full fill alpha, got {fill_alpha}"
        );
    }
}

#[tokio::test]
async fn test_ensure_capacity_grows_buffers() {
    let Some((device, _queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let offsets = AlphaOffsets::for_circle();
    let mut mask_buf = SelectionMaskBuffer::new(&device, 10, &offsets).unwrap();
    assert_eq!(mask_buf.capacity(), 10);

    // Grow to 100.
    mask_buf.ensure_capacity(&device, 100);
    assert!(mask_buf.capacity() >= 100);

    // Grow to 1000 — should still work.
    mask_buf.ensure_capacity(&device, 1000);
    assert!(mask_buf.capacity() >= 1000);

    // Same or smaller does nothing.
    let cap_before = mask_buf.capacity();
    mask_buf.ensure_capacity(&device, 500);
    assert_eq!(mask_buf.capacity(), cap_before);
}

#[tokio::test]
async fn test_update_and_dispatch_convenience() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let instances = make_circle_instances(5);
    let offsets = AlphaOffsets::for_circle();
    let mut mask_buf = SelectionMaskBuffer::new(&device, 5, &offsets).unwrap();

    let source_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("source_instances"),
        size: (5 * std::mem::size_of::<CircleInstance>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&source_buffer, 0, bytemuck::cast_slice(&instances));

    let state = SharedSelectionState::<usize>::new();
    state.select([1, 3]);
    let data: Vec<usize> = (0..5).collect();

    let updated = mask_buf.update_and_dispatch(
        &device,
        &queue,
        &data,
        |_item, idx| idx,
        &state,
        &source_buffer,
        5,
        0.3,
    );
    assert!(updated);

    // Verify: items 1 and 3 should have full alpha, others dimmed by 0.3.
    let floats_per_instance = offsets.floats_per_instance() as usize;
    let output = read_buffer_f32(&device, &queue, mask_buf.output_buffer(), 5 * floats_per_instance).await;

    // Selected items: full alpha.
    assert!((output[1 * floats_per_instance + 7] - 1.0).abs() < 1e-5);
    assert!((output[3 * floats_per_instance + 7] - 1.0).abs() < 1e-5);

    // Unselected items: dimmed by 0.3.
    assert!((output[0 * floats_per_instance + 7] - 0.3).abs() < 1e-5);
    assert!((output[2 * floats_per_instance + 7] - 0.3).abs() < 1e-5);
    assert!((output[4 * floats_per_instance + 7] - 0.3).abs() < 1e-5);
}

#[tokio::test]
async fn test_selection_clear_restores_full_opacity() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let instances = make_circle_instances(4);
    let offsets = AlphaOffsets::for_circle();
    let mut mask_buf = SelectionMaskBuffer::new(&device, 4, &offsets).unwrap();

    let source_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("source_instances"),
        size: (4 * std::mem::size_of::<CircleInstance>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&source_buffer, 0, bytemuck::cast_slice(&instances));

    let state = SharedSelectionState::<usize>::new();
    let data: Vec<usize> = (0..4).collect();

    // First: select items 0,1 → items 2,3 dimmed.
    state.select([0, 1]);
    mask_buf.update_and_dispatch(
        &device, &queue, &data, |_item, idx| idx, &state, &source_buffer, 4, 0.2,
    );

    let floats_per_instance = offsets.floats_per_instance() as usize;
    let output = read_buffer_f32(&device, &queue, mask_buf.output_buffer(), 4 * floats_per_instance).await;
    assert!((output[2 * floats_per_instance + 7] - 0.2).abs() < 1e-5);

    // Now clear selection → all items should be full opacity.
    state.clear();
    mask_buf.update_and_dispatch(
        &device, &queue, &data, |_item, idx| idx, &state, &source_buffer, 4, 0.2,
    );

    let output = read_buffer_f32(&device, &queue, mask_buf.output_buffer(), 4 * floats_per_instance).await;
    for i in 0..4 {
        assert!(
            (output[i * floats_per_instance + 7] - 1.0).abs() < 1e-5,
            "After clear, instance {i} should have full alpha"
        );
    }
}
