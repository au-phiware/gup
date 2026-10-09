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
/// must be pixel-identical to the single-chunk render and to the golden
/// PNG. The 4-chunk render is written next to the harness's artifacts for
/// inspection.
#[test]
fn multi_chunk_scatter_is_pixel_identical_to_the_golden() {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut single = scatter::plot();
    let resolved = single.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
    assert_eq!(marks(&resolved).chunks(), 1);
    let (one, _) = single.render_resolved(&cx, WIDTH, HEIGHT).unwrap();

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden = image::open(root.join("tests/golden/gup_core/scatter.png"))
        .expect("golden scatter")
        .to_rgba8();
    assert!(
        one == golden,
        "the single-chunk render no longer matches the golden"
    );

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
        let differing = image
            .pixels()
            .zip(one.pixels())
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            differing,
            0,
            "{chunks} chunks: {differing} pixels differ from one chunk ({})",
            out.display()
        );
        assert!(image == golden);
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
