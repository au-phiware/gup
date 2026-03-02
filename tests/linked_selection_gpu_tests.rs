// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU integration tests for LinkedSelection's automatic GPU dimming path.
//!
//! These tests verify that `LinkedSelection::prepare_render` correctly
//! switches between CPU and GPU dimming based on the configurable threshold,
//! and that the GPU path produces the same visual results as the CPU path.

use gup::linked_selection::{LinkedSelection, SharedSelectionState};
use gup::mark::circle::CircleInstance;

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
// Helper
// ---------------------------------------------------------------------------

fn make_data(count: usize) -> Vec<f32> {
    (0..count).map(|i| i as f32 / count as f32).collect()
}

fn circle_mapper(d: &f32) -> CircleInstance {
    CircleInstance {
        center: [*d, *d],
        radius: 0.05,
        _pad0: 0.0,
        fill_color: [1.0, 0.0, 0.0, 1.0],
        stroke_width: 1.0,
        _pad1: [0.0; 3],
        stroke_color: [0.0, 0.0, 1.0, 0.8],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Verify that the GPU path activates when instance count >= threshold.
#[tokio::test]
async fn gpu_path_activates_above_threshold() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let shared = SharedSelectionState::<usize>::new();
    shared.select([0usize, 2]);

    // 5 items with threshold 3 → should use GPU path.
    let data = make_data(5);
    let mut linked: LinkedSelection<f32, gup::Circle, usize> =
        LinkedSelection::new(data, shared.clone(), |_item, idx| idx)
            .gpu_dimming_threshold(3)
            .dim_opacity(0.2);

    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();

    assert!(linked.is_render_ready());
    assert!(linked.is_gpu_dimming_active());
}

/// Verify that the CPU path is used below the threshold.
#[tokio::test]
async fn cpu_path_below_threshold() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let shared = SharedSelectionState::<usize>::new();
    shared.select([0usize]);

    // 5 items with threshold 10 → should use CPU path.
    let data = make_data(5);
    let mut linked: LinkedSelection<f32, gup::Circle, usize> =
        LinkedSelection::new(data, shared.clone(), |_item, idx| idx)
            .gpu_dimming_threshold(10)
            .dim_opacity(0.2);

    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();

    assert!(linked.is_render_ready());
    assert!(!linked.is_gpu_dimming_active());
}

/// Verify that GPU dimming produces correct alpha values in the instance buffer.
#[tokio::test]
async fn gpu_dimming_produces_correct_alpha() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let shared = SharedSelectionState::<usize>::new();
    shared.select([0usize, 2]);

    let data = make_data(4);
    let mut linked: LinkedSelection<f32, gup::Circle, usize> = LinkedSelection::new(data, shared.clone(), |_item, idx| idx)
            .gpu_dimming_threshold(0) // Force GPU path for any count
            .dim_opacity(0.2);

    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();

    assert!(linked.is_gpu_dimming_active());

    // Read back the instance buffer to verify dimming.
    let instance_buffer = linked.selection().instance_buffer().unwrap();
    let floats_per_instance = 16; // CircleInstance is 64 bytes = 16 floats
    let total_floats = 4 * floats_per_instance;
    let output = read_buffer_f32(&device, &queue, instance_buffer, total_floats).await;

    // Instance 0 (selected): fill_color alpha at float 7 should be 1.0.
    assert!(
        (output[0 * floats_per_instance + 7] - 1.0).abs() < 1e-5,
        "Selected instance 0 fill alpha should be 1.0, got {}",
        output[0 * floats_per_instance + 7]
    );
    // Instance 0 (selected): stroke_color alpha at float 15 should be 0.8.
    assert!(
        (output[0 * floats_per_instance + 15] - 0.8).abs() < 1e-5,
        "Selected instance 0 stroke alpha should be 0.8, got {}",
        output[0 * floats_per_instance + 15]
    );

    // Instance 1 (unselected): fill alpha dimmed to 0.2.
    assert!(
        (output[floats_per_instance + 7] - 0.2).abs() < 1e-5,
        "Unselected instance 1 fill alpha should be 0.2, got {}",
        output[floats_per_instance + 7]
    );
    // Instance 1 (unselected): stroke alpha dimmed to 0.8 * 0.2 = 0.16.
    assert!(
        (output[floats_per_instance + 15] - 0.16).abs() < 1e-5,
        "Unselected instance 1 stroke alpha should be 0.16, got {}",
        output[floats_per_instance + 15]
    );

    // Instance 2 (selected): full alpha.
    assert!(
        (output[2 * floats_per_instance + 7] - 1.0).abs() < 1e-5,
        "Selected instance 2 fill alpha should be 1.0, got {}",
        output[2 * floats_per_instance + 7]
    );

    // Instance 3 (unselected): dimmed.
    assert!(
        (output[3 * floats_per_instance + 7] - 0.2).abs() < 1e-5,
        "Unselected instance 3 fill alpha should be 0.2, got {}",
        output[3 * floats_per_instance + 7]
    );
}

/// Verify that changing the selection updates the GPU dimmed output.
#[tokio::test]
async fn gpu_dimming_selection_change_updates_output() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let shared = SharedSelectionState::<usize>::new();
    shared.select([0usize]);

    let data = make_data(4);
    let mut linked: LinkedSelection<f32, gup::Circle, usize> =
        LinkedSelection::new(data, shared.clone(), |_item, idx| idx)
            .gpu_dimming_threshold(0)
            .dim_opacity(0.3);

    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();

    let floats_per_instance = 16;
    let total_floats = 4 * floats_per_instance;

    // Instance 1 should be dimmed (unselected).
    let output = read_buffer_f32(
        &device,
        &queue,
        linked.selection().instance_buffer().unwrap(),
        total_floats,
    )
    .await;
    assert!(
        (output[floats_per_instance + 7] - 0.3).abs() < 1e-5,
        "Instance 1 fill alpha should be 0.3, got {}",
        output[floats_per_instance + 7]
    );

    // Now select item 1 as well.
    shared.select([1usize]);
    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();

    let output = read_buffer_f32(
        &device,
        &queue,
        linked.selection().instance_buffer().unwrap(),
        total_floats,
    )
    .await;
    // Instance 1 should now be full alpha (selected).
    assert!(
        (output[floats_per_instance + 7] - 1.0).abs() < 1e-5,
        "Instance 1 fill alpha should now be 1.0, got {}",
        output[floats_per_instance + 7]
    );
}

/// Verify that clearing the selection restores full alpha via the GPU path.
#[tokio::test]
async fn gpu_dimming_clear_selection_restores_alpha() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let shared = SharedSelectionState::<usize>::new();
    shared.select([0usize]);

    let data = make_data(4);
    let mut linked: LinkedSelection<f32, gup::Circle, usize> =
        LinkedSelection::new(data, shared.clone(), |_item, idx| idx)
            .gpu_dimming_threshold(0)
            .dim_opacity(0.2);

    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();

    let floats_per_instance = 16;
    let total_floats = 4 * floats_per_instance;

    // Instance 3 is dimmed.
    let output = read_buffer_f32(
        &device,
        &queue,
        linked.selection().instance_buffer().unwrap(),
        total_floats,
    )
    .await;
    assert!(
        (output[3 * floats_per_instance + 7] - 0.2).abs() < 1e-5,
        "Instance 3 fill alpha should be 0.2"
    );

    // Clear selection → all instances should return to full alpha.
    shared.clear();
    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();

    let output = read_buffer_f32(
        &device,
        &queue,
        linked.selection().instance_buffer().unwrap(),
        total_floats,
    )
    .await;
    for i in 0..4 {
        assert!(
            (output[i * floats_per_instance + 7] - 1.0).abs() < 1e-5,
            "Instance {i} fill alpha should be 1.0 after clear, got {}",
            output[i * floats_per_instance + 7]
        );
    }
}

/// Verify set_data invalidates GPU resources and rebuilds on next prepare_render.
#[tokio::test]
async fn gpu_path_survives_set_data() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let shared = SharedSelectionState::<usize>::new();
    shared.select([0usize]);

    let data = make_data(4);
    let mut linked: LinkedSelection<f32, gup::Circle, usize> =
        LinkedSelection::new(data, shared.clone(), |_item, idx| idx)
            .gpu_dimming_threshold(0)
            .dim_opacity(0.2);

    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();
    assert!(linked.is_gpu_dimming_active());

    // Replace data — GPU resources should be cleared.
    linked.set_data(make_data(6));
    assert!(!linked.is_gpu_dimming_active());

    // Next prepare_render should rebuild.
    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();
    assert!(linked.is_render_ready());
    assert!(linked.is_gpu_dimming_active());

    // Verify dimming is correct for the new data.
    let floats_per_instance = 16;
    let total_floats = 6 * floats_per_instance;
    let output = read_buffer_f32(
        &device,
        &queue,
        linked.selection().instance_buffer().unwrap(),
        total_floats,
    )
    .await;

    // Instance 0 selected → full alpha.
    assert!(
        (output[0 * floats_per_instance + 7] - 1.0).abs() < 1e-5,
        "Instance 0 fill alpha should be 1.0"
    );
    // Instance 5 unselected → dimmed.
    assert!(
        (output[5 * floats_per_instance + 7] - 0.2).abs() < 1e-5,
        "Instance 5 fill alpha should be 0.2, got {}",
        output[5 * floats_per_instance + 7]
    );
}

/// No-op when prepare_render is called without a selection change.
#[tokio::test]
async fn gpu_path_no_op_without_change() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let shared = SharedSelectionState::<usize>::new();
    shared.select([0usize]);

    let data = make_data(4);
    let mut linked: LinkedSelection<f32, gup::Circle, usize> =
        LinkedSelection::new(data, shared.clone(), |_item, idx| idx)
            .gpu_dimming_threshold(0)
            .dim_opacity(0.2);

    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();

    // Second call with no change should be a no-op.
    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();

    // Still render-ready and GPU active.
    assert!(linked.is_render_ready());
    assert!(linked.is_gpu_dimming_active());
}

/// Performance: small datasets (<1K) should not trigger the GPU path.
#[tokio::test]
async fn small_dataset_uses_cpu_path() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping GPU test: no adapter available");
        return;
    };

    let shared = SharedSelectionState::<usize>::new();
    shared.select([0usize]);

    // 100 items — well below the default 10K threshold.
    let data = make_data(100);
    let mut linked: LinkedSelection<f32, gup::Circle, usize> =
        LinkedSelection::new(data, shared.clone(), |_item, idx| idx).dim_opacity(0.2);

    linked
        .prepare_render(&device, &queue, circle_mapper, None, None)
        .unwrap();

    assert!(linked.is_render_ready());
    assert!(!linked.is_gpu_dimming_active());
}
