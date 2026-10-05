// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! End-to-end, external-crate test of the S0a vertical slice: a scatter
//! with linear x, log y and a sequential fill, rendered headlessly to a PNG
//! and checked by the GUP-388 visual regression harness using layout
//! metadata from gup-core's own layout.

use gup_core::prelude::*;
use gup_core::{Layout, scene::TextRole};
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{
    ExpectedFailures, GoldenStore, Harness, LayoutMetadata, PxRect, Rgba8, RgbaImage,
    TextRole as VrTextRole,
};
use std::path::Path;

#[derive(Clone, Debug)]
struct Country {
    gdp_per_capita: f64,
    population: f64,
    life_expectancy: f64,
}

/// Deterministic, plausible-looking data (no RNG crate needed).
fn countries() -> Vec<Country> {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    (0..120)
        .map(|_| {
            let wealth = next();
            Country {
                gdp_per_capita: 500.0 + wealth * wealth * 58_000.0,
                population: 10f64.powf(5.2 + next() * 3.8),
                life_expectancy: 52.0 + wealth * 26.0 + next() * 6.0,
            }
        })
        .collect()
}

const WIDTH: u32 = 720;
const HEIGHT: u32 = 450;
const TITLE: &str = "Wealth, population and life expectancy";

fn render() -> (image::RgbaImage, Layout, Sequential) {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
    let heat = Sequential::viridis();
    plot.title(TITLE)
        .add(Selection::<Country, Circle>::new(countries()))
        .attr(Circle::X, x.encode(|c: &Country| c.gdp_per_capita))
        .attr(Circle::Y, y.encode(|c: &Country| c.population))
        .attr(Circle::FILL, heat.encode(|c: &Country| c.life_expectancy))
        .attr(Circle::RADIUS, Px(4.5));
    let (image, layout) = plot
        .render_resolved(&cx, WIDTH, HEIGHT)
        .expect("render scatter");
    // The fill domain the plot fitted, for the expected-colour check.
    let ext = countries()
        .iter()
        .map(|c| c.life_expectancy)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    (image, layout, Sequential::viridis().domain(ext.0, ext.1))
}

fn rgba8(c: Color) -> Rgba8 {
    let [r, g, b, _] = c.to_rgba8();
    Rgba8::rgb(r, g, b)
}

fn rect(r: gup_core::geom::Rect) -> PxRect {
    PxRect::new(r.x, r.y, r.width, r.height)
}

/// The gup-core adapter: everything comes from gup-core's own `Layout`.
fn metadata(layout: &Layout, fill: &Sequential) -> LayoutMetadata {
    let mut meta = LayoutMetadata::new(rect(layout.plot)).with_background(Rgba8::WHITE);
    for t in &layout.texts {
        let role = match t.run.role {
            TextRole::Title => VrTextRole::Title,
            TextRole::TickLabel => VrTextRole::TickLabel,
        };
        meta = meta.with_text(
            role,
            t.run.text.to_string(),
            rect(t.bounds),
            rgba8(t.run.style.color),
        );
    }
    for g in &layout.guides {
        // Pad by half a pixel for the edge pixels of snapped hairlines.
        meta = meta.with_guide(rect(*g).inflate(0.5));
    }
    // The extremes of the sequential palette: the darkest and brightest
    // points are drawn with exactly these colours at their centres.
    meta.with_expected_color(
        "fill at domain min (viridis start)",
        rgba8(fill.eval(f64::MIN)),
    )
    .with_expected_color(
        "fill at domain max (viridis end)",
        rgba8(fill.eval(f64::MAX)),
    )
}

fn harness() -> Harness {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden = GoldenStore::new(root.join("tests/golden"), default_artifact_dir(&root));
    let expected =
        ExpectedFailures::load(root.join("tests/visual_regression/expected_failures.toml"))
            .expect("expected-failure list parses");
    Harness::new(golden, expected)
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
