// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end, external-crate test of the S0a vertical slice: a scatter
//! with linear x, log y and a sequential fill, rendered headlessly to a PNG
//! and checked by the GUP-388 visual regression harness using layout
//! metadata from gup-core's own layout.

mod common;

use common::scatter::{self, HEIGHT, WIDTH};
use common::vr::{harness, layout_metadata, metadata, rgba8};
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

/// RFC-001 S4b (AC1, AC2, AC4): a scatter coloured by continent through a
/// dictionary column (`encode_nullable_key`), with missing continents and
/// some null positions.
///
/// - **Null positions are not drawn.** Six isolated rows get a NaN x, a
///   NaN y or an infinite y. Their would-be discs (where the same rows are
///   drawn in a plot with every position valid) hold no ink, and the
///   image equals, byte for byte, the plot with those rows removed: on the
///   same device in the same run, the other rows go through the same
///   arithmetic. (Intel/Mesa does not rasterise a NaN position even with
///   the degenerate quad disabled, so this shows the outcome;
///   `plot::tests::a_cleared_validity_bit_hides_a_finite_row` shows that
///   the validity bit alone hides a row.)
/// - **Every drawn point carries its category's colour.** At the centre
///   of every point that no later point overlaps, the pixel is the
///   Okabe-Ito colour of its continent's first-seen code (computed here,
///   independently of gup-core), or the null colour where the continent is
///   missing.
/// - **The GUP-388 harness** checks the image (title and ticks, marks
///   inside the plot, the five palette colours and the null colour) and
///   compares it with the golden through its ΔE tolerance.
#[test]
fn categorical_scatter_with_nulls() {
    use common::continents::{self, HEIGHT, Place, RADIUS, WIDTH};
    use gup_core::{NULL_COLOR, OKABE_ITO};

    let cx = Context::new_blocking().expect("headless GPU context");
    let rows = continents::places();
    let white = image::Rgba([255, 255, 255, 255]);

    // Every position valid: where each row is drawn.
    let (mut all, x, y) = continents::plot(rows.clone());
    let (all_image, all_layout) = all.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    let centre = |p: &Place| -> (f32, f32) {
        (
            x.read().eval(p.gdp_per_capita) as f32,
            y.read().eval(p.population) as f32,
        )
    };
    let centres: Vec<_> = rows.iter().map(centre).collect();
    let dist = |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    // Rows whose disc touches no other disc (and that are not the first
    // of their continent, so removing them keeps every code).
    let isolated: Vec<usize> = (20..rows.len())
        .filter(|&i| {
            (0..rows.len()).all(|j| j == i || dist(centres[i], centres[j]) > 2.0 * RADIUS + 3.0)
        })
        .take(6)
        .collect();
    assert_eq!(isolated.len(), 6, "{isolated:?}");

    let mut nulls = rows.clone();
    for (k, &i) in isolated.iter().enumerate() {
        match k % 3 {
            0 => nulls[i].gdp_per_capita = f64::NAN,
            1 => nulls[i].population = f64::NAN,
            _ => nulls[i].population = f64::INFINITY,
        }
    }
    let (mut plot, _, _) = continents::plot(nulls.clone());
    let (image, layout) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    assert_eq!(layout.plot, all_layout.plot);

    // AC1: no ink where the null rows would be; the control image has it.
    for &i in &isolated {
        let (cx_, cy_) = centres[i];
        let mut covered = 0;
        for py in (cy_ - RADIUS) as u32..=(cy_ + RADIUS) as u32 {
            for px in (cx_ - RADIUS) as u32..=(cx_ + RADIUS) as u32 {
                if dist((px as f32 + 0.5, py as f32 + 0.5), (cx_, cy_)) < RADIUS - 1.0 {
                    covered += 1;
                    assert_ne!(*all_image.get_pixel(px, py), white, "control: row {i}");
                    assert_eq!(*image.get_pixel(px, py), white, "null row {i} drawn");
                }
            }
        }
        assert!(covered > 30, "row {i}: {covered} pixels checked");
    }
    let kept: Vec<Place> = rows
        .iter()
        .enumerate()
        .filter(|(i, _)| !isolated.contains(i))
        .map(|(_, p)| p.clone())
        .collect();
    let (removed, _) = continents::plot(kept)
        .0
        .render_resolved(&cx, WIDTH, HEIGHT)
        .unwrap();
    assert!(
        image == removed,
        "null rows changed more than their own discs"
    );

    // AC2, AC4: each point's centre in its category's colour.
    let palette = |code: Option<u32>| match code {
        Some(c) => OKABE_ITO[c as usize % OKABE_ITO.len()],
        None => NULL_COLOR,
    };
    let codes = continents::codes(&nulls);
    let mut checked = [0usize; 6];
    for (i, p) in nulls.iter().enumerate() {
        if isolated.contains(&i) {
            continue;
        }
        let (cx_, cy_) = centre(p);
        let pixel = (cx_.floor(), cy_.floor());
        let at = (pixel.0 + 0.5, pixel.1 + 0.5);
        // Skip points that a later point (drawn on top) reaches.
        let covered_later = (i + 1..nulls.len())
            .filter(|j| !isolated.contains(j))
            .any(|j| dist(centres[j], at) < RADIUS + 1.5);
        if covered_later {
            continue;
        }
        let expected = palette(codes[i]).to_rgba8();
        let got = image.get_pixel(pixel.0 as u32, pixel.1 as u32).0;
        assert!(
            got.iter().zip(expected).all(|(g, e)| g.abs_diff(e) <= 1),
            "row {i} ({:?}, code {:?}): {got:?}, expected {expected:?}",
            p.continent,
            codes[i]
        );
        checked[codes[i].map_or(5, |c| c as usize)] += 1;
    }
    eprintln!("points checked per code (5 = null): {checked:?}");
    assert!(checked.iter().all(|&n| n >= 2), "{checked:?}");

    let mut meta = layout_metadata(&layout);
    for (k, c) in OKABE_ITO.iter().take(5).enumerate() {
        meta = meta.with_expected_color(format!("code {k}"), rgba8(*c));
    }
    meta = meta.with_expected_color("null", rgba8(NULL_COLOR));
    let vr = RgbaImage::new(WIDTH, HEIGHT, image.into_raw()).unwrap();
    harness()
        .run("gup_core/categorical_nulls", Ok((vr, meta)))
        .assert_ok();
}
