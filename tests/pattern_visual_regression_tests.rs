// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Visual regression tests for pattern rendering (GUP-160).
//!
//! These tests render patterns to offscreen textures and compare them with
//! reference images to catch visual regressions that unit tests might miss.

mod visual_regression_utils;

use gup::accessibility::{Color, Pattern, PatternUniforms};
use gup::error::GupResult;
use visual_regression_utils::{VisualTestConfig, VisualTestRenderer};
use wgpu::util::DeviceExt;

/// Helper to render a simple colored rectangle for pattern testing.
async fn render_pattern_test(
    renderer: &VisualTestRenderer,
    pattern: &Pattern,
    foreground: Color,
    background: Color,
) -> GupResult<()> {
    let context = renderer.context();
    let texture_view = renderer.texture_view();

    // Create pattern uniforms
    let pattern_uniforms = PatternUniforms::from_pattern(pattern, foreground, background);

    // Create a simple quad covering the screen
    let vertices: &[f32] = &[
        -1.0, -1.0, // Bottom-left
        1.0, -1.0, // Bottom-right
        -1.0, 1.0, // Top-left
        1.0, 1.0, // Top-right
    ];

    let vertex_buffer = context
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pattern_test_vertices"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

    // Create uniform buffer for pattern data
    let uniform_buffer = context
        .device
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("pattern_uniforms"),
            contents: bytemuck::bytes_of(&pattern_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

    // Create bind group
    let bind_group_layout =
        context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("pattern_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

    let bind_group = context
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pattern_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

    // Simple shader that just renders a quad with pattern
    let shader = context
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pattern_test_shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("visual_test_pattern_shader.wgsl").into(),
            ),
        });

    let pipeline_layout = context
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pattern_test_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

    let render_pipeline = context
        .device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pattern_test_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 8,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

    // Render
    let mut encoder = context
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("pattern_test_encoder"),
        });

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("pattern_test_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&render_pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..4, 0..1);
    }

    context.queue.submit(Some(encoder.finish()));

    Ok(())
}

//
// Visual Tests for Pattern Types
//

#[tokio::test]
async fn test_visual_solid_pattern() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    render_pattern_test(&renderer, &Pattern::Solid, Color::RED, Color::RED).await?;

    let result = renderer.capture_and_compare("solid_red").await?;
    assert!(result.passed, "{}", result);

    Ok(())
}

#[tokio::test]
async fn test_visual_dots_pattern() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    let pattern = Pattern::Dots { spacing: 16.0 };
    render_pattern_test(&renderer, &pattern, Color::BLACK, Color::WHITE).await?;

    let result = renderer.capture_and_compare("dots_16px").await?;
    assert!(result.passed, "{}", result);

    Ok(())
}

#[tokio::test]
async fn test_visual_dots_pattern_small_spacing() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    let pattern = Pattern::Dots { spacing: 8.0 };
    render_pattern_test(&renderer, &pattern, Color::BLUE, Color::YELLOW).await?;

    let result = renderer.capture_and_compare("dots_8px").await?;
    assert!(result.passed, "{}", result);

    Ok(())
}

#[tokio::test]
async fn test_visual_lines_pattern_horizontal() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    let pattern = Pattern::Lines {
        spacing: 12.0,
        angle: 0.0,
    };
    render_pattern_test(&renderer, &pattern, Color::BLACK, Color::WHITE).await?;

    let result = renderer
        .capture_and_compare("lines_horizontal_12px")
        .await?;
    assert!(result.passed, "{}", result);

    Ok(())
}

#[tokio::test]
async fn test_visual_lines_pattern_diagonal() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    let pattern = Pattern::Lines {
        spacing: 12.0,
        angle: std::f32::consts::PI / 4.0, // 45 degrees
    };
    render_pattern_test(&renderer, &pattern, Color::RED, Color::WHITE).await?;

    let result = renderer.capture_and_compare("lines_diagonal_12px").await?;
    assert!(result.passed, "{}", result);

    Ok(())
}

#[tokio::test]
async fn test_visual_crosshatch_pattern() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    let pattern = Pattern::Crosshatch { spacing: 16.0 };
    render_pattern_test(&renderer, &pattern, Color::BLACK, Color::WHITE).await?;

    let result = renderer.capture_and_compare("crosshatch_16px").await?;
    assert!(result.passed, "{}", result);

    Ok(())
}

#[tokio::test]
async fn test_visual_crosshatch_pattern_dense() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    let pattern = Pattern::Crosshatch { spacing: 8.0 };
    render_pattern_test(&renderer, &pattern, Color::BLUE, Color::YELLOW).await?;

    let result = renderer.capture_and_compare("crosshatch_8px").await?;
    assert!(result.passed, "{}", result);

    Ok(())
}

//
// Edge Case Tests
//

#[tokio::test]
async fn test_visual_pattern_edge_small_spacing() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    let pattern = Pattern::Dots { spacing: 4.0 };
    render_pattern_test(&renderer, &pattern, Color::BLACK, Color::WHITE).await?;

    let result = renderer.capture_and_compare("dots_4px_dense").await?;
    assert!(result.passed, "{}", result);

    Ok(())
}

#[tokio::test]
async fn test_visual_pattern_edge_large_spacing() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    let pattern = Pattern::Lines {
        spacing: 32.0,
        angle: 0.0,
    };
    render_pattern_test(&renderer, &pattern, Color::BLACK, Color::WHITE).await?;

    let result = renderer.capture_and_compare("lines_32px_sparse").await?;
    assert!(result.passed, "{}", result);

    Ok(())
}

#[tokio::test]
async fn test_visual_pattern_color_combinations() -> GupResult<()> {
    let config = VisualTestConfig::default();
    let renderer = VisualTestRenderer::new(config).await?;

    let pattern = Pattern::Dots { spacing: 12.0 };
    // Test with complementary colors
    let fg = Color::new(0.0, 0.5, 1.0, 1.0); // Light blue
    let bg = Color::new(1.0, 0.5, 0.0, 1.0); // Orange
    render_pattern_test(&renderer, &pattern, fg, bg).await?;

    let result = renderer
        .capture_and_compare("dots_color_combination")
        .await?;
    assert!(result.passed, "{}", result);

    Ok(())
}
