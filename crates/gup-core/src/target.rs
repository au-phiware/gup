// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Render targets (RFC-001 §7). A [`RenderTarget`] hands out a [`Frame`]
//! to draw into and presents it; [`Renderer::render`] draws a [`Scene`]
//! into any of them with the same single render pass:
//!
//! - [`ImageTarget`]: an offscreen texture read back to an
//!   [`image::RgbaImage`];
//! - `WindowTarget` (feature `window`): a winit window's surface.
//!
//! Texture and draw-in-pass targets are RFC-001 S3.

use crate::context::Context;
use crate::error::{Error, Result};
use crate::render::{Renderer, TargetDesc};
use crate::scene::Scene;
use std::path::Path;

/// Something a [`Renderer`] can draw a [`Scene`] into.
pub trait RenderTarget {
    /// What pipelines rendering to this target are keyed on.
    fn desc(&self) -> TargetDesc;
    /// The texture to draw the next frame into.
    fn acquire(&mut self, cx: &Context) -> Result<Frame>;
    /// Submit `commands` (which draw into `frame`) and show or keep the
    /// result.
    fn present(&mut self, cx: &Context, frame: Frame, commands: wgpu::CommandBuffer) -> Result<()>;
}

/// One frame of a [`RenderTarget`]: the texture and the (non-sRGB) view
/// to render into.
#[derive(Debug)]
pub struct Frame {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Set for window frames; presenting consumes it.
    #[cfg_attr(not(feature = "window"), allow(dead_code))]
    pub(crate) surface: Option<wgpu::SurfaceTexture>,
}

impl Frame {
    pub(crate) fn new(
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        surface: Option<wgpu::SurfaceTexture>,
    ) -> Self {
        Self {
            texture,
            view,
            surface,
        }
    }

    /// The view to attach as the pass's colour target.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// The texture behind [`view`](Self::view).
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

/// A copy of a rendered texture on its way to the CPU.
#[derive(Debug)]
pub(crate) struct Readback {
    buffer: wgpu::Buffer,
    bytes_per_row: u32,
    width: u32,
    height: u32,
    /// The texture stores blue first (window surfaces).
    bgra: bool,
}

impl Readback {
    /// Record a copy of `texture` into a new mappable buffer.
    pub(crate) fn encode(
        cx: &Context,
        texture: &wgpu::Texture,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Self> {
        use wgpu::TextureFormat as F;
        let bgra = match texture.format() {
            F::Rgba8Unorm | F::Rgba8UnormSrgb => false,
            F::Bgra8Unorm | F::Bgra8UnormSrgb => true,
            other => {
                return Err(Error::config(
                    "readback",
                    format!("cannot read back {other:?} textures; only 8-bit RGBA or BGRA"),
                ));
            }
        };
        let (width, height) = (texture.width(), texture.height());
        let bytes_per_row = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = cx.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("gup readback"),
            size: u64::from(bytes_per_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            texture.size(),
        );
        Ok(Self {
            buffer,
            bytes_per_row,
            width,
            height,
            bgra,
        })
    }

    /// Map the buffer (after the copy was submitted) and convert to
    /// straight-alpha RGBA.
    pub(crate) async fn read(self, cx: &Context) -> Result<image::RgbaImage> {
        let (tx, rx) = futures_channel::oneshot::channel();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        // Native backends need a poll to run the callback; on the web the
        // browser drives it.
        #[cfg(not(target_arch = "wasm32"))]
        cx.wait_idle()?;
        #[cfg(target_arch = "wasm32")]
        let _ = cx;
        rx.await
            .map_err(|_| Error::Readback("map callback dropped".into()))?
            .map_err(|e| Error::Readback(e.to_string()))?;

        let (w, h) = (self.width, self.height);
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        {
            let mapped = self.buffer.slice(..).get_mapped_range();
            for row in mapped.chunks_exact(self.bytes_per_row as usize) {
                for px in row[..(w * 4) as usize].chunks_exact(4) {
                    let rgba = if self.bgra {
                        [px[2], px[1], px[0], px[3]]
                    } else {
                        [px[0], px[1], px[2], px[3]]
                    };
                    pixels.extend_from_slice(&unpremultiply(rgba));
                }
            }
        }
        self.buffer.unmap();
        image::RgbaImage::from_raw(w, h, pixels)
            .ok_or_else(|| Error::Readback("readback size mismatch".into()))
    }
}

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
    /// The copy of the last presented frame.
    pending: Option<Readback>,
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
            pending: None,
        })
    }

    /// Draw `scene` with a fresh [`Renderer`] and read the pixels back.
    pub async fn render(&mut self, cx: &Context, scene: &Scene) -> Result<image::RgbaImage> {
        Renderer::new().render(cx, scene, self)?;
        self.read(cx).await
    }

    /// The pixels of the last presented frame.
    pub async fn read(&mut self, cx: &Context) -> Result<image::RgbaImage> {
        let readback = self.pending.take().ok_or_else(|| {
            Error::config(
                "image target",
                "nothing to read: render a scene into the target first",
            )
        })?;
        readback.read(cx).await
    }

    /// Blocking form of [`ImageTarget::render`] (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_blocking(&mut self, cx: &Context, scene: &Scene) -> Result<image::RgbaImage> {
        pollster::block_on(self.render(cx, scene))
    }
}

impl RenderTarget for ImageTarget {
    fn desc(&self) -> TargetDesc {
        self.desc
    }

    fn acquire(&mut self, _cx: &Context) -> Result<Frame> {
        Ok(Frame::new(self.texture.clone(), self.view.clone(), None))
    }

    fn present(&mut self, cx: &Context, frame: Frame, commands: wgpu::CommandBuffer) -> Result<()> {
        let mut encoder = cx
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gup image readback"),
            });
        let readback = Readback::encode(cx, frame.texture(), &mut encoder)?;
        cx.queue().submit([commands, encoder.finish()]);
        self.pending = Some(readback);
        Ok(())
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
