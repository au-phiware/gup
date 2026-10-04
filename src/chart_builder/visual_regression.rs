// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chart-builder adapter for the `gup-visual-regression` harness (test-only).
//!
//! The harness itself only understands an `RgbaImage` plus `LayoutMetadata`.
//! This module is the one place where chart-builder types are translated
//! into those two values: it renders a chart offscreen and derives the plot
//! rectangle, guide geometry (axis lines, ticks) and expected text regions
//! (title, tick labels) from the chart's own layout code. A future render
//! path supplies the same two values from an adapter of its own.

use crate::chart_builder::ComposedChart;
use crate::chart_builder::builders::composite::CompositeChart;
use crate::selection::Mark;
use crate::{MaybeSend, MaybeSync};
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{
    CaseReport, ExpectedFailures, GoldenStore, Harness, LayoutMetadata, PxRect, Rgba8, RgbaImage,
    TextRole,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::OnceLock;

mod builder_cases;

/// Every chart-builder case. Expected-failure entries under
/// `chart_builders/` must match one of these (checked by a unit test).
pub(crate) const BUILDER_CASES: &[&str] = &[
    "chart_builders/area",
    "chart_builders/area_explicit_scales",
    "chart_builders/bar",
    "chart_builders/boxplot",
    "chart_builders/choropleth",
    "chart_builders/composite",
    "chart_builders/density",
    "chart_builders/gpu_density",
    "chart_builders/heatmap",
    "chart_builders/line",
    "chart_builders/line_explicit_scales",
    "chart_builders/scatter",
    "chart_builders/violin",
];

/// Padding (px) around axis-line and tick geometry when declaring it as a
/// guide region, covering line width and anti-aliasing.
const GUIDE_PAD_PX: f32 = 1.5;

/// Estimated advance of an average glyph, as a fraction of the font size.
const GLYPH_ADVANCE_EM: f32 = 0.6;

/// Estimated line height, as a fraction of the font size.
const LINE_HEIGHT_EM: f32 = 1.2;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The shared harness: golden images in `tests/golden/`, the tracked list in
/// `tests/visual_regression/expected_failures.toml`, latest renders and diffs
/// in `target/visual-regression/`.
pub(crate) fn harness() -> &'static Harness {
    static HARNESS: OnceLock<Harness> = OnceLock::new();
    HARNESS.get_or_init(|| {
        let root = workspace_root();
        let golden = GoldenStore::new(root.join("tests/golden"), default_artifact_dir(root));
        let expected =
            ExpectedFailures::load(root.join("tests/visual_regression/expected_failures.toml"))
                .unwrap_or_else(|e| panic!("invalid expected-failure list: {e}"));
        Harness::new(golden, expected)
    })
}

/// Run every harness check for `case` and panic on anything unexpected.
pub(crate) fn assert_case(case: &str, capture: Result<(RgbaImage, LayoutMetadata), String>) {
    assert!(
        BUILDER_CASES.contains(&case),
        "{case} is not listed in BUILDER_CASES"
    );
    let report: CaseReport = harness().run(case, capture);
    report.assert_ok();
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "non-string panic payload".to_string())
}

fn chart_size<T, M>(chart: &ComposedChart<T, M>) -> (u32, u32)
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
    M: Mark,
{
    (
        chart.config.width.round() as u32,
        chart.config.height.round() as u32,
    )
}

/// Render a chart at its configured size via `render_to_rgba()` and derive
/// its layout metadata. Errors and panics become a render failure.
pub(crate) fn capture_chart<T, M>(
    chart: &mut ComposedChart<T, M>,
) -> Result<(RgbaImage, LayoutMetadata), String>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
    M: Mark,
{
    let (w, h) = chart_size(chart);
    let pixels = catch_unwind(AssertUnwindSafe(|| chart.render_to_rgba(w, h)))
        .map_err(|p| format!("render_to_rgba panicked: {}", panic_message(p)))?
        .map_err(|e| format!("render_to_rgba failed: {e}"))?;
    let image = RgbaImage::new(w, h, pixels).map_err(|e| e.to_string())?;
    Ok((image, chart_layout(chart)))
}

/// Render a composite chart offscreen through its public
/// `prepare_render()` + `draw()` API (it has no `render_to_rgba`).
pub(crate) fn capture_composite<T>(
    chart: &mut CompositeChart<T>,
) -> Result<(RgbaImage, LayoutMetadata), String>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    let (w, h) = chart_size(chart.primary());
    let context = chart
        .primary()
        .visualization
        .context()
        .cloned()
        .ok_or("composite primary layer has no RenderContext")?;
    let pixels = catch_unwind(AssertUnwindSafe(|| {
        let device = context.device();
        let queue = context.queue();
        let target = crate::export::png::OffscreenTarget::new(device, w, h);
        chart.prepare_render(device, queue, wgpu::TextureFormat::Bgra8UnormSrgb)?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("visual_regression_composite_encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("visual_regression_composite_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            chart.draw(&mut pass)?;
        }
        queue.submit(std::iter::once(encoder.finish()));
        target.readback_pixels(device, queue)
    }))
    .map_err(|p| format!("composite render panicked: {}", panic_message(p)))?
    .map_err(|e| format!("composite render failed: {e}"))?;
    let image = RgbaImage::new(w, h, pixels).map_err(|e| e.to_string())?;
    Ok((image, chart_layout(chart.primary())))
}

/// Capture a choropleth chart. `ChoroplethChart` resolves CPU fill/stroke
/// geometry only and has no raster render path, so this reports a render
/// failure describing what was built. Replace it with a real capture when a
/// render path exists.
pub(crate) fn capture_choropleth(
    chart: &crate::chart_builder::builders::choropleth::ChoroplethChart,
) -> Result<(RgbaImage, LayoutMetadata), String> {
    Err(format!(
        "ChoroplethChart has no raster render path (no render_to_rgba or draw); \
         build() produced {} regions, {} fill vertices and {} stroke vertices of CPU geometry only",
        chart.regions.len(),
        chart.fill_vertices.len(),
        chart.stroke_vertices.len()
    ))
}

/// Derive the harness layout metadata from a chart's own layout code.
pub(crate) fn chart_layout<T, M>(chart: &ComposedChart<T, M>) -> LayoutMetadata
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
    M: Mark,
{
    let (w, h) = (chart.config.width, chart.config.height);
    let to_px = |ndc: [f32; 2]| ((ndc[0] + 1.0) * 0.5 * w, (1.0 - ndc[1]) * 0.5 * h);
    let segment = |a: [f32; 2], b: [f32; 2]| {
        let (ax, ay) = to_px(a);
        let (bx, by) = to_px(b);
        PxRect::from_edges(ax, ay, bx, by).inflate(GUIDE_PAD_PX)
    };

    let area = chart.calculate_chart_area();
    let background = chart
        .config
        .background_color
        .map(Rgba8::from_unit_f32)
        .unwrap_or(Rgba8::WHITE);
    let mut layout = LayoutMetadata::new(PxRect::new(area.x, area.y, area.width, area.height))
        .with_background(background);

    let geometry = chart.generate_axis_geometry_instanced();
    for pair in geometry.line_vertices.chunks_exact(2) {
        layout = layout.with_guide(segment(pair[0].position, pair[1].position));
    }
    for tick in &geometry.tick_instances {
        let end = [
            tick.position[0] + tick.tick_vector[0],
            tick.position[1] + tick.tick_vector[1],
        ];
        layout = layout.with_guide(segment(tick.position, end));
    }

    let label_size = chart.config.label_style.font_size;
    let label_color = style_color(&chart.config.label_style);
    let title_color = style_color(&chart.config.title_style);
    for label in &geometry.labels {
        let rect = text_rect(
            &label.text,
            label_size,
            label.anchor,
            label.screen_position.x,
            label.screen_position.y,
        );
        layout = layout.with_text(TextRole::TickLabel, label.text.clone(), rect, label_color);
    }

    if let Some(title) = &chart.config.title_config {
        use crate::chart_builder::TitleAlignment;
        let margins = &chart.config.margins;
        let (anchor, x) = match title.alignment {
            TitleAlignment::Left => (crate::text::TextAnchor::TopLeft, margins.left),
            TitleAlignment::Center => (crate::text::TextAnchor::TopCenter, w / 2.0),
            TitleAlignment::Right => (crate::text::TextAnchor::TopRight, w - margins.right),
        };
        let y = title.y_offset.unwrap_or(margins.top / 2.0);
        let rect = text_rect(
            &title.text,
            chart.config.title_style.font_size,
            anchor,
            x,
            y,
        );
        layout = layout.with_text(TextRole::Title, title.text.clone(), rect, title_color);
    }

    if let Some(colorbar) = chart.colorbar_geometry() {
        for tri in colorbar.gradient_vertices.chunks_exact(3) {
            let (x0, y0) = to_px(tri[0].position);
            let (x1, y1) = to_px(tri[1].position);
            let (x2, y2) = to_px(tri[2].position);
            layout = layout.with_guide(
                PxRect::from_edges(
                    x0.min(x1).min(x2),
                    y0.min(y1).min(y2),
                    x0.max(x1).max(x2),
                    y0.max(y1).max(y2),
                )
                .inflate(GUIDE_PAD_PX),
            );
        }
        for pair in colorbar.line_vertices.chunks_exact(2) {
            layout = layout.with_guide(segment(pair[0].position, pair[1].position));
        }
        for tick in &colorbar.tick_instances {
            let end = [
                tick.position[0] + tick.tick_vector[0],
                tick.position[1] + tick.tick_vector[1],
            ];
            layout = layout.with_guide(segment(tick.position, end));
        }
        for label in &colorbar.labels {
            let rect = text_rect(
                &label.text,
                label_size,
                label.anchor,
                label.screen_position.x,
                label.screen_position.y,
            );
            layout = layout.with_text(TextRole::Legend, label.text.clone(), rect, label_color);
        }
    }

    layout
}

/// The configured colour of a text style as an 8-bit sRGB colour.
fn style_color(style: &crate::text::TextStyle) -> Rgba8 {
    let c = style.color;
    Rgba8::from_unit_f32([c.x, c.y, c.z, c.w])
}

/// Estimate the glyph box of `text` drawn at `(x, y)` with `anchor`.
fn text_rect(
    text: &str,
    font_size: f32,
    anchor: crate::text::TextAnchor,
    x: f32,
    y: f32,
) -> PxRect {
    let chars = text.chars().count().max(1) as f32;
    let width = chars * GLYPH_ADVANCE_EM * font_size;
    let height = LINE_HEIGHT_EM * font_size;
    let offset = anchor.offset();
    PxRect::new(x - offset.x * width, y - offset.y * height, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gup_visual_regression::checks::{Tolerances, check_text_present};

    #[test]
    fn expected_failures_reference_known_cases() {
        for entry in harness().expected().entries() {
            if let Some(rest) = entry.case.strip_prefix("chart_builders/") {
                let matched = BUILDER_CASES.iter().any(|c| entry.matches_case(c));
                assert!(
                    matched,
                    "expected-failure entry {:?} ({rest}) matches no chart-builder case",
                    entry.case
                );
            }
        }
    }

    #[test]
    fn text_rect_respects_anchor() {
        let r = text_rect("100", 10.0, crate::text::TextAnchor::TopCenter, 50.0, 20.0);
        assert_eq!(r, PxRect::new(41.0, 20.0, 18.0, 12.0));
        let r = text_rect("1", 10.0, crate::text::TextAnchor::CenterRight, 50.0, 20.0);
        assert_eq!(r, PxRect::new(44.0, 14.0, 6.0, 12.0));
    }

    /// The adapter's text regions must not be satisfied by axis/tick ink
    /// alone: a chart image containing only guides fails text presence.
    #[test]
    fn guides_alone_do_not_satisfy_text_regions() {
        let layout = LayoutMetadata::new(PxRect::new(60.0, 40.0, 280.0, 200.0))
            .with_guide(PxRect::new(58.5, 238.5, 283.0, 3.0))
            .with_text(
                TextRole::TickLabel,
                "0",
                PxRect::new(55.0, 246.0, 10.0, 16.0),
                Rgba8::BLACK,
            );
        let mut img = RgbaImage::filled(400, 300, Rgba8::WHITE);
        img.fill_rect(PxRect::new(60.0, 239.0, 280.0, 1.0), Rgba8::rgb(60, 60, 60));
        assert!(check_text_present(&img, &layout, &Tolerances::default()).is_err());
    }
}
