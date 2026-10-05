// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Zooming writes 0 column bytes (RFC-001 §1). 100K points, both domains
//! changed on every one of 300 frames, each resolved and drawn through
//! `Renderer::render` into an `ImageTarget` (the headless twin of the
//! window loop in `examples/zoom_bench.rs`). The upload counters on the
//! `Context` must show no column writes at all after the first frame, and
//! exactly three uniform writes per frame (encodings, chunk bases, view).

mod common;

use common::scatter;
use gup_core::prelude::*;
use gup_core::{ImageTarget, Renderer};

const POINTS: usize = 100_000;
const FRAMES: usize = 300;

#[test]
fn zooming_100k_points_writes_no_column_bytes() {
    let cx = Context::new_blocking().expect("headless context");
    let (mut plot, x, y) = scatter::plot_rows(scatter::countries_n(POINTS), Px(3.0));
    let mut target = ImageTarget::new(&cx, 640, 400).unwrap();
    let mut renderer = Renderer::new();

    // Frame 0 evaluates the accessors and uploads the column chunk once.
    let first = plot.resolve(&cx, 640.0, 400.0).unwrap();
    renderer.render(&cx, &first.scene, &mut target).unwrap();
    let after_upload = cx.upload_stats();
    assert_eq!(after_upload.columns.writes, 1);
    assert!(after_upload.columns.bytes >= (POINTS * 4 * 3) as u64);
    let (x0, x1) = x.read().current_domain().unwrap();
    let (y0, y1) = y.read().current_domain().unwrap();

    let mut images = Vec::new();
    for i in 1..=FRAMES {
        let k = 0.05f64.powf(i as f64 / FRAMES as f64);
        let c = x0 + 0.35 * (x1 - x0);
        x.write()
            .set_domain(c - (c - x0) * k, c + (x1 - c) * k)
            .unwrap();
        let (l0, l1) = (y0.log10(), y1.log10());
        let m = l0 + 0.6 * (l1 - l0);
        y.write()
            .set_domain(10f64.powf(m - (m - l0) * k), 10f64.powf(m + (l1 - m) * k))
            .unwrap();
        let resolved = plot.resolve(&cx, 640.0, 400.0).unwrap();
        renderer.render(&cx, &resolved.scene, &mut target).unwrap();
        // Read a few frames back to prove the zoom reached the GPU.
        if i % 100 == 0 {
            images.push(pollster::block_on(target.read(&cx)).unwrap());
        }
    }
    let written = cx.upload_stats() - after_upload;
    eprintln!("{FRAMES} zoomed frames of {POINTS} points wrote: {written:?}");

    assert_eq!(written.columns.bytes, 0, "{written:?}");
    assert_eq!(written.columns.writes, 0, "{written:?}");
    assert_eq!(written.uniforms.writes, 3 * FRAMES as u64, "{written:?}");
    // New label glyphs upload only the atlas rows they landed in, not
    // the whole 1 MB atlas.
    assert!(written.textures.bytes < 128 << 10, "{written:?}");
    assert_eq!(images.len(), 3);
    assert_ne!(images[0], images[1]);
    assert_ne!(images[1], images[2]);
}
