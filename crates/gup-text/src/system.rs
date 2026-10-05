// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! [`TextSystem`]: one font and glyph atlas per device, and glyph runs
//! drawn into a caller's render pass.

use crate::atlas::GlyphAtlas;
use crate::error::Result;
use crate::font::Font;
use crate::layout::{Bounds, Run, TextMetrics};
use std::collections::HashMap;

/// Where `gup-text` writes GPU memory: glyph instances and atlas texels.
///
/// Implemented for [`wgpu::Queue`]. A host that accounts for its uploads
/// (as `gup-core` does) passes its own implementation instead.
pub trait Uploader {
    /// Write glyph instances into a vertex buffer.
    fn write_buffer(&self, buffer: &wgpu::Buffer, offset: u64, data: &[u8]);

    /// Write a rectangle of atlas texels.
    fn write_texture(
        &self,
        texture: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    );
}

impl Uploader for wgpu::Queue {
    fn write_buffer(&self, buffer: &wgpu::Buffer, offset: u64, data: &[u8]) {
        wgpu::Queue::write_buffer(self, buffer, offset, data);
    }

    fn write_texture(
        &self,
        texture: wgpu::TexelCopyTextureInfo<'_>,
        data: &[u8],
        layout: wgpu::TexelCopyBufferLayout,
        size: wgpu::Extent3d,
    ) {
        wgpu::Queue::write_texture(self, texture, data, layout, size);
    }
}

/// The render target glyphs are drawn into.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DrawTarget {
    /// Colour format of the pass's attachment. Colours are written as
    /// given (sRGB-encoded), so use a non-sRGB view.
    pub format: wgpu::TextureFormat,
    /// MSAA sample count of the pass.
    pub samples: u32,
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

/// One glyph quad as the shader reads it.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphInstance {
    /// x0, y0, x1, y1: physical pixels in [`Glyphs`], clip space on the GPU.
    rect: [f32; 4],
    /// u0, v0, u1, v1 in atlas texels.
    uv: [f32; 4],
    /// sRGB-encoded, straight alpha.
    color: [f32; 4],
}

/// Laid-out glyph quads of one or more runs, for a target with `scale`
/// physical pixels per logical pixel. Reuse one across frames with
/// [`clear`](Self::clear).
#[derive(Clone, Debug)]
pub struct Glyphs {
    scale: f32,
    quads: Vec<GlyphInstance>,
}

impl Glyphs {
    /// No glyphs, for a target with `scale` physical pixels per logical
    /// pixel (the device pixel ratio).
    pub fn new(scale: f32) -> Self {
        Self {
            scale,
            quads: Vec::new(),
        }
    }

    /// Physical pixels per logical pixel.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Number of glyph quads (blank glyphs such as spaces have none).
    pub fn len(&self) -> usize {
        self.quads.len()
    }

    /// Whether there are no quads.
    pub fn is_empty(&self) -> bool {
        self.quads.is_empty()
    }

    /// Remove every quad, keeping the allocation, and set a new scale.
    pub fn clear(&mut self, scale: f32) {
        self.scale = scale;
        self.quads.clear();
    }

    /// Each quad's rectangle in physical pixels: x, y, width, height.
    pub fn rects(&self) -> impl Iterator<Item = [f32; 4]> + '_ {
        self.quads.iter().map(|q| {
            let [x0, y0, x1, y1] = q.rect;
            [x0, y0, x1 - x0, y1 - y0]
        })
    }
}

/// A grow-only vertex buffer for glyph instances. Keep one per draw slot
/// and reuse it across frames, so steady-state frames allocate nothing.
#[derive(Debug, Default)]
pub struct GlyphBuffer {
    buffer: Option<wgpu::Buffer>,
    scratch: Vec<GlyphInstance>,
}

impl GlyphBuffer {
    /// An empty buffer; the GPU buffer is created on first use.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Glyph draws ready to record into a render pass: the pipeline, atlas
/// bind group and instance buffer, all cheap handles.
#[derive(Clone, Debug)]
pub struct GlyphBatch {
    pipeline: wgpu::RenderPipeline,
    atlas: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    bytes: u64,
    count: u32,
}

impl GlyphBatch {
    /// Record the glyph draws into `pass`. Sets the pipeline, bind group
    /// 0 (the atlas) and vertex buffer 0; allocates nothing, takes no
    /// locks and never submits, so it can run inside any pass whose
    /// target matches the [`DrawTarget`] the batch was prepared for.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.atlas, &[]);
        pass.set_vertex_buffer(0, self.buffer.slice(..self.bytes));
        pass.draw(0..6, 0..self.count);
    }

    /// Number of glyph quads drawn.
    pub fn len(&self) -> u32 {
        self.count
    }

    /// Whether the batch draws nothing (never true for a prepared batch).
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// The atlas texture and its bind group.
struct AtlasTexture {
    texture: wgpu::Texture,
    size: u32,
    bind_group: wgpu::BindGroup,
}

/// One font and its glyph atlas for one device: measures text, lays out
/// runs and prepares glyph batches that draw into a caller's pass.
///
/// Keep one per device (`gup-core` keeps one per `Context`). Lay out and
/// prepare while holding it; the returned [`GlyphBatch`] draws without it.
pub struct TextSystem {
    device: wgpu::Device,
    font: Font,
    atlas: GlyphAtlas,
    texture: Option<AtlasTexture>,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    shader: Option<wgpu::ShaderModule>,
    pipelines: HashMap<(wgpu::TextureFormat, u32), wgpu::RenderPipeline>,
    /// The dirty atlas rectangle, packed for upload.
    upload: Vec<u8>,
}

impl std::fmt::Debug for TextSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextSystem")
            .field("font", &self.font)
            .field("atlas_size", &self.atlas.size())
            .field("glyphs", &self.atlas.len())
            .finish_non_exhaustive()
    }
}

impl TextSystem {
    /// A text system with the bundled Inter font.
    pub fn new(device: &wgpu::Device) -> Self {
        Self::with_font(device, Font::inter())
    }

    /// A text system with `font`.
    pub fn with_font(device: &wgpu::Device, font: Font) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gup-text atlas"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gup-text glyphs"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        // Nearest: quads and atlas rects are the same size on whole pixels.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gup-text atlas"),
            ..Default::default()
        });
        Self {
            device: device.clone(),
            font,
            atlas: GlyphAtlas::new(device.limits().max_texture_dimension_2d),
            texture: None,
            bind_group_layout,
            pipeline_layout,
            sampler,
            shader: None,
            pipelines: HashMap::new(),
            upload: Vec::new(),
        }
    }

    /// The font.
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// Measure `text` at `size` logical pixels.
    pub fn measure(&self, text: &str, size: f32) -> TextMetrics {
        self.font.measure(text, size)
    }

    /// The ink box of `run` in logical pixels (see [`Font::ink_bounds`]).
    pub fn ink_bounds(&self, run: &Run<'_>) -> Bounds {
        self.font.ink_bounds(run)
    }

    /// Width and height of the glyph atlas in texels.
    pub fn atlas_size(&self) -> u32 {
        self.atlas.size()
    }

    /// Distinct glyphs (character and physical size) in the atlas.
    pub fn atlas_glyphs(&self) -> usize {
        self.atlas.len()
    }

    /// Lay out `run` in `color` (sRGB-encoded, straight alpha) and append
    /// its glyph quads to `glyphs`, rasterising new glyphs at the glyphs'
    /// scale into the atlas.
    pub fn layout(&mut self, run: &Run<'_>, color: [f32; 4], glyphs: &mut Glyphs) -> Result<()> {
        let font = self.font.clone();
        let mut result = Ok(());
        font.walk(run, glyphs.scale, |g| {
            if result.is_err() || g.metrics.width == 0 || g.metrics.height == 0 {
                return;
            }
            match self.atlas.glyph(&font, g.c, g.px) {
                Ok(r) => {
                    let [u, v, w, h] = [r.x, r.y, r.width, r.height].map(|t| t as f32);
                    glyphs.quads.push(GlyphInstance {
                        rect: [g.x, g.y, g.x + w, g.y + h],
                        uv: [u, v, u + w, v + h],
                        color,
                    });
                }
                Err(e) => result = Err(e),
            }
        });
        result
    }

    /// Upload new glyphs, write `glyphs` into `buffer` and return a batch
    /// that draws them into a pass on `target`. `None` if there is nothing
    /// to draw.
    pub fn prepare(
        &mut self,
        uploader: &impl Uploader,
        buffer: &mut GlyphBuffer,
        glyphs: &Glyphs,
        target: &DrawTarget,
    ) -> Option<GlyphBatch> {
        if glyphs.is_empty() {
            return None;
        }
        let atlas = self.upload_atlas(uploader);
        let pipeline = self.pipeline(target);

        let (sx, sy) = (2.0 / target.width as f32, 2.0 / target.height as f32);
        buffer.scratch.clear();
        buffer.scratch.extend(glyphs.quads.iter().map(|q| {
            let [x0, y0, x1, y1] = q.rect;
            GlyphInstance {
                rect: [x0 * sx - 1.0, 1.0 - y0 * sy, x1 * sx - 1.0, 1.0 - y1 * sy],
                ..*q
            }
        }));
        let bytes: &[u8] = bytemuck::cast_slice(&buffer.scratch);
        let size = bytes.len() as u64;
        if buffer.buffer.as_ref().is_none_or(|b| b.size() < size) {
            buffer.buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gup-text glyphs"),
                size: size.next_power_of_two().max(256),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let gpu = buffer.buffer.clone().expect("created above");
        uploader.write_buffer(&gpu, 0, bytes);
        Some(GlyphBatch {
            pipeline,
            atlas,
            buffer: gpu,
            bytes: size,
            count: glyphs.quads.len() as u32,
        })
    }

    /// The atlas bind group, (re)creating the texture at the atlas's size
    /// and uploading only the dirty rectangle.
    fn upload_atlas(&mut self, uploader: &impl Uploader) -> wgpu::BindGroup {
        let size = self.atlas.size();
        if self.texture.as_ref().is_none_or(|t| t.size != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("gup-text atlas"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gup-text atlas"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            });
            self.texture = Some(AtlasTexture {
                texture,
                size,
                bind_group,
            });
        }
        let atlas = self.texture.as_ref().expect("created above");
        if let Some(d) = self.atlas.take_dirty() {
            let (w, h) = (d.x1 - d.x0, d.y1 - d.y0);
            self.upload.clear();
            for y in d.y0..d.y1 {
                let row = (y * size + d.x0) as usize;
                self.upload
                    .extend_from_slice(&self.atlas.pixels()[row..row + w as usize]);
            }
            uploader.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &atlas.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: d.x0,
                        y: d.y0,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &self.upload,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        atlas.bind_group.clone()
    }

    /// The glyph pipeline for `target`'s format and sample count.
    fn pipeline(&mut self, target: &DrawTarget) -> wgpu::RenderPipeline {
        let key = (target.format, target.samples);
        if let Some(p) = self.pipelines.get(&key) {
            return p.clone();
        }
        let device = &self.device;
        let shader = self.shader.get_or_insert_with(|| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("gup-text glyphs"),
                source: wgpu::ShaderSource::Wgsl(include_str!("glyph.wgsl").into()),
            })
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gup-text glyphs"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GlyphInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target.format,
                    // The shader outputs premultiplied, sRGB-encoded colour.
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: target.samples,
                ..Default::default()
            },
            multiview: None,
            cache: None,
        });
        self.pipelines.insert(key, pipeline.clone());
        pipeline
    }
}
