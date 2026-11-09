// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Level-of-detail spike (GUP-631): three candidate designs for drawing a
//! scatter of tens of millions of rows, measured against today's path.
//! Headless, 1920 x 1080 by default, release only.
//!
//! ```text
//! cargo run -p gup-core --release --example lod_spike -- [options]
//!   --rows N            rows (default 10000000); x sorted, y a random walk
//!   --candidates LIST   comma list of base,density,pyramid,tiles
//!                       (default all four)
//!   --views LIST        comma list of full,zoom (default both); zoom is 10x
//!                       on x and y about the middle row
//!   --frames N          measured frames per view (default 30)
//!   --warmup N          unmeasured frames first (default 3)
//!   --max-secs S        stop measuring a view after S s of GPU time, once
//!                       5 frames are in (default 20)
//!   --size WxH          image size (default 1920x1080)
//!   --radius PX         circle radius (default 1.5, as scale_bench)
//!   --shuffle           permute the rows (chunks then overlap)
//!   --out DIR           write <candidate>-<view>-<rows>.png into DIR
//!   --pyramid-bias N    (B) draws N tiers finer than the
//!                       one-point-per-pixel tier (default 0)
//!   --fidelity          compare every candidate's image with the base
//!                       image of the same view (needs base in the list)
//! ```
//!
//! Candidates:
//!
//! - `base`: today's path, every row a circle (`Plot::add_columns`), with
//!   chunk culling.
//! - `density` (A): every visible row counted into a per-pixel `u32`
//!   buffer by a compute pass (atomics), then colour-mapped through
//!   `Sequential::viridis` (log count) in a full-screen pass drawn after the
//!   plot's guides, in the same render pass.
//! - `pyramid` (B): per chunk, the rows ordered by the coarsest tier of a
//!   global power-of-two grid in which each row is its cell's lowest
//!   index; a tier is a prefix of that order. Each frame draws, per
//!   visible chunk, the prefix of the finest tier whose cells are at least
//!   one pixel in area, through the library's circle pipeline.
//! - `tiles` (C): per chunk, count histograms of the same grid at tiers 8,
//!   10, 12 and 14; each frame merges the visible chunks' cells of the
//!   finest stored tier with cells of at least one pixel and draws them as
//!   `Rect`s coloured by log count; past tier 14 it draws every row.
//!
//! The prototypes for (B) and (C) draw what they select as a separate
//! columnar layer, built once per view, so the GPU pass is the library's
//! own pipeline over exactly the rows (or cells) the design would draw;
//! the per-frame selection is timed inside the CPU frame. (A) keeps its
//! own copy of the chunk columns, in the library's `F32Relative` layout.
//!
//! Every frame moves both domains by 1e-7 of their span, alternately, so
//! each frame resolves and prepares as an interactive one does. Each
//! prints `metric lod.<candidate>.<view>.<name> <value>` lines and a
//! summary table.

use gup_core::column::{ColumnFormat, ColumnStore};
use gup_core::prelude::*;
use gup_core::{Column, Renderer, TargetDesc};
use gup_visual_regression::Rgba8;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The finest grid tier: `2^TMAX` cells on each axis over the data extent.
const TMAX: u32 = 16;
/// The tiers (C) stores a histogram for.
const TILE_TIERS: [u32; 4] = [8, 10, 12, 14];
/// The image's background and the tolerance of the fidelity check.
const DELTA_E: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cand {
    Base,
    Density,
    Pyramid,
    Tiles,
}

impl Cand {
    fn name(self) -> &'static str {
        match self {
            Cand::Base => "base",
            Cand::Density => "density",
            Cand::Pyramid => "pyramid",
            Cand::Tiles => "tiles",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ViewKind {
    Full,
    Zoom,
}

impl ViewKind {
    fn name(self) -> &'static str {
        match self {
            ViewKind::Full => "full",
            ViewKind::Zoom => "zoom",
        }
    }
}

struct Options {
    rows: usize,
    candidates: Vec<Cand>,
    views: Vec<ViewKind>,
    frames: usize,
    warmup: usize,
    max_secs: f64,
    size: (u32, u32),
    radius: f32,
    shuffle: bool,
    out: Option<PathBuf>,
    fidelity: bool,
    seed: u64,
    /// Tiers finer than the one-point-per-pixel tier that (B) draws.
    bias: u32,
}

fn parse() -> Options {
    let mut o = Options {
        rows: 10_000_000,
        candidates: vec![Cand::Base, Cand::Density, Cand::Pyramid, Cand::Tiles],
        views: vec![ViewKind::Full, ViewKind::Zoom],
        frames: 30,
        warmup: 3,
        max_secs: 20.0,
        size: (1920, 1080),
        radius: 1.5,
        shuffle: false,
        out: None,
        fidelity: false,
        seed: 1,
        bias: 0,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{arg} needs a value"));
        match arg.as_str() {
            "--rows" => o.rows = value().parse().expect("--rows N"),
            "--candidates" => {
                o.candidates = value()
                    .split(',')
                    .map(|c| match c {
                        "base" => Cand::Base,
                        "density" => Cand::Density,
                        "pyramid" => Cand::Pyramid,
                        "tiles" => Cand::Tiles,
                        _ => panic!("unknown candidate {c}"),
                    })
                    .collect()
            }
            "--views" => {
                o.views = value()
                    .split(',')
                    .map(|v| match v {
                        "full" => ViewKind::Full,
                        "zoom" => ViewKind::Zoom,
                        _ => panic!("unknown view {v}"),
                    })
                    .collect()
            }
            "--frames" => o.frames = value().parse().expect("--frames N"),
            "--warmup" => o.warmup = value().parse().expect("--warmup N"),
            "--max-secs" => o.max_secs = value().parse().expect("--max-secs S"),
            "--size" => {
                let v = value();
                let (w, h) = v.split_once('x').expect("--size WxH");
                o.size = (w.parse().expect("width"), h.parse().expect("height"));
            }
            "--radius" => o.radius = value().parse().expect("--radius PX"),
            "--shuffle" => o.shuffle = true,
            "--out" => o.out = Some(PathBuf::from(value())),
            "--fidelity" => o.fidelity = true,
            "--seed" => o.seed = value().parse().expect("--seed N"),
            "--pyramid-bias" => o.bias = value().parse().expect("--pyramid-bias N"),
            _ => panic!("unknown option {arg}"),
        }
    }
    o
}

/// A 64-bit LCG (Knuth's MMIX constants), as `scale_bench`.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }
    fn unit(&mut self) -> f64 {
        self.next() as f64 / (1u64 << 53) as f64
    }
}

fn metric(cand: Cand, view: Option<ViewKind>, name: &str, value: impl std::fmt::Display) {
    match view {
        Some(v) => println!("metric lod.{}.{}.{name} {value}", cand.name(), v.name()),
        None => println!("metric lod.{}.{name} {value}", cand.name()),
    }
}

fn read_line(path: &str, key: &str) -> Option<f64> {
    let s = std::fs::read_to_string(path).ok()?;
    let line = s.lines().find(|l| l.starts_with(key))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Median and nearest-rank p95.
fn stats(v: &[f64]) -> (f64, f64) {
    let mut v = v.to_vec();
    v.sort_by(f64::total_cmp);
    if v.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let p95 = v[((v.len() * 95).div_ceil(100)).clamp(1, v.len()) - 1];
    (v[v.len() / 2], p95)
}

/// The i915 actual GPU clock, if the driver exposes it.
fn gpu_clock_mhz() -> Option<f64> {
    let dir = std::fs::read_dir("/sys/class/drm").ok()?;
    for entry in dir.flatten() {
        let p = entry.path().join("gt_act_freq_mhz");
        if let Ok(s) = std::fs::read_to_string(&p) {
            return s.trim().parse().ok();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

/// One chunk of the source columns, as the library chunks them.
struct ChunkInfo {
    rows: std::ops::Range<usize>,
    x: (f64, f64),
    y: (f64, f64),
}

struct Data {
    xs: Arc<[f64]>,
    ys: Arc<[f64]>,
    chunks: Vec<ChunkInfo>,
    /// The data extent the grid covers.
    ext: ((f64, f64), (f64, f64)),
}

impl Data {
    fn new(o: &Options, chunk_rows: usize) -> Self {
        let n = o.rows;
        let mut rng = Lcg(o.seed);
        let step = 1000.0 / n.max(1) as f64;
        let mut y = 0.0f64;
        let mut xs: Arc<[f64]> = (0..n).map(|i| i as f64 * step).collect();
        let mut ys: Arc<[f64]> = (0..n)
            .map(|_| {
                y += rng.unit() - 0.5;
                y
            })
            .collect();
        if o.shuffle {
            let xm = Arc::get_mut(&mut xs).expect("unique");
            let ym = Arc::get_mut(&mut ys).expect("unique");
            for i in (1..n).rev() {
                let j = (rng.next() % (i as u64 + 1)) as usize;
                xm.swap(i, j);
                ym.swap(i, j);
            }
        }
        let extent = |v: &[f64]| {
            v.iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
                    (a.min(v), b.max(v))
                })
        };
        let chunks = (0..n.div_ceil(chunk_rows))
            .map(|k| {
                let rows = k * chunk_rows..((k + 1) * chunk_rows).min(n);
                ChunkInfo {
                    x: extent(&xs[rows.clone()]),
                    y: extent(&ys[rows.clone()]),
                    rows,
                }
            })
            .collect::<Vec<_>>();
        let ext = (
            (
                chunks.iter().map(|c| c.x.0).fold(f64::INFINITY, f64::min),
                chunks
                    .iter()
                    .map(|c| c.x.1)
                    .fold(f64::NEG_INFINITY, f64::max),
            ),
            (
                chunks.iter().map(|c| c.y.0).fold(f64::INFINITY, f64::min),
                chunks
                    .iter()
                    .map(|c| c.y.1)
                    .fold(f64::NEG_INFINITY, f64::max),
            ),
        );
        Self {
            xs,
            ys,
            chunks,
            ext,
        }
    }

    /// The domains of `view`: the extent padded by 2%, or a tenth of it
    /// on each axis about the middle row.
    fn domains(&self, view: ViewKind) -> ((f64, f64), (f64, f64)) {
        let ((x0, x1), (y0, y1)) = self.ext;
        let (px, py) = (0.02 * (x1 - x0), 0.02 * (y1 - y0));
        let full = ((x0 - px, x1 + px), (y0 - py, y1 + py));
        match view {
            ViewKind::Full => full,
            ViewKind::Zoom => {
                let mid = self.xs.len() / 2;
                let (cx, cy) = (self.xs[mid], self.ys[mid]);
                let (hx, hy) = ((full.0.1 - full.0.0) / 20.0, (full.1.1 - full.1.0) / 20.0);
                ((cx - hx, cx + hx), (cy - hy, cy + hy))
            }
        }
    }

    /// The grid cell of a value at [`TMAX`].
    fn cell(&self, x: f64, y: f64) -> (u32, u32) {
        let ((x0, x1), (y0, y1)) = self.ext;
        let m = f64::from(1u32 << TMAX);
        let q =
            |v: f64, a: f64, b: f64| (((v - a) / (b - a)) * m).floor().clamp(0.0, m - 1.0) as u32;
        (q(x, x0, x1), q(y, y0, y1))
    }
}

/// The plot rect and domains of a view: what the per-frame selections
/// need.
#[derive(Clone, Copy)]
struct View {
    dom: ((f64, f64), (f64, f64)),
    rect: gup_core::geom::Rect,
}

impl View {
    /// The finest tier whose cells are at least one pixel in area.
    fn tier(&self, data: &Data) -> u32 {
        let ((x0, x1), (y0, y1)) = data.ext;
        let ((d0, d1), (e0, e1)) = self.dom;
        let a0 = f64::from(self.rect.width) * (x1 - x0) / (d1 - d0)
            * f64::from(self.rect.height)
            * (y1 - y0)
            / (e1 - e0);
        // Past `TMAX` no chunk has a tier: every row is drawn.
        (0.5 * a0.log2()).floor().clamp(0.0, 31.0) as u32
    }

    /// Whether a chunk's box, grown by `margin` px, meets the plot rect.
    fn sees(&self, c: &ChunkInfo, margin: f64) -> bool {
        let ((d0, d1), (e0, e1)) = self.dom;
        let mx = margin * (d1 - d0) / f64::from(self.rect.width);
        let my = margin * (e1 - e0) / f64::from(self.rect.height);
        c.x.1 + mx >= d0 && c.x.0 - mx <= d1 && c.y.1 + my >= e0 && c.y.0 - my <= e1
    }
}

// ---------------------------------------------------------------------------
// The grid: Morton keys
// ---------------------------------------------------------------------------

fn part1by1(v: u32) -> u64 {
    let mut x = u64::from(v) & 0xffff;
    x = (x | (x << 8)) & 0x00ff_00ff;
    x = (x | (x << 4)) & 0x0f0f_0f0f;
    x = (x | (x << 2)) & 0x3333_3333;
    x = (x | (x << 1)) & 0x5555_5555;
    x
}

fn compact1by1(v: u64) -> u32 {
    let mut x = v & 0x5555_5555;
    x = (x | (x >> 1)) & 0x3333_3333;
    x = (x | (x >> 2)) & 0x0f0f_0f0f;
    x = (x | (x >> 4)) & 0x00ff_00ff;
    x = (x | (x >> 8)) & 0x0000_ffff;
    x as u32
}

/// A chunk's rows as `morton << 20 | local index`, sorted: each tier's
/// cells are runs of equal `key >> 2 (TMAX - t)`, the first of a run its
/// lowest index.
fn sorted_keys(data: &Data, c: &ChunkInfo) -> Vec<u64> {
    let mut keys: Vec<u64> = c
        .rows
        .clone()
        .map(|i| {
            let (cx, cy) = data.cell(data.xs[i], data.ys[i]);
            ((part1by1(cx) | (part1by1(cy) << 1)) << 20) | (i - c.rows.start) as u64
        })
        .collect();
    keys.sort_unstable();
    keys
}

// ---------------------------------------------------------------------------
// (B) the decimation pyramid
// ---------------------------------------------------------------------------

/// One chunk's tiers: `order[..counts[t]]` is tier `t` for `t <= stored`.
struct PyramidChunk {
    order: Vec<u32>,
    counts: Vec<u32>,
}

impl PyramidChunk {
    fn build(data: &Data, c: &ChunkInfo) -> Self {
        let keys = sorted_keys(data, c);
        // The coarsest tier at which each row starts a run (TMAX + 1: it
        // never does, a later row of its finest cell).
        let mut first = Vec::with_capacity(keys.len());
        let mut prev: Option<u64> = None;
        for &k in &keys {
            let key = k >> 20;
            let t = match prev {
                None => 0,
                Some(p) if p == key => TMAX + 1,
                Some(p) => TMAX - (63 - (p ^ key).leading_zeros()) / 2,
            };
            first.push((t, (k & 0xf_ffff) as u32));
            prev = Some(key);
        }
        let mut hist = vec![0u32; TMAX as usize + 2];
        for &(t, _) in &first {
            hist[t as usize] += 1;
        }
        let mut counts = Vec::with_capacity(TMAX as usize + 1);
        let mut sum = 0;
        let half = (c.rows.len() / 2) as u32;
        for &h in &hist[..=TMAX as usize] {
            sum += h;
            if sum > half {
                break;
            }
            counts.push(sum);
        }
        let stored = counts.len() as u32;
        let mut order: Vec<(u32, u32)> = first.into_iter().filter(|&(t, _)| t < stored).collect();
        order.sort_unstable();
        Self {
            // A fresh allocation: an in-place collect would keep 8 B a row.
            order: order.iter().map(|&(_, i)| i).collect(),
            counts,
        }
    }
}

struct Pyramid {
    chunks: Vec<PyramidChunk>,
}

impl Pyramid {
    fn build(data: &Data) -> Self {
        Self {
            chunks: data
                .chunks
                .iter()
                .map(|c| PyramidChunk::build(data, c))
                .collect(),
        }
    }

    /// Stored representatives.
    fn stored(&self) -> u64 {
        self.chunks.iter().map(|c| c.order.len() as u64).sum()
    }

    /// Per visible chunk: its tier prefix, or `None` for every row.
    fn select(
        &self,
        data: &Data,
        view: &View,
        margin: f64,
        bias: u32,
    ) -> Vec<(usize, Option<u32>)> {
        let t = (view.tier(data) + bias) as usize;
        data.chunks
            .iter()
            .enumerate()
            .filter(|(_, c)| view.sees(c, margin))
            .map(|(k, _)| (k, self.chunks[k].counts.get(t).copied()))
            .collect()
    }

    fn gather(&self, data: &Data, sel: &[(usize, Option<u32>)]) -> (Vec<f64>, Vec<f64>) {
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        for &(k, n) in sel {
            let c = &data.chunks[k];
            match n {
                Some(n) => {
                    for &i in &self.chunks[k].order[..n as usize] {
                        let i = c.rows.start + i as usize;
                        xs.push(data.xs[i]);
                        ys.push(data.ys[i]);
                    }
                }
                None => {
                    xs.extend_from_slice(&data.xs[c.rows.clone()]);
                    ys.extend_from_slice(&data.ys[c.rows.clone()]);
                }
            }
        }
        (xs, ys)
    }
}

// ---------------------------------------------------------------------------
// (C) binned tiles
// ---------------------------------------------------------------------------

/// One chunk's histograms: `(tier, cells as (key at that tier, count))`.
struct TileChunk {
    tiers: Vec<(u32, Vec<(u32, u32)>)>,
}

impl TileChunk {
    fn build(data: &Data, c: &ChunkInfo) -> Self {
        let keys = sorted_keys(data, c);
        let half = c.rows.len() / 2;
        let mut tiers = Vec::new();
        for t in TILE_TIERS {
            let shift = 20 + 2 * (TMAX - t);
            let mut cells: Vec<(u32, u32)> = Vec::new();
            for &k in &keys {
                let key = (k >> shift) as u32;
                match cells.last_mut() {
                    Some((last, n)) if *last == key => *n += 1,
                    _ => cells.push((key, 1)),
                }
            }
            if cells.len() > half {
                break;
            }
            cells.shrink_to_fit();
            tiers.push((t, cells));
        }
        Self { tiers }
    }
}

struct Tiles {
    chunks: Vec<TileChunk>,
}

/// What (C) draws for a view: cells of `tier` merged over the visible
/// chunks, `(key, count)`, and every row of the visible chunks that have
/// no histogram that fine (none past tier 14, or too dense for `tier`).
struct TileDraw {
    tier: u32,
    cells: Vec<(u32, u32)>,
    full: Vec<usize>,
}

impl Tiles {
    fn build(data: &Data) -> Self {
        Self {
            chunks: data
                .chunks
                .iter()
                .map(|c| TileChunk::build(data, c))
                .collect(),
        }
    }

    fn stored(&self) -> u64 {
        self.chunks
            .iter()
            .flat_map(|c| &c.tiers)
            .map(|(_, v)| v.len() as u64)
            .sum()
    }

    fn select(&self, data: &Data, view: &View, margin: f64) -> TileDraw {
        let t = view.tier(data);
        let tier = TILE_TIERS.iter().rev().find(|&&s| s <= t).copied();
        let mut draw = TileDraw {
            tier: tier.unwrap_or(0),
            cells: Vec::new(),
            full: Vec::new(),
        };
        for k in (0..data.chunks.len()).filter(|&k| view.sees(&data.chunks[k], margin)) {
            let stored = tier
                .filter(|_| t <= *TILE_TIERS.last().unwrap())
                .and_then(|tier| self.chunks[k].tiers.iter().find(|(s, _)| *s == tier));
            match stored {
                Some((_, v)) => draw.cells.extend_from_slice(v),
                None => draw.full.push(k),
            }
        }
        draw.cells.sort_unstable_by_key(|c| c.0);
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(draw.cells.len());
        for &(key, n) in &draw.cells {
            match merged.last_mut() {
                Some((last, m)) if *last == key => *m += n,
                _ => merged.push((key, n)),
            }
        }
        draw.cells = merged;
        draw
    }
}

// ---------------------------------------------------------------------------
// The GPU side: a target with timestamps
// ---------------------------------------------------------------------------

struct Gpu {
    cx: Context,
    desc: TargetDesc,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    queries: Option<wgpu::QuerySet>,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    renderer: Renderer,
}

impl Gpu {
    fn new(w: u32, h: u32) -> gup_core::Result<Self> {
        let cx = Context::new_blocking()?;
        let device = cx.device();
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lod spike target"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
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
        let queries = cx
            .caps()
            .features
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| {
                device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("lod spike pass"),
                    ty: wgpu::QueryType::Timestamp,
                    count: 2,
                })
            });
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamps"),
            size: 16,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamps readback"),
            size: 16,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let info = cx.adapter_info().expect("own device");
        println!(
            "adapter: {} ({:?}, driver {} {}); timestamps {}",
            info.name,
            info.backend,
            info.driver,
            info.driver_info,
            if queries.is_some() { "on" } else { "OFF" }
        );
        Ok(Self {
            desc: TargetDesc {
                format,
                width: w,
                height: h,
                dpr: 1.0,
                samples: 1,
            },
            cx,
            texture,
            view,
            queries,
            resolve,
            read,
            renderer: Renderer::new(),
        })
    }

    fn wait(&self) {
        self.cx
            .device()
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
    }

    /// The last frame's GPU time between its two timestamps.
    fn gpu_ms(&self) -> f64 {
        let slice = self.read.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        self.wait();
        let t: Vec<u64> = slice
            .get_mapped_range()
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
            .collect();
        self.read.unmap();
        let period = f64::from(self.cx.queue().get_timestamp_period());
        t[1].wrapping_sub(t[0]) as f64 * period / 1e6
    }

    fn colour_attachment(&self, clear: wgpu::Color) -> wgpu::RenderPassColorAttachment<'_> {
        wgpu::RenderPassColorAttachment {
            view: &self.view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear),
                store: wgpu::StoreOp::Store,
            },
        }
    }

    /// Resolve the timestamps into the readback buffer and submit.
    fn finish(&self, mut encoder: wgpu::CommandEncoder) {
        if let Some(q) = &self.queries {
            encoder.resolve_query_set(q, 0..2, &self.resolve, 0);
            encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, 16);
        }
        self.cx.queue().submit([encoder.finish()]);
    }

    fn image(&self) -> image::RgbaImage {
        let (w, h) = (self.desc.width, self.desc.height);
        let bpr = (w * 4).div_ceil(256) * 256;
        let buffer = self.cx.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("lod spike readback"),
            size: u64::from(bpr * h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.cx.device().create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.cx.queue().submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        self.wait();
        let bytes = slice.get_mapped_range();
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in bytes.chunks_exact(bpr as usize) {
            out.extend_from_slice(&row[..(w * 4) as usize]);
        }
        image::RgbaImage::from_raw(w, h, out).expect("size")
    }
}

// ---------------------------------------------------------------------------
// A plot drawn by the library, with its scale handles
// ---------------------------------------------------------------------------

struct LibPlot {
    plot: Plot,
    x: ScaleRef<Linear>,
    y: ScaleRef<Linear>,
}

impl LibPlot {
    fn new() -> Self {
        let mut plot = Plot::new();
        plot.legends(false);
        let x = plot.x(Linear::new());
        let y = plot.y(Linear::new());
        Self { plot, x, y }
    }

    fn circles(xs: impl Into<Column>, ys: impl Into<Column>, radius: f32) -> Self {
        let mut p = Self::new();
        let cols = Columns::new()
            .with("x", xs)
            .and_then(|c| c.with("y", ys))
            .expect("columns");
        p.plot
            .add_columns::<Circle>(&cols)
            .x_column(&cols["x"])
            .y_column(&cols["y"])
            .attr(Circle::RADIUS, Px(radius));
        p
    }

    fn set(&self, dom: ((f64, f64), (f64, f64)), jitter: f64) -> gup_core::Result<()> {
        let ((d0, d1), (e0, e1)) = dom;
        let (jx, jy) = (jitter * (d1 - d0), jitter * (e1 - e0));
        self.x.write().set_domain(d0 + jx, d1 + jx)?;
        self.y.write().set_domain(e0 + jy, e1 + jy)
    }
}

/// One measured view of one candidate.
#[derive(Default, Clone)]
struct Measured {
    gpu: Vec<f64>,
    cpu: Vec<f64>,
    clock: Vec<f64>,
    drawn: u64,
    draws: usize,
    image: Option<image::RgbaImage>,
    /// Per-view preparation the prototype does once (a gather) and the
    /// design does not, ms.
    setup_ms: f64,
}

fn jitter(frame: usize) -> f64 {
    if frame.is_multiple_of(2) { 1e-7 } else { -1e-7 }
}

/// Measure frames of a library plot: `extra` runs inside the CPU frame
/// (a candidate's per-frame selection).
fn measure_lib(
    gpu: &mut Gpu,
    o: &Options,
    lib: &mut LibPlot,
    dom: ((f64, f64), (f64, f64)),
    mut extra: impl FnMut(),
) -> gup_core::Result<Measured> {
    let (w, h) = o.size;
    let mut m = Measured::default();
    let mut spent = 0.0;
    let total = o.warmup + o.frames + 1;
    let mut frame = 0;
    while frame < total {
        let measured = frame >= o.warmup && frame < o.warmup + o.frames;
        let last = frame + 1 == total;
        let start = Instant::now();
        lib.set(dom, if last { 0.0 } else { jitter(frame) })?;
        extra();
        let resolved = lib.plot.resolve(&gpu.cx, w as f32, h as f32)?;
        let prepared = gpu.renderer.prepare(&gpu.cx, &resolved.scene, &gpu.desc)?;
        m.draws = prepared.draw_calls();
        let mut encoder = gpu.cx.device().create_command_encoder(&Default::default());
        {
            let timestamp_writes = gpu
                .queries
                .as_ref()
                .map(|q| wgpu::RenderPassTimestampWrites {
                    query_set: q,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lod spike scene"),
                color_attachments: &[Some(gpu.colour_attachment(prepared.clear_color()))],
                depth_stencil_attachment: None,
                timestamp_writes,
                occlusion_query_set: None,
            });
            prepared.draw(&mut pass);
        }
        gpu.finish(encoder);
        let cpu = ms(start.elapsed());
        let clock = gpu_clock_mhz();
        let g = gpu.gpu_ms();
        if measured {
            m.cpu.push(cpu);
            m.gpu.push(g);
            m.clock.extend(clock);
            spent += g / 1000.0;
            if spent > o.max_secs && m.gpu.len() >= 5 {
                // On to the unjittered last frame, for the image.
                frame = total - 1;
                continue;
            }
        }
        if last {
            m.image = Some(gpu.image());
        }
        frame += 1;
    }
    Ok(m)
}

// ---------------------------------------------------------------------------
// (A) density per frame
// ---------------------------------------------------------------------------

const DENSITY_WGSL: &str = r#"
struct Params {
    base: vec2<f32>,
    k: vec2<f32>,
    r0: vec2<f32>,
    rows: u32,
    width: u32,
    rect: vec4<u32>,
}

@group(0) @binding(0) var<uniform> p: Params;
@group(0) @binding(1) var<storage, read> xs: array<f32>;
@group(0) @binding(2) var<storage, read> ys: array<f32>;
@group(0) @binding(3) var<storage, read_write> counts: array<atomic<u32>>;

@compute @workgroup_size(256)
fn accumulate(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= p.rows {
        return;
    }
    let q = floor(p.r0 + (vec2<f32>(xs[i], ys[i]) + p.base) * p.k);
    let lo = vec2<f32>(p.rect.xy);
    let hi = vec2<f32>(p.rect.zw);
    if any(q < lo) || any(q >= hi) {
        return;
    }
    atomicAdd(&counts[u32(q.y) * p.width + u32(q.x)], 1u);
}
"#;

const REDUCE_WGSL: &str = r#"
struct Frame {
    width: u32,
    pixels: u32,
}

@group(0) @binding(0) var<uniform> f: Frame;
@group(0) @binding(1) var<storage, read_write> all: array<atomic<u32>>;

var<workgroup> partial: array<u32, 256>;

@compute @workgroup_size(256)
fn clear(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x <= f.pixels {
        atomicStore(&all[gid.x], 0u);
    }
}

@compute @workgroup_size(256)
fn maximum(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    var v = 0u;
    if gid.x < f.pixels {
        v = atomicLoad(&all[gid.x]);
    }
    partial[li] = v;
    workgroupBarrier();
    for (var s = 128u; s > 0u; s = s >> 1u) {
        if li < s {
            partial[li] = max(partial[li], partial[li + s]);
        }
        workgroupBarrier();
    }
    if li == 0u && partial[0] > 0u {
        atomicMax(&all[f.pixels], partial[0]);
    }
}
"#;

const COMPOSITE_WGSL: &str = r#"
struct Frame {
    width: u32,
    pixels: u32,
}

@group(0) @binding(0) var<uniform> f: Frame;
@group(0) @binding(1) var<storage, read> counts: array<u32>;
@group(0) @binding(2) var<uniform> lut: array<vec4<f32>, 256>;

@vertex
fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let c = counts[u32(pos.y) * f.width + u32(pos.x)];
    if c == 0u {
        discard;
    }
    let top = max(counts[f.pixels], 1u);
    let t = log(1.0 + f32(c)) / log(1.0 + f32(top));
    return lut[min(u32(t * 255.0 + 0.5), 255u)];
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DensityParams {
    base: [f32; 2],
    k: [f32; 2],
    r0: [f32; 2],
    rows: u32,
    width: u32,
    rect: [u32; 4],
}

/// One chunk's columns on the GPU, `F32Relative` (value minus origin).
struct DensityChunk {
    origin: (f64, f64),
    rows: u32,
    group: wgpu::BindGroup,
}

struct Density {
    chunks: Vec<DensityChunk>,
    params: wgpu::Buffer,
    /// Held for the bind groups that read it.
    _frame: wgpu::Buffer,
    accumulate: wgpu::ComputePipeline,
    clear: wgpu::ComputePipeline,
    maximum: wgpu::ComputePipeline,
    reduce_group: wgpu::BindGroup,
    composite: wgpu::RenderPipeline,
    composite_group: wgpu::BindGroup,
    pixels: u32,
    /// The design's extra GPU bytes (counts and LUT), and the prototype's
    /// own copy of the columns, which the design would not need.
    extra_bytes: u64,
    copy_bytes: u64,
}

const STRIDE: u64 = 256;

impl Density {
    fn new(gpu: &Gpu, data: &Data) -> Self {
        let device = gpu.cx.device();
        let (w, h) = (gpu.desc.width, gpu.desc.height);
        let pixels = w * h;
        let counts = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("density counts"),
            size: u64::from(pixels + 1) * 4,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("density params"),
            size: STRIDE * data.chunks.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("density frame"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.cx
            .queue()
            .write_buffer(&frame, 0, bytemuck::cast_slice(&[w, pixels, 0, 0]));
        let ramp = Sequential::viridis().domain(0.0, 1.0).ramp();
        let lut_data: Vec<[f32; 4]> = (0..256)
            .map(|i| {
                let c = ramp.color_at(f64::from(i) / 255.0);
                [c.r, c.g, c.b, 1.0]
            })
            .collect();
        let lut = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("density lut"),
            size: 256 * 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        gpu.cx
            .queue()
            .write_buffer(&lut, 0, bytemuck::cast_slice(&lut_data));

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("density"),
            source: wgpu::ShaderSource::Wgsl(DENSITY_WGSL.into()),
        });
        let reduce = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("density reduce"),
            source: wgpu::ShaderSource::Wgsl(REDUCE_WGSL.into()),
        });
        let uniform = |binding, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: None,
            },
            count: None,
        };
        let storage = |binding, read_only, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let cs = wgpu::ShaderStages::COMPUTE;
        let acc_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("density accumulate"),
            entries: &[
                uniform(0, true),
                storage(1, true, cs),
                storage(2, true, cs),
                storage(3, false, cs),
            ],
        });
        let red_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("density reduce"),
            entries: &[uniform(0, false), storage(1, false, cs)],
        });
        let pipeline =
            |module: &wgpu::ShaderModule, layout: &wgpu::BindGroupLayout, entry: &str| {
                let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[layout],
                    push_constant_ranges: &[],
                });
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&pl),
                    module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            };
        let accumulate = pipeline(&module, &acc_layout, "accumulate");
        let clear = pipeline(&reduce, &red_layout, "clear");
        let maximum = pipeline(&reduce, &red_layout, "maximum");

        let mut copy_bytes = 0;
        let chunks = data
            .chunks
            .iter()
            .map(|c| {
                let origin = (data.xs[c.rows.start], data.ys[c.rows.start]);
                let rel =
                    |v: &[f64], o: f64| -> Vec<f32> { v.iter().map(|&v| (v - o) as f32).collect() };
                let upload = |label, v: Vec<f32>| {
                    let b = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some(label),
                        size: (v.len() * 4) as u64,
                        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    gpu.cx.queue().write_buffer(&b, 0, bytemuck::cast_slice(&v));
                    b
                };
                let xb = upload("density x", rel(&data.xs[c.rows.clone()], origin.0));
                let yb = upload("density y", rel(&data.ys[c.rows.clone()], origin.1));
                copy_bytes += xb.size() + yb.size();
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("density chunk"),
                    layout: &acc_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &params,
                                offset: 0,
                                size: wgpu::BufferSize::new(
                                    std::mem::size_of::<DensityParams>() as u64
                                ),
                            }),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: xb.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: yb.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: counts.as_entire_binding(),
                        },
                    ],
                });
                DensityChunk {
                    origin,
                    rows: c.rows.len() as u32,
                    group,
                }
            })
            .collect();
        gpu.wait();
        let reduce_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("density reduce"),
            layout: &red_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: counts.as_entire_binding(),
                },
            ],
        });

        let cmodule = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("density composite"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_WGSL.into()),
        });
        let fs = wgpu::ShaderStages::FRAGMENT;
        let comp_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("density composite"),
            entries: &[uniform(0, false), storage(1, true, fs), uniform(2, false)],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&comp_layout],
            push_constant_ranges: &[],
        });
        let composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("density composite"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &cmodule,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &cmodule,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: gpu.desc.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });
        let composite_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("density composite"),
            layout: &comp_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: frame.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: counts.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: lut.as_entire_binding(),
                },
            ],
        });
        Self {
            chunks,
            params,
            _frame: frame,
            accumulate,
            clear,
            maximum,
            reduce_group,
            composite,
            composite_group,
            pixels,
            extra_bytes: counts.size() + lut.size() + 16,
            copy_bytes,
        }
    }

    /// Write the visible chunks' parameters; returns their indices.
    fn write_params(&self, gpu: &Gpu, data: &Data, view: &View) -> Vec<usize> {
        let ((d0, d1), (e0, e1)) = view.dom;
        let r = view.rect;
        let kx = f64::from(r.width) / (d1 - d0);
        let ky = -f64::from(r.height) / (e1 - e0);
        let rect = [
            r.left().floor().max(0.0) as u32,
            r.top().floor().max(0.0) as u32,
            (r.right().ceil() as u32).min(gpu.desc.width),
            (r.bottom().ceil() as u32).min(gpu.desc.height),
        ];
        let mut bytes = vec![0u8; self.chunks.len() * STRIDE as usize];
        let mut visible = Vec::new();
        for (k, (c, info)) in self.chunks.iter().zip(&data.chunks).enumerate() {
            if !view.sees(info, 0.0) {
                continue;
            }
            visible.push(k);
            let p = DensityParams {
                base: [(c.origin.0 - d0) as f32, (c.origin.1 - e0) as f32],
                k: [kx as f32, ky as f32],
                r0: [r.left(), r.bottom()],
                rows: c.rows,
                width: gpu.desc.width,
                rect,
            };
            let at = k * STRIDE as usize;
            bytes[at..at + std::mem::size_of::<DensityParams>()]
                .copy_from_slice(bytemuck::bytes_of(&p));
        }
        gpu.cx.queue().write_buffer(&self.params, 0, &bytes);
        visible
    }
}

fn measure_density(
    gpu: &mut Gpu,
    o: &Options,
    data: &Data,
    density: &Density,
    chrome: &mut LibPlot,
    dom: ((f64, f64), (f64, f64)),
) -> gup_core::Result<Measured> {
    let (w, h) = o.size;
    let mut m = Measured::default();
    let mut spent = 0.0;
    let total = o.warmup + o.frames + 1;
    let mut frame = 0;
    while frame < total {
        let measured = frame >= o.warmup && frame < o.warmup + o.frames;
        let last = frame + 1 == total;
        let start = Instant::now();
        chrome.set(dom, if last { 0.0 } else { jitter(frame) })?;
        let resolved = chrome.plot.resolve(&gpu.cx, w as f32, h as f32)?;
        let view = View {
            dom,
            rect: resolved.layout.plot,
        };
        let prepared = gpu.renderer.prepare(&gpu.cx, &resolved.scene, &gpu.desc)?;
        let visible = density.write_params(gpu, data, &view);
        m.drawn = visible
            .iter()
            .map(|&k| u64::from(density.chunks[k].rows))
            .sum();
        m.draws = prepared.draw_calls() + visible.len() + 3;
        let mut encoder = gpu.cx.device().create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("density"),
                timestamp_writes: gpu
                    .queries
                    .as_ref()
                    .map(|q| wgpu::ComputePassTimestampWrites {
                        query_set: q,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: None,
                    }),
            });
            pass.set_bind_group(0, &density.reduce_group, &[]);
            pass.set_pipeline(&density.clear);
            pass.dispatch_workgroups((density.pixels + 1).div_ceil(256), 1, 1);
            pass.set_pipeline(&density.accumulate);
            for &k in &visible {
                let c = &density.chunks[k];
                pass.set_bind_group(0, &c.group, &[(k as u64 * STRIDE) as u32]);
                pass.dispatch_workgroups(c.rows.div_ceil(256), 1, 1);
            }
            pass.set_bind_group(0, &density.reduce_group, &[]);
            pass.set_pipeline(&density.maximum);
            pass.dispatch_workgroups(density.pixels.div_ceil(256), 1, 1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("density scene"),
                color_attachments: &[Some(gpu.colour_attachment(prepared.clear_color()))],
                depth_stencil_attachment: None,
                timestamp_writes: gpu
                    .queries
                    .as_ref()
                    .map(|q| wgpu::RenderPassTimestampWrites {
                        query_set: q,
                        beginning_of_pass_write_index: None,
                        end_of_pass_write_index: Some(1),
                    }),
                occlusion_query_set: None,
            });
            prepared.draw(&mut pass);
            let [x0, y0, x1, y1] = [
                view.rect.left().floor().max(0.0) as u32,
                view.rect.top().floor().max(0.0) as u32,
                (view.rect.right().ceil() as u32).min(gpu.desc.width),
                (view.rect.bottom().ceil() as u32).min(gpu.desc.height),
            ];
            pass.set_scissor_rect(x0, y0, x1 - x0, y1 - y0);
            pass.set_pipeline(&density.composite);
            pass.set_bind_group(0, &density.composite_group, &[]);
            pass.draw(0..3, 0..1);
        }
        gpu.finish(encoder);
        let cpu = ms(start.elapsed());
        let clock = gpu_clock_mhz();
        let g = gpu.gpu_ms();
        if measured {
            m.cpu.push(cpu);
            m.gpu.push(g);
            m.clock.extend(clock);
            spent += g / 1000.0;
            if spent > o.max_secs && m.gpu.len() >= 5 {
                frame = total - 1;
                continue;
            }
        }
        if last {
            m.image = Some(gpu.image());
        }
        frame += 1;
    }
    Ok(m)
}

// ---------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------

/// The plot rect of a view: resolve a chrome-only plot once.
fn view_of(gpu: &Gpu, o: &Options, chrome: &mut LibPlot, dom: ((f64, f64), (f64, f64))) -> View {
    chrome.set(dom, 0.0).expect("domain");
    let resolved = chrome
        .plot
        .resolve(&gpu.cx, o.size.0 as f32, o.size.1 as f32)
        .expect("resolve");
    View {
        dom,
        rect: resolved.layout.plot,
    }
}

struct Row {
    cand: Cand,
    view: ViewKind,
    m: Measured,
}

fn report_view(o: &Options, cand: Cand, view: ViewKind, m: &Measured) {
    let (g50, g95) = stats(&m.gpu);
    let (c50, c95) = stats(&m.cpu);
    let (clock, _) = stats(&m.clock);
    let v = Some(view);
    metric(cand, v, "frames", m.gpu.len());
    metric(cand, v, "gpu_median_ms", format!("{g50:.2}"));
    metric(cand, v, "gpu_p95_ms", format!("{g95:.2}"));
    metric(cand, v, "cpu_median_ms", format!("{c50:.2}"));
    metric(cand, v, "cpu_p95_ms", format!("{c95:.2}"));
    metric(cand, v, "gpu_clock_mhz", format!("{clock:.0}"));
    metric(cand, v, "drawn", m.drawn);
    metric(cand, v, "draws", m.draws);
    metric(cand, v, "setup_ms", format!("{:.1}", m.setup_ms));
    if let (Some(dir), Some(image)) = (&o.out, &m.image) {
        let path = dir.join(format!("{}-{}-{}.png", cand.name(), view.name(), o.rows));
        image.save(&path).expect("png");
        println!("png {}", path.display());
    }
}

/// The share of pixels whose CIEDE2000 difference from `reference` is
/// over [`DELTA_E`], of the image and of the plot rect.
fn fidelity(
    reference: &image::RgbaImage,
    image: &image::RgbaImage,
    rect: gup_core::geom::Rect,
) -> (f64, f64, u64) {
    let mut over = 0u64;
    let mut in_rect = 0u64;
    let mut rect_pixels = 0u64;
    for (x, y, p) in reference.enumerate_pixels() {
        let q = image.get_pixel(x, y);
        let inside = (x as f32) >= rect.left()
            && (x as f32) < rect.right()
            && (y as f32) >= rect.top()
            && (y as f32) < rect.bottom();
        rect_pixels += u64::from(inside);
        if p == q {
            continue;
        }
        let a = Rgba8::new(p[0], p[1], p[2], p[3]);
        let b = Rgba8::new(q[0], q[1], q[2], q[3]);
        if a.delta_e(b) > DELTA_E {
            over += 1;
            in_rect += u64::from(inside);
        }
    }
    let total = u64::from(reference.width() * reference.height());
    (
        over as f64 / total as f64,
        in_rect as f64 / rect_pixels.max(1) as f64,
        over,
    )
}

fn main() -> gup_core::Result<()> {
    let o = parse();
    if let Some(dir) = &o.out {
        std::fs::create_dir_all(dir).expect("out dir");
    }
    println!(
        "lod_spike: {} rows{}, {}x{}, radius {}; load {}; MemAvailable {:.1} GB",
        o.rows,
        if o.shuffle { " shuffled" } else { " sorted" },
        o.size.0,
        o.size.1,
        o.radius,
        std::fs::read_to_string("/proc/loadavg")
            .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
            .unwrap_or_default(),
        read_line("/proc/meminfo", "MemAvailable:").unwrap_or(0.0) / 1_048_576.0
    );
    let mut gpu = Gpu::new(o.size.0, o.size.1)?;
    let formats = [ColumnFormat::F32Relative, ColumnFormat::F32Relative];
    let chunk_rows = ColumnStore::chunk_rows_for(&gpu.cx.caps().limits, &formats) as usize;
    let t = Instant::now();
    let data = Data::new(&o, chunk_rows);
    println!(
        "data: {} chunks of {chunk_rows} rows, built in {:.0} ms; extent x {:?} y {:?}",
        data.chunks.len(),
        ms(t.elapsed()),
        data.ext.0,
        data.ext.1
    );
    // Guides only, for the plot rect and for (A)'s pass.
    let mut chrome = LibPlot::circles(Vec::<f64>::new(), Vec::<f64>::new(), o.radius);
    let views: Vec<(ViewKind, View)> = o
        .views
        .iter()
        .map(|&v| (v, view_of(&gpu, &o, &mut chrome, data.domains(v))))
        .collect();
    for (v, view) in &views {
        println!(
            "view {}: x {:?} y {:?}, plot rect {:?}, tier {}",
            v.name(),
            view.dom.0,
            view.dom.1,
            view.rect,
            view.tier(&data)
        );
    }
    let margin = f64::from(o.radius) + 2.0;
    let mut rows: Vec<Row> = Vec::new();
    for &cand in &o.candidates {
        let rss0 = read_line("/proc/self/status", "VmRSS:").unwrap_or(0.0);
        match cand {
            Cand::Base => {
                let mut lib =
                    LibPlot::circles(Arc::clone(&data.xs), Arc::clone(&data.ys), o.radius);
                for (v, view) in &views {
                    let mut m = measure_lib(&mut gpu, &o, &mut lib, view.dom, || {})?;
                    m.drawn = data
                        .chunks
                        .iter()
                        .filter(|c| view.sees(c, margin))
                        .map(|c| c.rows.len() as u64)
                        .sum();
                    report_view(&o, cand, *v, &m);
                    rows.push(Row { cand, view: *v, m });
                }
            }
            Cand::Density => {
                let t = Instant::now();
                let density = Density::new(&gpu, &data);
                metric(cand, None, "build_ms", format!("{:.1}", ms(t.elapsed())));
                metric(cand, None, "extra_gpu_bytes", density.extra_bytes);
                metric(cand, None, "prototype_copy_gpu_bytes", density.copy_bytes);
                metric(cand, None, "extra_cpu_bytes", 0);
                for (v, view) in &views {
                    let m = measure_density(&mut gpu, &o, &data, &density, &mut chrome, view.dom)?;
                    report_view(&o, cand, *v, &m);
                    rows.push(Row { cand, view: *v, m });
                }
            }
            Cand::Pyramid => {
                let t = Instant::now();
                let pyramid = Pyramid::build(&data);
                let build = ms(t.elapsed());
                let stored = pyramid.stored();
                let tiers: u64 = pyramid.chunks.iter().map(|c| c.counts.len() as u64).sum();
                metric(cand, None, "build_ms", format!("{build:.1}"));
                metric(
                    cand,
                    None,
                    "build_ms_per_10m",
                    format!("{:.1}", build * 1e7 / o.rows as f64),
                );
                metric(cand, None, "stored_reps", stored);
                // Per stored representative: x and y (F32Relative) and a
                // u32 row id for picks; per chunk and tier, a u32 count.
                metric(cand, None, "extra_gpu_bytes", stored * 12);
                metric(cand, None, "extra_cpu_bytes", tiers * 4);
                metric(cand, None, "prototype_cpu_bytes", stored * 4 + tiers * 4);
                println!(
                    "pyramid tiers per chunk (stored, prefix counts of chunk 0): {} {:?}",
                    pyramid.chunks[0].counts.len(),
                    pyramid.chunks[0].counts
                );
                for (v, view) in &views {
                    let t = Instant::now();
                    let sel = pyramid.select(&data, view, margin, o.bias);
                    let (xs, ys) = pyramid.gather(&data, &sel);
                    let drawn = xs.len() as u64;
                    let mut lib = LibPlot::circles(xs, ys, o.radius);
                    let setup = ms(t.elapsed());
                    let mut m = measure_lib(&mut gpu, &o, &mut lib, view.dom, || {
                        std::hint::black_box(pyramid.select(&data, view, margin, o.bias));
                    })?;
                    m.drawn = drawn;
                    m.setup_ms = setup;
                    report_view(&o, cand, *v, &m);
                    rows.push(Row { cand, view: *v, m });
                }
            }
            Cand::Tiles => {
                let t = Instant::now();
                let tiles = Tiles::build(&data);
                let build = ms(t.elapsed());
                let stored = tiles.stored();
                metric(cand, None, "build_ms", format!("{build:.1}"));
                metric(
                    cand,
                    None,
                    "build_ms_per_10m",
                    format!("{:.1}", build * 1e7 / o.rows as f64),
                );
                metric(cand, None, "stored_cells", stored);
                // (key, count) per stored cell on the CPU; on the GPU the
                // drawn cells' rects (x, x_end, y, y_end, fill: 5 x 4 B).
                metric(cand, None, "extra_cpu_bytes", stored * 8);
                for (v, view) in &views {
                    let t = Instant::now();
                    let draw = tiles.select(&data, view, margin);
                    let mut lib = tile_plot(&data, &draw, o.radius);
                    let full_rows: u64 = draw
                        .full
                        .iter()
                        .map(|&k| data.chunks[k].rows.len() as u64)
                        .sum();
                    let drawn = draw.cells.len() as u64 + full_rows;
                    metric(cand, Some(*v), "tier", draw.tier);
                    metric(cand, Some(*v), "cells", draw.cells.len());
                    metric(cand, Some(*v), "full_rows", full_rows);
                    let setup = ms(t.elapsed());
                    let mut m = measure_lib(&mut gpu, &o, &mut lib, view.dom, || {
                        std::hint::black_box(tiles.select(&data, view, margin));
                    })?;
                    m.drawn = drawn;
                    m.setup_ms = setup;
                    metric(
                        cand,
                        Some(*v),
                        "extra_gpu_bytes",
                        draw.cells.len() as u64 * 20,
                    );
                    report_view(&o, cand, *v, &m);
                    rows.push(Row { cand, view: *v, m });
                }
            }
        }
        let rss1 = read_line("/proc/self/status", "VmRSS:").unwrap_or(0.0);
        metric(
            cand,
            None,
            "rss_delta_mb",
            format!("{:.0}", (rss1 - rss0) / 1024.0),
        );
    }

    println!();
    println!(
        "| candidate | view | GPU median ms | GPU p95 ms | CPU median ms | drawn | clock MHz |"
    );
    println!(
        "| --------- | ---- | ------------- | ---------- | ------------- | ----- | --------- |"
    );
    for r in &rows {
        let (g50, g95) = stats(&r.m.gpu);
        let (c50, _) = stats(&r.m.cpu);
        let (clock, _) = stats(&r.m.clock);
        println!(
            "| {} | {} | {g50:.2} | {g95:.2} | {c50:.2} | {} | {clock:.0} |",
            r.cand.name(),
            r.view.name(),
            r.m.drawn
        );
    }
    if o.fidelity {
        println!();
        println!("| candidate | view | ΔE00 > 10, image | ΔE00 > 10, plot rect | pixels |");
        println!("| --------- | ---- | ---------------- | -------------------- | ------ |");
        for (v, view) in &views {
            let Some(reference) = rows
                .iter()
                .find(|r| r.cand == Cand::Base && r.view == *v)
                .and_then(|r| r.m.image.as_ref())
            else {
                println!("fidelity needs base in --candidates");
                break;
            };
            for r in rows.iter().filter(|r| r.view == *v && r.cand != Cand::Base) {
                let image = r.m.image.as_ref().expect("image");
                let (share, rect_share, n) = fidelity(reference, image, view.rect);
                metric(r.cand, Some(*v), "fidelity_share", format!("{share:.5}"));
                metric(
                    r.cand,
                    Some(*v),
                    "fidelity_rect_share",
                    format!("{rect_share:.5}"),
                );
                println!(
                    "| {} | {} | {:.3}% | {:.3}% | {n} |",
                    r.cand.name(),
                    v.name(),
                    share * 100.0,
                    rect_share * 100.0
                );
            }
        }
    }
    Ok(())
}

/// (C)'s cells as a `Rect` layer coloured by log count, then the rows of
/// the chunks drawn in full as circles.
fn tile_plot(data: &Data, draw: &TileDraw, radius: f32) -> LibPlot {
    let ((x0, x1), (y0, y1)) = data.ext;
    let n = f64::from(1u32 << draw.tier);
    let (cw, ch) = ((x1 - x0) / n, (y1 - y0) / n);
    let mut cols: [Vec<f64>; 5] = Default::default();
    for &(key, count) in &draw.cells {
        let (cx, cy) = (
            f64::from(compact1by1(u64::from(key))),
            f64::from(compact1by1(u64::from(key) >> 1)),
        );
        cols[0].push(x0 + cx * cw);
        cols[1].push(x0 + (cx + 1.0) * cw);
        cols[2].push(y0 + cy * ch);
        cols[3].push(y0 + (cy + 1.0) * ch);
        cols[4].push(f64::from(count).log10());
    }
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for &k in &draw.full {
        let r = data.chunks[k].rows.clone();
        xs.extend_from_slice(&data.xs[r.clone()]);
        ys.extend_from_slice(&data.ys[r]);
    }
    let mut p = LibPlot::circles(xs, ys, radius);
    if draw.cells.is_empty() {
        return p;
    }
    let [a, b, c, d, e] = cols;
    let cols = Columns::new()
        .with("x0", a)
        .and_then(|k| k.with("x1", b))
        .and_then(|k| k.with("y0", c))
        .and_then(|k| k.with("y1", d))
        .and_then(|k| k.with("c", e))
        .expect("columns");
    let (x, y) = (p.x.clone(), p.y.clone());
    p.plot
        .add_columns::<gup_core::marks::Rect>(&cols)
        .x_column(&cols["x0"])
        .y_column(&cols["y0"])
        .attr(gup_core::marks::Rect::X_END, x.encode_column(&cols["x1"]))
        .attr(gup_core::marks::Rect::Y_END, y.encode_column(&cols["y1"]))
        .attr(
            gup_core::marks::Rect::FILL,
            Sequential::viridis().encode_column(&cols["c"]),
        );
    p
}
