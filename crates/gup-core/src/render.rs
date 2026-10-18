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
use crate::target::{RenderTarget, check_samples, msaa_view};
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
    /// MSAA sample count of the pass's colour attachment: 1 by default on
    /// Gup-owned targets, whatever the host's pass uses in draw-in-pass
    /// mode. Circles, rules, rects and text antialias analytically and
    /// look the same at any count; MSAA is for marks with
    /// [`Antialias::Msaa`](crate::Antialias::Msaa) (areas, polygons).
    pub samples: u32,
}

/// Uniform alignment for dynamic offsets (WebGPU's maximum
/// `min_uniform_buffer_offset_alignment`).
pub(crate) const DYNAMIC_ALIGN: u64 = 256;

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

/// One draw entry's instanced draw: a column chunk's, or a polyline's
/// bridge between two chunks (`order::draw_entries`).
#[derive(Clone, Debug)]
pub(crate) struct ChunkDraw {
    /// The buffer of the chunk the entry's rows are in.
    pub columns: wgpu::Buffer,
    /// Each vertex buffer slot's buffer and byte range, by slot: the
    /// columns from the entry's first row, then a polyline's next-row
    /// columns (in the same chunk, or in the next one for a bridge).
    pub slots: Vec<(wgpu::Buffer, std::ops::Range<u64>)>,
    pub instances: u32,
    /// Offset of the entry in the `Chunk` uniform buffer.
    pub dynamic_offset: u32,
    /// The chunk's validity bits, if the program reads them.
    pub validity: Option<wgpu::Buffer>,
    /// A polyline's next-row validity bits (the next chunk's for a
    /// bridge), if the program reads them.
    pub next_validity: Option<wgpu::Buffer>,
}

/// One instanced draw: rows `rows` of chunk `chunk`. A layer draws its
/// spans in order; one span per chunk draws rows in upload order, and a
/// layer ordered by size draws them largest first (GUP-431).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    pub chunk: u32,
    pub rows: std::ops::Range<u32>,
}

/// A layer's draws: its chunks' buffers and the spans of rows to draw,
/// in draw order.
#[derive(Clone, Debug)]
pub(crate) struct Draws {
    pub chunks: Vec<ChunkDraw>,
    pub spans: Vec<Span>,
}

/// The `Encodings` uniform and LUTs (group 1): shared by every
/// [`LayerGpu`] built for one program, so appending rows (new chunks)
/// rebuilds only the chunk half.
pub(crate) struct EncodingsGpu {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

/// A data layer's GPU state, ready to draw: one instanced draw per column
/// chunk (RFC-001 §3).
pub(crate) struct LayerGpu {
    pub context: ContextId,
    pub program: Arc<GlueProgram>,
    encodings: Arc<EncodingsGpu>,
    /// One `Chunk` entry per chunk, [`DYNAMIC_ALIGN`] apart.
    chunk_buffer: wgpu::Buffer,
    /// Group 2: one bind group shared by every chunk, or one per chunk
    /// when the program reads validity bits (each chunk binds its own).
    chunk_bind_groups: Vec<wgpu::BindGroup>,
    pub chunks: Vec<ChunkDraw>,
    /// The instanced draws, in order, over `chunks`.
    pub spans: Vec<Span>,
    pub vertices_per_instance: u32,
}

impl std::fmt::Debug for LayerGpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LayerGpu")
            .field("signature", &self.program.glue.signature)
            .field("chunks", &self.chunks.len())
            .field("spans", &self.spans.len())
            .field("instances", &self.instances())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum PipelineKind {
    Mark(String),
    /// A mark's GPU pick variant (RFC-001 S9d), by glue signature.
    Pick(String),
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
    pick_bgl: Option<wgpu::BindGroupLayout>,
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

/// A chunk's validity bits: read-only storage in the vertex stage.
fn validity_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
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
    /// Forget every program and pipeline (after a GPU error). Layers keep
    /// their own `Arc`s and rebuild when their program changes.
    pub(crate) fn forget(&mut self) {
        self.programs.clear();
        self.pipelines.clear();
    }

    pub(crate) fn view_bgl(&mut self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        self.view_bgl
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("gup view"),
                    entries: &[uniform_entry(0, false)],
                })
            })
            .clone()
    }

    /// Group 3 of a pick pipeline: the `Pick` uniform, one entry per layer
    /// at a dynamic offset.
    pub(crate) fn pick_bgl(&mut self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        self.pick_bgl
            .get_or_insert_with(|| {
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("gup pick"),
                    entries: &[uniform_entry(0, true)],
                })
            })
            .clone()
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

/// Pipeline-cache lookups that create on a miss (GUP-410). A miss runs
/// inside a GPU error scope, which also serialises it with other threads'
/// scopes, so it checks the cache again before creating. The cache lock
/// is taken only briefly (scopes are always taken before context locks).
/// If the scope catches an error, the context forgets its pipelines, so
/// a failed creation is never cached.
impl Context {
    /// The linked program for `glue`, linking it on first use.
    pub(crate) fn program(&self, glue: &Glue) -> Result<Arc<GlueProgram>> {
        if let Some(p) = self.pipelines().programs.get(&glue.signature) {
            return Ok(Arc::clone(p));
        }
        self.scoped(
            || format!("glue program `{}` (bind group layouts)", glue.signature),
            || {
                if let Some(p) = self.pipelines().programs.get(&glue.signature) {
                    return Ok(Arc::clone(p));
                }
                let start = Instant::now();
                let wgsl = link(&glue.signature, &glue.source, &glue.modules)?;
                let link_time = start.elapsed();
                let device = self.device();
                let mut enc_entries = vec![uniform_entry(0, false)];
                for lut in &glue.luts {
                    enc_entries.extend(lut_entries(lut.binding));
                }
                let enc_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("gup encodings"),
                    entries: &enc_entries,
                });
                let mut chunk_entries = vec![uniform_entry(0, true)];
                if glue.validity {
                    chunk_entries.push(validity_entry(1));
                    if glue.polyline {
                        chunk_entries.push(validity_entry(2));
                    }
                }
                let chunk_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("gup chunk"),
                    entries: &chunk_entries,
                });
                let view_bgl = self.pipelines().view_bgl(device);
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("gup mark layout"),
                    bind_group_layouts: &[&view_bgl, &enc_bgl, &chunk_bgl],
                    push_constant_ranges: &[],
                });
                let program = Arc::new(GlueProgram {
                    glue: glue.clone(),
                    wgsl,
                    encodings: glue.encodings.clone(),
                    chunk: glue.chunk.clone(),
                    enc_bgl,
                    chunk_bgl,
                    layout,
                });
                let mut cache = self.pipelines();
                cache.stats.programs_linked += 1;
                cache.stats.last_link = link_time;
                cache
                    .programs
                    .insert(glue.signature.clone(), Arc::clone(&program));
                Ok(program)
            },
        )
    }

    /// The mark pipeline for `program` on a target like `desc`.
    pub(crate) fn mark_pipeline(
        &self,
        program: &GlueProgram,
        desc: &TargetDesc,
    ) -> Result<wgpu::RenderPipeline> {
        let key = PipelineKey {
            kind: PipelineKind::Mark(program.glue.signature.clone()),
            format: desc.format,
            samples: desc.samples,
        };
        if let Some(p) = self.pipelines().hit(&key) {
            return Ok(p);
        }
        let signature = &program.glue.signature;
        self.scoped(
            || {
                format!(
                    "mark pipeline `{signature}` ({:?}, {}× MSAA)",
                    desc.format, desc.samples
                )
            },
            || {
                if let Some(p) = self.pipelines().pipelines.get(&key) {
                    return Ok(p.clone());
                }
                let start = Instant::now();
                let module = self
                    .device()
                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some(signature),
                        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&program.wgsl)),
                    });
                let attributes = vertex_attributes(&program.glue);
                let buffers = vertex_buffers(&attributes);
                let pipeline = create_pipeline(
                    self.device(),
                    signature,
                    &program.layout,
                    &module,
                    &buffers,
                    desc,
                );
                let mut cache = self.pipelines();
                cache.stats.pipelines_created += 1;
                cache.stats.last_create = start.elapsed();
                cache.pipelines.insert(key.clone(), pipeline.clone());
                Ok(pipeline)
            },
        )
    }

    /// The GPU pick pipeline for `program` (RFC-001 S9d), or `None` when
    /// its mark does not draw in picks. Links the glue's pick variant on
    /// first use, into the same bind group layouts as the colour program
    /// plus the `Pick` uniform at group 3; one sample, an `Rg32Uint`
    /// colour target and a `LessEqual` depth test.
    pub(crate) fn pick_pipeline(
        &self,
        program: &GlueProgram,
    ) -> Result<Option<wgpu::RenderPipeline>> {
        let Some(source) = &program.glue.pick else {
            return Ok(None);
        };
        let signature = &program.glue.signature;
        let key = PipelineKey {
            kind: PipelineKind::Pick(signature.clone()),
            format: crate::gpu_pick::PICK_FORMAT,
            samples: 1,
        };
        if let Some(p) = self.pipelines().hit(&key) {
            return Ok(Some(p));
        }
        self.scoped(
            || format!("pick pipeline `{signature}`"),
            || {
                if let Some(p) = self.pipelines().pipelines.get(&key) {
                    return Ok(Some(p.clone()));
                }
                let start = Instant::now();
                let label = format!("{signature} (pick)");
                let wgsl = link(&label, source, &program.glue.modules)?;
                let device = self.device();
                let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(&label),
                    source: wgpu::ShaderSource::Wgsl(Cow::Owned(wgsl)),
                });
                let (view_bgl, pick_bgl) = {
                    let mut cache = self.pipelines();
                    (cache.view_bgl(device), cache.pick_bgl(device))
                };
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(&label),
                    bind_group_layouts: &[
                        &view_bgl,
                        &program.enc_bgl,
                        &program.chunk_bgl,
                        &pick_bgl,
                    ],
                    push_constant_ranges: &[],
                });
                let attributes = vertex_attributes(&program.glue);
                let buffers = vertex_buffers(&attributes);
                let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(&label),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs_pick"),
                        compilation_options: Default::default(),
                        buffers: &buffers,
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some("fs_pick"),
                        compilation_options: Default::default(),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: crate::gpu_pick::PICK_FORMAT,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: Some(wgpu::DepthStencilState {
                        format: crate::gpu_pick::PICK_DEPTH,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::LessEqual,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });
                let mut cache = self.pipelines();
                cache.stats.pipelines_created += 1;
                cache.stats.last_create = start.elapsed();
                cache.pipelines.insert(key.clone(), pipeline.clone());
                Ok(Some(pipeline))
            },
        )
    }

    /// The rule, rect or gradient pipeline on a target like `desc`.
    fn guide_pipeline(
        &self,
        kind: PipelineKind,
        desc: &TargetDesc,
    ) -> Result<wgpu::RenderPipeline> {
        let key = PipelineKey {
            kind: kind.clone(),
            format: desc.format,
            samples: desc.samples,
        };
        if let Some(p) = self.pipelines().hit(&key) {
            return Ok(p);
        }
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
            PipelineKind::Mark(_) | PipelineKind::Pick(_) => {
                unreachable!("mark pipelines come from glue programs")
            }
        };
        let device = self.device();
        self.scoped(
            || {
                format!(
                    "guide pipeline `{label}` ({:?}, {}× MSAA)",
                    desc.format, desc.samples
                )
            },
            || {
                let (view_bgl, lut_bgl) = {
                    let mut cache = self.pipelines();
                    if let Some(p) = cache.pipelines.get(&key) {
                        return Ok(p.clone());
                    }
                    (cache.view_bgl(device), cache.lut_bgl(device))
                };
                let groups = match kind {
                    PipelineKind::Gradient => vec![&view_bgl, &lut_bgl],
                    _ => vec![&view_bgl],
                };
                // Guide shaders are authored WGSL, flattened completely at
                // build time.
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
                let mut cache = self.pipelines();
                cache.stats.pipelines_created += 1;
                cache.pipelines.insert(key.clone(), pipeline.clone());
                Ok(pipeline)
            },
        )
    }
}

impl PipelineCache {
    /// A cached pipeline, counted as a hit.
    fn hit(&mut self, key: &PipelineKey) -> Option<wgpu::RenderPipeline> {
        let pipeline = self.pipelines.get(key).cloned();
        if pipeline.is_some() {
            self.stats.pipeline_hits += 1;
        }
        pipeline
    }
}

/// A mark's vertex attributes: its columns, then a polyline's next-row
/// columns, one buffer slot each.
fn vertex_attributes(glue: &Glue) -> Vec<wgpu::VertexAttribute> {
    glue.columns
        .iter()
        .chain(glue.successors.iter().map(|&c| &glue.columns[c]))
        .enumerate()
        .map(|(loc, (_, f))| wgpu::VertexAttribute {
            format: f.vertex_format(),
            offset: 0,
            shader_location: loc as u32,
        })
        .collect()
}

/// One per-instance buffer slot per attribute.
fn vertex_buffers(attributes: &[wgpu::VertexAttribute]) -> Vec<wgpu::VertexBufferLayout<'_>> {
    attributes
        .iter()
        .map(|a| wgpu::VertexBufferLayout {
            array_stride: a.format.size(),
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: std::slice::from_ref(a),
        })
        .collect()
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
    /// One `Chunk` struct per chunk, [`DYNAMIC_ALIGN`] apart (see
    /// [`chunk_uniforms`]).
    pub chunks: Vec<u8>,
    /// One LUT per `program.glue.luts` entry.
    pub luts: Vec<&'a [[u8; 4]]>,
}

/// Bytes of one `Chunk` uniform entry of `span` bytes per chunk, each at
/// a multiple of the 256-byte dynamic-offset alignment, with nothing past
/// the last entry (so one chunk writes only its `span` bytes). At least
/// one entry, so the binding is valid with no chunks.
pub(crate) fn chunk_uniforms(chunks: usize, span: u32) -> Vec<u8> {
    vec![0; chunk_uniform_offset(chunks.max(1) - 1) + span as usize]
}

/// The dynamic offset of chunk `index`'s `Chunk` uniform entry.
pub(crate) fn chunk_uniform_offset(index: usize) -> usize {
    index * DYNAMIC_ALIGN as usize
}

impl LayerUniforms<'_> {
    /// Upload uniforms and LUTs and build the layer's bind groups, inside
    /// a GPU error scope naming the layer's glue.
    pub(crate) fn build(
        self,
        cx: &Context,
        draws: Draws,
        vertices_per_instance: u32,
    ) -> Result<LayerGpu> {
        let signature = self.program.glue.signature.clone();
        cx.scoped(
            || format!("layer `{signature}` (uniforms and bind groups)"),
            || Ok(self.build_unscoped(cx, draws, vertices_per_instance)),
        )
    }

    fn build_unscoped(self, cx: &Context, draws: Draws, vertices_per_instance: u32) -> LayerGpu {
        let device = cx.device();
        let enc = cx.buffer_with_data(
            Upload::Uniform,
            "gup encodings",
            wgpu::BufferUsages::UNIFORM,
            &self.encodings,
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
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gup encodings"),
            layout: &self.program.enc_bgl,
            entries: &entries,
        });
        #[cfg_attr(
            target_arch = "wasm32",
            expect(
                clippy::arc_with_non_send_sync,
                reason = "`Arc` so native layers stay `Send + Sync`; wgpu's wasm handles \
                          are neither, so this `Arc` never crosses a thread there"
            )
        )]
        let encodings = Arc::new(EncodingsGpu {
            buffer: enc,
            bind_group,
        });
        LayerGpu::new(
            cx,
            self.program,
            encodings,
            &self.chunks,
            draws,
            vertices_per_instance,
        )
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
    /// The chunk half of a layer: the `Chunk` uniform buffer (written
    /// with `chunk_bytes`), its bind group and the chunk draws.
    fn new(
        cx: &Context,
        program: Arc<GlueProgram>,
        encodings: Arc<EncodingsGpu>,
        chunk_bytes: &[u8],
        Draws { chunks, spans }: Draws,
        vertices_per_instance: u32,
    ) -> Self {
        let chunk_buffer = cx.buffer_with_data(
            Upload::Uniform,
            "gup chunk uniforms",
            wgpu::BufferUsages::UNIFORM,
            chunk_bytes,
        );
        let uniform = wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &chunk_buffer,
                offset: 0,
                size: wgpu::BufferSize::new(u64::from(program.chunk.span)),
            }),
        };
        let bind_group = |c: Option<&ChunkDraw>| {
            let mut entries = vec![uniform.clone()];
            let validity = c.into_iter().flat_map(|c| [&c.validity, &c.next_validity]);
            entries.extend(validity.zip(1..).filter_map(|(v, binding)| {
                v.as_ref().map(|v| wgpu::BindGroupEntry {
                    binding,
                    resource: v.as_entire_binding(),
                })
            }));
            cx.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gup chunk"),
                layout: &program.chunk_bgl,
                entries: &entries,
            })
        };
        let chunk_bind_groups = if program.glue.validity {
            chunks.iter().map(|c| bind_group(Some(c))).collect()
        } else {
            vec![bind_group(None)]
        };
        Self {
            context: cx.id(),
            program,
            encodings,
            chunk_buffer,
            chunk_bind_groups,
            chunks,
            spans,
            vertices_per_instance,
        }
    }

    /// Instances across every chunk.
    pub(crate) fn instances(&self) -> u64 {
        self.chunks.iter().map(|c| u64::from(c.instances)).sum()
    }

    /// The `Encodings` bind group (group 1).
    pub(crate) fn enc_bind_group(&self) -> &wgpu::BindGroup {
        &self.encodings.bind_group
    }

    /// Whether this GPU state's `Encodings` half can take new uniform
    /// values from `program`: same context and program.
    pub(crate) fn reusable(&self, cx: &Context, program: &Arc<GlueProgram>) -> bool {
        self.context == cx.id() && Arc::ptr_eq(&self.program, program)
    }

    /// Whether the chunk draws are exactly `chunks` (same column and
    /// validity buffers and instance counts) drawn as `spans`, so a frame
    /// needs only uniform writes.
    pub(crate) fn draws_chunks<'a>(
        &self,
        mut chunks: impl ExactSizeIterator<Item = (&'a wgpu::Buffer, Option<&'a wgpu::Buffer>, u32)>,
        spans: &[Span],
    ) -> bool {
        self.spans == spans
            && chunks.len() == self.chunks.len()
            && self
                .chunks
                .iter()
                .zip(&mut chunks)
                .all(|(d, (buffer, validity, rows))| {
                    &d.columns == buffer && d.validity.as_ref() == validity && d.instances == rows
                })
    }

    /// Group 2 for chunk `index`: its own when the program reads validity
    /// bits, the shared one otherwise.
    /// Bind groups 1 and 2 and record the layer's draws into `pass`, whose
    /// pipeline and group 0 (and a pick's group 3) are set: one instanced
    /// draw per span of a chunk's rows, with the chunk's `Chunk` entry
    /// (row base, relative bases) and columns bound when the chunk changes.
    pub(crate) fn record(&self, pass: &mut wgpu::RenderPass<'_>, spans: &[Span]) {
        pass.set_bind_group(1, self.enc_bind_group(), &[]);
        let mut bound = None;
        for span in spans {
            let k = span.chunk as usize;
            if bound != Some(k) {
                let chunk = &self.chunks[k];
                pass.set_bind_group(2, self.chunk_bind_group(k), &[chunk.dynamic_offset]);
                for (slot, (buffer, range)) in chunk.slots.iter().enumerate() {
                    pass.set_vertex_buffer(slot as u32, buffer.slice(range.clone()));
                }
                bound = Some(k);
            }
            pass.draw(0..self.vertices_per_instance, span.rows.clone());
        }
    }

    pub(crate) fn chunk_bind_group(&self, index: usize) -> &wgpu::BindGroup {
        self.chunk_bind_groups
            .get(index)
            .unwrap_or(&self.chunk_bind_groups[0])
    }

    /// The same encodings drawn over new chunks (after an append): a new
    /// `Chunk` uniform buffer and draws; the `Encodings` uniform and LUTs
    /// are shared, not uploaded again.
    pub(crate) fn with_chunks(
        &self,
        cx: &Context,
        chunk_bytes: &[u8],
        draws: Draws,
    ) -> Result<Self> {
        let signature = &self.program.glue.signature;
        cx.scoped(
            || format!("layer `{signature}` (chunk uniforms)"),
            || {
                Ok(Self::new(
                    cx,
                    Arc::clone(&self.program),
                    Arc::clone(&self.encodings),
                    chunk_bytes,
                    draws,
                    self.vertices_per_instance,
                ))
            },
        )
    }

    /// Write new `Encodings` and `Chunk` uniform values (every chunk's
    /// entry, in one write) into the existing buffers: the per-frame path
    /// of a zoom or pan. Writes no column bytes and creates no GPU
    /// objects.
    pub(crate) fn write_uniforms(&self, cx: &Context, encodings: &[u8], chunks: &[u8]) {
        self.write_encodings(cx, encodings);
        cx.write_buffer(Upload::Uniform, &self.chunk_buffer, 0, chunks);
    }

    /// Write new `Encodings` uniform values into the existing buffer.
    pub(crate) fn write_encodings(&self, cx: &Context, encodings: &[u8]) {
        cx.write_buffer(Upload::Uniform, &self.encodings.buffer, 0, encodings);
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
pub(crate) struct ViewUniform {
    pub size: [f32; 2],
    pub dpr: f32,
    /// How far marks grow their quads (`gup::view::View::pick_margin`):
    /// 0 for colour, the pick tolerance in a GPU pick.
    pub pick_margin: f32,
}

/// One recorded draw.
enum Draw {
    Marks {
        pipeline: wgpu::RenderPipeline,
        layer: Arc<LayerGpu>,
        /// The layer's spans that can be seen (`MarkBatch::visible`).
        spans: Arc<[Span]>,
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
    /// The multisampled view a scene that needs MSAA draws into on a
    /// target with fewer samples ([`Scene::min_samples`]), kept for the
    /// target it was made for.
    msaa: Option<(TargetDesc, wgpu::TextureView)>,
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
                msaa: None,
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
        // Errors that a browser reported after an earlier call returned.
        cx.take_gpu_errors()?;
        cx.scoped(
            || {
                format!(
                    "scene preparation for a {}×{} {:?} target ({}× MSAA)",
                    desc.width, desc.height, desc.format, desc.samples
                )
            },
            || self.prepare_unscoped(cx, scene, desc),
        )
    }

    fn prepare_unscoped(
        &mut self,
        cx: &Context,
        scene: &Scene,
        desc: &TargetDesc,
    ) -> Result<Prepared> {
        let dpr = desc.dpr;
        let gpu = self.gpu(cx);
        cx.write_buffer(
            Upload::Uniform,
            &gpu.view_buffer,
            0,
            bytemuck::bytes_of(&ViewUniform {
                size: [scene.width, scene.height],
                dpr,
                pick_margin: 0.0,
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
                    let pipeline = cx.mark_pipeline(&batch.gpu.program, desc)?;
                    Some(Draw::Marks {
                        pipeline,
                        layer: Arc::clone(&batch.gpu),
                        spans: Arc::clone(&batch.visible),
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
    /// scene's background and, when multisampled, resolves into the
    /// frame's texture. A scene that needs more samples than the target
    /// has ([`Scene::min_samples`]: an area or polygon mark) is drawn into
    /// a multisampled texture the renderer keeps for the target's size.
    ///
    /// [`ImageTarget`]: crate::ImageTarget
    /// [`TextureTarget`]: crate::TextureTarget
    pub fn render(
        &mut self,
        cx: &Context,
        scene: &Scene,
        target: &mut dyn RenderTarget,
    ) -> Result<()> {
        let own = target.desc();
        let wanted = scene.min_samples();
        let desc = if wanted > own.samples && check_samples(cx, own.format, wanted).is_ok() {
            TargetDesc {
                samples: wanted,
                ..own
            }
        } else {
            own
        };
        cx.take_gpu_errors()?;
        cx.scoped(
            || {
                format!(
                    "drawing a scene into a {}×{} {:?} target ({}× MSAA)",
                    desc.width, desc.height, desc.format, desc.samples
                )
            },
            || {
                let frame = target.acquire(cx)?;
                let prepared = self.prepare(cx, scene, &desc)?;
                let msaa = (desc.samples != own.samples).then(|| self.msaa_view(cx, &desc));
                let mut encoder =
                    cx.device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("gup scene"),
                        });
                {
                    let clear = prepared.clear_color();
                    let attachment = match &msaa {
                        Some(view) => frame.attachment(Some(view), clear),
                        None => frame.color_attachment(clear),
                    };
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("gup scene"),
                        color_attachments: &[Some(attachment)],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    prepared.draw(&mut pass);
                }
                target.present(cx, frame, encoder.finish())
            },
        )
    }

    /// A multisampled colour view for a target like `desc`, reused while
    /// the target's size, format and the sample count stay the same.
    fn msaa_view(&mut self, cx: &Context, desc: &TargetDesc) -> wgpu::TextureView {
        let gpu = self.gpu(cx);
        let same = |d: &TargetDesc| {
            (d.format, d.width, d.height, d.samples)
                == (desc.format, desc.width, desc.height, desc.samples)
        };
        match &gpu.msaa {
            Some((d, view)) if same(d) => view.clone(),
            _ => {
                let view = msaa_view(cx, desc.format, desc.width, desc.height, desc.samples)
                    .expect("only called with more than one sample");
                gpu.msaa = Some((*desc, view.clone()));
                view
            }
        }
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
        let pipeline = cx.guide_pipeline(kind, desc)?;
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

    /// Number of draw calls [`draw`](Self::draw) records: one per span of
    /// rows of each data layer (one per column chunk, unless the layer is
    /// ordered by size and rows were appended) and one per guide or text
    /// batch. Tests pin
    /// it as a deterministic proxy for GPU cost (`PERF_BUDGETS.md`).
    pub fn draw_calls(&self) -> usize {
        self.draws
            .iter()
            .map(|(_, draw)| match draw {
                Draw::Marks { spans, .. } => spans.len(),
                Draw::Instanced { .. } | Draw::Text(_) => 1,
            })
            .sum()
    }

    /// Record every draw into `pass`. Allocates nothing, takes no locks
    /// and never submits, so it can run inside a host's pass.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>) {
        for ([x, y, w, h], draw) in &self.draws {
            pass.set_scissor_rect(*x, *y, *w, *h);
            match draw {
                Draw::Marks {
                    pipeline,
                    layer,
                    spans,
                } => {
                    pass.set_pipeline(pipeline);
                    pass.set_bind_group(0, &self.view_bind_group, &[]);
                    layer.record(pass, spans);
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

#[cfg(test)]
impl LayerGpu {
    /// This layer's GPU state drawn with `program` instead: seeds an
    /// invalid pipeline for the error-scope tests (GUP-410).
    pub(crate) fn with_program(&self, program: Arc<GlueProgram>) -> Self {
        Self {
            context: self.context,
            program,
            encodings: Arc::clone(&self.encodings),
            chunk_buffer: self.chunk_buffer.clone(),
            chunk_bind_groups: self.chunk_bind_groups.clone(),
            chunks: self.chunks.clone(),
            spans: self.spans.clone(),
            vertices_per_instance: self.vertices_per_instance,
        }
    }

    /// The shared `Encodings` half.
    pub(crate) fn encodings(&self) -> &Arc<EncodingsGpu> {
        &self.encodings
    }
}

#[cfg(test)]
impl PipelineCache {
    /// Cached mark pipelines (by glue signature).
    pub(crate) fn mark_pipelines(&self) -> Vec<String> {
        self.pipelines
            .keys()
            .filter_map(|k| match &k.kind {
                PipelineKind::Mark(signature) => Some(signature.clone()),
                _ => None,
            })
            .collect()
    }

    /// The view bind group layout (group 0 of every pipeline).
    pub(crate) fn view_layout(&mut self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
        self.view_bgl(device)
    }
}
