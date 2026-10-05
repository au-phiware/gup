// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The RFC-001 S0 zoom benchmark: 100K points (linear x, log y, viridis
//! fill, constant radius) in a window, zoomed continuously — both domains
//! change every frame — while timing every frame and counting every byte
//! written to the GPU.
//!
//! ```text
//! cargo run -p gup-core --release --example zoom_bench -- [options]
//!   --present fifo|mailbox|immediate   present mode (default fifo = vsync)
//!   --frames N                         measured frames (default 600)
//!   --warmup N                         unmeasured frames first (default 60)
//!   --points N                         rows (default 100000)
//!   --radius PX                        circle radius (default 3)
//!   --size WxH                         windowed at this physical size
//!                                      (default: borderless fullscreen)
//!   --uncapped                         draw frames back to back instead of
//!                                      on each redraw request (winit paces
//!                                      those to the compositor; use with
//!                                      --present mailbox to measure without
//!                                      vsync)
//! ```
//!
//! Each frame runs the same path as `gup_core::show`: `Plot::resolve`
//! (ticks, text layout, uniform writes), `Renderer::prepare`, one render
//! pass with `Prepared::draw`, `WindowTarget::present`. The pass carries
//! timestamp queries when the adapter supports `TIMESTAMP_QUERY`; they are
//! resolved once after the run, so timing never stalls the loop.

#[path = "../tests/common/scatter.rs"]
mod scatter;

use gup_core::prelude::*;
use gup_core::{RenderTarget, Renderer, ScaleRef, UploadStats, WindowTarget};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Fullscreen, Window, WindowId};

#[derive(Debug)]
struct Options {
    present: wgpu::PresentMode,
    frames: usize,
    warmup: usize,
    points: usize,
    radius: f32,
    size: Option<(u32, u32)>,
    uncapped: bool,
}

fn options() -> Options {
    let mut o = Options {
        present: wgpu::PresentMode::Fifo,
        frames: 600,
        warmup: 60,
        points: 100_000,
        radius: 3.0,
        size: None,
        uncapped: false,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| panic!("{flag} needs a value"));
        match flag.as_str() {
            "--present" => {
                o.present = match value().as_str() {
                    "fifo" => wgpu::PresentMode::Fifo,
                    "mailbox" => wgpu::PresentMode::Mailbox,
                    "immediate" => wgpu::PresentMode::Immediate,
                    other => panic!("unknown present mode {other}"),
                }
            }
            "--frames" => o.frames = value().parse().expect("--frames N"),
            "--warmup" => o.warmup = value().parse().expect("--warmup N"),
            "--points" => o.points = value().parse().expect("--points N"),
            "--radius" => o.radius = value().parse().expect("--radius PX"),
            "--uncapped" => o.uncapped = true,
            "--size" => {
                let (w, h) = value().split_once('x').expect("--size WxH");
                o.size = Some((w.parse().expect("width"), h.parse().expect("height")));
            }
            other => panic!("unknown option {other}"),
        }
    }
    assert!(
        o.frames >= 1 && o.frames <= 2000,
        "--frames must be 1..=2000"
    );
    o
}

/// Timings of one measured frame.
#[derive(Copy, Clone, Debug, Default)]
struct FrameTimes {
    /// From this frame's start to the next frame's start.
    interval: Duration,
    /// `Plot::resolve`: zoomed domains to scene and uniforms.
    resolve: Duration,
    /// `Renderer::prepare`.
    prepare: Duration,
    /// `RenderTarget::acquire` (blocks on vsync with `Fifo`).
    acquire: Duration,
    /// Encoding the pass, submit and present.
    submit: Duration,
}

struct Gpu {
    cx: Context,
    target: WindowTarget,
    renderer: Renderer,
    timestamps: Option<(wgpu::QuerySet, u32)>,
}

struct Bench {
    o: Options,
    plot: Plot,
    x: ScaleRef<Linear>,
    y: ScaleRef<Log>,
    full: Option<((f64, f64), (f64, f64))>,
    gpu: Option<Gpu>,
    frame: usize,
    started: Option<Instant>,
    times: Vec<FrameTimes>,
    uploads_before: UploadStats,
    uploads_during: Option<UploadStats>,
    gpu_ms: Vec<f64>,
    error: Option<String>,
    finished: bool,
}

impl Bench {
    fn open(&mut self, event_loop: &ActiveEventLoop) -> gup_core::Result<Gpu> {
        let mut attributes = Window::default_attributes().with_title("gup-core zoom bench");
        attributes = match self.o.size {
            Some((w, h)) => attributes
                .with_inner_size(PhysicalSize::new(w, h))
                .with_resizable(false),
            None => attributes.with_fullscreen(Some(Fullscreen::Borderless(None))),
        };
        let window = Arc::new(event_loop.create_window(attributes).expect("window"));
        // The default options enable TIMESTAMP_QUERY where the adapter has it.
        let cx = Context::new_blocking()?;
        let mut target = WindowTarget::new(&cx, window)?;
        target.set_present_mode(&cx, self.o.present)?;
        let timestamps = cx
            .caps()
            .features
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            .then(|| {
                let count = 2 * self.o.frames as u32;
                let set = cx.device().create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("zoom bench pass"),
                    ty: wgpu::QueryType::Timestamp,
                    count,
                });
                (set, count)
            });
        let info = cx.adapter_info().expect("own device");
        println!(
            "adapter: {} ({:?}, driver {} {})",
            info.name, info.backend, info.driver, info.driver_info
        );
        println!(
            "surface: {target:?}; supported present modes {:?}",
            target.present_modes()
        );
        println!(
            "timestamps: {}",
            if timestamps.is_some() {
                "TIMESTAMP_QUERY enabled"
            } else {
                "not supported; GPU time not measured"
            }
        );
        Ok(Gpu {
            cx,
            target,
            renderer: Renderer::new(),
            timestamps,
        })
    }

    /// Both domains at zoom phase `t`: zoom in to 5% of the full extent
    /// about a point off-centre and back out, twice per run.
    fn zoom(&self, t: f64) -> gup_core::Result<()> {
        let ((x0, x1), (y0, y1)) = self.full.expect("fitted");
        let level = 0.5 * (1.0 - (std::f64::consts::TAU * 2.0 * t).cos());
        let k = 0.05f64.powf(level);
        let cx = x0 + 0.35 * (x1 - x0);
        let (ly0, ly1) = (y0.log10(), y1.log10());
        let cy = ly0 + 0.6 * (ly1 - ly0);
        self.x
            .write()
            .set_domain(cx - (cx - x0) * k, cx + (x1 - cx) * k)?;
        self.y.write().set_domain(
            10f64.powf(cy - (cy - ly0) * k),
            10f64.powf(cy + (ly1 - cy) * k),
        )
    }

    fn redraw(&mut self) -> gup_core::Result<bool> {
        let start = Instant::now();
        let measured = self.frame.checked_sub(self.o.warmup);
        if let (Some(i), Some(prev)) = (measured, self.started)
            && i > 0
        {
            self.times[i - 1].interval = start - prev;
        }
        if measured == Some(self.o.frames) {
            self.uploads_during = Some(self.gpu.as_ref().unwrap().cx.upload_stats());
            return Ok(true);
        }
        if measured == Some(0) {
            self.uploads_before = self.gpu.as_ref().unwrap().cx.upload_stats();
        }
        self.started = Some(start);

        // Frame 0 fits the data domains; every later frame zooms.
        if self.frame > 0 {
            let t = self.frame as f64 / (self.o.warmup + self.o.frames) as f64;
            self.zoom(t)?;
        }
        let gpu = self.gpu.as_mut().unwrap();
        let (w, h) = gpu.target.size();
        let dpr = gpu.target.dpr();
        let resolved = self.plot.resolve(&gpu.cx, w as f32 / dpr, h as f32 / dpr)?;
        if self.full.is_none() {
            self.full = Some((
                self.x.read().current_domain().unwrap(),
                self.y.read().current_domain().unwrap(),
            ));
        }
        let t_resolve = Instant::now();
        let desc = gpu.target.desc();
        let prepared = gpu.renderer.prepare(&gpu.cx, &resolved.scene, &desc)?;
        let t_prepare = Instant::now();
        let frame = gpu.target.acquire(&gpu.cx)?;
        let t_acquire = Instant::now();
        let mut encoder = gpu.cx.device().create_command_encoder(&Default::default());
        {
            let timestamp_writes = match (&gpu.timestamps, measured) {
                (Some((set, _)), Some(i)) => Some(wgpu::RenderPassTimestampWrites {
                    query_set: set,
                    beginning_of_pass_write_index: Some(2 * i as u32),
                    end_of_pass_write_index: Some(2 * i as u32 + 1),
                }),
                _ => None,
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gup scene"),
                color_attachments: &[Some(frame.color_attachment(prepared.clear_color()))],
                depth_stencil_attachment: None,
                timestamp_writes,
                occlusion_query_set: None,
            });
            prepared.draw(&mut pass);
        }
        gpu.target.present(&gpu.cx, frame, encoder.finish())?;
        let t_submit = Instant::now();
        if let Some(i) = measured {
            self.times[i] = FrameTimes {
                interval: Duration::ZERO,
                resolve: t_resolve - start,
                prepare: t_prepare - t_resolve,
                acquire: t_acquire - t_prepare,
                submit: t_submit - t_acquire,
            };
        }
        self.frame += 1;
        Ok(false)
    }

    /// Resolve every pass's timestamps once, after the run.
    fn read_gpu_times(&mut self) -> gup_core::Result<()> {
        let Some(gpu) = &self.gpu else { return Ok(()) };
        let Some((set, count)) = &gpu.timestamps else {
            return Ok(());
        };
        let device = gpu.cx.device();
        let size = u64::from(*count) * 8;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamps"),
            size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("timestamps readback"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.resolve_query_set(set, 0..*count, &resolve, 0);
        encoder.copy_buffer_to_buffer(&resolve, 0, &read, 0, size);
        gpu.cx.queue().submit([encoder.finish()]);
        read.slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| gup_core::Error::Readback(e.to_string()))?;
        let period = f64::from(gpu.cx.queue().get_timestamp_period());
        let ticks: Vec<u64> = read
            .slice(..)
            .get_mapped_range()
            .chunks_exact(8)
            .map(|c| u64::from_le_bytes(c.try_into().unwrap()))
            .collect();
        self.gpu_ms = ticks
            .chunks_exact(2)
            .map(|p| p[1].wrapping_sub(p[0]) as f64 * period / 1e6)
            .collect();
        Ok(())
    }

    fn report(&self) {
        let o = &self.o;
        let gpu = self.gpu.as_ref().unwrap();
        let (w, h) = gpu.target.size();
        println!(
            "run: {} points, radius {} px, {}x{} physical (dpr {}), present {:?}{}, \
             {} warm-up + {} measured frames, {} build",
            o.points,
            o.radius,
            w,
            h,
            gpu.target.dpr(),
            o.present,
            if o.uncapped {
                " uncapped"
            } else {
                " paced by redraw requests"
            },
            o.warmup,
            o.frames,
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
        let ms = |d: Duration| d.as_secs_f64() * 1e3;
        let stats = |name: &str, mut v: Vec<f64>| {
            v.sort_by(f64::total_cmp);
            let q = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize];
            println!(
                "{name:<30} median {:>8.3} ms   p95 {:>8.3}   min {:>8.3}   max {:>8.3}",
                q(0.5),
                q(0.95),
                v[0],
                v[v.len() - 1]
            );
            (q(0.5), q(0.95))
        };
        let t = &self.times;
        let (med, p95) = stats("frame interval", t.iter().map(|f| ms(f.interval)).collect());
        stats(
            "  resolve (Plot::resolve)",
            t.iter().map(|f| ms(f.resolve)).collect(),
        );
        stats(
            "  prepare (Renderer::prepare)",
            t.iter().map(|f| ms(f.prepare)).collect(),
        );
        stats(
            "  acquire (surface texture)",
            t.iter().map(|f| ms(f.acquire)).collect(),
        );
        stats(
            "  encode + submit + present",
            t.iter().map(|f| ms(f.submit)).collect(),
        );
        stats(
            "  CPU work (all but acquire)",
            t.iter()
                .map(|f| ms(f.resolve + f.prepare + f.submit))
                .collect(),
        );
        if !self.gpu_ms.is_empty() {
            stats("GPU render pass (timestamps)", self.gpu_ms.clone());
        }
        let over = |limit: f64| t.iter().filter(|f| ms(f.interval) > limit).count();
        println!(
            "fps: median {:.1}, p95 {:.1} (from the median and p95 frame interval); \
             frames over 16.7 ms: {}, over 25 ms (a missed 60 Hz refresh): {} of {}",
            1e3 / med,
            1e3 / p95,
            over(1e3 / 60.0),
            over(25.0),
            t.len()
        );
        let d = self.uploads_during.unwrap() - self.uploads_before;
        println!(
            "GPU writes over the {} measured frames: columns {} B in {} writes; \
             uniforms {} B in {} writes; guide instances {} B in {} writes; \
             textures {} B in {} writes",
            o.frames,
            d.columns.bytes,
            d.columns.writes,
            d.uniforms.bytes,
            d.uniforms.writes,
            d.instances.bytes,
            d.instances.writes,
            d.textures.bytes,
            d.textures.writes
        );
        let all = self.uploads_during.unwrap();
        println!(
            "GPU writes since start: columns {} B in {} writes (the one upload)",
            all.columns.bytes, all.columns.writes
        );
    }
}

impl Bench {
    /// Draw one frame; report and exit when done. Whether to go on.
    fn step(&mut self, event_loop: &ActiveEventLoop) -> bool {
        if self.finished {
            return false;
        }
        match self.redraw() {
            Ok(false) => return true,
            Ok(true) => {
                if let Err(e) = self.read_gpu_times() {
                    self.error = Some(e.to_string());
                }
                self.report();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
        self.finished = true;
        event_loop.exit();
        false
    }
}

impl ApplicationHandler for Bench {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        match self.open(event_loop) {
            Ok(gpu) => {
                gpu.target.window().request_redraw();
                self.gpu = Some(gpu);
            }
            Err(e) => {
                self.error = Some(e.to_string());
                event_loop.exit();
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.error = Some("window closed before the run finished".into());
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.target.resize(&gpu.cx, size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested if !self.o.uncapped => {
                if self.step(event_loop) {
                    self.gpu.as_ref().unwrap().target.window().request_redraw();
                }
            }
            _ => {}
        }
    }

    /// `--uncapped`: render as fast as possible, outside winit's
    /// frame-callback throttling of `RedrawRequested`.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.o.uncapped && self.gpu.is_some() && !self.finished {
            event_loop.set_control_flow(ControlFlow::Poll);
            self.step(event_loop);
        }
    }

    /// Release GPU objects while the display connection is open.
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.gpu = None;
        self.plot = Plot::new();
    }
}

fn main() {
    let o = options();
    let t = Instant::now();
    let (plot, x, y) = scatter::plot_rows(scatter::countries_n(o.points), Px(o.radius));
    println!(
        "generated {} rows in {:.1} ms",
        o.points,
        t.elapsed().as_secs_f64() * 1e3
    );
    let frames = o.frames;
    let mut bench = Bench {
        o,
        plot,
        x,
        y,
        full: None,
        gpu: None,
        frame: 0,
        started: None,
        times: vec![FrameTimes::default(); frames],
        uploads_before: UploadStats::default(),
        uploads_during: None,
        gpu_ms: Vec::new(),
        error: None,
        finished: false,
    };
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.run_app(&mut bench).expect("run");
    if let Some(e) = bench.error {
        eprintln!("zoom bench failed: {e}");
        std::process::exit(1);
    }
}
