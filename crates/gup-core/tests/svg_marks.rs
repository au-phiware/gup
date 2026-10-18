// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-467: `SvgTarget` writes data marks. The golden comparison of every
//! scene lives in the `*_png.rs` tests (`common::svg_marks`); this file
//! holds the seeded proofs that the comparison catches real errors, and
//! the element-level, skip, cull and size checks.

mod common;

use common::culling;
use common::scatter::{self, HEIGHT, WIDTH};
use common::svg_marks::{compare, within};
use common::vr::metadata;
use gup_core::prelude::*;
use gup_core::{ChartExt, SvgTarget, VectorTarget};
use gup_visual_regression::RgbaImage;

fn svg_of(scene: &gup_core::Scene) -> String {
    let mut t = SvgTarget::new();
    t.render(scene).unwrap();
    t.into_string()
}

/// The golden scatter's SVG, the GPU image and the metadata.
fn golden_scatter() -> (String, RgbaImage, gup_visual_regression::LayoutMetadata) {
    let cx = Context::new_blocking().unwrap();
    let mut plot = scatter::plot();
    let (image, layout) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    let scene = plot
        .resolve(&cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .scene
        .clone();
    let gpu = RgbaImage::new(WIDTH, HEIGHT, image.into_raw()).unwrap();
    (svg_of(&scene), gpu, metadata(&layout, &scatter::fill()))
}

/// Seeded proofs: the block comparison passes the real SVG and fails each
/// of three seeded errors in one mark of 120 (the first, radius 4.5 px).
#[test]
fn the_comparison_catches_a_shifted_a_recoloured_and_a_missing_mark() {
    let (svg, gpu, meta) = golden_scatter();
    let (_, ok, _) = compare(&svg, &gpu, &meta);
    assert!(within(&ok), "the real SVG fails: {ok:?}");

    let line = svg
        .lines()
        .find(|l| l.starts_with("<circle "))
        .expect("a circle");
    let cx: f32 = line
        .split("cx=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let mutations = [
        (
            "shifted by 1 px",
            line.replacen(&format!("cx=\"{cx}\""), &format!("cx=\"{}\"", cx + 1.0), 1),
        ),
        (
            "in the wrong colour",
            line.replacen("fill=\"#", "fill=\"#ff0000\" data-was=\"#", 1),
        ),
        ("missing", String::new()),
    ];
    for (what, replacement) in mutations {
        assert_ne!(replacement, line);
        let bad = svg.replacen(line, &replacement, 1);
        let (_, stats, _) = compare(&bad, &gpu, &meta);
        eprintln!("one mark {what}: {stats:?}");
        assert!(!within(&stats), "one mark {what} passed: {stats:?}");
    }
}

/// A one-layer plot of every mark kind, element by element.
#[test]
fn each_mark_writes_its_element() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.legends(false);
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.circles(vec![(1.0, 1.0), (2.0, f64::NAN), (3.0, 2.0)])
        .x_with(x.encode(|r: &(f64, f64)| r.0))
        .y_with(y.encode(|r: &(f64, f64)| r.1))
        .radius(Px(5.0))
        .stroke(Color::hex(0x222222))
        .stroke_width(Px(2.0))
        .fill_opacity(Unit(0.5));
    plot.segments(vec![(1.0, 1.0, 3.0, 2.0)])
        .x_with(x.encode(|r: &(f64, f64, f64, f64)| r.0))
        .y_with(y.encode(|r: &(f64, f64, f64, f64)| r.1))
        .x_end_with(x.encode(|r: &(f64, f64, f64, f64)| r.2))
        .y_end_with(y.encode(|r: &(f64, f64, f64, f64)| r.3));
    plot.line(vec![(1.0, 1.0), (2.0, 2.0), (3.0, 1.5)])
        .x_with(x.encode(|r: &(f64, f64)| r.0))
        .y_with(y.encode(|r: &(f64, f64)| r.1));
    plot.rects(vec![(1.0, 0.5)])
        .x_with(x.encode(|r: &(f64, f64)| r.0))
        .y_with(y.encode(|r: &(f64, f64)| r.1))
        .width(Px(10.0))
        .y_end_with(y.encode(|_: &(f64, f64)| 0.0));
    let svg = svg_of(&plot.resolve(&cx, 300.0, 200.0).unwrap().scene);

    // The NaN row is skipped: 2 circles, each with a fill and a stroke ring.
    assert_eq!(svg.matches("<circle ").count(), 4, "{svg}");
    assert!(svg.contains(r#"fill-opacity="0.5""#));
    assert_eq!(svg.matches("<polyline ").count(), 1);
    assert!(svg.contains(r#"stroke-linejoin="round" stroke-linecap="round""#));
    // Segment: a <line> with square caps (rules are lines too).
    assert!(svg.matches(r#"stroke-linecap="square""#).count() > 1);
    assert!(svg.contains("<rect x="));
    // Layers are written in z order, one <g> each when they have no clip.
    assert!(!svg.contains("skipped"));
    // At most two decimals in a mark's coordinates.
    for l in svg.lines().filter(|l| l.starts_with("<circle ")) {
        for n in l.split('"').skip(1).step_by(2).filter(|v| v.contains('.')) {
            if n.parse::<f32>().is_ok() {
                assert!(n.split('.').nth(1).unwrap().len() <= 2, "{l}");
            }
        }
    }
}

/// `Retain::GpuOnly` drops the CPU columns, so the layer is skipped with a
/// comment (AC3), not an error.
#[test]
fn a_layer_without_cpu_columns_is_skipped_with_a_comment() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.legends(false);
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.circles(vec![(1.0, 1.0), (2.0, 2.0)])
        .x_with(x.encode(|r: &(f64, f64)| r.0))
        .y_with(y.encode(|r: &(f64, f64)| r.1))
        .retain(Retain::GpuOnly);
    let svg = svg_of(&plot.resolve(&cx, 300.0, 200.0).unwrap().scene);
    assert!(
        svg.contains(
            "<!-- layer 0 skipped: its CPU columns are not retained (Retain::GpuOnly, 2 rows) -->"
        ),
        "{svg}"
    );
    assert!(!svg.contains("<circle "));
}

/// The SVG is the view: chunks that cannot reach the plot rect are not
/// written, and culling changes nothing a pixel could show. Zoomed to four
/// of 64 chunks, the culled SVG has only those chunks' circles, and its
/// rasterisation matches the unculled one's.
#[test]
fn culled_chunks_are_not_written_and_the_picture_is_the_same() {
    let cx = Context::new_blocking().unwrap();
    let render = |culling: bool| {
        let (mut plot, (x, _)) = culling::circles(culling);
        culling::zoom_circles(&x);
        let scene = plot.resolve(&cx, 400.0, 300.0).unwrap().scene.clone();
        svg_of(&scene)
    };
    let (culled, all) = (render(true), render(false));
    let (n_culled, n_all) = (
        culled.matches("<circle ").count(),
        all.matches("<circle ").count(),
    );
    eprintln!("circles written: {n_culled} culled, {n_all} unculled");
    assert!(n_culled < n_all / 8, "{n_culled} vs {n_all}");
    assert!(n_culled > 0);
    // Every circle that could show is in both.
    let a = common::svg_marks::rasterise(&culled);
    let b = common::svg_marks::rasterise(&all);
    // Not byte-equal: resvg places its clip layer by the group's bounds,
    // which the culled circles shrink, so a few edge pixels (4 here) move by
    // one quarter-step of coverage. No circle that shows is missing.
    let moved: Vec<_> = a
        .pixels()
        .zip(b.pixels())
        .filter(|(p, q)| p.2 != q.2)
        .collect();
    assert!(moved.len() <= 8, "culling changed {} pixels", moved.len());
    for (p, q) in moved {
        let d = [(p.2.r, q.2.r), (p.2.g, q.2.g), (p.2.b, q.2.b)]
            .iter()
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap();
        assert!(d <= 20, "({}, {}): {:?} vs {:?}", p.0, p.1, p.2, q.2);
    }
}

/// The size of a 10,000-point scatter's SVG (reported in the story).
#[test]
fn ten_thousand_points_write_a_modest_svg() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, _, _) = scatter::plot_rows(scatter::countries_n(10_000), Px(2.5));
    let svg = svg_of(
        &plot
            .resolve(&cx, WIDTH as f32, HEIGHT as f32)
            .unwrap()
            .scene,
    );
    assert_eq!(svg.matches("<circle ").count(), 10_000);
    let gz = std::process::Command::new("gzip")
        .arg("-c")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write as _;
            c.stdin.take().unwrap().write_all(svg.as_bytes())?;
            c.wait_with_output()
        })
        .map(|o| o.stdout.len());
    eprintln!("10k scatter SVG: {} bytes, gzipped {gz:?}", svg.len());
    assert!(svg.len() < 1_000_000);
}
