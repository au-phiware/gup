// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The reference scatter (linear x, log y, viridis fill, title and tick
//! labels) drawn through gup-core into an `ImageTarget` and read back
//! asynchronously, plus its guides as SVG: everything a browser chart
//! needs from gup-core today.

use gup_core::prelude::*;
use gup_core::{ImageTarget, SvgTarget, VectorTarget};
use wasm_bindgen::prelude::*;

#[derive(Clone)]
struct Row {
    x: f64,
    y: f64,
    t: f64,
}

fn js(e: gup_core::Error) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// Render the scatter; returns straight-alpha RGBA pixels.
#[wasm_bindgen]
pub async fn render_scatter(width: u32, height: u32) -> Result<Vec<u8>, JsValue> {
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
    plot.title("Wealth, population and life expectancy")
        .add(Selection::<Row, Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &Row| r.x))
        .attr(Circle::Y, y.encode(|r: &Row| r.y))
        .attr(Circle::FILL, Sequential::viridis().encode(|r: &Row| r.t))
        .attr(Circle::RADIUS, Px(4.5));
    let resolved = plot.resolve(&cx, width as f32, height as f32).map_err(js)?;
    let image = ImageTarget::new(&cx, width, height)
        .map_err(js)?
        .render(&cx, &resolved.scene)
        .await
        .map_err(js)?;
    let mut svg = SvgTarget::new();
    svg.render(&resolved.scene.guides()).map_err(js)?;
    web_log(svg.svg().len());
    Ok(image.into_raw())
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn web_log(n: usize);
}
