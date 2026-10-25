// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end, external-crate test of the S0a vertical slice: a scatter
//! with linear x, log y and a sequential fill, rendered headlessly to a PNG
//! and checked by the GUP-388 visual regression harness using layout
//! metadata from gup-core's own layout.

mod common;

use common::scatter::{self, HEIGHT, WIDTH};
use common::svg_marks;
use common::vr::{harness, layout_metadata, metadata, rgba8};
use gup_core::prelude::*;
use gup_core::{Layout, scene::TextRole};
use gup_visual_regression::RgbaImage;

fn render() -> (image::RgbaImage, Layout, Sequential, gup_core::Scene) {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut plot = scatter::plot();
    let (image, layout) = plot
        .render_resolved(&cx, WIDTH, HEIGHT)
        .expect("render scatter");
    let scene = plot
        .resolve(&cx, WIDTH as f32, HEIGHT as f32)
        .expect("cached resolve")
        .scene
        .clone();
    (image, layout, scatter::fill(), scene)
}

#[test]
fn scatter_png_passes_the_visual_regression_harness() {
    let (image, layout, fill, scene) = render();

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
    let meta = metadata(&layout, &fill);
    harness()
        .run("gup_core/scatter", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    // GUP-467: the SVG draws the same marks.
    let svg = svg_marks::assert_matches("gup_core/scatter", &scene, &vr, &meta);
    assert_eq!(svg.matches("<circle ").count(), 120);
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
    assert_eq!(marks(resolved).chunks(), 1);
    let (one, _) = single.render_resolved(&cx, WIDTH, HEIGHT).unwrap();

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (chunk_rows, chunks) in [(40, 3), (32, 4)] {
        let mut plot = scatter::plot_chunked(chunk_rows);
        let resolved = plot.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
        let batch = marks(resolved);
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
    let scene = plot
        .resolve(&cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .scene
        .clone();
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
        .run("gup_core/categorical_nulls", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    // GUP-467: nulls are skipped (a null colour is drawn, a null position
    // is not); the SVG draws the same marks.
    svg_marks::assert_matches("gup_core/categorical_nulls", &scene, &vr, &meta);
}

/// GUP-540: with explicit domains, a point on the domain's edge is drawn
/// whole: the range is inset by the radius at its near end too, so every
/// disc lies inside the plot rect and the first tick sits a radius in.
#[test]
fn points_on_an_explicit_domain_edge_are_whole() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 10.0));
    let y = plot.y(Linear::new().domain(0.0, 10.0));
    plot.legends(false);
    plot.add(Selection::<(f64, f64), Circle>::new(vec![
        (0.0, 0.0),
        (10.0, 10.0),
        (0.0, 10.0),
    ]))
    .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
    .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
    .attr(Circle::RADIUS, Px(5.0))
    .attr(Circle::FILL, Color::hex(0x0072b2));
    let (image, layout) = plot.render_resolved(&cx, 480, 320).unwrap();
    let p = layout.plot;
    let tick = layout.x_ticks[0].1;
    assert!(
        (tick - (p.left() + 5.0)).abs() <= 0.5,
        "first x tick {tick} in {p:?}"
    );
    let centre = |a: f64, b: f64| (x.read().eval(a) as f32, y.read().eval(b) as f32);
    for (a, b) in [(0.0, 0.0), (10.0, 10.0), (0.0, 10.0)] {
        let (cx_, cy) = centre(a, b);
        for (dx, dy) in [(-1.0f32, 0.0f32), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            // 4 px inside the edge of a radius-5 disc is 1 px from its centre.
            let (px, py) = (cx_ + dx, cy + dy);
            assert!(
                px >= p.left() && px < p.right() && py >= p.top() && py < p.bottom(),
                "({a}, {b}) probe ({px}, {py}) outside {p:?}"
            );
            assert_eq!(
                image.get_pixel(px as u32, py as u32).0[..3],
                [0x00, 0x72, 0xb2],
                "disc at ({a}, {b}) near ({px}, {py})"
            );
            // The disc's outer edge is in the plot too.
            let (ex, ey) = (cx_ + 5.0 * dx, cy + 5.0 * dy);
            assert!(
                ex >= p.left() - 0.5
                    && ex <= p.right() + 0.5
                    && ey >= p.top() - 0.5
                    && ey <= p.bottom() + 0.5,
                "({a}, {b}) edge ({ex}, {ey}) outside {p:?}"
            );
        }
    }
}

/// Discs `(x, y, radius)` on explicit `0..10` domains; the radius is a
/// column on a pixel-identity scale, so it is a size, not a position.
fn edge_discs(
    rows: Vec<(f64, f64, f64)>,
    retain: Retain,
) -> (image::RgbaImage, Layout, ScaleRef<Linear>, ScaleRef<Linear>) {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 10.0));
    let y = plot.y(Linear::new().domain(0.0, 10.0));
    plot.legends(false);
    let pixels = || Linear::new().domain(0.0, 1.0).range(Px(0.0), Px(1.0));
    plot.add(Selection::<(f64, f64, f64), Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &(f64, f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64, f64)| r.1))
        .attr(Circle::RADIUS, pixels().encode(|r: &(f64, f64, f64)| r.2))
        .attr(Circle::FILL, Color::hex(0x0072b2))
        .retain(retain);
    let (image, layout) = plot.render_resolved(&cx, 480, 320).unwrap();
    (image, layout, x, y)
}

const EDGE_ROWS: [(f64, f64, f64); 4] = [
    (0.5, 5.0, 30.0),
    (9.9, 5.0, 20.0),
    (5.0, 9.8, 15.0),
    (5.0, 4.0, 10.0),
];

/// Every disc's colour is present 4 px inside its edge, in four directions.
fn assert_discs_whole(
    image: &image::RgbaImage,
    layout: &Layout,
    x: &ScaleRef<Linear>,
    y: &ScaleRef<Linear>,
) {
    let p = layout.plot;
    for (a, b, r) in EDGE_ROWS {
        let (cx_, cy) = (x.read().eval(a) as f32, y.read().eval(b) as f32);
        for (dx, dy) in [(-1.0f32, 0.0f32), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            let (px, py) = (cx_ + dx * (r as f32 - 4.0), cy + dy * (r as f32 - 4.0));
            assert!(
                px >= p.left() && px < p.right() && py >= p.top() && py < p.bottom(),
                "({a}, {b}) probe ({px}, {py}) outside {p:?}"
            );
            assert_eq!(
                image.get_pixel(px as u32, py as u32).0[..3],
                [0x00, 0x72, 0xb2],
                "disc at ({a}, {b}) near ({px}, {py})"
            );
        }
    }
}

/// The four insets of an `edge_discs` plot: near x, near y, far x, far y.
fn edge_insets(layout: &Layout, x: &ScaleRef<Linear>, y: &ScaleRef<Linear>) -> [f32; 4] {
    let p = layout.plot;
    let (x0, x10) = (x.read().eval(0.0) as f32, x.read().eval(10.0) as f32);
    let (y0, y10) = (y.read().eval(0.0) as f32, y.read().eval(10.0) as f32);
    [
        x0 - p.left(),
        p.bottom() - y0,
        p.right() - x10,
        y10 - p.top(),
    ]
}

/// GUP-528: each end of an explicit axis is inset by the marks that reach
/// it, not by the largest mark: the bottom has no mark, the left disc's
/// edge lies on the plot's edge, and the far discs keep the padding only.
#[test]
fn explicit_domain_insets_follow_the_edge_marks() {
    let (image, layout, x, y) = edge_discs(EDGE_ROWS.to_vec(), Retain::Auto);
    let p = layout.plot;
    let pad = Theme::default().plot_padding.0;
    let ex = |v: f64| x.read().eval(v) as f32;
    let ey = |v: f64| y.read().eval(v) as f32;
    let left = ex(0.5) - 30.0 - p.left();
    assert!((0.0..=1.5).contains(&left), "left disc edge {left} px in");
    let right = p.right() - (ex(9.9) + 20.0);
    assert!(
        (pad..=pad + 1.5).contains(&right),
        "right disc edge {right} px in"
    );
    let top = (ey(9.8) - 15.0) - p.top();
    assert!(
        (pad..=pad + 1.5).contains(&top),
        "top disc edge {top} px in"
    );
    assert!(
        (ey(0.0) - p.bottom()).abs() <= 0.5,
        "y=0 at {} vs {}",
        ey(0.0),
        p.bottom()
    );
    assert_discs_whole(&image, &layout, &x, &y);
}

/// GUP-528: with the CPU columns released, chunk statistics give insets
/// that are no smaller than the exact ones and no larger than GUP-540's.
#[test]
fn released_columns_inset_from_chunk_statistics() {
    let (_, exact_layout, ex, ey) = edge_discs(EDGE_ROWS.to_vec(), Retain::Auto);
    let exact = edge_insets(&exact_layout, &ex, &ey);
    let (image, layout, x, y) = edge_discs(EDGE_ROWS.to_vec(), Retain::GpuOnly);
    assert_discs_whole(&image, &layout, &x, &y);
    let pad = Theme::default().plot_padding.0;
    let got = edge_insets(&layout, &x, &y);
    let max = [30.0, 30.0, 30.0 + pad, 30.0 + pad];
    for i in 0..4 {
        assert!(
            got[i] >= exact[i] - 0.01 && got[i] <= max[i] + 0.01,
            "inset {i}: {} not in {}..={}",
            got[i],
            exact[i],
            max[i]
        );
    }
}

/// GUP-528: above the row budget both retain policies use the statistics.
#[test]
fn a_layer_over_the_row_budget_insets_from_statistics_under_every_policy() {
    let rows = || -> Vec<(f64, f64, f64)> {
        let mut rows: Vec<(f64, f64, f64)> = (0..70_000)
            .map(|i| {
                (
                    1.0 + f64::from(i % 97) / 12.0,
                    1.0 + f64::from(i % 89) / 11.0,
                    2.0,
                )
            })
            .collect();
        rows[0] = (0.0, 0.0, 20.0);
        rows
    };
    let (_, auto, ..) = edge_discs(rows(), Retain::Auto);
    let (_, gpu, ..) = edge_discs(rows(), Retain::GpuOnly);
    assert_eq!(auto.x_ticks, gpu.x_ticks);
    assert_eq!(auto.y_ticks, gpu.y_ticks);
}
