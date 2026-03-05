// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! PNG export via off-screen GPU rendering.
//!
//! This module provides utilities for rendering a Gup chart to a PNG image by
//! performing an off-screen GPU render, reading the pixel data back through a
//! staging buffer, and encoding it with the [`image`] crate.
//!
//! # Row-Padding
//!
//! wgpu requires that `bytes_per_row` in texture-to-buffer copies is a multiple
//! of [`COPY_BYTES_PER_ROW_ALIGNMENT`](wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) (256
//! bytes). For textures whose `width * 4` is not a multiple of 256 the GPU
//! buffer contains padding bytes at the end of each row that must be stripped
//! before PNG encoding.
//!
//! # Example
//!
//! ```rust,ignore
//! use gup::export::png;
//!
//! let png_bytes = png::readback_texture_as_png(
//!     &device, &queue, &texture, width, height,
//! )?;
//! std::fs::write("chart.png", &png_bytes)?;
//! ```

use crate::error::{GupError, GupResult};

/// wgpu requires `bytes_per_row` to be a multiple of this value.
const COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

/// Number of bytes per pixel (RGBA8 / BGRA8).
const BYTES_PER_PIXEL: u32 = 4;

// ---------------------------------------------------------------------------
// Row-padding helpers
// ---------------------------------------------------------------------------

/// Compute the padded `bytes_per_row` value that satisfies wgpu's alignment
/// requirement for a texture of the given `width` (in pixels).
///
/// The unpadded row size is `width * 4` (RGBA, one byte per channel). The
/// result is rounded up to the next multiple of
/// `COPY_BYTES_PER_ROW_ALIGNMENT`.
pub fn padded_bytes_per_row(width: u32) -> u32 {
    let unpadded = width * BYTES_PER_PIXEL;
    let align = COPY_BYTES_PER_ROW_ALIGNMENT;
    // Round up: (unpadded + align - 1) / align * align
    unpadded.div_ceil(align) * align
}

/// Strip wgpu row padding from a pixel buffer.
///
/// Given a buffer where each row is `padded_row_bytes` wide, this function
/// copies only the first `width * 4` bytes of each row into a tightly-packed
/// output buffer suitable for PNG encoding.
///
/// If the padded and unpadded row sizes are identical no copy is performed
/// and the input data is returned as-is.
pub fn strip_row_padding(data: &[u8], width: u32, height: u32, padded_row_bytes: u32) -> Vec<u8> {
    let unpadded = (width * BYTES_PER_PIXEL) as usize;
    let padded = padded_row_bytes as usize;

    // Fast path: no padding to strip.
    if unpadded == padded {
        return data.to_vec();
    }

    let mut out = Vec::with_capacity(unpadded * height as usize);
    for row in 0..height as usize {
        let start = row * padded;
        let end = start + unpadded;
        if end <= data.len() {
            out.extend_from_slice(&data[start..end]);
        }
    }
    out
}

/// Convert BGRA pixel data to RGBA in place.
///
/// wgpu's default surface format is `Bgra8UnormSrgb`. PNG requires RGBA
/// channel order, so we swap the R and B channels for every pixel.
pub fn bgra_to_rgba(data: &mut [u8]) {
    for pixel in data.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
}

// ---------------------------------------------------------------------------
// PNG encoding
// ---------------------------------------------------------------------------

/// Encode raw RGBA pixel data as a PNG image.
///
/// `pixels` must contain exactly `width * height * 4` bytes in RGBA order.
/// Returns the PNG file bytes as a `Vec<u8>`.
pub fn encode_png(pixels: &[u8], width: u32, height: u32) -> GupResult<Vec<u8>> {
    let expected = (width as usize) * (height as usize) * (BYTES_PER_PIXEL as usize);
    if pixels.len() != expected {
        return Err(GupError::BufferSizeMismatch {
            expected,
            actual: pixels.len(),
        });
    }

    let mut png_bytes: Vec<u8> = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut png_bytes));
        image::ImageEncoder::write_image(encoder, pixels, width, height, image::ColorType::Rgba8)
            .map_err(|e| GupError::RenderError {
            message: format!("PNG encoding failed: {e}"),
        })?;
    }

    Ok(png_bytes)
}

// ---------------------------------------------------------------------------
// GPU texture readback
// ---------------------------------------------------------------------------

/// Read back a GPU texture's pixel data, strip row padding, convert from
/// BGRA to RGBA, and encode as PNG.
///
/// The texture must have been created with
/// [`TextureUsages::COPY_SRC`](wgpu::TextureUsages::COPY_SRC).
///
/// This is a **blocking** operation that polls the GPU device until the
/// staging buffer is mapped. It is not suitable for use on WASM targets.
pub fn readback_texture_as_png(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> GupResult<Vec<u8>> {
    let pixels = readback_texture(device, queue, texture, width, height)?;
    encode_png(&pixels, width, height)
}

/// Read back a GPU texture's pixel data as tightly-packed RGBA bytes.
///
/// Handles wgpu row-padding alignment and BGRA→RGBA conversion.
pub fn readback_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
) -> GupResult<Vec<u8>> {
    let padded_row = padded_bytes_per_row(width);
    let buffer_size = (padded_row * height) as u64;

    // Create a staging buffer for CPU readback.
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("png_export_staging_buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    // Encode a copy from texture → staging buffer.
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("png_export_copy_encoder"),
    });

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &staging_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    // Map the staging buffer for reading.
    let buffer_slice = staging_buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });

    // Block until the GPU has finished the copy.
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    receiver
        .recv()
        .map_err(|_| GupError::RenderError {
            message: "Staging buffer mapping callback was dropped".to_string(),
        })?
        .map_err(|e| GupError::RenderError {
            message: format!("Failed to map staging buffer: {e:?}"),
        })?;

    let mapped = buffer_slice.get_mapped_range();
    let mut pixels = strip_row_padding(&mapped, width, height, padded_row);
    drop(mapped);
    staging_buffer.unmap();

    // Convert from BGRA (wgpu default) to RGBA (PNG).
    bgra_to_rgba(&mut pixels);

    Ok(pixels)
}

// ---------------------------------------------------------------------------
// Off-screen render target
// ---------------------------------------------------------------------------

/// A temporary off-screen render target backed by a wgpu texture.
///
/// Create one, render into [`view()`](Self::view), then call
/// [`readback_as_png`](Self::readback_as_png) to get the PNG bytes.
/// All GPU resources are released when the struct is dropped.
pub struct OffscreenTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl OffscreenTarget {
    /// Create a new off-screen render target at the given pixel dimensions.
    ///
    /// The texture format is `Bgra8UnormSrgb` (matching wgpu's default
    /// surface format) with `RENDER_ATTACHMENT | COPY_SRC` usage.
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("png_export_offscreen_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            width,
            height,
        }
    }

    /// Borrow the [`TextureView`](wgpu::TextureView) to use as a render
    /// attachment.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Pixel width of the render target.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Pixel height of the render target.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Read back the rendered pixels and encode them as PNG.
    ///
    /// Call this **after** submitting the render commands that target
    /// [`view()`](Self::view).
    pub fn readback_as_png(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> GupResult<Vec<u8>> {
        readback_texture_as_png(device, queue, &self.texture, self.width, self.height)
    }

    /// Read back the rendered pixels as tightly-packed RGBA bytes.
    pub fn readback_pixels(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> GupResult<Vec<u8>> {
        readback_texture(device, queue, &self.texture, self.width, self.height)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- padded_bytes_per_row ------------------------------------------

    #[test]
    fn test_padded_bytes_per_row_aligned() {
        // 64 pixels → 256 bytes, already aligned.
        assert_eq!(padded_bytes_per_row(64), 256);
        // 128 → 512.
        assert_eq!(padded_bytes_per_row(128), 512);
    }

    #[test]
    fn test_padded_bytes_per_row_unaligned() {
        // 1 pixel → 4 bytes, padded to 256.
        assert_eq!(padded_bytes_per_row(1), 256);
        // 100 pixels → 400 bytes, padded to 512.
        assert_eq!(padded_bytes_per_row(100), 512);
        // 65 → 260 → 512.
        assert_eq!(padded_bytes_per_row(65), 512);
    }

    #[test]
    fn test_padded_bytes_per_row_large() {
        // 800 pixels → 3200 bytes, padded to 3328 (13 * 256).
        assert_eq!(padded_bytes_per_row(800), 3328);
        // 1920 → 7680, already aligned.
        assert_eq!(padded_bytes_per_row(1920), 7680);
    }

    // --- strip_row_padding --------------------------------------------

    #[test]
    fn test_strip_row_padding_no_padding() {
        // 64-pixel wide image, unpadded = padded = 256.
        let width = 64u32;
        let height = 2u32;
        let row_bytes = 256u32;
        let data = vec![0xABu8; (row_bytes * height) as usize];
        let result = strip_row_padding(&data, width, height, row_bytes);
        assert_eq!(result.len(), (width * 4 * height) as usize);
    }

    #[test]
    fn test_strip_row_padding_with_padding() {
        // 3-pixel wide image: unpadded = 12, padded = 256.
        let width = 3u32;
        let height = 2u32;
        let padded = 256u32;
        let unpadded = (width * 4) as usize;

        // Fill with recognisable data: row N uses byte value N+1.
        let mut data = vec![0u8; (padded * height) as usize];
        for row in 0..height as usize {
            for i in 0..unpadded {
                data[row * padded as usize + i] = (row + 1) as u8;
            }
        }

        let result = strip_row_padding(&data, width, height, padded);
        assert_eq!(result.len(), unpadded * height as usize);
        // First row: all 1s.
        assert!(result[..unpadded].iter().all(|&b| b == 1));
        // Second row: all 2s.
        assert!(result[unpadded..].iter().all(|&b| b == 2));
    }

    #[test]
    fn test_strip_row_padding_single_pixel() {
        let width = 1u32;
        let height = 1u32;
        let padded = 256u32;
        let mut data = vec![0u8; padded as usize];
        data[0] = 0xFF;
        data[1] = 0xFE;
        data[2] = 0xFD;
        data[3] = 0xFC;

        let result = strip_row_padding(&data, width, height, padded);
        assert_eq!(result, &[0xFF, 0xFE, 0xFD, 0xFC]);
    }

    // --- bgra_to_rgba -------------------------------------------------

    #[test]
    fn test_bgra_to_rgba() {
        let mut data = vec![
            0, 128, 255, 200, // B=0, G=128, R=255, A=200
            10, 20, 30, 40, // B=10, G=20, R=30, A=40
        ];
        bgra_to_rgba(&mut data);
        assert_eq!(
            data,
            vec![
                255, 128, 0, 200, // R=255, G=128, B=0, A=200
                30, 20, 10, 40, // R=30, G=20, B=10, A=40
            ]
        );
    }

    #[test]
    fn test_bgra_to_rgba_identity() {
        // When R == B, swapping is a no-op for those channels.
        let mut data = vec![42, 100, 42, 255];
        bgra_to_rgba(&mut data);
        assert_eq!(data, vec![42, 100, 42, 255]);
    }

    // --- encode_png ---------------------------------------------------

    #[test]
    fn test_encode_png_basic() {
        let width = 2u32;
        let height = 2u32;
        // 4 red pixels.
        let pixels = vec![
            255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        ];
        let png = encode_png(&pixels, width, height).unwrap();

        // PNG magic bytes.
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn test_encode_png_size_mismatch() {
        let result = encode_png(&[0u8; 10], 2, 2);
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_png_round_trip() {
        let width = 4u32;
        let height = 3u32;
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        // Some recognisable pattern.
        for (i, pixel) in pixels.chunks_exact_mut(4).enumerate() {
            pixel[0] = (i * 21) as u8; // R
            pixel[1] = (i * 37) as u8; // G
            pixel[2] = (i * 53) as u8; // B
            pixel[3] = 255; // A
        }

        let png = encode_png(&pixels, width, height).unwrap();

        // Decode with the image crate and verify dimensions.
        let decoded =
            image::load_from_memory_with_format(&png, image::ImageFormat::Png).expect("decode PNG");
        assert_eq!(decoded.width(), width);
        assert_eq!(decoded.height(), height);

        // Verify pixel data round-trips.
        let rgba = decoded.to_rgba8();
        assert_eq!(rgba.as_raw(), &pixels);
    }

    #[test]
    fn test_encode_png_transparent() {
        let width = 1u32;
        let height = 1u32;
        // Fully transparent pixel.
        let pixels = vec![0, 0, 0, 0];
        let png = encode_png(&pixels, width, height).unwrap();

        let decoded =
            image::load_from_memory_with_format(&png, image::ImageFormat::Png).expect("decode PNG");
        let rgba = decoded.to_rgba8();
        assert_eq!(rgba.as_raw(), &pixels);
    }
}
