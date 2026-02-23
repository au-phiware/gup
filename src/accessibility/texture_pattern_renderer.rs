// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Texture-based pattern renderer for comparison with procedural rendering.
//!
//! This module provides a texture-based alternative to the procedural pattern renderer,
//! allowing performance comparison between memory-based (textures) and computation-based
//! (procedural) pattern rendering approaches.

use crate::accessibility::high_contrast::{Color, Pattern};
use crate::accessibility::texture_pattern_generator::{TexturePatternGenerator, TextureResolution};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use wgpu;

/// GPU uniform buffer structure for texture-based pattern rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TexturePatternUniforms {
    /// Foreground color (pattern color) - RGBA
    pub foreground_color: [f32; 4],
    /// Background color (base color) - RGBA
    pub background_color: [f32; 4],
    /// Pattern scaling factor
    pub scale: f32,
    /// Padding for alignment
    pub _padding: [f32; 3],
}

impl TexturePatternUniforms {
    /// Create texture pattern uniforms from colors and scale
    pub fn new(foreground: Color, background: Color, scale: f32) -> Self {
        Self {
            foreground_color: [foreground.r, foreground.g, foreground.b, foreground.a],
            background_color: [background.r, background.g, background.b, background.a],
            scale,
            _padding: [0.0; 3],
        }
    }
}

/// Texture-based pattern renderer
pub struct TexturePatternRenderer {
    /// Pattern textures cache
    textures: HashMap<PatternKey, wgpu::Texture>,
    /// Texture views cache
    texture_views: HashMap<PatternKey, wgpu::TextureView>,
    /// Sampler for texture sampling
    sampler: wgpu::Sampler,
    /// Uniform buffer
    uniform_buffer: wgpu::Buffer,
    /// Bind group
    bind_group: Option<wgpu::BindGroup>,
    /// Current uniforms
    current_uniforms: TexturePatternUniforms,
    /// Current resolution
    resolution: TextureResolution,
    /// Texture generator
    generator: TexturePatternGenerator,
}

/// Key for pattern texture cache
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PatternKey {
    pattern_type: u32,
    spacing: u32, // Stored as integer for hashing
    angle: u32,   // Stored as integer for hashing (radians * 1000)
}

impl PatternKey {
    fn from_pattern(pattern: &Pattern) -> Self {
        Self {
            pattern_type: pattern.pattern_type_id(),
            spacing: (pattern.spacing() * 10.0) as u32,
            angle: (pattern.angle() * 1000.0) as u32,
        }
    }
}

impl TexturePatternRenderer {
    /// Create a new texture-based pattern renderer
    pub fn new(
        device: &wgpu::Device,
        resolution: TextureResolution,
        initial_uniforms: TexturePatternUniforms,
    ) -> Self {
        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Texture Pattern Uniform Buffer"),
            size: std::mem::size_of::<TexturePatternUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create sampler with tiling and linear filtering
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Pattern Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });

        Self {
            textures: HashMap::new(),
            texture_views: HashMap::new(),
            sampler,
            uniform_buffer,
            bind_group: None,
            current_uniforms: initial_uniforms,
            resolution,
            generator: TexturePatternGenerator::new(resolution),
        }
    }

    /// Create the bind group layout for texture-based patterns
    pub fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Texture Pattern Bind Group Layout"),
            entries: &[
                // Uniform buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    /// Get or create a texture for the given pattern
    pub fn get_or_create_texture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pattern: &Pattern,
    ) -> &wgpu::TextureView {
        let key = PatternKey::from_pattern(pattern);

        if !self.textures.contains_key(&key) {
            let texture = self.generator.create_texture(device, queue, pattern);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            self.textures.insert(key, texture);
            self.texture_views.insert(key, view);
        }

        self.texture_views.get(&key).unwrap()
    }

    /// Update the bind group with a new pattern texture
    pub fn update_bind_group(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pattern: &Pattern,
    ) {
        // Get key and ensure texture exists
        let key = PatternKey::from_pattern(pattern);
        
        if !self.textures.contains_key(&key) {
            let texture = self.generator.create_texture(device, queue, pattern);
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            self.textures.insert(key, texture);
            self.texture_views.insert(key, view);
        }
        
        // Get texture view (now we know it exists)
        let texture_view = self.texture_views.get(&key).unwrap();
        let bind_group_layout = Self::create_bind_group_layout(device);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Texture Pattern Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        self.bind_group = Some(bind_group);
    }

    /// Update pattern uniforms
    pub fn update(&mut self, queue: &wgpu::Queue, uniforms: TexturePatternUniforms) {
        self.current_uniforms = uniforms;
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Get the bind group for rendering
    pub fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.bind_group.as_ref()
    }

    /// Get the current uniforms
    pub fn current_uniforms(&self) -> &TexturePatternUniforms {
        &self.current_uniforms
    }

    /// Get the resolution
    pub fn resolution(&self) -> TextureResolution {
        self.resolution
    }

    /// Get memory usage estimate in bytes
    pub fn memory_usage(&self) -> usize {
        let texture_size = self.resolution.size() as usize;
        let bytes_per_texture = texture_size * texture_size * 4; // RGBA
        bytes_per_texture * self.textures.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_pattern_uniforms_size() {
        assert_eq!(
            std::mem::size_of::<TexturePatternUniforms>(),
            48,
            "TexturePatternUniforms size must be 48 bytes for proper GPU alignment"
        );
    }

    #[test]
    fn test_pattern_key_from_pattern() {
        let pattern = Pattern::Dots { spacing: 8.0 };
        let key = PatternKey::from_pattern(&pattern);

        assert_eq!(key.pattern_type, 1); // Dots
        assert_eq!(key.spacing, 80);     // 8.0 * 10
    }

    #[test]
    fn test_pattern_key_uniqueness() {
        let pattern1 = Pattern::Dots { spacing: 8.0 };
        let pattern2 = Pattern::Dots { spacing: 10.0 };
        let pattern3 = Pattern::Lines {
            spacing: 8.0,
            angle: 0.0,
        };

        let key1 = PatternKey::from_pattern(&pattern1);
        let key2 = PatternKey::from_pattern(&pattern2);
        let key3 = PatternKey::from_pattern(&pattern3);

        assert_ne!(key1, key2, "Different spacing should create different keys");
        assert_ne!(key1, key3, "Different patterns should create different keys");
    }
}
