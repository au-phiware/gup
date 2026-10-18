// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! One golden-image + structural regression case per chart builder.
//!
//! Each case builds a small representative chart through the builder's
//! public API, with explicitly configured colours where the builder accepts
//! them, and runs it through the harness. Failures that reflect known bugs
//! are tracked in `tests/visual_regression/expected_failures.toml`; never
//! adjust a case to dodge a check.
//!
//! Run: `cargo test --lib visual_regression -- --test-threads=1`
//! Bless: `GUP_BLESS=1 cargo test --lib visual_regression -- --test-threads=1`

use super::{assert_case, capture_chart, capture_choropleth};
use crate::Rectangle;
use crate::RenderContext;
use crate::chart_builder::accessor::AccessorValue;
use crate::chart_builder::builders::{
    AccessorFunction, ConfigurableBuilder, area, boxplot, density_plot, heatmap, violin,
};
use crate::chart_builder::{AxisScale, ChartBuilder, ComposedChart};
use crate::shader_function::{ColorScale, LinearScale};
use gup_visual_regression::Rgba8;
use std::sync::Arc;

/// Chart size used by every case.
const W: f32 = 400.0;
const H: f32 = 300.0;

const ORANGE: [f32; 4] = [1.0, 127.0 / 255.0, 14.0 / 255.0, 1.0];
const PURPLE: [f32; 4] = [148.0 / 255.0, 103.0 / 255.0, 189.0 / 255.0, 1.0];
const CYAN: [f32; 4] = [23.0 / 255.0, 190.0 / 255.0, 207.0 / 255.0, 1.0];

async fn context() -> Arc<RenderContext> {
    Arc::new(RenderContext::new().await.expect("RenderContext"))
}

fn rgba(c: [f32; 4]) -> Rgba8 {
    Rgba8::from_unit_f32(c)
}

#[derive(Debug, Clone)]
struct Xy {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone)]
struct Category {
    name: String,
    value: f32,
}

#[derive(Debug, Clone)]
struct Samples {
    name: String,
    values: Vec<f32>,
}

#[derive(Debug, Clone)]
struct Cell {
    x: f32,
    y: f32,
    value: f32,
}

fn x_of(d: &Xy) -> AccessorValue {
    AccessorValue::Float(d.x)
}

fn y_of(d: &Xy) -> AccessorValue {
    AccessorValue::Float(d.y)
}

fn series() -> Vec<Xy> {
    [
        (0.0, 10.0),
        (1.0, 50.0),
        (2.0, 30.0),
        (3.0, 70.0),
        (4.0, 55.0),
    ]
    .into_iter()
    .map(|(x, y)| Xy { x, y })
    .collect()
}

/// A deterministic pseudo-random value in `-0.5..0.5`.
fn lcg(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
}

#[tokio::test]
async fn golden_area() {
    let mut chart = area()
        .x(AccessorFunction::new(x_of))
        .y(AccessorFunction::new(y_of))
        .color(AccessorFunction::new(|_: &Xy| AccessorValue::Color(PURPLE)))
        .opacity(1.0)
        .title("Area")
        .width(W)
        .height(H)
        .build_with_data(series(), context().await)
        .expect("area build");
    let capture = capture_chart(&mut chart)
        .map(|(img, layout)| (img, layout.with_expected_color("area fill", rgba(PURPLE))));
    assert_case("chart_builders/area", capture);
}

/// Explicit-scale path (GUP-364) for the area builder.
#[tokio::test]
async fn golden_area_explicit_scales() {
    let mut chart = area()
        .x(AccessorFunction::new(x_of))
        .y(AccessorFunction::new(y_of))
        .x_scale(AxisScale::Linear(LinearScale::new(0.0, 4.0, -1.0, 1.0)))
        .y_scale(AxisScale::Linear(LinearScale::new(0.0, 80.0, -1.0, 1.0)))
        .width(W)
        .height(H)
        .build_with_data(series(), context().await)
        .expect("area build");
    assert!(chart.visualization.is_render_ready());
    assert_case(
        "chart_builders/area_explicit_scales",
        capture_chart(&mut chart),
    );
}

fn box_samples() -> Vec<Samples> {
    [
        ("A", vec![10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0]),
        ("B", vec![22.0, 26.0, 28.0, 30.0, 33.0, 38.0, 55.0]),
        ("C", vec![5.0, 12.0, 14.0, 18.0, 21.0, 24.0, 29.0]),
    ]
    .into_iter()
    .map(|(n, values)| Samples {
        name: n.to_string(),
        values,
    })
    .collect()
}

#[tokio::test]
async fn golden_boxplot() {
    let mut chart = boxplot()
        .category(AccessorFunction::new(|d: &Samples| {
            AccessorValue::String(d.name.clone())
        }))
        .y(AccessorFunction::new(|d: &Samples| {
            AccessorValue::FloatArray(d.values.clone())
        }))
        .fill_color(CYAN)
        .title("Box plot")
        .width(W)
        .height(H)
        .build_with_data(box_samples(), context().await)
        .expect("boxplot build");
    let capture = capture_chart(&mut chart)
        .map(|(img, layout)| (img, layout.with_expected_color("box fill", rgba(CYAN))));
    assert_case("chart_builders/boxplot", capture);
}

#[tokio::test]
async fn golden_violin() {
    let data: Vec<Category> = (0..60)
        .map(|i| {
            let (name, centre, spread) = if i % 2 == 0 {
                ("A", 20.0, 8.0)
            } else {
                ("B", 35.0, 5.0)
            };
            let t = (i / 2) as f32 / 29.0 - 0.5;
            Category {
                name: name.to_string(),
                value: centre + spread * 2.0 * t * t.abs().sqrt(),
            }
        })
        .collect();
    // `ViolinPlotBuilder::width` sets the violin width, so the chart size
    // goes through `ConfigurableBuilder` explicitly.
    let builder = violin()
        .x(AccessorFunction::new(|d: &Category| {
            AccessorValue::String(d.name.clone())
        }))
        .y(AccessorFunction::new(|d: &Category| {
            AccessorValue::Float(d.value)
        }))
        .color(AccessorFunction::new(|_: &Category| {
            AccessorValue::Color(ORANGE)
        }))
        .title("Violin");
    let builder = ConfigurableBuilder::width(builder, W);
    let builder = ConfigurableBuilder::height(builder, H);
    let mut chart = builder
        .build_with_data(data, context().await)
        .expect("violin build");
    let capture = capture_chart(&mut chart)
        .map(|(img, layout)| (img, layout.with_expected_color("violin fill", rgba(ORANGE))));
    assert_case("chart_builders/violin", capture);
}

/// Two deterministic point clusters.
fn clusters() -> Vec<Xy> {
    let mut seed = 0x2545_f491_u32;
    (0..200)
        .map(|i| {
            let (cx, cy) = if i % 2 == 0 { (3.0, 3.0) } else { (7.0, 6.0) };
            Xy {
                x: cx + 2.0 * (lcg(&mut seed) + lcg(&mut seed)),
                y: cy + 2.0 * (lcg(&mut seed) + lcg(&mut seed)),
            }
        })
        .collect()
}

async fn density_chart(title: &str, gpu_threshold: usize) -> ComposedChart<Xy, Rectangle> {
    density_plot()
        .x(AccessorFunction::new(x_of))
        .y(AccessorFunction::new(y_of))
        .gpu_threshold(gpu_threshold)
        .title(title)
        .width(W)
        .height(H)
        .build_with_data(clusters(), context().await)
        .expect("density build")
}

/// Density plot with the CPU KDE path.
#[tokio::test]
async fn golden_density() {
    let mut chart = density_chart("Density", usize::MAX).await;
    assert_case("chart_builders/density", capture_chart(&mut chart));
}

/// The GPU KDE backend (`gpu_density`): the same chart with the GPU
/// threshold below the sample count.
#[tokio::test]
async fn golden_gpu_density() {
    let mut chart = density_chart("GPU density", 10).await;
    assert_case("chart_builders/gpu_density", capture_chart(&mut chart));
}

#[tokio::test]
async fn golden_heatmap() {
    let data: Vec<Cell> = (0..10)
        .flat_map(|xi| {
            (0..8).map(move |yi| {
                let (dx, dy) = (xi as f32 - 4.5, yi as f32 - 3.5);
                Cell {
                    x: xi as f32 + 0.5,
                    y: yi as f32 + 0.5,
                    value: (100.0 - 4.0 * (dx * dx + dy * dy)).max(0.0),
                }
            })
        })
        .collect();
    let mut chart = heatmap()
        .x(AccessorFunction::new(|d: &Cell| AccessorValue::Float(d.x)))
        .y(AccessorFunction::new(|d: &Cell| AccessorValue::Float(d.y)))
        .fill(AccessorFunction::new(|d: &Cell| {
            AccessorValue::Float(d.value)
        }))
        .x_bins(10)
        .y_bins(8)
        .x_domain(0.0, 10.0)
        .y_domain(0.0, 8.0)
        .color_scale(ColorScale::viridis(0.0, 100.0))
        .title("Heatmap")
        .width(W)
        .height(H)
        .build_with_data(data, context().await)
        .expect("heatmap build");
    assert_case("chart_builders/heatmap", capture_chart(&mut chart));
}

/// `ChoroplethChart` has no raster render path; the adapter reports that as
/// a tracked render failure, so adding a render path turns this into a real
/// golden test.
#[test]
fn golden_choropleth() {
    use crate::chart_builder::builders::choropleth::ChoroplethChartBuilder;
    use crate::mark::geo_path::GeoJsonSource;

    let geojson = serde_json::json!({
        "type": "FeatureCollection",
        "features": [
            { "type": "Feature", "properties": { "iso_a3": "AAA" },
              "geometry": { "type": "Polygon", "coordinates": [[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]]] } },
            { "type": "Feature", "properties": { "iso_a3": "BBB" },
              "geometry": { "type": "Polygon", "coordinates": [[[10.0, 0.0], [20.0, 0.0], [20.0, 10.0], [10.0, 10.0], [10.0, 0.0]]] } }
        ]
    });
    let chart = ChoroplethChartBuilder::new()
        .boundaries(GeoJsonSource::from_str(&geojson.to_string()).expect("geojson"))
        .data(vec![("AAA", 10.0), ("BBB", 90.0)])
        .region_id(|f| {
            f.properties
                .as_ref()
                .and_then(|p| p.get("iso_a3"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .color_scale(ColorScale::viridis(0.0, 100.0))
        .build()
        .expect("choropleth build");
    assert_case("chart_builders/choropleth", capture_choropleth(&chart));
}
