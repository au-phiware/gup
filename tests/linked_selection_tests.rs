// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU integration tests for LinkedSelection (GUP-287).
//!
//! These tests verify that LinkedSelection correctly manages
//! generation-based change detection and GPU resource creation.

use gup::Circle;
use gup::linked_selection::{LinkedSelection, SharedSelectionState};
use gup::mark::circle::CircleInstance;
use gup::pipeline_cache::PipelineCache;

/// Test helper to create GPU context.
async fn create_gpu_context() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to find an appropriate adapter");

    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: Default::default(),
            experimental_features: Default::default(),
        })
        .await
        .expect("Failed to create device")
}

fn make_circle(d: &f32) -> CircleInstance {
    CircleInstance {
        center: [*d, 0.0],
        radius: 0.05,
        _pad0: 0.0,
        fill_color: [1.0, 0.0, 0.0, 1.0],
        stroke_width: 0.0,
        _pad1: [0.0; 3],
        stroke_color: [0.0; 4],
    }
}

#[tokio::test]
async fn prepare_render_creates_gpu_resources() {
    let (device, queue) = create_gpu_context().await;
    let shared = SharedSelectionState::<usize>::new();
    let data = vec![1.0f32, 2.0, 3.0];

    let mut linked: LinkedSelection<f32, Circle, usize> =
        LinkedSelection::new(data, shared, |_item, idx| idx).dim_opacity(0.2);

    assert!(!linked.is_render_ready());

    linked
        .prepare_render(&device, &queue, make_circle, None, None)
        .expect("prepare_render should succeed");

    assert!(linked.is_render_ready());
}

#[tokio::test]
async fn prepare_render_skips_when_no_state_change() {
    let (device, queue) = create_gpu_context().await;
    let shared = SharedSelectionState::<usize>::new();
    let data = vec![1.0f32, 2.0, 3.0];

    let mut linked: LinkedSelection<f32, Circle, usize> =
        LinkedSelection::new(data, shared, |_item, idx| idx);

    // First call: creates GPU resources.
    linked
        .prepare_render(&device, &queue, make_circle, None, None)
        .expect("first prepare_render should succeed");

    let gen_after_first = linked.last_generation();

    // Second call: no state change → should skip rebuild.
    linked
        .prepare_render(&device, &queue, make_circle, None, None)
        .expect("second prepare_render should succeed");

    // Generation counter should not have changed.
    assert_eq!(linked.last_generation(), gen_after_first);
    assert!(linked.is_render_ready());
}

#[tokio::test]
async fn prepare_render_rebuilds_on_state_change() {
    let (device, queue) = create_gpu_context().await;
    let shared = SharedSelectionState::<usize>::new();
    let data = vec![1.0f32, 2.0, 3.0];

    let mut linked: LinkedSelection<f32, Circle, usize> =
        LinkedSelection::new(data, shared.clone(), |_item, idx| idx);

    // First call: creates GPU resources at generation 0.
    linked
        .prepare_render(&device, &queue, make_circle, None, None)
        .expect("first prepare_render should succeed");

    let gen_before = linked.last_generation();

    // Mutate shared state → generation increases.
    shared.select([0, 2]);

    // Next call: detects change → rebuilds.
    linked
        .prepare_render(&device, &queue, make_circle, None, None)
        .expect("prepare_render after state change should succeed");

    assert!(linked.last_generation() > gen_before);
}

#[tokio::test]
async fn two_linked_selections_share_state() {
    let (device, queue) = create_gpu_context().await;
    let shared = SharedSelectionState::<usize>::new();

    let data_a = vec![1.0f32, 2.0, 3.0];
    let data_b = vec![10.0f32, 20.0, 30.0];

    let mut linked_a: LinkedSelection<f32, Circle, usize> =
        LinkedSelection::new(data_a, shared.clone(), |_item, idx| idx);
    let mut linked_b: LinkedSelection<f32, Circle, usize> =
        LinkedSelection::new(data_b, shared.clone(), |_item, idx| idx);

    let mut cache = PipelineCache::new();

    // Initial render of both.
    linked_a
        .prepare_render(&device, &queue, make_circle, Some(&mut cache), None)
        .expect("a first prepare");
    linked_b
        .prepare_render(&device, &queue, make_circle, Some(&mut cache), None)
        .expect("b first prepare");

    assert!(linked_a.is_render_ready());
    assert!(linked_b.is_render_ready());

    // Select items — both should detect the change on next prepare.
    shared.select([1]);

    let gen_a_before = linked_a.last_generation();
    let gen_b_before = linked_b.last_generation();

    linked_a
        .prepare_render(&device, &queue, make_circle, Some(&mut cache), None)
        .expect("a second prepare");
    linked_b
        .prepare_render(&device, &queue, make_circle, Some(&mut cache), None)
        .expect("b second prepare");

    assert!(linked_a.last_generation() > gen_a_before);
    assert!(linked_b.last_generation() > gen_b_before);
}

#[tokio::test]
async fn prepare_render_rebuilds_after_set_data() {
    let (device, queue) = create_gpu_context().await;
    let shared = SharedSelectionState::<usize>::new();

    let mut linked: LinkedSelection<f32, Circle, usize> =
        LinkedSelection::new(vec![1.0f32], shared, |_item, idx| idx);

    linked
        .prepare_render(&device, &queue, make_circle, None, None)
        .expect("initial prepare");
    assert!(linked.is_render_ready());

    // set_data invalidates render state.
    linked.set_data(vec![1.0, 2.0, 3.0, 4.0]);
    assert!(!linked.is_render_ready());

    // Next prepare_render should rebuild.
    linked
        .prepare_render(&device, &queue, make_circle, None, None)
        .expect("prepare after set_data");
    assert!(linked.is_render_ready());
    assert_eq!(linked.data().len(), 4);
}
