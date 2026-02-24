// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Utilities for visual regression testing of pattern rendering.
//!
//! This module provides infrastructure for screenshot-based testing to catch
//! visual regressions in pattern rendering that unit tests might miss.

use gup::context::GupContext;
use gup::error::GupResult;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Configuration for visual regression tests.
#[derive(Debug, Clone)]
pub struct VisualTestConfig {
    /// Width of the test render surface
    pub width: u32,
    /// Height of the test render surface
    pub height: u32,
    /// Maximum acceptable pixel difference (0.0-1.0 per channel)
    pub pixel_tolerance: f32,
    /// Maximum percentage of pixels that can differ
    pub pixel_diff_threshold: f32,
    /// Directory for storing reference images
    pub reference_dir: PathBuf,
    /// Directory for storing test outputs
    pub output_dir: PathBuf,
    /// Directory for storing diff images
    pub diff_dir: PathBuf,
}

impl Default for VisualTestConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            pixel_tolerance: 0.02,      // 2% per channel (~5/255)
            pixel_diff_threshold: 0.01, // 1% of pixels can differ
            reference_dir: PathBuf::from("tests/visual_references"),
            output_dir: PathBuf::from("target/visual_test_outputs"),
            diff_dir: PathBuf::from("target/visual_test_diffs"),
        }
    }
}

impl VisualTestConfig {
    /// Create directories if they don't exist.
    pub fn ensure_directories(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.reference_dir)?;
        std::fs::create_dir_all(&self.output_dir)?;
        std::fs::create_dir_all(&self.diff_dir)?;
        Ok(())
    }

    /// Get the path for a reference image.
    pub fn reference_path(&self, name: &str) -> PathBuf {
        self.reference_dir.join(format!("{}.png", name))
    }

    /// Get the path for an output image.
    pub fn output_path(&self, name: &str) -> PathBuf {
        self.output_dir.join(format!("{}.png", name))
    }

    /// Get the path for a diff image.
    pub fn diff_path(&self, name: &str) -> PathBuf {
        self.diff_dir.join(format!("{}.png", name))
    }
}

/// Result of a visual comparison.
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// Whether the test passed
    pub passed: bool,
    /// Percentage of pixels that differed (0.0-1.0)
    pub pixel_diff_percentage: f32,
    /// Maximum pixel difference found (0.0-1.0 per channel)
    pub max_pixel_diff: f32,
    /// Number of pixels that differed
    pub differing_pixels: usize,
    /// Total number of pixels
    pub total_pixels: usize,
}

impl ComparisonResult {
    /// Create a passing result.
    pub fn pass(total_pixels: usize) -> Self {
        Self {
            passed: true,
            pixel_diff_percentage: 0.0,
            max_pixel_diff: 0.0,
            differing_pixels: 0,
            total_pixels,
        }
    }

    /// Create a failing result.
    pub fn fail(
        pixel_diff_percentage: f32,
        max_pixel_diff: f32,
        differing_pixels: usize,
        total_pixels: usize,
    ) -> Self {
        Self {
            passed: false,
            pixel_diff_percentage,
            max_pixel_diff,
            differing_pixels,
            total_pixels,
        }
    }
}

impl std::fmt::Display for ComparisonResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.passed {
            write!(f, "PASS - Visual test passed")
        } else {
            write!(
                f,
                "FAIL - {:.2}% pixels differ (max diff: {:.2}%), {} / {} pixels",
                self.pixel_diff_percentage * 100.0,
                self.max_pixel_diff * 100.0,
                self.differing_pixels,
                self.total_pixels
            )
        }
    }
}

/// Helper for rendering tests to offscreen textures.
pub struct VisualTestRenderer {
    context: Arc<GupContext>,
    config: VisualTestConfig,
    texture: wgpu::Texture,
    output_buffer: wgpu::Buffer,
}

impl VisualTestRenderer {
    /// Create a new visual test renderer.
    pub async fn new(config: VisualTestConfig) -> GupResult<Self> {
        config
            .ensure_directories()
            .map_err(|e| gup::error::GupError::FileError {
                path: "test directories".to_string(),
                error: format!("Failed to create test directories: {}", e),
            })?;

        let context = GupContext::headless().await?;

        // Create an offscreen texture for rendering
        let texture = context.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("visual_test_texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // Create a buffer to copy the texture data to CPU
        // Note: bytes_per_row must be aligned to COPY_BYTES_PER_ROW_ALIGNMENT (256)
        let unpadded_bytes_per_row = config.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = (padded_bytes_per_row * config.height) as u64;

        let output_buffer = context.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("visual_test_output_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Ok(Self {
            context,
            config,
            texture,
            output_buffer,
        })
    }

    /// Get the texture view for rendering.
    pub fn texture_view(&self) -> wgpu::TextureView {
        self.texture
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    /// Get the render context.
    pub fn context(&self) -> &Arc<GupContext> {
        &self.context
    }

    /// Get the test configuration.
    #[allow(dead_code)]
    pub fn config(&self) -> &VisualTestConfig {
        &self.config
    }

    /// Capture the current texture to a PNG file and compare with reference.
    pub async fn capture_and_compare(&self, test_name: &str) -> GupResult<ComparisonResult> {
        // Copy texture to buffer
        let mut encoder =
            self.context
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("visual_test_capture"),
                });

        // Calculate padded bytes per row for alignment
        let unpadded_bytes_per_row = self.config.width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.config.height),
                },
            },
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );

        self.context.queue.submit(Some(encoder.finish()));

        // Map the buffer and read the data
        let buffer_slice = self.output_buffer.slice(..);
        let (tx, rx) = futures_channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        let _ = self.context.device.poll(wgpu::PollType::Wait);
        rx.await
            .map_err(|_| gup::error::GupError::resource_error("Failed to map buffer".to_string()))?
            .map_err(|e| {
                gup::error::GupError::resource_error(format!("Buffer mapping failed: {:?}", e))
            })?;

        let data = buffer_slice.get_mapped_range();

        // Copy data, removing padding
        let mut image_data =
            Vec::with_capacity((self.config.width * self.config.height * 4) as usize);
        for row in 0..self.config.height {
            let row_start = (row * padded_bytes_per_row) as usize;
            let row_end = row_start + (self.config.width * 4) as usize;
            image_data.extend_from_slice(&data[row_start..row_end]);
        }

        drop(data);
        self.output_buffer.unmap();

        // Save the captured image
        let output_path = self.config.output_path(test_name);
        self.save_image(&image_data, &output_path)?;

        // Compare with reference
        let reference_path = self.config.reference_path(test_name);
        if !reference_path.exists() {
            // No reference image yet - this is the first run
            // Copy the output as the reference
            std::fs::copy(&output_path, &reference_path).map_err(|e| {
                gup::error::GupError::FileError {
                    path: reference_path.display().to_string(),
                    error: format!("Failed to create reference image: {}", e),
                }
            })?;
            println!(
                "Created reference image for '{}' at {:?}",
                test_name, reference_path
            );
            Ok(ComparisonResult::pass(
                (self.config.width * self.config.height) as usize,
            ))
        } else {
            // Load reference and compare
            self.compare_with_reference(test_name, &image_data, &reference_path)
        }
    }

    /// Save image data to a PNG file.
    fn save_image(&self, data: &[u8], path: &Path) -> GupResult<()> {
        image::save_buffer(
            path,
            data,
            self.config.width,
            self.config.height,
            image::ColorType::Rgba8,
        )
        .map_err(|e| gup::error::GupError::FileError {
            path: path.display().to_string(),
            error: format!("Failed to save image: {}", e),
        })
    }

    /// Compare captured image with reference.
    fn compare_with_reference(
        &self,
        test_name: &str,
        captured_data: &[u8],
        reference_path: &Path,
    ) -> GupResult<ComparisonResult> {
        // Load reference image
        let reference_img = image::open(reference_path)
            .map_err(|e| gup::error::GupError::FileError {
                path: reference_path.display().to_string(),
                error: format!("Failed to load reference image: {}", e),
            })?
            .to_rgba8();

        let reference_data = reference_img.as_raw();

        // Check dimensions match
        if reference_img.width() != self.config.width
            || reference_img.height() != self.config.height
        {
            return Err(gup::error::GupError::validation_error(format!(
                "Reference image dimensions ({}x{}) don't match test dimensions ({}x{})",
                reference_img.width(),
                reference_img.height(),
                self.config.width,
                self.config.height
            )));
        }

        // Compare pixels
        let total_pixels = (self.config.width * self.config.height) as usize;
        let mut differing_pixels = 0;
        let mut max_diff = 0.0f32;
        let mut diff_image = vec![0u8; captured_data.len()];

        for i in 0..total_pixels {
            let base = i * 4;
            let mut pixel_diff = 0.0f32;

            for c in 0..3 {
                // RGB channels only
                let captured = captured_data[base + c] as f32 / 255.0;
                let reference = reference_data[base + c] as f32 / 255.0;
                let channel_diff = (captured - reference).abs();
                pixel_diff = pixel_diff.max(channel_diff);

                // Create diff visualization (red where different)
                if channel_diff > self.config.pixel_tolerance {
                    diff_image[base] = 255; // R
                    diff_image[base + 1] = 0; // G
                    diff_image[base + 2] = 0; // B
                } else {
                    // Show original in grayscale where similar
                    let gray = (captured * 255.0) as u8;
                    diff_image[base] = gray;
                    diff_image[base + 1] = gray;
                    diff_image[base + 2] = gray;
                }
            }
            diff_image[base + 3] = 255; // Alpha

            max_diff = max_diff.max(pixel_diff);

            if pixel_diff > self.config.pixel_tolerance {
                differing_pixels += 1;
            }
        }

        let pixel_diff_percentage = differing_pixels as f32 / total_pixels as f32;

        // Save diff image if there are differences
        if differing_pixels > 0 {
            let diff_path = self.config.diff_path(test_name);
            self.save_image(&diff_image, &diff_path)?;
        }

        // Determine pass/fail
        let passed = pixel_diff_percentage <= self.config.pixel_diff_threshold;

        Ok(if passed {
            ComparisonResult::pass(total_pixels)
        } else {
            ComparisonResult::fail(
                pixel_diff_percentage,
                max_diff,
                differing_pixels,
                total_pixels,
            )
        })
    }
}

/// Assert that a visual test passes.
#[macro_export]
macro_rules! assert_visual_match {
    ($result:expr, $test_name:expr) => {
        if !$result.passed {
            panic!(
                "Visual regression test '{}' failed:\n  {}\n  \
                Reference: {:?}\n  Output: {:?}\n  Diff: {:?}",
                $test_name,
                $result,
                $result
                    .config
                    .as_ref()
                    .map(|c| c.reference_path($test_name)),
                $result.config.as_ref().map(|c| c.output_path($test_name)),
                $result.config.as_ref().map(|c| c.diff_path($test_name)),
            );
        }
    };
}
