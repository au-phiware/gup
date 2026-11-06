// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Scale benchmark (GUP-627): what it costs to get N rows in, split by
//! phase, and the peak memory of each phase. Headless (`ImageTarget`).
//!
//! ```text
//! cargo run -p gup-core --release --example scale_bench -- [options]
//!   --rows N              rows (default 1000000)
//!   --retain MODE         auto|rows|columns|gpu-only (default auto)
//!   --shuffle             permute the rows (default: sorted by x, so chunk
//!                         culling can act)
//!   --size WxH            image size (default 1920x1080)
//!   --zoom-frames N       zooms measured (default 30)
//!   --seed N              random-walk seed (default 1)
//! ```
//!
//! Phases: `build` (generate the rows; not a cost of Gup), `accessor` (the
//! two closures over every row into two `Vec<f64>`), `store`
//! (`ColumnStore::from_columns` + `upload` of two `F32Relative` columns),
//! `first_resolve` (`Plot::resolve`, which repeats accessor and store
//! internally), `first_frame` (render and readback) and `zoom` (CPU time of
//! `Plot::zoom` + `resolve`). Each prints `metric scale.<name> <value>`.
//! Peak RSS is `VmHWM` after each phase (Linux only).

use gup_core::column::{ColumnData, ColumnFormat, ColumnStore};
use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::{ImageTarget, Retain};
use std::time::Instant;

struct Options {
    rows: usize,
    retain: Retain,
    retain_name: String,
    shuffle: bool,
    size: (u32, u32),
    zoom_frames: usize,
    seed: u64,
}

fn parse() -> Options {
    let mut o = Options {
        rows: 1_000_000,
        retain: Retain::Auto,
        retain_name: "auto".into(),
        shuffle: false,
        size: (1920, 1080),
        zoom_frames: 30,
        seed: 1,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| panic!("{arg} needs a value"));
        match arg.as_str() {
            "--rows" => o.rows = value().replace('_', "").parse().expect("--rows N"),
            "--retain" => {
                let v = value();
                o.retain = match v.as_str() {
                    "auto" => Retain::Auto,
                    "rows" => Retain::Rows,
                    "columns" => Retain::Columns,
                    "gpu-only" => Retain::GpuOnly,
                    _ => panic!("--retain auto|rows|columns|gpu-only"),
                };
                o.retain_name = v;
            }
            "--shuffle" => o.shuffle = true,
            "--size" => {
                let v = value();
                let (w, h) = v.split_once('x').expect("--size WxH");
                o.size = (w.parse().expect("width"), h.parse().expect("height"));
            }
            "--zoom-frames" => o.zoom_frames = value().parse().expect("--zoom-frames N"),
            "--seed" => o.seed = value().parse().expect("--seed N"),
            other => panic!("unknown option {other}"),
        }
    }
    o
}

/// A 64-bit LCG (Knuth's MMIX constants); the top bits are the output.
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

/// Peak resident set (`VmHWM`) in MB, or `None` off Linux.
fn peak_rss_mb() -> Option<f64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|l| l.starts_with("VmHWM:"))?;
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

fn load_average() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|_| "n/a".into())
}

fn mem_available_gb() -> String {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            let line = s.lines().find(|l| l.starts_with("MemAvailable:"))?;
            let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
            Some(format!("{:.1}", kb / 1_048_576.0))
        })
        .unwrap_or_else(|| "n/a".into())
}

fn metric(name: &str, value: impl std::fmt::Display) {
    println!("metric scale.{name} {value}");
}

fn rss(phase: &str) {
    match peak_rss_mb() {
        Some(mb) => metric(&format!("rss.{phase}.mb"), format!("{mb:.0}")),
        None => metric(&format!("rss.{phase}.mb"), "n/a"),
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() -> gup_core::Result<()> {
    let o = parse();
    let n = o.rows;
    println!(
        "scale_bench: {n} rows, retain {}, {}, {}x{}, seed {}, load average {}, MemAvailable {} GB",
        o.retain_name,
        if o.shuffle { "shuffled" } else { "sorted" },
        o.size.0,
        o.size.1,
        o.seed,
        load_average(),
        mem_available_gb(),
    );
    let cx = Context::new_blocking()?;
    if let Some(info) = cx.adapter_info() {
        println!("adapter: {} ({:?})", info.name, info.backend);
    }

    // 1. build
    let t = Instant::now();
    let mut rng = Lcg(o.seed);
    let step = 1000.0 / n.max(1) as f64;
    let mut y = 0.0f64;
    let mut rows: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            y += rng.unit() - 0.5;
            (i as f64 * step, y)
        })
        .collect();
    if o.shuffle {
        for i in (1..rows.len()).rev() {
            let j = (rng.next() % (i as u64 + 1)) as usize;
            rows.swap(i, j);
        }
    }
    metric("build.ms", format!("{:.1}", ms(t)));
    rss("build");

    // 2. accessor: the closures the layer runs inside `evaluate`.
    let (fx, fy) = (|r: &(f64, f64)| r.0, |r: &(f64, f64)| r.1);
    let t = Instant::now();
    let mut xs = Vec::with_capacity(n);
    let mut ys = Vec::with_capacity(n);
    for r in &rows {
        xs.push(fx(r));
        ys.push(fy(r));
    }
    let accessor_ms = ms(t);
    metric("accessor.ms", format!("{accessor_ms:.1}"));
    metric(
        "accessor.ns_per_row",
        format!("{:.2}", accessor_ms * 1e6 / n.max(1) as f64),
    );
    rss("accessor");

    // 3. store
    let formats = [ColumnFormat::F32Relative, ColumnFormat::F32Relative];
    let chunk_rows = ColumnStore::chunk_rows_for(&cx.caps().limits, &formats);
    let before = cx.upload_stats();
    let t = Instant::now();
    let mut store = ColumnStore::from_columns(
        vec![
            (ColumnFormat::F32Relative, ColumnData::from(xs)),
            (ColumnFormat::F32Relative, ColumnData::from(ys)),
        ],
        chunk_rows,
    )?;
    store.upload(&cx)?;
    metric("store.ms", format!("{:.1}", ms(t)));
    metric("columns.bytes", (cx.upload_stats() - before).columns.bytes);
    metric("chunks", store.chunks().len());
    rss("store");
    drop(store);

    // 4. first resolve (consumes the rows)
    let (w, h) = o.size;
    let mut plot = Plot::new();
    plot.legends(false);
    plot.circles(rows)
        .x(fx)
        .y(fy)
        .attr(Circle::RADIUS, Px(1.5))
        .retain(o.retain);
    let before = cx.upload_stats();
    let t = Instant::now();
    let scene = plot.resolve(&cx, w as f32, h as f32)?.scene.clone();
    metric("first_resolve.ms", format!("{:.1}", ms(t)));
    metric(
        "first_resolve.columns.bytes",
        (cx.upload_stats() - before).columns.bytes,
    );
    rss("first_resolve");

    // 5. first frame
    let t = Instant::now();
    let image = ImageTarget::new(&cx, w, h)?.render_blocking(&cx, &scene)?;
    metric("first_frame.ms", format!("{:.1}", ms(t)));
    println!("image {}x{}", image.width(), image.height());
    rss("first_frame");

    // 6. zoom
    let centre = Point::new(w as f32 * 0.5, h as f32 * 0.5);
    let mut times = Vec::with_capacity(o.zoom_frames);
    for _ in 0..o.zoom_frames {
        let t = Instant::now();
        plot.zoom(centre, 0.97)?;
        plot.resolve(&cx, w as f32, h as f32)?;
        times.push(ms(t));
    }
    if !times.is_empty() {
        times.sort_by(f64::total_cmp);
        let p95 = times[((times.len() * 95).div_ceil(100)).clamp(1, times.len()) - 1];
        metric(
            "zoom.resolve.median_ms",
            format!("{:.2}", times[times.len() / 2]),
        );
        metric("zoom.resolve.p95_ms", format!("{p95:.2}"));
    }
    rss("zoom");
    let total = cx.upload_stats().columns.bytes;
    println!(
        "total column bytes uploaded {total} ({:.1} per row)",
        total as f64 / n.max(1) as f64
    );
    Ok(())
}
