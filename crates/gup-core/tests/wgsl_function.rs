// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S5c (GUP-420): `#[wgsl_function]` from outside gup-core, the
//! way a user writes one. A custom transfer function (seismic energy from
//! magnitude) starts a chain into a built-in square-root scale to size an
//! aftershock sequence's discs; a custom colour function over dictionary
//! codes highlights one key, fitted by `fit_keys`; and two functions that
//! claim one import path are refused, naming both. Rendered headlessly and
//! checked by the GUP-388 visual regression harness.

mod common;

use common::continents::{self, CONTINENTS, Place};
use common::legend;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::geom::Rect;
use gup_core::prelude::*;
use gup_core::{Error, ImageTarget, Layout, Scene};
use gup_visual_regression::RgbaImage;
use std::sync::Arc;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 450;

/// The energy an earthquake releases, in joules, from its magnitude:
/// `log10 E = 1.5 M + 4.8`.
#[derive(Clone, Debug)]
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

/// Draws one key of a dictionary-encoded column in an accent colour and
/// every other key, and missing keys, in a neutral grey.
#[derive(Clone, Debug)]
struct Highlight {
    key: &'static str,
    /// The key's code, found by `fit_keys`.
    code: u32,
}

impl Highlight {
    fn new(key: &'static str) -> Self {
        Self { key, code: 0 }
    }
}

#[wgsl_function(wgsl = r#"
    struct Params { accent: vec4<f32>, rest: vec4<f32>, code: u32 }

    fn highlight(code: u32, p: Params) -> vec4<f32> {
        return select(p.rest, p.accent, code == p.code);
    }
"#)]
impl ShaderFn for Highlight {
    fn params(&self) -> HighlightParams {
        HighlightParams {
            accent: ACCENT,
            rest: REST,
            code: self.code,
        }
    }

    fn fit_keys(&mut self, keys: &[Arc<str>]) -> gup_core::Result<()> {
        // Without the key, a code no key has yet: nothing is highlighted.
        let code = keys.iter().position(|k| **k == *self.key);
        self.code = code.unwrap_or(keys.len()) as u32;
        Ok(())
    }
}

const ACCENT: Color = Color::hex(0xd55e00);
const REST: Color = Color::hex(0xc4c7cc);

fn check(case: &str, image: image::RgbaImage, layout: &Layout, colors: &[(&str, Color)]) {
    let mut meta = layout_metadata(layout);
    for (label, c) in colors {
        meta = meta.with_expected_color(*label, rgba8(*c));
    }
    let vr = RgbaImage::new(image.width(), image.height(), image.into_raw()).unwrap();
    harness().run(case, Ok((vr, meta))).assert_ok();
}

/// The `i`-th element of the base-`b` Halton sequence, in `[0, 1)`.
fn halton(mut i: u32, b: u32) -> f64 {
    let (mut f, mut r) = (1.0, 0.0);
    while i > 0 {
        f /= f64::from(b);
        r += f * f64::from(i % b);
        i /= b;
    }
    r
}

/// One earthquake.
#[derive(Clone, Debug)]
struct Quake {
    lon: f64,
    lat: f64,
    magnitude: f64,
    /// Hypocentre depth (km).
    depth: f64,
}

/// A synthetic aftershock sequence: a M7.8 main shock, three large
/// aftershocks, and 400 more along a 300 km fault striking N20°E that
/// dips to the west (deeper away from the trench), their magnitudes
/// following Gutenberg–Richter (b = 1, from M3.5). Largest first, so
/// small discs draw over large ones.
fn aftershocks() -> Vec<Quake> {
    let (lon0, lat0): (f64, f64) = (142.4, 38.3);
    let strike = 20f64.to_radians();
    // `along` and `across` the fault in degrees; `across` is positive to
    // the east-south-east, towards the trench, where the fault is shallow.
    let quake = |along: f64, across: f64, jitter: f64, magnitude: f64| {
        let (dn, de) = (
            along * strike.cos() - across * strike.sin(),
            along * strike.sin() + across * strike.cos(),
        );
        Quake {
            lon: lon0 + de / lat0.to_radians().cos(),
            lat: lat0 + dn,
            magnitude,
            depth: (24.0 - 60.0 * across + jitter).clamp(3.0, 45.0),
        }
    };
    let mut quakes = vec![
        quake(0.0, 0.0, 0.0, 7.8),
        quake(0.9, 0.05, 2.0, 7.1),
        quake(-0.8, -0.1, -3.0, 6.6),
        quake(0.3, 0.12, 1.0, 6.2),
    ];
    for i in 1..=400 {
        // Along the fault (±1.4°), denser near the main shock; across it,
        // a band about ±0.24° wide.
        let u = 2.0 * halton(i, 2) - 1.0;
        let along = 1.4 * u * (0.35 + 0.65 * u.abs());
        let across = 0.18 * (2.0 * halton(i, 3) - 1.0) + 0.06 * (2.0 * halton(i, 11) - 1.0);
        let jitter = 6.0 * (2.0 * halton(i, 13) - 1.0);
        let magnitude = (3.5 - (1.0 - halton(i, 5)).log10()).min(6.0);
        quakes.push(quake(along, across, jitter, magnitude));
    }
    quakes.sort_by(|a, b| b.magnitude.total_cmp(&a.magnitude));
    quakes
}

/// The plot resolved `legend_width` narrower than the image, so a legend
/// fits to its right.
fn resolve(cx: &Context, plot: &mut Plot, legend_width: f32) -> (Scene, Layout) {
    let resolved = plot
        .resolve(cx, WIDTH as f32 - legend_width, HEIGHT as f32)
        .unwrap();
    let mut scene = resolved.scene;
    scene.width = WIDTH as f32;
    (scene, resolved.layout)
}

/// AC6: the motivating example. A user's transfer function (energy from
/// magnitude, `monotonic` so its image comes from the mirror) starts a
/// chain into the built-in `Pow::sqrt()`, so a disc's area grows with the
/// energy released; depth is coloured through built-in viridis, with its
/// ramp legend.
#[test]
fn aftershocks_sized_by_energy() {
    let cx = Context::new_blocking().unwrap();
    let quakes = aftershocks();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    // Area ∝ energy above a 1.5 px floor, so the smallest quakes show.
    let radius = Energy.then(
        Pow::sqrt()
            .domain(0.0, Energy.eval(7.8))
            .range(Px(1.5), Px(36.0)),
    );
    let depth = Sequential::viridis().domain(0.0, 40.0).reversed();
    plot.title("Aftershocks: area by energy released, colour by depth (km)")
        .add(Selection::<Quake, Circle>::new(quakes.clone()))
        .attr(Circle::X, x.encode(|q: &Quake| q.lon))
        .attr(Circle::Y, y.encode(|q: &Quake| q.lat))
        .attr(Circle::RADIUS, radius.encode(|q: &Quake| q.magnitude))
        .attr(Circle::FILL, depth.encode(|q: &Quake| q.depth));
    let (mut scene, layout) = resolve(&cx, &mut plot, 60.0);
    let bar = Rect::from_edges(
        layout.plot.right() + 16.0,
        layout.plot.top(),
        layout.plot.right() + 30.0,
        layout.plot.bottom(),
    );
    let placed = legend::ramp(&mut scene, &depth.ramp(), bar);
    let main = &quakes[0];
    let meta = legend::with_legend(layout_metadata(&layout), &placed)
        .with_expected_color("main shock", rgba8(depth.eval(main.depth)));
    let image = ImageTarget::new(&cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(&cx, &scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    harness()
        .run("gup_core/wgsl_function_aftershocks", Ok((vr, meta)))
        .assert_ok();

    // The plot was inset by the chain's image, the main shock's 36 px
    // radius, so every disc lies inside the plot rect.
    let p = layout.plot;
    for q in &quakes {
        let r = radius.eval(q.magnitude) as f32;
        let (px, py) = (x.read().eval(q.lon) as f32, y.read().eval(q.lat) as f32);
        assert!(
            px - r >= p.x - 0.5
                && px + r <= p.x + p.width + 0.5
                && py - r >= p.y - 0.5
                && py + r <= p.y + p.height + 0.5,
            "M{} at ({px}, {py}) r {r} leaves {p:?}",
            q.magnitude
        );
    }
    // The main shock's disc is drawn at the mirror's radius: in its
    // colour 2 px inside its edge, and white 2 px outside.
    let (mx, my) = (x.read().eval(main.lon), y.read().eval(main.lat));
    let r = radius.eval(main.magnitude);
    assert_eq!(r, 36.0);
    // Smaller discs draw over it, so one of four directions suffices.
    let want = depth.eval(main.depth).to_rgba8();
    let at = |(dx, dy): (f64, f64), d: f64| {
        image
            .get_pixel((mx + dx * d) as u32, (my + dy * d) as u32)
            .0
    };
    let directions = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)];
    assert!(
        directions.iter().any(|&dir| {
            let got = at(dir, r - 2.0);
            (0..3).all(|k| got[k].abs_diff(want[k]) <= 2)
        }),
        "the main shock's colour 2 px inside its edge"
    );
    assert!(
        directions
            .iter()
            .any(|&dir| at(dir, r + 2.0)[..3] == [255, 255, 255]),
        "white 2 px outside its edge"
    );
}

/// S5b's adjustment for S5c: a user function over dictionary codes takes
/// its key domain through `fit_keys`. One continent is highlighted;
/// every other, and every missing continent, is grey.
#[test]
fn one_key_highlighted_by_a_user_function() {
    let cx = Context::new_blocking().unwrap();
    let rows = continents::places();
    let mut plot = Plot::new();
    let (x, y) = (
        plot.x(Linear::new().domain(0.0, 60_000.0)),
        plot.y(Log::new().domain(1e5, 1e9)),
    );
    plot.title("Asia among the world's countries")
        .add(Selection::<Place, Circle>::new(rows.clone()))
        .attr(Circle::X, x.encode(|p: &Place| p.gdp_per_capita))
        .attr(Circle::Y, y.encode(|p: &Place| p.population))
        .attr(Circle::RADIUS, Px(continents::RADIUS))
        .attr(
            Circle::FILL,
            Highlight::new("Asia").encode_nullable_key(|p: &Place| p.continent.as_deref()),
        );
    let (image, layout) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();

    // Each centre is the accent colour exactly when its continent is Asia
    // (and the data has every continent).
    for c in CONTINENTS {
        assert!(rows.iter().any(|p| p.continent.as_deref() == Some(c)));
    }
    let centres: Vec<(f64, f64)> = rows
        .iter()
        .map(|p| (x.read().eval(p.gdp_per_capita), y.read().eval(p.population)))
        .collect();
    let (mut asia, mut checked) = (0, 0);
    for (i, (p, &(px, py))) in rows.iter().zip(&centres).enumerate() {
        let want = if p.continent.as_deref() == Some("Asia") {
            asia += 1;
            ACCENT
        } else {
            REST
        };
        // Discs draw in data order: skip a centre a later disc covers.
        let reach = f64::from(continents::RADIUS) + 1.5;
        if centres[i + 1..]
            .iter()
            .any(|&(qx, qy)| (qx - px).hypot(qy - py) < reach)
        {
            continue;
        }
        let got = image.get_pixel(px as u32, py as u32).0;
        let w = want.to_rgba8();
        assert!(
            (0..3).all(|k| got[k].abs_diff(w[k]) <= 2),
            "{:?} drawn {got:?}",
            p.continent
        );
        checked += 1;
    }
    assert!(asia >= 20, "{asia} Asian countries");
    assert!(checked >= 80, "{checked} centres checked");

    check(
        "gup_core/wgsl_function_highlight",
        image,
        &layout,
        &[("highlighted key", ACCENT), ("other keys", REST)],
    );
}

/// Halves its input.
#[derive(Clone, Debug)]
struct Half;

#[wgsl_function(
    path = "clash::scale",
    wgsl = r#"
    struct Params { k: f32 }
    fn map(v: f32, p: Params) -> f32 { return v * p.k; }
"#
)]
impl ShaderFn for Half {
    fn params(&self) -> HalfParams {
        HalfParams { k: 0.5 }
    }
}

impl CpuMirror for Half {
    fn eval(&self, v: f64) -> f64 {
        v * 0.5
    }
}

/// Pixels: its input plus three.
#[derive(Clone, Debug)]
struct Pad;

#[wgsl_function(
    path = "clash::scale",
    wgsl = r#"
    struct Params { add: f32 }
    fn map(v: f32, p: Params) -> f32 { return v + p.add; }
"#
)]
impl ShaderFn for Pad {
    type Out = Px;

    fn params(&self) -> PadParams {
        PadParams { add: 3.0 }
    }
}

/// AC1: two functions overriding their path to the same import path
/// would share the glue's import alias, so one would call the other's
/// WGSL (both entries are `map`). The layer refuses them, naming both.
#[test]
fn two_functions_with_one_import_path_are_refused() {
    assert_eq!(<Half as ShaderFn>::MODULE.import_path, "clash::scale");
    assert_eq!(<Pad as ShaderFn>::MODULE.import_path, "clash::scale");
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(Selection::<(f64, f64), Circle>::new(vec![
        (1.0, 2.0),
        (3.0, 4.0),
    ]))
    .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
    .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
    .attr(Circle::RADIUS, Half.then(Pad).encode(|r: &(f64, f64)| r.1));
    let err = plot.render_resolved(&cx, 200, 100).unwrap_err();
    let Error::Compose { report, .. } = &err else {
        panic!("expected Error::Compose, got {err:?}");
    };
    assert!(report.contains("wgsl_function::Half"), "{report}");
    assert!(report.contains("wgsl_function::Pad"), "{report}");
    assert!(report.contains("`clash::scale`"), "{report}");
    eprintln!("{err}");
}

/// The macro's static half, from the WGSL.
#[test]
fn the_macro_derives_the_static_half() {
    assert_eq!(<Energy as ShaderFn>::ENTRY, "energy");
    let module = <Energy as ShaderFn>::MODULE;
    assert_eq!(module.import_path, "wgsl_function::energy");
    assert!(module.imports.is_empty());
    // Padded to 16 bytes in the flattened WGSL, not in the twin.
    assert!(module.wgsl.contains("gup_pad_b: f32"), "{}", module.wgsl);
    assert_eq!(
        <EnergyParams as gup_core::__private::encase::ShaderType>::min_size().get(),
        16
    );
    assert_eq!(
        ShaderFn::input_format(&Energy),
        gup_core::column::ColumnFormat::F32
    );
    // `monotonic`: the image is the mirror's, at the extent's ends.
    let image = ShaderFn::image(&Energy, (4.0, 6.0)).unwrap();
    assert_eq!(image, (Energy.eval(4.0), Energy.eval(6.0)));
    assert_eq!(
        ShaderFn::input_format(&Highlight::new("x")),
        gup_core::column::ColumnFormat::U32
    );
    assert_eq!(ShaderFn::image(&Highlight::new("x"), (0.0, 3.0)), None);
}

/// Softplus, whose WGSL is a file (`file = "…"`, relative to the
/// crate's manifest), importing one of Gup's library modules.
#[derive(Clone, Debug)]
struct Softplus;

#[wgsl_function(monotonic, file = "tests/wgsl/softplus.wgsl")]
impl ShaderFn for Softplus {
    fn params(&self) -> SoftplusParams {
        SoftplusParams { k: 1.0 }
    }
}

impl CpuMirror for Softplus {
    fn eval(&self, v: f64) -> f64 {
        v.exp().ln_1p()
    }
}

/// AC2: a function read from a file, importing a library module, composes
/// at expansion and links against the library at run time: a softplus
/// colour ramp draws each point in the colour its mirror gives.
#[test]
fn a_wgsl_file_importing_the_library() {
    let module = <Softplus as ShaderFn>::MODULE;
    assert_eq!(module.import_path, "wgsl_function::softplus");
    let imports: Vec<&str> = module.imports.iter().map(|m| m.import_path).collect();
    assert_eq!(imports, ["gup::scale::symlog"]);

    let cx = Context::new_blocking().unwrap();
    let rows: Vec<f64> = (0..9).map(|i| f64::from(i) - 4.0).collect();
    let viridis = Sequential::viridis().domain(0.0, 4.5);
    let colour = Softplus.then(viridis.clone());
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(Selection::<f64, Circle>::new(rows.clone()))
        .attr(Circle::X, x.encode(|v: &f64| *v))
        .attr(Circle::Y, y.encode(|v: &f64| *v))
        .attr(Circle::RADIUS, Px(6.0))
        .attr(Circle::FILL, colour.encode(|v: &f64| *v));
    let (image, _) = plot.render_resolved(&cx, 320, 200).unwrap();
    for v in rows {
        let (px, py) = (x.read().eval(v), y.read().eval(v));
        let got = image.get_pixel(px as u32, py as u32).0;
        let want = colour.eval(v).to_rgba8();
        assert!(
            (0..3).all(|k| got[k].abs_diff(want[k]) <= 2),
            "{v}: {got:?} vs {want:?}"
        );
    }
}
