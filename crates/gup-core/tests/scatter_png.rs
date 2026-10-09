// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end, external-crate test of the S0a vertical slice: a scatter
//! with linear x, log y and a sequential fill, rendered headlessly to a PNG
//! and checked by the GUP-388 visual regression harness using layout
//! metadata from gup-core's own layout.

mod common;

use common::scatter::{self, HEIGHT, WIDTH};
use common::vr::{harness, metadata};
use gup_core::prelude::*;
use gup_core::{Layout, scene::TextRole};
use gup_visual_regression::RgbaImage;

fn render() -> (image::RgbaImage, Layout, Sequential) {
    let cx = Context::new_blocking().expect("headless GPU context");
    let (image, layout) = scatter::plot()
        .render_resolved(&cx, WIDTH, HEIGHT)
        .expect("render scatter");
    (image, layout, scatter::fill())
}

#[test]
fn scatter_png_passes_the_visual_regression_harness() {
    let (image, layout, fill) = render();

    // Non-trivial output (AC7), independently of the harness.
    assert_eq!(image.dimensions(), (WIDTH, HEIGHT));
    let first = image.get_pixel(0, 0);
    let distinct = image.pixels().filter(|p| *p != first).count();
    assert!(distinct > 2_000, "only {distinct} non-background pixels");

    // Layout claims from AC5: a title, ≥ 4 tick labels per axis.
    let count = |role| layout.texts.iter().filter(|t| t.run.role == role).count();
    assert_eq!(count(TextRole::Title), 1);
    assert!(layout.x_ticks.len() >= 4, "{:?}", layout.x_ticks);
    assert!(layout.y_ticks.len() >= 4, "{:?}", layout.y_ticks);
    for t in &layout.texts {
        assert!(
            t.bounds.width > 0.0 && t.bounds.height > 0.0,
            "empty text {t:?}"
        );
        let (b, p) = (t.bounds, layout.plot);
        let disjoint = b.right() <= p.left()
            || b.left() >= p.right()
            || b.bottom() <= p.top()
            || b.top() >= p.bottom();
        assert!(
            disjoint,
            "text {:?} overlaps the plot rect {p:?}",
            t.run.text
        );
    }

    let vr = RgbaImage::new(WIDTH, HEIGHT, image.into_raw()).unwrap();
    harness()
        .run("gup_core/scatter", Ok((vr, metadata(&layout, &fill))))
        .assert_ok();
}

/// RFC-001 S4a (AC3): the golden scatter's 120 rows forced into 3 and 4
/// column chunks, so the layer is drawn by one instanced draw per chunk,
/// each with its own `Chunk` uniform entry (row base, x origin). The image
/// is compared with the single-chunk render on the same device in the same
/// run, never with a golden byte for byte. Each chunk stores x relative to
/// its own origin, so positions are rounded differently, by about 1e-5 px
/// (see the chunk-boundary conformance test): on Intel/Mesa the images are
/// identical, on lavapipe 7 and 9 edge pixels differ by 1/255. Hence at
/// most 64 pixels by 1/255; one wrong dynamic offset changes 10,503
/// pixels. The single-chunk render is held to the golden PNG by the test
/// above, through the harness's ΔE tolerance. The multi-chunk renders are
/// written next to the harness's artifacts for inspection.
#[test]
fn multi_chunk_scatter_matches_one_chunk() {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut single = scatter::plot();
    let resolved = single.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
    assert_eq!(marks(&resolved).chunks(), 1);
    let (one, _) = single.render_resolved(&cx, WIDTH, HEIGHT).unwrap();

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (chunk_rows, chunks) in [(40, 3), (32, 4)] {
        let mut plot = scatter::plot_chunked(chunk_rows);
        let resolved = plot.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
        let batch = marks(&resolved);
        assert_eq!(batch.chunks(), chunks, "{chunk_rows} rows per chunk");
        assert_eq!(batch.instances(), 120);
        let (image, _) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
        let out = gup_visual_regression::golden::default_artifact_dir(&root)
            .join(format!("gup_core/scatter_{chunks}_chunks.png"));
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        image.save(&out).unwrap();
        // Per pixel, the largest channel difference.
        let deltas: Vec<u8> = image
            .pixels()
            .zip(one.pixels())
            .map(|(a, b)| (0..4).map(|c| a[c].abs_diff(b[c])).max().unwrap())
            .collect();
        let differing = deltas.iter().filter(|&&d| d > 0).count();
        let worst = deltas.iter().copied().max().unwrap();
        eprintln!(
            "{chunks} chunks: {differing} pixels differ from one chunk, by at most {worst}/255"
        );
        assert!(
            differing <= 64 && worst <= 1,
            "{chunks} chunks: {differing} pixels differ from one chunk, by up to {worst}/255 ({})",
            out.display()
        );
    }
}

fn marks(resolved: &gup_core::Resolved) -> &gup_core::scene::MarkBatch {
    resolved
        .scene
        .items
        .iter()
        .find_map(|i| match &i.kind {
            gup_core::scene::ItemKind::Marks(b) => Some(b),
            _ => None,
        })
        .expect("a mark layer")
}
