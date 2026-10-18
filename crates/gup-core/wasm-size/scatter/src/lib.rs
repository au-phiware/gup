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
//! mirrors. A fourth and fifth draw the colour and ordinal scales of
//! RFC-001 S5b: a `Band` axis with an 11-key `Categorical` and its swatch
//! legend, a hidden null key, and a `Diverging` scale with its ramp. A
//! sixth draws user functions written with `#[wgsl_function]` (RFC-001
//! S5c).

use gup_core::geom::Rect;
use gup_core::prelude::*;
use gup_core::scale::PositionScale;
use gup_core::scene::{
    GradientBar, GradientDirection, Item, ItemKind, RectPrim, TextRole, Z_GRID, Z_TITLE,
};
use gup_core::{ImageTarget, Resolved, RowId, SvgTarget, VectorTarget};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
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
    let (mut scene, plot_rect) = (resolved.scene.clone(), resolved.layout.plot);
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

/// The colour scales and ordinal positions of RFC-001 S5b (GUP-419) on
/// WebGPU: eleven keys on a `Band` x axis, coloured by the full
/// `Categorical` (Okabe-Ito's 8 and 3 generated colours), with a swatch
/// legend to the right; every fifth row's key is missing. Checks, in
/// Rust, that every keyed row's centre is drawn in its key's colour (the
/// mirror's), that each legend swatch is drawn in its key's colour and
/// that the 11 colours differ. Then that a null key is hidden by the
/// glue, not by its position: a `Band` with an empty range maps every
/// code, the null code too, to one row of pixels, where the keyed rows
/// draw and the null rows must not. Returns the first plot's
/// straight-alpha RGBA pixels.
#[wasm_bindgen]
pub async fn render_band_categorical(width: u32, height: u32) -> Result<Vec<u8>, JsValue> {
    std::panic::set_hook(Box::new(|info| web_error(&info.to_string())));
    let cx = Context::new().await.map_err(js)?;
    const KEYS: [&str; 11] = [
        "Ash", "Birch", "Cedar", "Elm", "Fir", "Hazel", "Larch", "Maple", "Oak", "Pine", "Yew",
    ];
    // (key, value)
    let rows: Vec<(Option<&'static str>, f64)> = (0..66u32)
        .map(|i| {
            let k = (i % 11) as usize;
            let key = (i % 5 != 4).then_some(KEYS[k]);
            (key, 2.0 + f64::from((i * 7) % 13) + k as f64 * 0.5)
        })
        .collect();
    type Row = (Option<&'static str>, f64);
    let legend_width = 64.0;
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new()));
    let colour = ScaleRef::new(Categorical::okabe_ito());
    plot.legends(false)
        .title("Band and categorical")
        .add(Selection::<Row, Circle>::new(rows.clone()))
        .attr(Circle::X, x.encode_nullable_key(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::RADIUS, Px(3.5))
        .attr(Circle::FILL, colour.encode_nullable_key(|r: &Row| r.0));
    let resolved = plot
        .resolve(&cx, width as f32 - legend_width, height as f32)
        .map_err(js)?;
    let mut scene = resolved.scene.clone();
    scene.width = width as f32;
    let swatches = colour.read().legend().unwrap().swatches().to_vec();
    let left = width as f32 - legend_width + 6.0;
    let rects: Vec<RectPrim> = swatches
        .iter()
        .enumerate()
        .map(|(i, s)| RectPrim {
            rect: Rect::new(left, resolved.layout.plot.top() + i as f32 * 12.0, 9.0, 9.0),
            color: s.color,
        })
        .collect();
    scene.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Rects(rects.clone()),
    });
    let image = ImageTarget::new(&cx, width, height)
        .map_err(js)?
        .render(&cx, &scene)
        .await
        .map_err(js)?;

    let fail = |m: String| Err(JsValue::from_str(&format!("band/categorical: {m}")));
    if swatches.len() != KEYS.len() {
        return fail(format!("{} swatches", swatches.len()));
    }
    for (i, a) in swatches.iter().enumerate() {
        if swatches[..i].iter().any(|b| b.color == a.color) {
            return fail(format!("{} repeats a colour", a.label));
        }
        let r = rects[i].rect;
        let got = image
            .get_pixel((r.x + r.width / 2.0) as u32, (r.y + r.height / 2.0) as u32)
            .0;
        if got != a.color.to_rgba8() {
            return fail(format!(
                "swatch {} drew {got:?}, not {:?}",
                a.label, a.color
            ));
        }
    }
    // A keyed row's code (first-seen order, which the scales share) and
    // centre, unless a later row's disc covers it.
    let code = |r: &Row| {
        swatches
            .iter()
            .position(|s| Some(&*s.label) == r.0)
            .unwrap() as f64
    };
    let centre = |r: &Row| (x.read().eval(code(r)), y.read().eval(r.1));
    for (i, r) in rows.iter().enumerate().filter(|(_, r)| r.0.is_some()) {
        let (px, py) = centre(r);
        let covered = rows[i + 1..].iter().filter(|o| o.0.is_some()).any(|o| {
            let (qx, qy) = centre(o);
            (qx - px).hypot(qy - py) < 4.5
        });
        if covered {
            continue;
        }
        let want = colour.read().eval(code(r)).to_rgba8();
        let got = image.get_pixel(px as u32, py as u32).0;
        if (0..3).any(|k| got[k].abs_diff(want[k]) > 1) {
            return fail(format!(
                "{r:?} drew {got:?} at ({px:.1}, {py:.1}), not {want:?}"
            ));
        }
    }

    // The null key is hidden by the glue's comparison: on an empty band
    // every code, the null code too, maps to y = 60. Every third row has
    // no key; rows are about 8 px apart, so a hidden row's spot is clear.
    let proof_rows: Vec<Row> = (0..30u32)
        .map(|i| ((i % 3 != 2).then_some("k"), f64::from(i)))
        .collect();
    let mut proof = Plot::new();
    let (px_scale, _) = (
        proof.x(Linear::new().domain(-1.0, 30.0)),
        proof.y(Linear::new().domain(0.0, 1.0)),
    );
    proof
        .add(Selection::<Row, Circle>::new(proof_rows.clone()))
        .attr(Circle::X, px_scale.encode(|r: &Row| r.1))
        .attr(
            Circle::Y,
            Band::new()
                .range(Px(60.0), Px(60.0))
                .encode_nullable_key(|r: &Row| r.0),
        )
        .attr(Circle::RADIUS, Px(2.0))
        .attr(Circle::FILL, Color::BLACK);
    let resolved = proof
        .resolve(&cx, width as f32, height as f32)
        .map_err(js)?;
    let hidden = ImageTarget::new(&cx, width, height)
        .map_err(js)?
        .render(&cx, &resolved.scene)
        .await
        .map_err(js)?;
    let (mut keyed, mut nulls) = (0, 0);
    for r in &proof_rows {
        let px = px_scale.read().eval(r.1);
        let ink = hidden.get_pixel(px as u32, 60).0[0] < 128;
        match (r.0, ink) {
            (Some(_), false) => {
                return fail(format!("keyed row {r:?} not drawn on the empty band"));
            }
            (None, true) => return fail(format!("null-key row {r:?} drawn on the empty band")),
            (Some(_), true) => keyed += 1,
            (None, false) => nulls += 1,
        }
    }
    web_log_text(&format!(
        "GUP band/categorical: {} colours, {keyed} keyed rows drawn, {nulls} null-key rows hidden",
        swatches.len()
    ));
    Ok(image.into_raw())
}

/// A diverging colour scale (RFC-001 S5b) on WebGPU: a signed quantity,
/// blue below zero and red above, with a ramp legend bar. Checks, in
/// Rust, every point's centre against the scale's CPU mirror, and that the
/// legend bar's ends are the colours of the domain's ends. Returns the
/// straight-alpha RGBA pixels.
#[wasm_bindgen]
pub async fn render_diverging(width: u32, height: u32) -> Result<Vec<u8>, JsValue> {
    std::panic::set_hook(Box::new(|info| web_error(&info.to_string())));
    let cx = Context::new().await.map_err(js)?;
    let rows: Vec<(f64, f64)> = (0..40u32)
        .map(|i| {
            let t = f64::from(i);
            (t, 0.06 * (t - 14.0) + 0.3 * (t * 0.7).sin())
        })
        .collect();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let colour = ScaleRef::new(Diverging::blue_red());
    plot.legends(false)
        .title("Diverging")
        .add(Selection::<(f64, f64), Circle>::new(rows.clone()))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Circle::RADIUS, Px(4.5))
        .attr(Circle::FILL, colour.encode(|r: &(f64, f64)| r.1));
    let resolved = plot
        .resolve(&cx, width as f32 - LEGEND_WIDTH, height as f32)
        .map_err(js)?;
    let (mut scene, plot_rect) = (resolved.scene.clone(), resolved.layout.plot);
    scene.width = width as f32;
    let clip = scene.add_clip(plot_rect);
    scene.push(Item {
        z: Z_GRID,
        clip: Some(clip),
        kind: ItemKind::Rects(vec![RectPrim {
            rect: plot_rect,
            color: Color::hex(0xe4e7ee),
        }]),
    });
    let right = width as f32 - LEGEND_WIDTH;
    let bar = Rect::from_edges(
        right + 6.0,
        plot_rect.top(),
        right + 18.0,
        plot_rect.bottom(),
    );
    scene.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Gradient(GradientBar::new(
            &colour.read().ramp(),
            bar,
            GradientDirection::Vertical,
        )),
    });
    let image = ImageTarget::new(&cx, width, height)
        .map_err(js)?
        .render(&cx, &scene)
        .await
        .map_err(js)?;
    let fail = |m: String| Err(JsValue::from_str(&format!("diverging: {m}")));
    // Probe where the render put each dot (the scales it used), but only
    // the dots whose centre no other dot's edge reaches: a neighbour's
    // antialiased rim would tint the pixel, whatever the layout.
    let at = |r: &(f64, f64)| (x.read().eval(r.0), y.read().eval(r.1));
    let mut probed = 0;
    for r in &rows {
        let (px, py) = at(r);
        let crowded = rows.iter().filter(|o| *o != r).any(|o| {
            let (ox, oy) = at(o);
            (ox - px).hypot(oy - py) < 4.5 + 1.5
        });
        if crowded {
            continue;
        }
        probed += 1;
        let want = colour.read().eval(r.1).to_rgba8();
        let got = image.get_pixel(px as u32, py as u32).0;
        if (0..3).any(|k| got[k].abs_diff(want[k]) > 2) {
            return fail(format!("{r:?} drew {got:?}, not the mirror's {want:?}"));
        }
    }
    if probed < 10 {
        return fail(format!(
            "only {probed} dots stood clear of their neighbours"
        ));
    }
    let [lo, mid, hi] = colour.read().current_domain().unwrap_or([0.0; 3]);
    if mid != 0.0 || lo != -hi {
        return fail(format!(
            "domain {lo} | {mid} | {hi} is not symmetric about 0"
        ));
    }
    let cx_ = (bar.x + bar.width / 2.0) as u32;
    for (py, value, end) in [
        (bar.top() as u32, hi, "top"),
        (bar.bottom() as u32 - 1, lo, "bottom"),
    ] {
        let (got, want) = (
            image.get_pixel(cx_, py).0,
            colour.read().eval(value).to_rgba8(),
        );
        if (0..3).any(|k| got[k].abs_diff(want[k]) > 3) {
            return fail(format!("legend {end} drew {got:?}, not {want:?}"));
        }
    }
    Ok(image.into_raw())
}

/// The energy an earthquake releases, in joules, from its magnitude
/// (`log10 E = 1.5 M + 4.8`): a user's function (RFC-001 S5c), composed
/// and checked when this crate compiles.
#[derive(Clone)]
struct Energy;

#[wgsl_function(
    monotonic,
    wgsl = r#"
    struct Params { slope: f32, offset: f32 }

    fn energy(magnitude: f32, p: Params) -> f32 {
        return pow(10.0, p.slope * magnitude + p.offset);
    }
"#
)]
impl ShaderFn for Energy {
    fn params(&self) -> EnergyParams {
        EnergyParams {
            slope: 1.5,
            offset: 4.8,
        }
    }
}

impl CpuMirror for Energy {
    fn eval(&self, magnitude: f64) -> f64 {
        10f64.powf(1.5 * magnitude + 4.8)
    }
}

/// A two-colour ramp over `[d0, d1]`: a user colour function whose
/// `Params` hold colours.
#[derive(Clone)]
struct Blend {
    from: Color,
    to: Color,
}

#[wgsl_function(wgsl = r#"
    struct Params { low: vec4<f32>, high: vec4<f32>, lo: f32, hi: f32 }

    fn blend(v: f32, p: Params) -> vec4<f32> {
        return mix(p.low, p.high, clamp((v - p.lo) / (p.hi - p.lo), 0.0, 1.0));
    }
"#)]
impl ShaderFn for Blend {
    fn params(&self) -> BlendParams {
        BlendParams {
            low: self.from,
            high: self.to,
            lo: 0.0,
            hi: 40.0,
        }
    }
}

impl CpuMirror for Blend {
    fn eval(&self, v: f64) -> Color {
        let t = (v / 40.0).clamp(0.0, 1.0) as f32;
        let mix = |a: f32, b: f32| a + (b - a) * t;
        Color {
            r: mix(self.from.r, self.to.r),
            g: mix(self.from.g, self.to.g),
            b: mix(self.from.b, self.to.b),
            a: mix(self.from.a, self.to.a),
        }
    }
}

/// RFC-001 S5c (GUP-420): user functions written with `#[wgsl_function]`
/// in this crate. Each quake's radius goes through `Energy` then the
/// built-in `Pow::sqrt()` (area grows with energy), and its fill through
/// `Blend`, whose `Params` hold two colours. Checks each centre's colour
/// and the largest disc's radius against the mirrors, and throws on a
/// mismatch.
#[wasm_bindgen]
pub async fn render_user_function(width: u32, height: u32) -> Result<Vec<u8>, JsValue> {
    std::panic::set_hook(Box::new(|info| web_error(&info.to_string())));
    let cx = Context::new().await.map_err(js)?;
    // (lon, lat, magnitude, depth km), largest first.
    let quakes: Vec<(f64, f64, f64, f64)> = (0..30u32)
        .map(|i| {
            let t = f64::from(i);
            let magnitude = if i == 0 { 7.4 } else { 6.0 - 0.08 * t };
            (
                0.5 * (t * 2.399).cos() * (t / 30.0).sqrt(),
                0.5 * (t * 2.399).sin() * (t / 30.0).sqrt(),
                magnitude,
                (t * 7.3) % 40.0,
            )
        })
        .collect();
    let radius = Energy.then(
        Pow::sqrt()
            .domain(0.0, Energy.eval(7.4))
            .range(Px(2.0), Px(24.0)),
    );
    let fill = Blend {
        from: Color::hex(0xf0c419),
        to: Color::hex(0x3b1f6e),
    };
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    type Quake = (f64, f64, f64, f64);
    plot.title("User functions")
        .add(Selection::<Quake, Circle>::new(quakes.clone()))
        .attr(Circle::X, x.encode(|q: &Quake| q.0))
        .attr(Circle::Y, y.encode(|q: &Quake| q.1))
        .attr(Circle::RADIUS, radius.encode(|q: &Quake| q.2))
        .attr(Circle::FILL, fill.encode(|q: &Quake| q.3));
    let resolved = plot.resolve(&cx, width as f32, height as f32).map_err(js)?;
    let image = ImageTarget::new(&cx, width, height)
        .map_err(js)?
        .render(&cx, &resolved.scene)
        .await
        .map_err(js)?;
    let fail = |m: String| Err(JsValue::from_str(&format!("user function: {m}")));
    // Later (smaller) discs draw over earlier ones: check the centres no
    // later disc covers.
    let centres: Vec<(f64, f64, f64)> = quakes
        .iter()
        .map(|q| (x.read().eval(q.0), y.read().eval(q.1), radius.eval(q.2)))
        .collect();
    let mut checked = 0;
    for (i, q) in quakes.iter().enumerate() {
        let (px, py, _) = centres[i];
        if centres[i + 1..]
            .iter()
            .any(|&(cx_, cy, r)| (cx_ - px).hypot(cy - py) < r + 1.5)
        {
            continue;
        }
        let want = fill.eval(q.3).to_rgba8();
        let got = image.get_pixel(px as u32, py as u32).0;
        if (0..3).any(|k| got[k].abs_diff(want[k]) > 2) {
            return fail(format!("{q:?} drew {got:?}, not the mirror's {want:?}"));
        }
        checked += 1;
    }
    if checked < 10 {
        return fail(format!("only {checked} centres were visible"));
    }
    // The largest disc's edge is at the chain's mirror radius, 24 px:
    // ink 2 px inside it, white 2 px outside, along each axis.
    let (mx, my, r) = centres[0];
    if (r - 24.0).abs() > 1e-9 {
        return fail(format!("the largest radius is {r}, not 24"));
    }
    let white = |dx: f64, dy: f64| {
        image.get_pixel((mx + dx) as u32, (my + dy) as u32).0[..3] == [255, 255, 255]
    };
    let outside = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)]
        .iter()
        .filter(|&&(dx, dy)| white(dx * (r + 2.0), dy * (r + 2.0)))
        .count();
    let inside = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)]
        .iter()
        .filter(|&&(dx, dy)| !white(dx * (r - 2.0), dy * (r - 2.0)))
        .count();
    if outside == 0 || inside < 4 {
        return fail(format!(
            "the largest disc's edge: {inside} of 4 inked inside, {outside} white outside"
        ));
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

/// Six named rows on a canvas, driven by DOM events through
/// [`gup_core::web::CanvasApp`] (RFC-001 S8f). The page dispatches synthetic
/// pointer and wheel events and reads the state back through these hooks.
#[wasm_bindgen]
pub struct InteractiveScatter {
    app: gup_core::web::CanvasApp,
    x: ScaleRef<Linear>,
    hovered: Arc<AtomicI32>,
    clicks: Arc<AtomicU32>,
    last_clicked: Arc<AtomicI32>,
    size: (f32, f32),
}

const NAMES: [&str; 6] = ["Tui", "Weka", "Kiwi", "Kea", "Kaka", "Pukeko"];

/// Show the six rows on `canvas`. Rejects if the canvas cannot take a
/// WebGPU surface, or the first frame fails.
#[wasm_bindgen]
pub async fn show_scatter(
    canvas: web_sys::HtmlCanvasElement,
) -> Result<InteractiveScatter, JsValue> {
    std::panic::set_hook(Box::new(|info| web_error(&info.to_string())));
    let size = (canvas.client_width() as f32, canvas.client_height() as f32);
    let hovered = Arc::new(AtomicI32::new(-1));
    let clicks = Arc::new(AtomicU32::new(0));
    let last_clicked = Arc::new(AtomicI32::new(-1));
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 100.0));
    let y = plot.y(Linear::new().domain(0.0, 10.0));
    let rows: Vec<(usize, f64, f64)> = (0..6)
        .map(|i| (i, 10.0 + 15.0 * i as f64, [3.0, 7.0, 4.0, 8.0, 2.0, 6.0][i]))
        .collect();
    let (enter, leave) = (Arc::clone(&hovered), Arc::clone(&hovered));
    let (count, last) = (Arc::clone(&clicks), Arc::clone(&last_clicked));
    plot.title("Interactive scatter")
        .add(Selection::<(usize, f64, f64), Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &(usize, f64, f64)| r.1))
        .attr(Circle::Y, y.encode(|r: &(usize, f64, f64)| r.2))
        .attr(Circle::FILL, Color::hex(0x2a6fdb))
        .attr(Circle::RADIUS, Px(8.0))
        .tooltip(|r| format!("{}\nx {}", NAMES[r.0], r.1))
        .on(Event::Enter, move |info, _| {
            enter.store(info.row.row as i32, Ordering::SeqCst);
        })
        .on(Event::Leave, move |_, _| leave.store(-1, Ordering::SeqCst))
        .on(Event::Click, move |info, _| {
            count.fetch_add(1, Ordering::SeqCst);
            last.store(info.row.row as i32, Ordering::SeqCst);
        });
    let app = gup_core::web::show(canvas, plot).await.map_err(js)?;
    Ok(InteractiveScatter {
        app,
        x,
        hovered,
        clicks,
        last_clicked,
        size,
    })
}

impl InteractiveScatter {
    fn resolved(&self) -> Result<Resolved, JsValue> {
        let cx = self.app.context();
        let (w, h) = self.size;
        self.app
            .with_chart(|chart| chart.resolve(&cx, w, h).cloned())
            .map_err(js)
    }
}

#[wasm_bindgen]
impl InteractiveScatter {
    /// Where row `row`'s dot is, relative to the canvas's top left (CSS
    /// pixels): `[x, y]`.
    pub fn row_px(&self, row: u32) -> Vec<f32> {
        self.app
            .with_chart(|chart| {
                let plot = chart.as_any_mut().downcast_mut::<Plot>()?;
                plot.pick_anchor(RowId {
                    layer: 0,
                    row: row as usize,
                })
            })
            .map(|p| vec![p.x, p.y])
            .unwrap_or_default()
    }

    /// The centre of the plot rectangle: `[x, y]`.
    pub fn plot_centre(&self) -> Result<Vec<f32>, JsValue> {
        let r = self.resolved()?.layout.plot;
        Ok(vec![r.x + r.width / 2.0, r.y + r.height / 2.0])
    }

    /// The hovered row, or -1.
    pub fn hovered_row(&self) -> i32 {
        self.hovered.load(Ordering::SeqCst)
    }

    /// The tooltip's lines joined with `\n`, if one is shown.
    pub fn tooltip_text(&self) -> Result<Option<String>, JsValue> {
        let resolved = self.resolved()?;
        let lines: Vec<&str> = resolved
            .scene
            .items
            .iter()
            .filter_map(|i| match &i.kind {
                ItemKind::Text(runs) => Some(runs),
                _ => None,
            })
            .flatten()
            .filter(|r| r.role == TextRole::Tooltip)
            .map(|r| &*r.text)
            .collect();
        Ok((!lines.is_empty()).then(|| lines.join("\n")))
    }

    /// The tooltip's box `[x, y, width, height]`, empty when none is shown.
    pub fn tooltip_rect(&self) -> Result<Vec<f32>, JsValue> {
        Ok(self
            .resolved()?
            .layout
            .tooltip
            .map(|r| vec![r.x, r.y, r.width, r.height])
            .unwrap_or_default())
    }

    /// The x axis's current domain `[lo, hi]`.
    pub fn x_domain(&self) -> Vec<f64> {
        self.x
            .read()
            .current_domain()
            .map(|(a, b)| vec![a, b])
            .unwrap_or_default()
    }

    /// How many clicks reached the handler.
    pub fn clicks(&self) -> u32 {
        self.clicks.load(Ordering::SeqCst)
    }

    /// The row of the last click, or -1.
    pub fn last_clicked(&self) -> i32 {
        self.last_clicked.load(Ordering::SeqCst)
    }

    /// The current scene through an awaited `ImageTarget`, as RGBA.
    pub async fn render_rgba(&self) -> Result<Vec<u8>, JsValue> {
        let cx = self.app.context();
        let resolved = self.resolved()?;
        let (w, h) = self.size;
        let image = ImageTarget::new(&cx, w as u32, h as u32)
            .map_err(js)?
            .render(&cx, &resolved.scene)
            .await
            .map_err(js)?;
        Ok(image.into_raw())
    }

    /// The last frame error, if any.
    pub fn last_error(&self) -> Option<String> {
        self.app.last_error()
    }
}

/// A row type no other scene uses (GUP-450).
#[cfg(feature = "extra-row-type")]
struct Extra {
    a: f64,
    b: f64,
    label: &'static str,
}

/// An interactive layer over [`Extra`] rows, resolved, picked at the plot
/// centre and rendered: it links every `Layer` method for one more row type
/// (`bind_axes`, `fit_domains`, `prepare`, `hit_items`, `tooltip_text` and
/// `dispatch`). The page never calls it.
#[cfg(feature = "extra-row-type")]
#[wasm_bindgen]
pub async fn render_extra_row_type(width: u32, height: u32) -> Result<Vec<u8>, JsValue> {
    std::panic::set_hook(Box::new(|info| web_error(&info.to_string())));
    let cx = Context::new().await.map_err(js)?;
    let rows: Vec<Extra> = (0..12)
        .map(|i| Extra {
            a: f64::from(i),
            b: f64::from(i * i % 7),
            label: "extra",
        })
        .collect();
    let mut plot = Plot::new();
    plot.circles(rows)
        .x(|r| r.a)
        .y(|r| r.b)
        .tooltip(|r| r.label.to_owned())
        .on(Event::Click, |_, _| {});
    let resolved = plot
        .resolve(&cx, width as f32, height as f32)
        .map_err(js)?
        .clone();
    let r = resolved.layout.plot;
    let picked = plot.pick_row(gup_core::geom::Point::new(
        r.x + r.width / 2.0,
        r.y + r.height / 2.0,
    ));
    web_log(picked.map_or(0, |p| p.row + 1));
    let image = ImageTarget::new(&cx, width, height)
        .map_err(js)?
        .render(&cx, &resolved.scene)
        .await
        .map_err(js)?;
    Ok(image.into_raw())
}
