// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Render targets (RFC-001 §7). A [`RenderTarget`] hands out a [`Frame`]
//! to draw into and presents it; [`Renderer::render`] draws a [`Scene`]
//! into any of them with the same single render pass:
//!
//! - [`ImageTarget`]: an offscreen texture read back to an
//!   [`image::RgbaImage`];
//! - [`TextureTarget`]: a texture the host owns (to show in egui, bevy or
//!   its own UI);
//! - `WindowTarget` (feature `window`): a winit window's surface.
//!
//! Hosts that own the render pass itself skip targets altogether and use
//! [`Renderer::prepare`] and [`Prepared::draw`](crate::Prepared::draw)
//! (draw-in-pass). Gup-owned passes are multisampled (4× by default) and
//! resolve into the frame's texture.

use crate::context::Context;
use crate::error::{Error, Result};
use crate::render::{Renderer, TargetDesc};
use crate::scene::Scene;
use std::path::Path;

/// Something a [`Renderer`] can draw a [`Scene`] into.
pub trait RenderTarget {
    /// What pipelines rendering to this target are keyed on.
    fn desc(&self) -> TargetDesc;
    /// The texture (and multisampled view) to draw the next frame into.
    fn acquire(&mut self, cx: &Context) -> Result<Frame>;
    /// Submit `commands` (which draw into `frame`) and show or keep the
    /// result.
    fn present(&mut self, cx: &Context, frame: Frame, commands: wgpu::CommandBuffer) -> Result<()>;
}

/// The MSAA sample count of Gup-owned targets (RFC-001 §7). WebGPU
/// guarantees 1 and 4 for every renderable 8-bit format.
pub const DEFAULT_SAMPLES: u32 = 4;

/// Options of a target Gup draws into but whose size the caller chose
/// ([`ImageTarget`], [`TextureTarget`]).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TargetOptions {
    /// Physical pixels per logical pixel. A scene `w × h` logical pixels
    /// in size fills the target when it is `w·dpr × h·dpr` physical
    /// pixels; text is rasterised at `size × dpr`.
    pub dpr: f32,
    /// MSAA sample count: [`DEFAULT_SAMPLES`] (4) or 1.
    pub samples: u32,
}

impl Default for TargetOptions {
    fn default() -> Self {
        Self {
            dpr: 1.0,
            samples: DEFAULT_SAMPLES,
        }
    }
}

impl TargetOptions {
    /// Single-sampled (no MSAA): analytic antialiasing only.
    pub fn single_sample(self) -> Self {
        Self { samples: 1, ..self }
    }

    /// With `dpr` physical pixels per logical pixel.
    pub fn with_dpr(self, dpr: f32) -> Self {
        Self { dpr, ..self }
    }

    fn validate(&self, cx: &Context, format: wgpu::TextureFormat) -> Result<()> {
        if !(self.dpr.is_finite() && self.dpr > 0.0) {
            return Err(Error::config(
                "target dpr",
                format!("{} must be finite and positive", self.dpr),
            ));
        }
        check_samples(cx, format, self.samples)
    }
}

/// Whether `format` can be rendered with `samples` samples on `cx`.
pub(crate) fn check_samples(cx: &Context, format: wgpu::TextureFormat, samples: u32) -> Result<()> {
    if samples == 1 {
        return Ok(());
    }
    // Without adapter-specific format features, wgpu allows only the
    // WebGPU counts (1 and 4).
    let adapter_specific = cx
        .caps()
        .features
        .contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
    let supported = match cx.adapter() {
        Some(adapter) => {
            (samples == 4 || adapter_specific)
                && adapter
                    .get_texture_format_features(format)
                    .flags
                    .sample_count_supported(samples)
        }
        // A host's device (`Context::from_wgpu`): trust WebGPU's guarantee.
        None => samples == 4,
    };
    if supported {
        Ok(())
    } else {
        Err(Error::config(
            "target samples",
            format!("{format:?} cannot be multisampled {samples}× on this device; use 1 or 4"),
        ))
    }
}

/// A multisampled colour texture's view, or `None` for one sample.
pub(crate) fn msaa_view(
    cx: &Context,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    samples: u32,
) -> Option<wgpu::TextureView> {
    (samples > 1).then(|| {
        cx.device()
            .create_texture(&wgpu::TextureDescriptor {
                label: Some("gup msaa colour"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default())
    })
}

/// One frame of a [`RenderTarget`]: the texture and the (non-sRGB) view
/// that end up holding the image, plus the multisampled view the pass
/// draws into when the target has more than one sample.
#[derive(Debug)]
pub struct Frame {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    msaa: Option<wgpu::TextureView>,
    /// Set for window frames; presenting consumes it.
    #[cfg_attr(not(feature = "window"), allow(dead_code))]
    pub(crate) surface: Option<wgpu::SurfaceTexture>,
}

impl Frame {
    pub(crate) fn new(
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        msaa: Option<wgpu::TextureView>,
        surface: Option<wgpu::SurfaceTexture>,
    ) -> Self {
        Self {
            texture,
            view,
            msaa,
            surface,
        }
    }

    /// The single-sample view that holds the finished frame (the resolve
    /// target when the frame is multisampled).
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// The texture behind [`view`](Self::view).
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// The pass's colour attachment: clear to `clear`, draw into the
    /// multisampled view if there is one and resolve it into
    /// [`view`](Self::view), else draw into `view` directly.
    pub fn color_attachment(&self, clear: wgpu::Color) -> wgpu::RenderPassColorAttachment<'_> {
        let (view, resolve_target, store) = match &self.msaa {
            // The samples are only needed until they are resolved.
            Some(msaa) => (msaa, Some(&self.view), wgpu::StoreOp::Discard),
            None => (&self.view, None, wgpu::StoreOp::Store),
        };
        wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store,
            },
        }
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
/// (RFC-001 §7). The pass is 4× multisampled by default and resolves into
/// that texture. The readback un-premultiplies alpha.
///
/// Reading back is `async` on every platform; native code can use
/// [`render_blocking`](Self::render_blocking). In a browser, await
/// [`render`](Self::render) (or [`read`](Self::read)): the browser maps
/// the buffer, so nothing blocks.
#[derive(Debug)]
pub struct ImageTarget {
    desc: TargetDesc,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    msaa: Option<wgpu::TextureView>,
    /// The copy of the last presented frame.
    pending: Option<Readback>,
}

impl ImageTarget {
    /// A `width × height` physical-pixel target with the default
    /// [`TargetOptions`] (dpr 1, 4× MSAA).
    pub fn new(cx: &Context, width: u32, height: u32) -> Result<Self> {
        Self::with_options(cx, width, height, TargetOptions::default())
    }

    /// A `width × height` physical-pixel target with `options`.
    pub fn with_options(
        cx: &Context,
        width: u32,
        height: u32,
        options: TargetOptions,
    ) -> Result<Self> {
        let max = cx.caps().limits.max_texture_dimension_2d;
        if width == 0 || height == 0 || width > max || height > max {
            return Err(Error::config(
                "image target size",
                format!("{width}×{height} must be between 1 and {max} pixels on each side"),
            ));
        }
        let format = wgpu::TextureFormat::Rgba8Unorm;
        options.validate(cx, format)?;
        let desc = TargetDesc {
            format,
            width,
            height,
            dpr: options.dpr,
            samples: options.samples,
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
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Ok(Self {
            desc,
            texture,
            view,
            msaa: msaa_view(cx, format, width, height, options.samples),
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
        Ok(Frame::new(
            self.texture.clone(),
            self.view.clone(),
            self.msaa.clone(),
            None,
        ))
    }

    fn present(&mut self, cx: &Context, frame: Frame, commands: wgpu::CommandBuffer) -> Result<()> {
        let mut encoder = cx
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gup image readback"),
            });
        let readback = Readback::encode(cx, frame.texture(), &mut encoder)?;
        cx.submit([commands, encoder.finish()]);
        self.pending = Some(readback);
        Ok(())
    }
}

/// A texture the host owns, as a [`RenderTarget`]: Gup clears it and
/// draws a scene into it in its own pass and submission, and the host
/// samples or composites the texture however it likes (an egui image, a
/// bevy material, a UI layer).
///
/// The texture must be 2D, single-sampled and have `RENDER_ATTACHMENT`
/// usage. Gup draws through a non-sRGB view of it (RFC-001 §7), so an
/// sRGB texture must list its non-sRGB twin in `view_formats`
/// (`Rgba8UnormSrgb` → `Rgba8Unorm`). With more than one sample, Gup
/// draws into its own multisampled texture and resolves into the host's.
///
/// Hosts that want Gup's draws inside **their** pass use
/// [`Renderer::prepare`] and [`Prepared::draw`](crate::Prepared::draw)
/// instead; Gup then never submits.
#[derive(Debug)]
pub struct TextureTarget {
    desc: TargetDesc,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    msaa: Option<wgpu::TextureView>,
}

impl TextureTarget {
    /// Draw into `texture` (which must come from `cx`'s device) with
    /// `options`.
    pub fn new(cx: &Context, texture: wgpu::Texture, options: TargetOptions) -> Result<Self> {
        let problem = if !texture
            .usage()
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        {
            Some(format!(
                "needs RENDER_ATTACHMENT usage (it has {:?})",
                texture.usage()
            ))
        } else if texture.dimension() != wgpu::TextureDimension::D2
            || texture.depth_or_array_layers() != 1
        {
            Some("must be a single 2D layer".to_owned())
        } else if texture.sample_count() != 1 {
            Some(format!(
                "is {}× multisampled; give Gup the single-sample texture to resolve into, or \
                 draw in your own pass with Renderer::prepare and Prepared::draw",
                texture.sample_count()
            ))
        } else {
            None
        };
        if let Some(problem) = problem {
            return Err(Error::config(
                "texture target",
                format!("the {:?} texture {problem}", texture.format()),
            ));
        }
        let format = texture.format().remove_srgb_suffix();
        options.validate(cx, format)?;
        let (width, height) = (texture.width(), texture.height());
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("gup texture target"),
            format: Some(format),
            ..Default::default()
        });
        Ok(Self {
            desc: TargetDesc {
                format,
                width,
                height,
                dpr: options.dpr,
                samples: options.samples,
            },
            msaa: msaa_view(cx, format, width, height, options.samples),
            texture,
            view,
        })
    }

    /// The host's texture.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

impl RenderTarget for TextureTarget {
    fn desc(&self) -> TargetDesc {
        self.desc
    }

    fn acquire(&mut self, _cx: &Context) -> Result<Frame> {
        Ok(Frame::new(
            self.texture.clone(),
            self.view.clone(),
            self.msaa.clone(),
            None,
        ))
    }

    fn present(
        &mut self,
        cx: &Context,
        _frame: Frame,
        commands: wgpu::CommandBuffer,
    ) -> Result<()> {
        cx.submit([commands]);
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
