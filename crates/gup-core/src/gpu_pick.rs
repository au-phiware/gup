// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU picking (RFC-001 §9, step S9d): which row is drawn under a point,
//! for layers the CPU cannot read (`Retain::GpuOnly` past one chunk).
//!
//! A pick draws every pickable layer into a **one-texel** `Rg32Uint`
//! target whose centre is the pointer. Each mark's quad grows by the pick
//! tolerance (`View::pick_margin`), and its fragment computes the mark's
//! own signed distance to the pointer (`hit_distance`). The fragment
//! writes `(layer + 1, row + 1)` and a depth of 0 inside the mark, else
//! `0.5 + 0.5 · d / 3`; a `LessEqual` depth test in the colour pass's
//! draw order (layer `z`, layer index, draw rank) then keeps exactly the
//! CPU pick's winner: the topmost containing mark, else the nearest within
//! tolerance, ties to the topmost. The texel is copied into one of four
//! 256-byte readback buffers and mapped without blocking.

use crate::context::{Context, ContextId, Upload};
use crate::error::{Error, Result};
use crate::geom::Point;
use crate::pick::{PICK_TOLERANCE, RowId};
use crate::plot::Plot;
use crate::render::{DYNAMIC_ALIGN, ViewUniform};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

/// The pick target's colour format: `(layer + 1, row + 1)`, `(0, 0)` empty.
pub(crate) const PICK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Uint;
/// The pick target's depth format: the encoded distance.
pub(crate) const PICK_DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Picks in flight at once.
const IN_FLIGHT: usize = 4;
/// Bytes of one readback buffer: one copy row.
const READBACK_BYTES: u64 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as u64;
/// Where a test copies the depth texel in the readback buffer.
#[cfg(test)]
const DEPTH_OFFSET: u64 = 128;

/// Identifies a GPU pick from [`Plot::request_pick`] to its result.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PickTicket(u64);

/// A finished GPU pick, from [`Plot::poll_picks`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct GpuPick {
    /// The request's ticket.
    pub ticket: PickTicket,
    /// Where the pick was drawn (logical pixels).
    pub at: Point,
    /// The plot's resolve generation the pick was drawn at
    /// ([`Plot::resolve_generation`]).
    pub generation: u64,
    /// The topmost row drawn under `at`, or `None`.
    pub row: Option<RowId>,
}

/// The `Pick` uniform: the pointer in clip space and the layer's index.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct PickUniform {
    at: [f32; 2],
    layer: u32,
    pad: u32,
}

/// A readback buffer and the pick it holds, if any.
struct Slot {
    buffer: wgpu::Buffer,
    pending: Option<Pending>,
}

struct Pending {
    ticket: PickTicket,
    at: Point,
    generation: u64,
    /// Each layer's [`epoch`](crate::selection::Layer::epoch) when the
    /// drawn state was resolved: a layer that renumbered its stored rows
    /// since (a window eviction) reports no row.
    epochs: Vec<u64>,
    /// 0 while mapping, 1 mapped, 2 failed.
    state: Arc<AtomicU8>,
}

/// A plot's GPU pick state: the one-texel target, uniforms, readback
/// buffers and finished picks. Made on the first request.
pub(crate) struct GpuPicker {
    context: ContextId,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    #[cfg_attr(not(test), expect(dead_code, reason = "tests read the depth back"))]
    depth: wgpu::Texture,
    depth_view: wgpu::TextureView,
    view_buffer: wgpu::Buffer,
    view_bind_group: wgpu::BindGroup,
    /// One `Pick` entry per layer, [`DYNAMIC_ALIGN`] apart.
    pick_buffer: wgpu::Buffer,
    pick_bind_group: wgpu::BindGroup,
    slots: Vec<Slot>,
    next: u64,
    done: Vec<GpuPick>,
    /// The depth texel of each finished pick, by ticket (tests).
    #[cfg(test)]
    depths: Vec<(PickTicket, f32)>,
}

impl std::fmt::Debug for GpuPicker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuPicker")
            .field("in_flight", &self.in_flight())
            .field("done", &self.done.len())
            .finish_non_exhaustive()
    }
}

impl GpuPicker {
    fn new(cx: &Context, layers: usize) -> Self {
        let device = cx.device();
        let texture = |label, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC;
        let color = texture("gup pick", PICK_FORMAT, usage);
        let depth = texture("gup pick depth", PICK_DEPTH, usage);
        let view_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gup pick view"),
            size: std::mem::size_of::<ViewUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let (view_bgl, pick_bgl) = {
            let mut cache = cx.pipelines();
            (cache.view_bgl(device), cache.pick_bgl(device))
        };
        let view_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gup pick view"),
            layout: &view_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view_buffer.as_entire_binding(),
            }],
        });
        let (pick_buffer, pick_bind_group) = pick_entries(device, &pick_bgl, layers);
        let slots = (0..IN_FLIGHT)
            .map(|_| Slot {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gup pick readback"),
                    size: READBACK_BYTES,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                pending: None,
            })
            .collect();
        Self {
            context: cx.id(),
            color_view: color.create_view(&Default::default()),
            color,
            depth_view: depth.create_view(&Default::default()),
            depth,
            view_buffer,
            view_bind_group,
            pick_buffer,
            pick_bind_group,
            slots,
            next: 0,
            done: Vec::new(),
            #[cfg(test)]
            depths: Vec::new(),
        }
    }

    fn in_flight(&self) -> usize {
        self.slots.iter().filter(|s| s.pending.is_some()).count()
    }

    fn ticket(&mut self) -> PickTicket {
        self.next += 1;
        PickTicket(self.next)
    }
}

/// The `Pick` uniform buffer for `layers` layers and its bind group.
fn pick_entries(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    layers: usize,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let entry = std::mem::size_of::<PickUniform>() as u64;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("gup pick layers"),
        size: DYNAMIC_ALIGN * (layers.max(1) as u64 - 1) + entry,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("gup pick layers"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(entry),
            }),
        }],
    });
    (buffer, bind_group)
}

impl Plot {
    /// Draw a pick at `at` (logical pixels) on the GPU; its result comes
    /// from [`poll_picks`](Self::poll_picks) a frame or two later. Needs a
    /// resolve first.
    ///
    /// The rules are [`pick_row`](Self::pick_row)'s, and so are the
    /// results, but the GPU pick also reaches layers whose rows and CPU
    /// columns were released (`Retain::GpuOnly` past one chunk). It draws
    /// the marks of the last resolve. Layers whose mark does not declare
    /// `gpu_pick` (`#[mark(.., gpu_pick)]`, which the library marks do)
    /// are skipped. A point outside the plot rect picks nothing and does
    /// no GPU work. At most 4 picks are in flight; a fifth request is an
    /// error until [`poll_picks`](Self::poll_picks) returns one.
    pub fn request_pick(&mut self, cx: &Context, at: Point) -> Result<PickTicket> {
        let Some(layout) = self.layout() else {
            return Err(Error::config(
                "gpu pick",
                "the plot has not been resolved; call `resolve` before `request_pick`",
            ));
        };
        let (plot, size) = (layout.plot, [layout.width, layout.height]);
        if self.drawn.iter().any(|(_, gpu, _)| gpu.context != cx.id()) {
            return Err(Error::config(
                "gpu pick",
                "the plot was resolved on a different Context; resolve it again with this one",
            ));
        }
        let layers = self.layers.len();
        if self.gpu_pick.as_ref().is_none_or(|p| p.context != cx.id()) {
            // Picks in flight on another context are never reported.
            self.gpu_pick = Some(GpuPicker::new(cx, layers));
        }
        let generation = self.generation;
        let picker = self.gpu_pick.as_mut().expect("made above");
        if picker.pick_slots() < layers {
            let layout = cx.pipelines().pick_bgl(cx.device());
            (picker.pick_buffer, picker.pick_bind_group) =
                pick_entries(cx.device(), &layout, layers);
        }
        let inside = at.x >= plot.x
            && at.x <= plot.x + plot.width
            && at.y >= plot.y
            && at.y <= plot.y + plot.height;
        if !inside {
            let ticket = picker.ticket();
            picker.done.push(GpuPick {
                ticket,
                at,
                generation,
                row: None,
            });
            return Ok(ticket);
        }
        let Some(slot) = picker.slots.iter().position(|s| s.pending.is_none()) else {
            return Err(Error::config(
                "gpu pick",
                format!(
                    "{IN_FLIGHT} picks are in flight; take their results with `poll_picks` \
                     before requesting another"
                ),
            ));
        };

        // The draw order of the colour pass: z, then layer index.
        let mut order: Vec<usize> = (0..self.drawn.len()).collect();
        order.sort_by_key(|&i| self.drawn[i].0);
        let mut draws = Vec::with_capacity(order.len());
        for &i in &order {
            let gpu = &self.drawn[i].1;
            if let Some(pipeline) = cx.pick_pipeline(&gpu.program)? {
                draws.push((i, pipeline, Arc::clone(gpu), Arc::clone(&self.drawn[i].2)));
            }
        }

        // Uniforms: the view at dpr 1 with the pick margin, and each
        // layer's `Pick` entry.
        cx.write_buffer(
            Upload::Uniform,
            &picker.view_buffer,
            0,
            bytemuck::bytes_of(&ViewUniform {
                size,
                dpr: 1.0,
                pick_margin: PICK_TOLERANCE.0,
            }),
        );
        let ndc = [at.x / size[0] * 2.0 - 1.0, 1.0 - at.y / size[1] * 2.0];
        let entry = std::mem::size_of::<PickUniform>();
        let mut entries = vec![0u8; DYNAMIC_ALIGN as usize * (layers.max(1) - 1) + entry];
        for layer in 0..layers {
            let at = DYNAMIC_ALIGN as usize * layer;
            entries[at..at + entry].copy_from_slice(bytemuck::bytes_of(&PickUniform {
                at: ndc,
                layer: layer as u32,
                pad: 0,
            }));
        }
        cx.write_buffer(Upload::Uniform, &picker.pick_buffer, 0, &entries);

        let mut encoder = cx
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gup pick"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gup pick"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &picker.color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &picker.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            for (layer, pipeline, gpu, spans) in &draws {
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &picker.view_bind_group, &[]);
                pass.set_bind_group(
                    3,
                    &picker.pick_bind_group,
                    &[(DYNAMIC_ALIGN as usize * layer) as u32],
                );
                gpu.record(&mut pass, spans);
            }
        }
        let buffer = &picker.slots[slot].buffer;
        let copy = |encoder: &mut wgpu::CommandEncoder, texture: &wgpu::Texture, offset| {
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset,
                        bytes_per_row: Some(READBACK_BYTES as u32),
                        rows_per_image: Some(1),
                    },
                },
                texture.size(),
            );
        };
        copy(&mut encoder, &picker.color, 0);
        #[cfg(test)]
        copy(&mut encoder, &picker.depth, DEPTH_OFFSET);
        cx.submit([encoder.finish()]);

        let state = Arc::new(AtomicU8::new(0));
        let flag = Arc::clone(&state);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                flag.store(if result.is_ok() { 1 } else { 2 }, Ordering::Release);
            });
        let ticket = picker.ticket();
        picker.slots[slot].pending = Some(Pending {
            ticket,
            at,
            generation,
            epochs: self.drawn_epochs.clone(),
            state,
        });
        Ok(ticket)
    }

    /// Finished picks, oldest first. Never blocks: on native it polls the
    /// device once; on the web the browser maps buffers between frames.
    ///
    /// A row is mapped to its data row through its layer's draw order as
    /// it is when polled; compare [`GpuPick::generation`] with
    /// [`resolve_generation`](Self::resolve_generation) to discard picks
    /// drawn before the last resolve. A pick whose readback failed (a lost
    /// device), or whose layer evicted rows ([`Window`](crate::Window)) or
    /// evaluated its columns again since the pick was drawn, reports no row:
    /// its stored rows were renumbered, so it is dropped rather than mapped
    /// to another row.
    pub fn poll_picks(&mut self, cx: &Context) -> Vec<GpuPick> {
        self.collect_picks(cx);
        let Some(picker) = &mut self.gpu_pick else {
            return Vec::new();
        };
        let mut done = std::mem::take(&mut picker.done);
        done.sort_by_key(|p| p.ticket.0);
        done
    }

    /// Request a pick at `at`, wait for it and return its row (native
    /// tools and tests). Other requests' results stay for
    /// [`poll_picks`](Self::poll_picks).
    ///
    /// ```
    /// use gup_core::geom::Point;
    /// use gup_core::prelude::*;
    ///
    /// struct Row { x: f64, y: f64 }
    /// # fn main() -> gup_core::Result<()> {
    /// let rows = vec![Row { x: 1.0, y: 1.0 }, Row { x: 2.0, y: 3.0 }, Row { x: 3.0, y: 2.0 }];
    /// let cx = Context::new_blocking()?;
    /// let mut plot = Plot::new();
    /// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    /// plot.circles(rows).x(|r| r.x).y(|r| r.y);
    /// plot.resolve(&cx, 320.0, 200.0)?;
    /// for (a, b) in [(1.0, 1.0), (2.0, 3.0), (3.0, 2.0)] {
    ///     let at = Point::new(x.read().eval(a) as f32, y.read().eval(b) as f32);
    ///     assert!(plot.pick_row(at).is_some());
    ///     assert_eq!(plot.pick_row_gpu(&cx, at)?, plot.pick_row(at));
    /// }
    /// // Between the marks: nothing on either path.
    /// let empty = Point::new(x.read().eval(1.5) as f32, y.read().eval(2.5) as f32);
    /// assert_eq!(plot.pick_row_gpu(&cx, empty)?, None);
    /// assert_eq!(plot.pick_row(empty), None);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn pick_row_gpu(&mut self, cx: &Context, at: impl Into<Point>) -> Result<Option<RowId>> {
        let ticket = self.request_pick(cx, at.into())?;
        for wait in [false, true] {
            if wait {
                cx.wait_idle()?;
            }
            self.collect_picks(cx);
            let picker = self.gpu_pick.as_mut().expect("requested above");
            if let Some(i) = picker.done.iter().position(|p| p.ticket == ticket) {
                return Ok(picker.done.remove(i).row);
            }
        }
        Err(Error::Readback(
            "the GPU pick's readback did not finish after the device was idle".into(),
        ))
    }

    /// Move every finished readback into the picker's `done` list.
    fn collect_picks(&mut self, cx: &Context) {
        let Some(picker) = &mut self.gpu_pick else {
            return;
        };
        if picker.context != cx.id() {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = cx.device().poll(wgpu::PollType::Poll);
        for slot in &mut picker.slots {
            let state = match &slot.pending {
                Some(p) => p.state.load(Ordering::Acquire),
                None => continue,
            };
            if state == 0 {
                continue;
            }
            let pending = slot.pending.take().expect("checked above");
            let mut id = [0u32; 2];
            if state == 1 {
                {
                    let mapped = slot.buffer.slice(..).get_mapped_range();
                    id = bytemuck::pod_read_unaligned(&mapped[..8]);
                    #[cfg(test)]
                    {
                        let at = DEPTH_OFFSET as usize;
                        let depth: f32 = bytemuck::pod_read_unaligned(&mapped[at..at + 4]);
                        picker.depths.push((pending.ticket, depth));
                    }
                }
                slot.buffer.unmap();
            }
            let row = (id[0] != 0)
                .then(|| (id[0] as usize - 1, u64::from(id[1] - 1)))
                .and_then(|(layer, stored)| {
                    let l = self.layers.get(layer)?;
                    (pending.epochs.get(layer) == Some(&l.epoch())).then(|| RowId {
                        layer,
                        row: l.row_id(stored),
                    })
                });
            picker.done.push(GpuPick {
                ticket: pending.ticket,
                at: pending.at,
                generation: pending.generation,
                row,
            });
        }
    }
}

impl GpuPicker {
    /// How many layers' `Pick` entries the uniform buffer holds.
    fn pick_slots(&self) -> usize {
        let entry = std::mem::size_of::<PickUniform>() as u64;
        ((self.pick_buffer.size() - entry) / DYNAMIC_ALIGN + 1) as usize
    }
}

#[cfg(test)]
mod tests;
