// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-accelerated pattern rendering for accessibility.
//!
//! This module provides pattern-based rendering as an alternative to color encoding,
//! enabling colorblind users to distinguish between data categories using texture patterns.

use crate::accessibility::high_contrast::{Color, Pattern};
use bytemuck::{Pod, Zeroable};
use wgpu;

/// GPU uniform buffer structure for pattern rendering.
///
/// This structure must match the layout in the WGSL shader code.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct PatternUniforms {
    /// Pattern type ID (0=solid, 1=dots, 2=lines, 3=crosshatch)
    pub pattern_type: u32,
    /// Pattern spacing in pixels
    pub spacing: f32,
    /// Pattern angle in radians (for line patterns)
    pub angle: f32,
    /// Padding for alignment
    pub _padding1: u32,
    /// Foreground color (pattern color) - RGBA
    pub foreground_color: [f32; 4],
    /// Background color (base color) - RGBA
    pub background_color: [f32; 4],
    /// Line thickness for line patterns
    pub thickness: f32,
    /// Padding for alignment
    pub _padding2: [f32; 3],
}

impl PatternUniforms {
    /// Create pattern uniforms from a pattern and colors.
    pub fn from_pattern(pattern: &Pattern, foreground: Color, background: Color) -> Self {
        Self {
            pattern_type: pattern.pattern_type_id(),
            spacing: pattern.spacing(),
            angle: pattern.angle(),
            _padding1: 0,
            foreground_color: [foreground.r, foreground.g, foreground.b, foreground.a],
            background_color: [background.r, background.g, background.b, background.a],
            thickness: pattern.thickness(),
            _padding2: [0.0; 3],
        }
    }

    /// Create a solid pattern (no pattern, just color).
    pub fn solid(color: Color) -> Self {
        Self {
            pattern_type: 0,
            spacing: 0.0,
            angle: 0.0,
            _padding1: 0,
            foreground_color: [color.r, color.g, color.b, color.a],
            background_color: [color.r, color.g, color.b, color.a],
            thickness: 0.0,
            _padding2: [0.0; 3],
        }
    }
}

/// Pattern renderer for GPU-accelerated pattern generation.
pub struct PatternRenderer {
    /// Pattern uniform buffer
    uniform_buffer: wgpu::Buffer,
    /// Bind group for pattern uniforms
    bind_group: wgpu::BindGroup,
    /// Current pattern uniforms
    current_uniforms: PatternUniforms,
}

impl PatternRenderer {
    /// Create a new pattern renderer.
    pub fn new(device: &wgpu::Device, initial_uniforms: PatternUniforms) -> Self {
        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Pattern Uniform Buffer"),
            size: std::mem::size_of::<PatternUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = Self::create_bind_group_layout(device);

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Pattern Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let renderer = Self {
            uniform_buffer,
            bind_group,
            current_uniforms: initial_uniforms,
        };

        // Initialize buffer with initial uniforms (requires queue, so we'll do it in update)
        renderer
    }

    /// Create the bind group layout for pattern uniforms.
    pub fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Pattern Bind Group Layout"),
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
        })
    }

    /// Update pattern uniforms.
    pub fn update(&mut self, queue: &wgpu::Queue, uniforms: PatternUniforms) {
        self.current_uniforms = uniforms;
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Get the bind group for rendering.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Get the current pattern uniforms.
    pub fn current_uniforms(&self) -> &PatternUniforms {
        &self.current_uniforms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_uniforms_size() {
        // Ensure uniforms are properly aligned
        assert_eq!(
            std::mem::size_of::<PatternUniforms>(),
            64,
            "PatternUniforms size must be 64 bytes for proper GPU alignment"
        );
    }

    #[test]
    fn test_pattern_uniforms_from_pattern() {
        let pattern = Pattern::Dots { spacing: 8.0 };
        let fg = Color::BLACK;
        let bg = Color::WHITE;

        let uniforms = PatternUniforms::from_pattern(&pattern, fg, bg);

        assert_eq!(uniforms.pattern_type, 1); // Dots pattern
        assert_eq!(uniforms.spacing, 8.0);
        assert_eq!(uniforms.foreground_color, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(uniforms.background_color, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_solid_pattern_uniforms() {
        let color = Color::RED;
        let uniforms = PatternUniforms::solid(color);

        assert_eq!(uniforms.pattern_type, 0); // Solid pattern
        assert_eq!(uniforms.foreground_color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(uniforms.background_color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_line_pattern_angle() {
        let pattern = Pattern::Lines {
            spacing: 6.0,
            angle: std::f32::consts::PI / 4.0, // 45 degrees
        };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);

        assert_eq!(uniforms.pattern_type, 2); // Lines pattern
        assert_eq!(uniforms.angle, std::f32::consts::PI / 4.0);
        assert_eq!(uniforms.spacing, 6.0);
    }

    #[test]
    fn test_crosshatch_pattern() {
        let pattern = Pattern::Crosshatch { spacing: 8.0 };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);

        assert_eq!(uniforms.pattern_type, 3); // Crosshatch pattern
        assert_eq!(uniforms.spacing, 8.0);
    }
}
