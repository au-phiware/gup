// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Drawing a [`Scene`]: the per-context pipeline cache, prepared layer GPU
//! state and the single render pass that draws marks, rules and text.

use crate::channel::Color;
use crate::context::{Context, ContextId, Upload};
use crate::error::{Error, Result};
use crate::scene::{GradientDirection, ItemKind, Rule, Scene, TextRun};
use crate::shader::glue::Glue;
use crate::shader::{GRADIENT_SHADER, RECT_SHADER, RULE_SHADER, StructLayout, link};
use crate::target::RenderTarget;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
// `std::time::Instant::now` panics on wasm32-unknown-unknown.
use web_time::Instant;

/// Everything about a target that pipelines and layout depend on
/// (RFC-001 §7).
///
/// Pipelines are keyed by `(kind, format, samples)`; `width`, `height`
/// and `dpr` only feed uniforms, scissors and glyph rasterisation, so they
/// are never part of a key. Pick passes (RFC-001 S9) always use one
/// sample, whatever the colour target's count.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TargetDesc {
    /// Colour format. Gup renders into non-sRGB views and blends in sRGB
    /// space.
    pub format: wgpu::TextureFormat,
    /// Physical width.
    pub width: u32,
    /// Physical height.
    pub height: u32,
    /// Physical pixels per logical pixel. A scene `w` logical pixels wide
    /// fills a target `w × dpr` physical pixels wide.
    pub dpr: f32,
    /// MSAA sample count of the pass's colour attachment: 4 by default on
    /// Gup-owned targets, whatever the host's pass uses in draw-in-pass
    /// mode. Points, lines and text antialias analytically at any count;
    /// MSAA smooths the geometric edges of rects and diagonal rules.
    pub samples: u32,
}

/// Uniform alignment for dynamic offsets (WebGPU's maximum
/// `min_uniform_buffer_offset_alignment`).
const DYNAMIC_ALIGN: u64 = 256;

/// A linked glue module and the layouts to bind it.
pub(crate) struct GlueProgram {
    pub glue: Glue,
    /// The glue linked against the flattened library: what wgpu compiles.
    pub wgsl: String,
    /// Layout of the `Encodings` uniform struct.
    pub encodings: StructLayout,
    /// Layout of the per-chunk `Chunk` uniform struct.
    pub chunk: StructLayout,
    pub enc_bgl: wgpu::BindGroupLayout,
    pub chunk_bgl: wgpu::BindGroupLayout,
    pub layout: wgpu::PipelineLayout,
}

impl std::fmt::Debug for GlueProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlueProgram")
            .field("signature", &self.glue.signature)
            .finish_non_exhaustive()
    }
}

/// A data layer's GPU state, ready to draw.
pub(crate) struct LayerGpu {
    pub context: ContextId,
    pub program: Arc<GlueProgram>,
    enc_buffer: wgpu::Buffer,
    chunk_buffer: wgpu::Buffer,
    pub enc_bind_group: wgpu::BindGroup,
    pub chunk_bind_group: wgpu::BindGroup,
    /// The column chunk and each vertex column's byte range in it.
    pub columns: wgpu::Buffer,
    pub column_ranges: Vec<std::ops::Range<u64>>,
    pub instances: u32,
    pub vertices_per_instance: u32,
}

impl std::fmt::Debug for LayerGpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerGpu")
            .field("signature", &self.program.glue.signature)
            .field("instances", &self.instances)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum PipelineKind {
    Mark(String),
    Rule,
    Rect,
    Gradient,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PipelineKey {
    kind: PipelineKind,
    format: wgpu::TextureFormat,
    samples: u32,
}

/// Counters and the most recent timings, for tests and the RFC record.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct PipelineStats {
    /// Glue modules linked (cache misses).
    pub programs_linked: u32,
    /// Render pipelines created (cache misses).
    pub pipelines_created: u32,
    /// Pipeline cache hits.
    pub pipeline_hits: u32,
    /// Link time of the last linked glue module (concatenation with the
    /// library flattened at build time).
    pub last_link: Duration,
    /// `create_render_pipeline` time of the last created mark pipeline.
    pub last_create: Duration,
}

/// Per-context cache: glue programs by signature and pipelines by
/// `(kind, format, samples)`. Uniform values are never part of a key.
#[derive(Default)]
pub(crate) struct PipelineCache {
    view_bgl: Option<wgpu::BindGroupLayout>,
    lut_bgl: Option<wgpu::BindGroupLayout>,
    programs: HashMap<String, Arc<GlueProgram>>,
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
    pub stats: PipelineStats,
}

fn uniform_entry(binding: u32, dynamic: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: dynamic,
            min_binding_size: None,
        },
        count: None,
    }
}

/// A palette LUT texture and its filtering sampler at `binding`, `binding + 1`.
fn lut_entries(binding: u32) -> [wgpu::BindGroupLayoutEntry; 2] {
    [
        wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: binding + 1,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ]
}

impl PipelineCache {
    fn view_bgl(&mut self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        self.view_bgl
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("gup view"),
                    entries: &[uniform_entry(0, false)],
                })
            })
            .clone()
    }

    /// The linked program for `glue`, linking it on first use.
    pub(crate) fn program(&mut self, cx: &Context, glue: &Glue) -> Result<Arc<GlueProgram>> {
        if let Some(p) = self.programs.get(&glue.signature) {
            return Ok(Arc::clone(p));
        }
        let start = Instant::now();
        let wgsl = link(&glue.signature, &glue.source, &glue.modules)?;
        let link_time = start.elapsed();

        let device = cx.device();
        let mut enc_entries = vec![uniform_entry(0, false)];
        for lut in &glue.luts {
            enc_entries.extend(lut_entries(lut.binding));
        }
        let enc_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gup encodings"),
            entries: &enc_entries,
        });
        let chunk_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gup chunk"),
            entries: &[uniform_entry(0, true)],
        });
        let view_bgl = self.view_bgl(device);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gup mark layout"),
            bind_group_layouts: &[&view_bgl, &enc_bgl, &chunk_bgl],
            push_constant_ranges: &[],
        });
        self.stats.programs_linked += 1;
        self.stats.last_link = link_time;
        let program = Arc::new(GlueProgram {
            glue: glue.clone(),
            wgsl,
            encodings: glue.encodings.clone(),
            chunk: glue.chunk.clone(),
            enc_bgl,
            chunk_bgl,
            layout,
        });
        self.programs
            .insert(glue.signature.clone(), Arc::clone(&program));
        Ok(program)
    }

    pub(crate) fn mark_pipeline(
        &mut self,
        cx: &Context,
        program: &GlueProgram,
        desc: &TargetDesc,
    ) -> wgpu::RenderPipeline {
        let key = PipelineKey {
            kind: PipelineKind::Mark(program.glue.signature.clone()),
            format: desc.format,
            samples: desc.samples,
        };
        if let Some(p) = self.pipelines.get(&key) {
            self.stats.pipeline_hits += 1;
            return p.clone();
        }
        let start = Instant::now();
        let module = cx
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&program.glue.signature),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&program.wgsl)),
            });
        let attributes: Vec<wgpu::VertexAttribute> = (0..program.glue.columns.len())
            .map(|loc| wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 0,
                shader_location: loc as u32,
            })
            .collect();
        let buffers: Vec<wgpu::VertexBufferLayout> = attributes
            .iter()
            .map(|a| wgpu::VertexBufferLayout {
                array_stride: 4,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: std::slice::from_ref(a),
            })
            .collect();
        let pipeline = create_pipeline(
            cx.device(),
            &program.glue.signature,
            &program.layout,
            &module,
            &buffers,
            desc,
        );
        self.stats.pipelines_created += 1;
        self.stats.last_create = start.elapsed();
        self.pipelines.insert(key, pipeline.clone());
        pipeline
    }

    fn guide_pipeline(
        &mut self,
        cx: &Context,
        kind: PipelineKind,
        desc: &TargetDesc,
    ) -> Result<wgpu::RenderPipeline> {
        let key = PipelineKey {
            kind: kind.clone(),
            format: desc.format,
            samples: desc.samples,
        };
        if let Some(p) = self.pipelines.get(&key) {
            self.stats.pipeline_hits += 1;
            return Ok(p.clone());
        }
        let device = cx.device();
        let view_bgl = self.view_bgl(device);
        let (label, source, attributes, stride): (_, _, &[wgpu::VertexAttribute], _) = match kind {
            PipelineKind::Rule => (
                "gup rules",
                RULE_SHADER,
                &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32, 3 => Float32x4],
                std::mem::size_of::<RuleInstance>(),
            ),
            PipelineKind::Rect => (
                "gup rects",
                RECT_SHADER,
                &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                std::mem::size_of::<RectInstance>(),
            ),
            PipelineKind::Gradient => (
                "gup gradient",
                GRADIENT_SHADER,
                &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32, 3 => Uint32],
                std::mem::size_of::<GradientInstance>(),
            ),
            PipelineKind::Mark(_) => unreachable!("mark pipelines come from glue programs"),
        };
        let lut_bgl = self.lut_bgl(device);
        let groups = match kind {
            PipelineKind::Gradient => vec![&view_bgl, &lut_bgl],
            _ => vec![&view_bgl],
        };
        // Guide shaders are authored WGSL, flattened completely at build
        // time.
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(source)),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: &groups,
            push_constant_ranges: &[],
        });
        let buffers = [wgpu::VertexBufferLayout {
            array_stride: stride as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes,
        }];
        let pipeline = create_pipeline(device, label, &layout, &module, &buffers, desc);
        self.stats.pipelines_created += 1;
        self.pipelines.insert(key, pipeline.clone());
        Ok(pipeline)
    }

    /// Group 1 of the gradient pipeline: a palette LUT and its sampler.
    fn lut_bgl(&mut self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        self.lut_bgl
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("gup gradient lut"),
                    entries: &lut_entries(0),
                })
            })
            .clone()
    }
}

fn create_pipeline(
    device: &wgpu::Device,
    label: &str,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    buffers: &[wgpu::VertexBufferLayout<'_>],
    desc: &TargetDesc,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: desc.format,
                // Shaders output premultiplied sRGB-encoded colour.
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: desc.samples,
            ..Default::default()
        },
        multiview: None,
        cache: None,
    })
}

/// The program, uniform buffers and bind groups for one data layer.
pub(crate) struct LayerUniforms<'a> {
    pub program: Arc<GlueProgram>,
    /// Bytes of the `Encodings` struct.
    pub encodings: Vec<u8>,
    /// Bytes of the `Chunk` struct.
    pub chunk: Vec<u8>,
    /// One LUT per `program.glue.luts` entry.
    pub luts: Vec<&'a [[u8; 4]]>,
}

impl LayerUniforms<'_> {
    /// Upload uniforms and LUTs and build the layer's bind groups.
    pub(crate) fn build(
        self,
        cx: &Context,
        columns: wgpu::Buffer,
        column_ranges: Vec<std::ops::Range<u64>>,
        instances: u32,
        vertices_per_instance: u32,
    ) -> LayerGpu {
        let device = cx.device();
        let enc = cx.buffer_with_data(
            Upload::Uniform,
            "gup encodings",
            wgpu::BufferUsages::UNIFORM,
            &self.encodings,
        );
        let mut chunk_bytes = self.chunk;
        chunk_bytes.resize(DYNAMIC_ALIGN as usize, 0);
        let chunk = cx.buffer_with_data(
            Upload::Uniform,
            "gup chunk uniforms",
            wgpu::BufferUsages::UNIFORM,
            &chunk_bytes,
        );
        let sampler = lut_sampler(device);
        let lut_views: Vec<wgpu::TextureView> =
            self.luts.iter().map(|lut| lut_view(cx, lut)).collect();
        let mut entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: enc.as_entire_binding(),
        }];
        for (lut, view) in self.program.glue.luts.iter().zip(&lut_views) {
            entries.push(wgpu::BindGroupEntry {
                binding: lut.binding,
                resource: wgpu::BindingResource::TextureView(view),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: lut.binding + 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            });
        }
        let enc_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gup encodings"),
            layout: &self.program.enc_bgl,
            entries: &entries,
        });
        let chunk_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gup chunk"),
            layout: &self.program.chunk_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &chunk,
                    offset: 0,
                    size: wgpu::BufferSize::new(u64::from(self.program.chunk.span)),
                }),
            }],
        });
        LayerGpu {
            context: cx.id(),
            program: self.program,
            enc_buffer: enc,
            chunk_buffer: chunk,
            enc_bind_group,
            chunk_bind_group,
            columns,
            column_ranges,
            instances,
            vertices_per_instance,
        }
    }
}

/// The linear-filtering sampler palette LUTs are read with.
fn lut_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("gup lut sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    })
}

/// A palette LUT as a 1-row texture (uploaded, counted).
fn lut_view(cx: &Context, lut: &[[u8; 4]]) -> wgpu::TextureView {
    let size = wgpu::Extent3d {
        width: lut.len() as u32,
        height: 1,
        depth_or_array_layers: 1,
    };
    let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("gup palette lut"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Not *Srgb: the LUT holds sRGB-encoded values and must not be
        // linearised by the sampler.
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    cx.write_texture(
        texture.as_image_copy(),
        bytemuck::cast_slice(lut),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(size.width * 4),
            rows_per_image: None,
        },
        size,
    );
    texture.create_view(&Default::default())
}
impl LayerGpu {
    /// Whether this GPU state can take new uniform values from `program`
    /// without being rebuilt: same context, program and column chunk.
    pub(crate) fn reusable(
        &self,
        cx: &Context,
        program: &Arc<GlueProgram>,
        columns: &wgpu::Buffer,
    ) -> bool {
        self.context == cx.id() && Arc::ptr_eq(&self.program, program) && &self.columns == columns
    }

    /// Write new `Encodings` and `Chunk` uniform values into the existing
    /// buffers: the per-frame path of a zoom or pan. Writes no column
    /// bytes and creates no GPU objects.
    pub(crate) fn write_uniforms(&self, cx: &Context, encodings: &[u8], chunk: &[u8]) {
        cx.write_buffer(Upload::Uniform, &self.enc_buffer, 0, encodings);
        cx.write_buffer(Upload::Uniform, &self.chunk_buffer, 0, chunk);
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct RuleInstance {
    p0: [f32; 2],
    p1: [f32; 2],
    width: f32,
    color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct RectInstance {
    lo: [f32; 2],
    hi: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GradientInstance {
    lo: [f32; 2],
    hi: [f32; 2],
    vertical: u32,
    reverse: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ViewUniform {
    size: [f32; 2],
    dpr: f32,
    padding: f32,
}

/// One recorded draw.
enum Draw {
    Marks {
        pipeline: wgpu::RenderPipeline,
        layer: Arc<LayerGpu>,
    },
    Instanced {
        pipeline: wgpu::RenderPipeline,
        buffer: wgpu::Buffer,
        /// Bytes of `buffer` in use (it is pooled and may be larger).
        bytes: u64,
        count: u32,
        /// Bind group 1, if the pipeline has one (a gradient's LUT).
        group1: Option<wgpu::BindGroup>,
    },
    /// Glyph quads, drawn by `gup-text` (it binds its atlas at group 0).
    Text(gup_text::GlyphBatch),
}

impl Draw {
    /// Attach bind group 1 to an instanced draw.
    fn with_group1(mut self, group: wgpu::BindGroup) -> Self {
        if let Draw::Instanced { group1, .. } = &mut self {
            *group1 = Some(group);
        }
        self
    }
}

/// A scissor rectangle in physical pixels.
type Scissor = [u32; 4];

/// Per-renderer GPU state reused across frames: the view uniform and the
/// guide and glyph instance buffers. Pipelines, programs and the text
/// system (font and glyph atlas) live in the [`Context`].
struct RendererGpu {
    context: ContextId,
    view_buffer: wgpu::Buffer,
    view_bind_group: wgpu::BindGroup,
    /// Scratch glyph quads, reused across text items and frames.
    glyphs: gup_text::Glyphs,
    /// Grow-only glyph instance buffers, one per text draw.
    glyph_buffers: Vec<gup_text::GlyphBuffer>,
    /// Grow-only guide vertex buffers, one per guide draw, reused across
    /// frames.
    instances: Vec<wgpu::Buffer>,
    /// Gradient LUT bind groups by palette (pointer identity of the
    /// scale's shared LUT), so a legend uploads its palette once.
    luts: Vec<(Arc<[[u8; 4]]>, wgpu::BindGroup)>,
}

/// Turns a [`Scene`] into GPU draws (RFC-001 §7).
///
/// [`prepare`](Self::prepare) does everything that allocates or takes a
/// lock; [`Prepared::draw`] only records commands into a render pass, so
/// the same draw serves every target: Gup-owned ones through
/// [`render`](Self::render), and a host's pass (egui, bevy) directly. A
/// `Renderer` reuses its view uniform and guide buffers from frame to
/// frame, so steady-state frames write only uniforms and guide instances.
#[derive(Default)]
pub struct Renderer {
    gpu: Option<RendererGpu>,
}

impl std::fmt::Debug for Renderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Renderer")
            .field("context", &self.gpu.as_ref().map(|g| g.context))
            .finish_non_exhaustive()
    }
}

/// A scene ready to record into a render pass.
pub struct Prepared {
    view_bind_group: wgpu::BindGroup,
    draws: Vec<(Scissor, Draw)>,
    clear: wgpu::Color,
}

impl std::fmt::Debug for Prepared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prepared")
            .field("draws", &self.draws.len())
            .finish_non_exhaustive()
    }
}

impl Renderer {
    /// A renderer with no GPU state yet.
    pub fn new() -> Self {
        Self::default()
    }

    fn gpu(&mut self, cx: &Context) -> &mut RendererGpu {
        if self.gpu.as_ref().is_none_or(|g| g.context != cx.id()) {
            let device = cx.device();
            let view_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gup view"),
                size: std::mem::size_of::<ViewUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let view_bgl = cx.pipelines().view_bgl(device);
            let view_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gup view"),
                layout: &view_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_buffer.as_entire_binding(),
                }],
            });
            self.gpu = Some(RendererGpu {
                context: cx.id(),
                view_buffer,
                view_bind_group,
                glyphs: gup_text::Glyphs::new(1.0),
                glyph_buffers: Vec::new(),
                instances: Vec::new(),
                luts: Vec::new(),
            });
        }
        self.gpu.as_mut().expect("created above")
    }

    /// Prepare `scene` for a target described by `desc`: write the view
    /// uniform, fetch (or create) pipelines, upload guide instances and
    /// new glyphs.
    ///
    /// This is the draw-in-pass entry point for hosts that own the render
    /// pass (egui paint callbacks, a bevy render graph node): pass the
    /// format and sample count of the host's colour attachment, then call
    /// [`Prepared::draw`] inside the host's pass. Gup writes uniforms and
    /// instances to the queue (flushed by the host's next submit) but
    /// never submits on the host's behalf (RFC-001 §2).
    pub fn prepare(&mut self, cx: &Context, scene: &Scene, desc: &TargetDesc) -> Result<Prepared> {
        if !(desc.dpr.is_finite() && desc.dpr > 0.0) || desc.samples == 0 {
            return Err(Error::config(
                "target",
                format!(
                    "dpr {} and samples {} must be positive (a single-sample target has \
                     samples 1)",
                    desc.dpr, desc.samples
                ),
            ));
        }
        let dpr = desc.dpr;
        let gpu = self.gpu(cx);
        cx.write_buffer(
            Upload::Uniform,
            &gpu.view_buffer,
            0,
            bytemuck::bytes_of(&ViewUniform {
                size: [scene.width, scene.height],
                dpr,
                padding: 0.0,
            }),
        );

        let full = [0, 0, desc.width, desc.height];
        let mut draws = Vec::new();
        let mut next_instances = 0;
        let mut next_glyphs = 0;
        for item in &scene.items {
            let scissor = item
                .clip
                .map_or(full, |c| scissor(scene.clips[c.0], dpr, desc));
            let draw = match &item.kind {
                ItemKind::Marks(batch) => {
                    if batch.gpu.context != cx.id() {
                        return Err(Error::config(
                            "scene",
                            "a mark batch was prepared on a different Context; resolve the \
                             chart again with this context",
                        ));
                    }
                    let pipeline = cx.pipelines().mark_pipeline(cx, &batch.gpu.program, desc);
                    Some(Draw::Marks {
                        pipeline,
                        layer: Arc::clone(&batch.gpu),
                    })
                }
                ItemKind::Rules(rules) => {
                    let instances: Vec<RuleInstance> = rules.iter().map(rule_instance).collect();
                    gpu.instanced(
                        cx,
                        &mut next_instances,
                        PipelineKind::Rule,
                        desc,
                        &instances,
                    )?
                }
                ItemKind::Rects(rects) => {
                    let instances: Vec<RectInstance> = rects
                        .iter()
                        .map(|r| RectInstance {
                            lo: [r.rect.left(), r.rect.top()],
                            hi: [r.rect.right(), r.rect.bottom()],
                            color: r.color.to_array(),
                        })
                        .collect();
                    gpu.instanced(
                        cx,
                        &mut next_instances,
                        PipelineKind::Rect,
                        desc,
                        &instances,
                    )?
                }
                ItemKind::Gradient(bar) => {
                    let instance = GradientInstance {
                        lo: [bar.rect.left(), bar.rect.top()],
                        hi: [bar.rect.right(), bar.rect.bottom()],
                        vertical: u32::from(bar.direction == GradientDirection::Vertical),
                        reverse: u32::from(bar.reverse),
                    };
                    let lut = gpu.lut_bind_group(cx, &bar.lut);
                    gpu.instanced(
                        cx,
                        &mut next_instances,
                        PipelineKind::Gradient,
                        desc,
                        &[instance],
                    )?
                    .map(|draw| draw.with_group1(lut))
                }
                ItemKind::Text(runs) => gpu.text(cx, &mut next_glyphs, runs, dpr, desc)?,
            };
            if let Some(draw) = draw {
                draws.push((scissor, draw));
            }
        }
        Ok(Prepared {
            view_bind_group: gpu.view_bind_group.clone(),
            draws,
            clear: premultiplied_clear(scene.background),
        })
    }

    /// Draw `scene` into `target` in one render pass and present it: the
    /// one render path every Gup-owned pass ([`ImageTarget`],
    /// [`TextureTarget`], `WindowTarget`) shares. The pass clears to the
    /// scene's background and, on a multisampled target, resolves into the
    /// frame's texture.
    ///
    /// [`ImageTarget`]: crate::ImageTarget
    /// [`TextureTarget`]: crate::TextureTarget
    pub fn render(
        &mut self,
        cx: &Context,
        scene: &Scene,
        target: &mut dyn RenderTarget,
    ) -> Result<()> {
        let desc = target.desc();
        let frame = target.acquire(cx)?;
        let prepared = self.prepare(cx, scene, &desc)?;
        let mut encoder = cx
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gup scene"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gup scene"),
                color_attachments: &[Some(frame.color_attachment(prepared.clear_color()))],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            prepared.draw(&mut pass);
        }
        target.present(cx, frame, encoder.finish())
    }
}

impl RendererGpu {
    /// Lay out `runs` and prepare their glyph draw in the next pooled glyph
    /// buffer. Holds only the `text` lock (RFC-001 S1 lock order); the
    /// returned batch draws without it.
    fn text(
        &mut self,
        cx: &Context,
        next: &mut usize,
        runs: &[TextRun],
        dpr: f32,
        desc: &TargetDesc,
    ) -> Result<Option<Draw>> {
        if *next == self.glyph_buffers.len() {
            self.glyph_buffers.push(gup_text::GlyphBuffer::new());
        }
        let buffer = &mut self.glyph_buffers[*next];
        *next += 1;
        self.glyphs.clear(dpr);
        let mut text = cx.text();
        for run in runs {
            text.layout(
                &run.layout_run(),
                run.style.color.to_array(),
                &mut self.glyphs,
            )?;
        }
        let target = gup_text::DrawTarget {
            format: desc.format,
            samples: desc.samples,
            width: desc.width,
            height: desc.height,
        };
        Ok(text
            .prepare(&cx.text_uploads(), buffer, &self.glyphs, &target)
            .map(Draw::Text))
    }

    /// The bind group of `lut` for the gradient pipeline, uploading the
    /// palette the first time this renderer sees it.
    fn lut_bind_group(&mut self, cx: &Context, lut: &Arc<[[u8; 4]]>) -> wgpu::BindGroup {
        if let Some((_, group)) = self.luts.iter().find(|(l, _)| Arc::ptr_eq(l, lut)) {
            return group.clone();
        }
        let device = cx.device();
        let layout = cx.pipelines().lut_bgl(device);
        let view = lut_view(cx, lut);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gup gradient lut"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&lut_sampler(device)),
                },
            ],
        });
        self.luts.push((Arc::clone(lut), group.clone()));
        group
    }

    /// Write `instances` into the next pooled buffer (growing it if
    /// needed) and record an instanced guide draw.
    fn instanced<I: bytemuck::Pod>(
        &mut self,
        cx: &Context,
        next: &mut usize,
        kind: PipelineKind,
        desc: &TargetDesc,
        instances: &[I],
    ) -> Result<Option<Draw>> {
        if instances.is_empty() {
            return Ok(None);
        }
        let pipeline = cx.pipelines().guide_pipeline(cx, kind, desc)?;
        let bytes: &[u8] = bytemuck::cast_slice(instances);
        let size = bytes.len() as u64;
        if self.instances.get(*next).is_none_or(|b| b.size() < size) {
            let buffer = cx.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("gup guide instances"),
                size: size.next_power_of_two().max(256),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if *next < self.instances.len() {
                self.instances[*next] = buffer;
            } else {
                self.instances.push(buffer);
            }
        }
        let buffer = self.instances[*next].clone();
        *next += 1;
        cx.write_buffer(Upload::Instances, &buffer, 0, bytes);
        Ok(Some(Draw::Instanced {
            pipeline,
            buffer,
            bytes: size,
            count: instances.len() as u32,
            group1: None,
        }))
    }
}

impl Prepared {
    /// The scene's background, premultiplied, for a pass that clears.
    pub fn clear_color(&self) -> wgpu::Color {
        self.clear
    }

    /// Record every draw into `pass`. Allocates nothing, takes no locks
    /// and never submits, so it can run inside a host's pass.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        for ([x, y, w, h], draw) in &self.draws {
            pass.set_scissor_rect(*x, *y, *w, *h);
            match draw {
                Draw::Marks { pipeline, layer } => {
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &self.view_bind_group, &[]);
                    pass.set_bind_group(1, &layer.enc_bind_group, &[]);
                    pass.set_bind_group(2, &layer.chunk_bind_group, &[0]);
                    for (slot, range) in layer.column_ranges.iter().enumerate() {
                        pass.set_vertex_buffer(slot as u32, layer.columns.slice(range.clone()));
                    }
                    pass.draw(0..layer.vertices_per_instance, 0..layer.instances);
                }
                Draw::Instanced {
                    pipeline,
                    buffer,
                    bytes,
                    count,
                    group1,
                } => {
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &self.view_bind_group, &[]);
                    if let Some(group) = group1 {
                        pass.set_bind_group(1, group, &[]);
                    }
                    pass.set_vertex_buffer(0, buffer.slice(..*bytes));
                    pass.draw(0..6, 0..*count);
                }
                // Binds its atlas at group 0, so the others rebind the view.
                Draw::Text(batch) => batch.draw(pass),
            }
        }
    }
}

/// A logical-pixel clip rectangle as a physical-pixel scissor.
fn scissor(rect: crate::geom::Rect, dpr: f32, desc: &TargetDesc) -> Scissor {
    let x0 = (rect.left() * dpr).floor().clamp(0.0, desc.width as f32) as u32;
    let y0 = (rect.top() * dpr).floor().clamp(0.0, desc.height as f32) as u32;
    let x1 = (rect.right() * dpr).ceil().clamp(0.0, desc.width as f32) as u32;
    let y1 = (rect.bottom() * dpr).ceil().clamp(0.0, desc.height as f32) as u32;
    [x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)]
}

fn rule_instance(r: &Rule) -> RuleInstance {
    RuleInstance {
        p0: [r.p0.x, r.p0.y],
        p1: [r.p1.x, r.p1.y],
        width: r.width.0,
        color: r.color.to_array(),
    }
}

fn premultiplied_clear(c: Color) -> wgpu::Color {
    let [r, g, b, a] = c.premultiplied().map(f64::from);
    wgpu::Color { r, g, b, a }
}
