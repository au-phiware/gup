// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for GPU path tessellation performance.

use gup::mark::{GpuPathTessellator, PathCommand};
use gup::Vec2;
use std::sync::Arc;
use std::time::Instant;

async fn create_gpu_context() -> Option<(Arc<wgpu::Device>, Arc<wgpu::Queue>)> {
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
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: Default::default(),
        })
        .await
        .ok()?;

    Some((Arc::new(device), Arc::new(queue)))
}

fn create_complex_path(complexity: usize) -> Vec<PathCommand> {
    let mut commands = vec![PathCommand::MoveTo(Vec2 { x: 0.0, y: 0.0 })];

    // Create a complex path with curves
    for i in 0..complexity {
        let t = i as f32 / complexity as f32;
        let angle = t * 2.0 * std::f32::consts::PI;

        commands.push(PathCommand::CubicCurveTo {
            control1: Vec2 {
                x: angle.cos() * 0.5,
                y: angle.sin() * 0.5,
            },
            control2: Vec2 {
                x: angle.cos() * 1.5,
                y: angle.sin() * 1.5,
            },
            end: Vec2 {
                x: angle.cos() * 2.0,
                y: angle.sin() * 2.0,
            },
        });
    }

    commands.push(PathCommand::Close);
    commands
}

#[tokio::test]
async fn test_gpu_tessellation_single_path() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping test: GPU not available");
        return;
    };

    let tessellator = GpuPathTessellator::new(device, queue);

    // Create a moderately complex path
    let path = create_complex_path(20);

    let start = Instant::now();
    let result = tessellator.tessellate(&path, 0.1).await;
    let duration = start.elapsed();

    assert!(result.is_ok());
    let (_vbuf, _ibuf, vertex_count, _index_count) = result.unwrap();

    println!("Single path tessellation:");
    println!("  Commands: {}", path.len());
    println!("  Vertices generated: {}", vertex_count);
    println!("  Time: {:?}", duration);
    println!("  Throughput: {:.2} paths/sec", 1.0 / duration.as_secs_f32());

    assert!(vertex_count > 0);
}

#[tokio::test]
async fn test_gpu_tessellation_multiple_paths() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping test: GPU not available");
        return;
    };

    let tessellator = Arc::new(GpuPathTessellator::new(device, queue));

    // Create 100 paths
    let path_count = 100;
    let paths: Vec<_> = (0..path_count).map(|_| create_complex_path(10)).collect();

    println!("Tessellating {} paths...", path_count);

    let start = Instant::now();
    let mut total_vertices = 0;

    for (i, path) in paths.iter().enumerate() {
        let result = tessellator.tessellate(path, 0.1).await;
        assert!(result.is_ok());

        let (_vbuf, _ibuf, vertex_count, _index_count) = result.unwrap();
        total_vertices += vertex_count;

        if i % 25 == 0 {
            println!("  Progress: {}/{} paths", i + 1, path_count);
        }
    }

    let duration = start.elapsed();

    println!("\nMultiple path tessellation:");
    println!("  Paths: {}", path_count);
    println!("  Total vertices: {}", total_vertices);
    println!("  Total time: {:?}", duration);
    println!(
        "  Average per path: {:?}",
        duration / path_count as u32
    );
    println!(
        "  Throughput: {:.2} paths/sec",
        path_count as f32 / duration.as_secs_f32()
    );

    // Performance target: should process paths reasonably fast
    let paths_per_sec = path_count as f32 / duration.as_secs_f32();
    assert!(
        paths_per_sec > 10.0,
        "Expected at least 10 paths/sec, got {:.2}",
        paths_per_sec
    );
}

#[tokio::test]
async fn test_gpu_tessellation_various_curve_types() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping test: GPU not available");
        return;
    };

    let tessellator = GpuPathTessellator::new(device, queue);

    // Test different curve types
    let test_cases = vec![
        (
            "Lines only",
            vec![
                PathCommand::MoveTo(Vec2 { x: 0.0, y: 0.0 }),
                PathCommand::LineTo(Vec2 { x: 1.0, y: 0.0 }),
                PathCommand::LineTo(Vec2 { x: 1.0, y: 1.0 }),
                PathCommand::LineTo(Vec2 { x: 0.0, y: 1.0 }),
                PathCommand::Close,
            ],
        ),
        (
            "Quadratic curves",
            vec![
                PathCommand::MoveTo(Vec2 { x: 0.0, y: 0.0 }),
                PathCommand::QuadraticCurveTo {
                    control: Vec2 { x: 0.5, y: 1.0 },
                    end: Vec2 { x: 1.0, y: 0.0 },
                },
                PathCommand::QuadraticCurveTo {
                    control: Vec2 { x: 1.5, y: -1.0 },
                    end: Vec2 { x: 2.0, y: 0.0 },
                },
            ],
        ),
        (
            "Cubic curves",
            vec![
                PathCommand::MoveTo(Vec2 { x: 0.0, y: 0.0 }),
                PathCommand::CubicCurveTo {
                    control1: Vec2 { x: 0.33, y: 1.0 },
                    control2: Vec2 { x: 0.66, y: -1.0 },
                    end: Vec2 { x: 1.0, y: 0.0 },
                },
            ],
        ),
    ];

    for (name, commands) in test_cases {
        let result = tessellator.tessellate(&commands, 0.1).await;
        assert!(result.is_ok(), "Failed to tessellate: {}", name);

        let (_vbuf, _ibuf, vertex_count, _index_count) = result.unwrap();
        println!(
            "{}: {} commands -> {} vertices",
            name,
            commands.len(),
            vertex_count
        );
        assert!(vertex_count > 0, "No vertices generated for: {}", name);
    }
}

#[tokio::test]
async fn test_gpu_tessellation_tolerance_levels() {
    let Some((device, queue)) = create_gpu_context().await else {
        eprintln!("Skipping test: GPU not available");
        return;
    };

    let tessellator = GpuPathTessellator::new(device, queue);

    // Test with different tolerance levels
    let path = vec![
        PathCommand::MoveTo(Vec2 { x: 0.0, y: 0.0 }),
        PathCommand::CubicCurveTo {
            control1: Vec2 { x: 0.5, y: 2.0 },
            control2: Vec2 { x: 1.5, y: -2.0 },
            end: Vec2 { x: 2.0, y: 0.0 },
        },
    ];

    println!("\nTessellation quality comparison:");
    for &tolerance in &[0.01, 0.1, 0.5, 1.0] {
        let result = tessellator.tessellate(&path, tolerance).await;
        assert!(result.is_ok());

        let (_vbuf, _ibuf, vertex_count, _index_count) = result.unwrap();
        println!(
            "  Tolerance {:.2}: {} vertices",
            tolerance, vertex_count
        );

        // Higher tolerance should produce fewer vertices
        assert!(vertex_count > 0);
    }
}
