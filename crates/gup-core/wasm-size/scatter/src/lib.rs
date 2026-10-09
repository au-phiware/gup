// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The reference scatter (linear x, log y, viridis fill, title and tick
//! labels) on a tinted plot background with a viridis legend bar, drawn
//! through gup-core into an `ImageTarget` and read back asynchronously,
//! plus its guides as SVG: everything a browser chart needs from gup-core
//! today, and every pipeline kind (marks, rules, rects, gradients, text).

use gup_core::geom::Rect;
use gup_core::prelude::*;
use gup_core::scene::{GradientBar, GradientDirection, Item, ItemKind, RectPrim, Z_GRID, Z_TITLE};
use gup_core::{ImageTarget, SvgTarget, VectorTarget};
use wasm_bindgen::prelude::*;

/// Room on the right for the legend bar.
const LEGEND_WIDTH: f32 = 24.0;

#[derive(Clone)]
struct Row {
    x: f64,
    y: f64,
    t: f64,
}

fn js(e: gup_core::Error) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Render the scatter; returns straight-alpha RGBA pixels. A non-zero
/// `max_chunk_rows` splits its 200 rows into column chunks of at most that
/// many rows, so the layer is drawn by one instanced draw per chunk, each
/// at its own dynamic uniform offset (RFC-001 S4a); the page checks that
/// the pixels do not change.
#[wasm_bindgen]
pub async fn render_scatter(
    width: u32,
    height: u32,
    max_chunk_rows: u32,
) -> Result<Vec<u8>, JsValue> {
    std::panic::set_hook(Box::new(|info| web_error(&info.to_string())));
    let cx = Context::new().await.map_err(js)?;
    let rows: Vec<Row> = (1..=200)
        .map(|i| {
            let i = f64::from(i);
            Row {
                x: i * 300.0,
                y: 10f64.powf(5.0 + (i * 0.37) % 4.0),
                t: 50.0 + (i * 7.0) % 30.0,
            }
        })
        .collect();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
    let layer = plot
        .title("Wealth, population and life expectancy")
        .add(Selection::<Row, Circle>::new(rows));
    if max_chunk_rows > 0 {
        layer.max_chunk_rows(max_chunk_rows);
    }
    layer
        .attr(Circle::X, x.encode(|r: &Row| r.x))
        .attr(Circle::Y, y.encode(|r: &Row| r.y))
        .attr(Circle::FILL, Sequential::viridis().encode(|r: &Row| r.t))
        .attr(Circle::RADIUS, Px(4.5));
    let resolved = plot
        .resolve(&cx, width as f32 - LEGEND_WIDTH, height as f32)
        .map_err(js)?;
    let chunks = resolved
        .scene
        .items
        .iter()
        .find_map(|i| match &i.kind {
            ItemKind::Marks(batch) => Some(batch.chunks()),
            _ => None,
        })
        .unwrap_or(0);
    web_log_text(&format!(
        "GUP CHUNKS {chunks} (max_chunk_rows {max_chunk_rows})"
    ));
    let (mut scene, plot_rect) = (resolved.scene, resolved.layout.plot);
    scene.width = width as f32;
    let clip = scene.add_clip(plot_rect);
    scene.push(Item {
        z: Z_GRID,
        clip: Some(clip),
        kind: ItemKind::Rects(vec![RectPrim {
            rect: plot_rect,
            color: Color::hex(0xeef1f6),
        }]),
    });
    let right = width as f32 - LEGEND_WIDTH;
    scene.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Gradient(GradientBar::sequential(
            &Sequential::viridis().domain(50.0, 80.0),
            Rect::from_edges(
                right + 6.0,
                plot_rect.top(),
                right + 18.0,
                plot_rect.bottom(),
            ),
            GradientDirection::Vertical,
        )),
    });
    let image = ImageTarget::new(&cx, width, height)
        .map_err(js)?
        .render(&cx, &scene)
        .await
        .map_err(js)?;
    let mut svg = SvgTarget::new();
    svg.render(&scene.guides()).map_err(js)?;
    web_log(svg.svg().len());
    Ok(image.into_raw())
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn web_log(n: usize);
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn web_log_text(message: &str);
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn web_error(message: &str);
}
