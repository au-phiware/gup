// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Texture-based pattern generation for performance comparison with procedural patterns.
//!
//! This module generates pre-rendered pattern textures that can be sampled in shaders
//! as an alternative to procedural pattern generation. This provides a memory/computation
//! trade-off for pattern rendering.

use crate::accessibility::high_contrast::Pattern;
use wgpu;

/// Texture resolution options for pattern generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureResolution {
    /// 128x128 pixels
    Low = 128,
    /// 256x256 pixels
    Medium = 256,
    /// 512x512 pixels
    High = 512,
}

impl TextureResolution {
    /// Get the size in pixels
    pub fn size(&self) -> u32 {
        *self as u32
    }
}

/// Texture-based pattern generator
pub struct TexturePatternGenerator {
    /// Texture resolution
    resolution: TextureResolution,
}

impl TexturePatternGenerator {
    /// Create a new texture pattern generator
    pub fn new(resolution: TextureResolution) -> Self {
        Self { resolution }
    }

    /// Generate a texture for the given pattern
    pub fn generate_pattern_texture(&self, pattern: &Pattern) -> Vec<u8> {
        let size = self.resolution.size() as usize;
        let mut pixels = vec![0u8; size * size * 4]; // RGBA

        match pattern {
            Pattern::Solid => self.generate_solid(&mut pixels, size),
            Pattern::Dots { spacing } => self.generate_dots(&mut pixels, size, *spacing),
            Pattern::Lines { spacing, angle } => {
                self.generate_lines(&mut pixels, size, *spacing, *angle)
            }
            Pattern::Crosshatch { spacing } => {
                self.generate_crosshatch(&mut pixels, size, *spacing)
            }
        }

        pixels
    }

    /// Generate a solid pattern texture (all white)
    fn generate_solid(&self, pixels: &mut [u8], size: usize) {
        for i in 0..size * size {
            let idx = i * 4;
            pixels[idx] = 255; // R
            pixels[idx + 1] = 255; // G
            pixels[idx + 2] = 255; // B
            pixels[idx + 3] = 255; // A
        }
    }

    /// Generate a dots pattern texture
    fn generate_dots(&self, pixels: &mut [u8], size: usize, spacing: f32) {
        let spacing_px = (spacing * size as f32 / 100.0).max(4.0);
        let dot_radius = spacing_px * 0.2;

        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;

                // Calculate position within pattern grid
                let fx = x as f32;
                let fy = y as f32;
                let grid_x = fx % spacing_px;
                let grid_y = fy % spacing_px;

                // Distance from cell center
                let dx = grid_x - spacing_px / 2.0;
                let dy = grid_y - spacing_px / 2.0;
                let dist = (dx * dx + dy * dy).sqrt();

                // Anti-aliased dot
                let alpha = if dist < dot_radius - 1.0 {
                    1.0
                } else if dist < dot_radius + 1.0 {
                    1.0 - (dist - (dot_radius - 1.0)) / 2.0
                } else {
                    0.0
                };

                let alpha_byte = (alpha * 255.0) as u8;
                pixels[idx] = 255; // R
                pixels[idx + 1] = 255; // G
                pixels[idx + 2] = 255; // B
                pixels[idx + 3] = alpha_byte; // A
            }
        }
    }

    /// Generate a lines pattern texture
    fn generate_lines(&self, pixels: &mut [u8], size: usize, spacing: f32, angle: f32) {
        let spacing_px = (spacing * size as f32 / 100.0).max(4.0);
        let thickness = spacing_px * 0.2;
        let half_thickness = thickness / 2.0;

        let cos_angle = angle.cos();
        let sin_angle = angle.sin();

        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;

                let fx = x as f32;
                let fy = y as f32;

                // Rotate position by angle
                let rotated_x = fx * cos_angle - fy * sin_angle;

                // Calculate position within pattern
                let pattern_pos = (rotated_x % spacing_px) / spacing_px;

                // Distance from line center (0.5 is the center of each stripe)
                let dist_from_line = (pattern_pos - 0.5).abs() * spacing_px;

                // Anti-aliased line
                let alpha = if dist_from_line < half_thickness - 1.0 {
                    1.0
                } else if dist_from_line < half_thickness + 1.0 {
                    1.0 - (dist_from_line - (half_thickness - 1.0)) / 2.0
                } else {
                    0.0
                };

                let alpha_byte = (alpha * 255.0) as u8;
                pixels[idx] = 255; // R
                pixels[idx + 1] = 255; // G
                pixels[idx + 2] = 255; // B
                pixels[idx + 3] = alpha_byte; // A
            }
        }
    }

    /// Generate a crosshatch pattern texture
    fn generate_crosshatch(&self, pixels: &mut [u8], size: usize, spacing: f32) {
        let spacing_px = (spacing * size as f32 / 100.0).max(4.0);
        let thickness = spacing_px * 0.2;
        let half_thickness = thickness / 4.0;

        for y in 0..size {
            for x in 0..size {
                let idx = (y * size + x) * 4;

                let fx = x as f32;
                let fy = y as f32;

                // Horizontal lines
                let h_pos = (fy % spacing_px) / spacing_px;
                let h_dist = (h_pos - 0.5).abs() * spacing_px;
                let h_alpha = if h_dist < half_thickness {
                    1.0
                } else if h_dist < half_thickness + 1.0 {
                    1.0 - (h_dist - half_thickness)
                } else {
                    0.0
                };

                // Vertical lines
                let v_pos = (fx % spacing_px) / spacing_px;
                let v_dist = (v_pos - 0.5).abs() * spacing_px;
                let v_alpha = if v_dist < half_thickness {
                    1.0
                } else if v_dist < half_thickness + 1.0 {
                    1.0 - (v_dist - half_thickness)
                } else {
                    0.0
                };

                // Combine both directions
                let alpha = h_alpha.max(v_alpha);

                let alpha_byte = (alpha * 255.0) as u8;
                pixels[idx] = 255; // R
                pixels[idx + 1] = 255; // G
                pixels[idx + 2] = 255; // B
                pixels[idx + 3] = alpha_byte; // A
            }
        }
    }

    /// Create a wgpu texture from the generated pattern
    pub fn create_texture(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pattern: &Pattern,
    ) -> wgpu::Texture {
        let pixels = self.generate_pattern_texture(pattern);
        let size = self.resolution.size();

        let texture_size = wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Pattern Texture {:?}", pattern)),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            texture_size,
        );

        texture
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_resolution_sizes() {
        assert_eq!(TextureResolution::Low.size(), 128);
        assert_eq!(TextureResolution::Medium.size(), 256);
        assert_eq!(TextureResolution::High.size(), 512);
    }

    #[test]
    fn test_generate_solid_texture() {
        let generator = TexturePatternGenerator::new(TextureResolution::Low);
        let pixels = generator.generate_pattern_texture(&Pattern::Solid);

        assert_eq!(pixels.len(), 128 * 128 * 4);

        // All pixels should be white
        for chunk in pixels.chunks(4) {
            assert_eq!(chunk, &[255, 255, 255, 255]);
        }
    }

    #[test]
    fn test_generate_dots_texture() {
        let generator = TexturePatternGenerator::new(TextureResolution::Low);
        let pixels = generator.generate_pattern_texture(&Pattern::Dots { spacing: 8.0 });

        assert_eq!(pixels.len(), 128 * 128 * 4);

        // Should have some transparent pixels (background) and some opaque pixels (dots)
        let has_transparent = pixels.chunks(4).any(|chunk| chunk[3] < 128);
        let has_opaque = pixels.chunks(4).any(|chunk| chunk[3] > 200);

        assert!(has_transparent, "Should have transparent background");
        assert!(has_opaque, "Should have opaque dots");
    }

    #[test]
    fn test_generate_lines_texture() {
        let generator = TexturePatternGenerator::new(TextureResolution::Low);
        let pixels = generator.generate_pattern_texture(&Pattern::Lines {
            spacing: 6.0,
            angle: 0.0,
        });

        assert_eq!(pixels.len(), 128 * 128 * 4);

        // Should have variation in alpha channel
        let has_transparent = pixels.chunks(4).any(|chunk| chunk[3] < 128);
        let has_opaque = pixels.chunks(4).any(|chunk| chunk[3] > 200);

        assert!(has_transparent, "Should have transparent background");
        assert!(has_opaque, "Should have opaque lines");
    }

    #[test]
    fn test_generate_crosshatch_texture() {
        let generator = TexturePatternGenerator::new(TextureResolution::Low);
        let pixels = generator.generate_pattern_texture(&Pattern::Crosshatch { spacing: 8.0 });

        assert_eq!(pixels.len(), 128 * 128 * 4);

        // Should have variation in alpha channel
        let has_transparent = pixels.chunks(4).any(|chunk| chunk[3] < 128);
        let has_opaque = pixels.chunks(4).any(|chunk| chunk[3] > 200);

        assert!(has_transparent, "Should have transparent background");
        assert!(has_opaque, "Should have opaque crosshatch");
    }
}
