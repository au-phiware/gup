// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Render targets. S0a has [`ImageTarget`] (offscreen texture + async
//! readback); `WindowTarget` is S0b and the `RenderTarget` trait with
//! texture and draw-in-pass targets is RFC-001 S3.

use crate::context::Context;
use crate::error::{Error, Result};
use crate::render::{TargetDesc, encode_scene};
use crate::scene::Scene;
use std::path::Path;

/// An offscreen colour target that reads back to an [`image::RgbaImage`].
///
/// Renders into an `Rgba8Unorm` (non-sRGB) texture: colours are
/// sRGB-encoded and blended in sRGB space, as browsers, SVG and PDF do
/// (RFC-001 §7). The readback un-premultiplies alpha.
#[derive(Debug)]
pub struct ImageTarget {
    desc: TargetDesc,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl ImageTarget {
    /// A `width × height` physical-pixel target.
    pub fn new(cx: &Context, width: u32, height: u32) -> Result<Self> {
        let max = cx.caps().limits.max_texture_dimension_2d;
        if width == 0 || height == 0 || width > max || height > max {
            return Err(Error::config(
                "image target size",
                format!("{width}×{height} must be between 1 and {max} pixels on each side"),
            ));
        }
        let desc = TargetDesc {
            format: wgpu::TextureFormat::Rgba8Unorm,
            width,
            height,
            samples: 1,
        };
        let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("gup image target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: desc.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Ok(Self {
            desc,
            texture,
            view,
        })
    }

    /// What pipelines rendering to this target are keyed on.
    pub fn desc(&self) -> TargetDesc {
        self.desc
    }

    /// Draw `scene` and read the pixels back.
    pub async fn render(&self, cx: &Context, scene: &Scene) -> Result<image::RgbaImage> {
        let (w, h) = (self.desc.width, self.desc.height);
        let bytes_per_row = (w * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let readback = cx.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("gup image readback"),
            size: u64::from(bytes_per_row) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = cx
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gup image target"),
            });
        encode_scene(cx, scene, &self.desc, &self.view, &mut encoder)?;
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(h),
                },
            },
            self.texture.size(),
        );
        cx.queue().submit([encoder.finish()]);

        let (tx, rx) = futures_channel::oneshot::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        // Native backends need a poll to run the callback; on the web the
        // browser drives it.
        #[cfg(not(target_arch = "wasm32"))]
        cx.wait_idle()?;
        rx.await
            .map_err(|_| Error::Readback("map callback dropped".into()))?
            .map_err(|e| Error::Readback(e.to_string()))?;

        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        {
            let mapped = readback.slice(..).get_mapped_range();
            for row in mapped.chunks_exact(bytes_per_row as usize) {
                for px in row[..(w * 4) as usize].chunks_exact(4) {
                    pixels.extend_from_slice(&unpremultiply([px[0], px[1], px[2], px[3]]));
                }
            }
        }
        readback.unmap();
        image::RgbaImage::from_raw(w, h, pixels)
            .ok_or_else(|| Error::Readback("readback size mismatch".into()))
    }

    /// Blocking form of [`ImageTarget::render`] (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_blocking(&self, cx: &Context, scene: &Scene) -> Result<image::RgbaImage> {
        pollster::block_on(self.render(cx, scene))
    }
}

/// Straight alpha from premultiplied, rounding to nearest.
fn unpremultiply([r, g, b, a]: [u8; 4]) -> [u8; 4] {
    match a {
        0 => [0, 0, 0, 0],
        255 => [r, g, b, a],
        _ => {
            let f = |c: u8| ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8;
            [f(r), f(g), f(b), a]
        }
    }
}

/// Write `image` to `path` as PNG.
pub fn save_png(image: &image::RgbaImage, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|source| Error::Io {
            path: dir.display().to_string(),
            source,
        })?;
    }
    image.save_with_format(path, image::ImageFormat::Png)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpremultiply_restores_straight_alpha() {
        assert_eq!(unpremultiply([0, 0, 0, 0]), [0, 0, 0, 0]);
        assert_eq!(unpremultiply([10, 20, 30, 255]), [10, 20, 30, 255]);
        assert_eq!(unpremultiply([64, 0, 128, 128]), [128, 0, 255, 128]);
    }
}
