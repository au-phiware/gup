// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Plots whose chunks are spatially coherent (x-sorted), so chunk culling
//! (RFC-001 S9f) has something to skip. Shared by `tests/zoom_uploads.rs`
//! and `tests/pick.rs`.

#![allow(dead_code)] // each includer uses a different subset

use gup_core::ScaleRef;
use gup_core::prelude::*;

/// Rows per chunk.
pub const CHUNK: u32 = 1024;
/// 64 chunks of circles.
pub const ROWS: usize = 64 * CHUNK as usize;
/// Plot size.
pub const W: f32 = 640.0;
pub const H: f32 = 400.0;

/// A row at `x = i` with a scrambled `y` in `0..100`.
#[derive(Clone, Debug)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

pub fn rows(n: usize) -> Vec<Pt> {
    (0..n)
        .map(|i| Pt {
            x: i as f64,
            y: (i * 7919 % 1000) as f64 / 10.0,
        })
        .collect()
}

pub type Scales = (ScaleRef<Linear>, ScaleRef<Linear>);

/// 65,536 circles in 64 x-sorted chunks.
pub fn circles(culling: bool) -> (Plot, Scales) {
    let mut plot = Plot::new();
    plot.chunk_culling(culling);
    plot.legends(false);
    let x = plot.x(Linear::new());
    let y = plot.y(Linear::new().domain(0.0, 100.0));
    plot.circles(rows(ROWS))
        .x_with(x.encode(|r: &Pt| r.x))
        .y_with(y.encode(|r: &Pt| r.y))
        .radius(Px(3.0))
        .max_chunk_rows(CHUNK);
    (plot, (x, y))
}

/// The x domain of chunks `20.5..23.5`: chunks 20 to 23 are on screen.
pub fn zoom_circles(x: &ScaleRef<Linear>) {
    let c = f64::from(CHUNK);
    x.write().set_domain(20.5 * c, 23.5 * c).unwrap();
}

/// A line of 4,096 rows in 64-row chunks.
pub fn line(culling: bool) -> (Plot, Scales) {
    let mut plot = Plot::new();
    plot.chunk_culling(culling);
    plot.legends(false);
    let x = plot.x(Linear::new());
    let y = plot.y(Linear::new().domain(0.0, 100.0));
    plot.line(rows(4096))
        .x_with(x.encode(|r: &Pt| r.x))
        .y_with(y.encode(|r: &Pt| r.y))
        .max_chunk_rows(64);
    (plot, (x, y))
}

/// Bars of a `Derived` width on 64 x-sorted chunks, from y = 0.
pub fn bars(culling: bool) -> (Plot, Scales) {
    let mut plot = Plot::new();
    plot.chunk_culling(culling);
    plot.legends(false);
    let x = plot.x(Linear::new());
    let y = plot.y(Linear::new().domain(0.0, 100.0));
    let width = Derived::new(|| Px(12.0));
    plot.rects(rows(ROWS))
        .x_with(x.encode(|r: &Pt| r.x))
        .y_with(y.encode(|r: &Pt| r.y))
        .x_end_with(x.encode(|r: &Pt| r.x))
        .y_end_with(y.encode(|_: &Pt| 0.0))
        .width(width)
        .max_chunk_rows(CHUNK);
    (plot, (x, y))
}

/// Bars in the `rect_bars` form: a centre, a constant width and a baseline,
/// with no `x_end` set.
pub fn plain_bars(culling: bool) -> (Plot, Scales) {
    let mut plot = Plot::new();
    plot.chunk_culling(culling);
    plot.legends(false);
    let x = plot.x(Linear::new());
    let y = plot.y(Linear::new().domain(0.0, 100.0));
    plot.rects(rows(ROWS))
        .x_with(x.encode(|r: &Pt| r.x))
        .y_with(y.encode(|r: &Pt| r.y))
        .y_end_with(y.encode(|_: &Pt| 0.0))
        .width(Px(12.0))
        .max_chunk_rows(CHUNK);
    (plot, (x, y))
}
