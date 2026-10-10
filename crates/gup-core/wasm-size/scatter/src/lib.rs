// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The reference scatter (linear x, log y, viridis fill, title and tick
//! labels) on a tinted plot background with a viridis legend bar, drawn
//! through gup-core into an `ImageTarget` and read back asynchronously,
//! plus its guides as SVG: everything a browser chart needs from gup-core
//! today, and every pipeline kind (marks, rules, rects, gradients, text). A
//! second scene colours points by a dictionary-encoded key, with nulls
//! (RFC-001 S4b). A third draws the numeric scales of RFC-001 S5a (`Time`,
//! `Symlog`, `Sqrt` and a `then` chain) and checks them against their CPU
//! mirrors.

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
        kind: ItemKind::Gradient(GradientBar::new(
            &Sequential::viridis().domain(50.0, 80.0).ramp(),
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

/// A scatter coloured by a dictionary-encoded key (RFC-001 S4b): 60 rows
/// over four keys through `Categorical::encode_nullable_key`, every ninth
/// key missing (the null colour) and row 40's x NaN (not drawn, through the
/// validity bits read in the vertex stage). With `drop_null_row` row 40 is
/// left out instead, and the page checks that the pixels do not change.
/// Returns straight-alpha RGBA pixels.
#[wasm_bindgen]
pub async fn render_categorical(
    width: u32,
    height: u32,
    drop_null_row: bool,
) -> Result<Vec<u8>, JsValue> {
    std::panic::set_hook(Box::new(|info| web_error(&info.to_string())));
    let cx = Context::new().await.map_err(js)?;
    let keys = ["Europe", "Asia", "Africa", "Americas"];
    let rows: Vec<Place> = (0..60u32)
        .filter(|&i| !(drop_null_row && i == 40))
        .map(|i| Place {
            x: if i == 40 { f64::NAN } else { f64::from(i) },
            y: f64::from((i * 7) % 11 + 1),
            key: (i % 9 != 4).then_some(keys[(i % 4) as usize]),
        })
        .collect();
    let mut plot = Plot::new();
    let (x, y) = (
        plot.x(Linear::new().domain(-1.0, 60.0)),
        plot.y(Linear::new().domain(0.0, 12.0)),
    );
    plot.title("Dictionary colours")
        .add(Selection::<Place, Circle>::new(rows))
        .attr(Circle::X, x.encode(|p: &Place| p.x))
        .attr(Circle::Y, y.encode(|p: &Place| p.y))
        .attr(
            Circle::FILL,
            Categorical::okabe_ito().encode_nullable_key(|p: &Place| p.key),
        )
        .attr(Circle::RADIUS, Px(4.5));
    let resolved = plot.resolve(&cx, width as f32, height as f32).map_err(js)?;
    let image = ImageTarget::new(&cx, width, height)
        .map_err(js)?
        .render(&cx, &resolved.scene)
        .await
        .map_err(js)?;
    Ok(image.into_raw())
}

/// The numeric scale family (RFC-001 S5a, GUP-418) on WebGPU: a day of
/// hourly readings on a `Time` x axis (a hi/lo column), a signed quantity
/// on a `Symlog` y axis, a `Sqrt` radius and a fill through a three-link
/// chain, `Linear.then(Sqrt).then(viridis)`. Checks, in Rust, that every
/// point's centre is drawn in the chain's colour (its links' CPU mirrors in
/// order), then that a `Time` axis zoomed to one millisecond, 65,536
/// seconds from its chunk's origin, draws each of eight points' discs
/// within 0.25 px of the CPU mirror (the hi/lo arithmetic survives the
/// browser's shader compiler). Returns the first plot's straight-alpha
/// RGBA pixels.
#[wasm_bindgen]
pub async fn render_scales(width: u32, height: u32) -> Result<Vec<u8>, JsValue> {
    std::panic::set_hook(Box::new(|info| web_error(&info.to_string())));
    let cx = Context::new().await.map_err(js)?;
    let t0 = 1.7e9;
    // (time, signed value, size)
    let rows: Vec<(f64, f64, f64)> = (0..24u32)
        .map(|i| {
            let h = f64::from(i) - 11.5;
            (
                t0 + f64::from(i) * 3_600.0,
                h * h * h,
                f64::from(i % 7 + 1) * 1e6,
            )
        })
        .collect();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Time::new()), plot.y(Symlog::new()));
    plot.title("Scales")
        .add(Selection::<(f64, f64, f64), Circle>::new(rows.clone()))
        .attr(Circle::X, x.encode(|r: &(f64, f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64, f64)| r.1))
        .attr(
            Circle::RADIUS,
            Pow::sqrt()
                .domain(0.0, 7e6)
                .range(Px(0.0), Px(5.0))
                .encode(|r: &(f64, f64, f64)| r.2),
        )
        .attr(
            Circle::FILL,
            Linear::new()
                .range(Px(0.0), Px(1.0))
                .then(Pow::sqrt())
                .then(Sequential::viridis())
                .encode(|r: &(f64, f64, f64)| r.2),
        );
    let resolved = plot.resolve(&cx, width as f32, height as f32).map_err(js)?;
    let image = ImageTarget::new(&cx, width, height)
        .map_err(js)?
        .render(&cx, &resolved.scene)
        .await
        .map_err(js)?;
    // The fill chain fits Linear to [1e6, 7e6], then Sqrt and viridis to
    // [0, 1]: a point's colour is viridis(sqrt((size - 1e6) / 6e6)).
    let viridis = Sequential::viridis().domain(0.0, 1.0);
    for r in &rows {
        let (px, py) = (x.read().eval(r.0), y.read().eval(r.1));
        let want = viridis.eval(((r.2 - 1e6) / 6e6).sqrt()).to_rgba8();
        let got = image.get_pixel(px as u32, py as u32).0;
        if (0..3).any(|k| got[k].abs_diff(want[k]) > 2) {
            return Err(JsValue::from_str(&format!(
                "scales: the point at {r:?} drew {got:?} at ({px:.1}, {py:.1}), not the \
                 chain's {want:?}"
            )));
        }
    }

    // One millisecond across the plot, 65,536 s from the chunk's origin.
    let window = (t0 - 0.4e-3, t0 + 0.6e-3);
    let mut seconds: Vec<(f64, f64, f64)> = (0..65_536u32)
        .map(|i| (t0 - f64::from(65_536 - i) + 0.37, 0.5, 0.0))
        .collect();
    let burst: Vec<(f64, f64, f64)> = (0..8u32)
        .map(|k| {
            let t = window.0 + (window.1 - window.0) * (f64::from(k) + 0.5) / 8.0;
            (t, if k % 2 == 0 { 0.25 } else { 0.75 }, 0.0)
        })
        .collect();
    seconds.extend(&burst);
    let mut zoomed = Plot::new();
    let (x, y) = (
        zoomed.x(Time::new().domain(window.0, window.1)),
        zoomed.y(Linear::new().domain(0.0, 1.0)),
    );
    zoomed
        .add(Selection::<(f64, f64, f64), Circle>::new(seconds))
        .attr(Circle::X, x.encode(|r: &(f64, f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64, f64)| r.1))
        .attr(Circle::RADIUS, Px(5.0))
        .attr(Circle::FILL, Color::BLACK);
    let resolved = zoomed
        .resolve(&cx, width as f32, height as f32)
        .map_err(js)?;
    let deep = ImageTarget::new(&cx, width, height)
        .map_err(js)?
        .render(&cx, &resolved.scene)
        .await
        .map_err(js)?;
    let mut worst = 0.0f64;
    for r in &burst {
        let (ex, ey) = (x.read().eval(r.0), y.read().eval(r.1));
        // The coverage-weighted centre of the disc's ink (black on white).
        let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
        for py in (ey - 9.0).max(0.0) as u32..((ey + 9.0) as u32).min(height) {
            for px in (ex - 9.0).max(0.0) as u32..((ex + 9.0) as u32).min(width) {
                let w = 1.0 - f64::from(deep.get_pixel(px, py).0[0]) / 255.0;
                sx += w * (f64::from(px) + 0.5);
                sy += w * (f64::from(py) + 0.5);
                sw += w;
            }
        }
        if sw < 10.0 {
            return Err(JsValue::from_str(&format!(
                "scales: no disc near ({ex:.1}, {ey:.1}) at a 1 ms zoom"
            )));
        }
        worst = worst.max((sx / sw - ex).abs()).max((sy / sw - ey).abs());
    }
    web_log_text(&format!(
        "GUP scales 1 ms zoom: max |drawn - mirror| = {worst:.3} px"
    ));
    if worst > 0.25 {
        return Err(JsValue::from_str(&format!(
            "scales: a 1 ms Time zoom drew points {worst:.3} px from the mirror"
        )));
    }
    Ok(image.into_raw())
}

/// A row of the dictionary scene.
struct Place {
    x: f64,
    y: f64,
    key: Option<&'static str>,
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
