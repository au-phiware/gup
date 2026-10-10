// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S5b (GUP-419): the colour scales and the ordinal position
//! scales end to end, rendered headlessly and checked by the GUP-388
//! visual regression harness: a categorical scatter with more than 8
//! keys and a swatch legend whose colours are the drawn colours, a signed
//! quantity on a diverging scale with a ramp legend whose ends are the
//! data's colours, and a band axis labelled by its keys (an enum's
//! `Display`) with a null key hidden.

mod common;

use common::legend;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::geom::{self, Rect};
use gup_core::prelude::*;
use gup_core::scene::{Item, ItemKind, RectPrim, Z_GRID};
use gup_core::{ImageTarget, Layout, OKABE_ITO, Scene};
use gup_visual_regression::{LayoutMetadata, RgbaImage};
use std::fmt;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 450;
/// A neutral plot background for diverging colours, whose midpoint is
/// near-white.
const PLOT_TINT: Color = Color::hex(0xe4e7ee);

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

/// Render `scene`, then run the harness case with `meta`.
fn check(cx: &Context, case: &str, scene: &Scene, meta: LayoutMetadata) -> image::RgbaImage {
    let image = ImageTarget::new(cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(cx, scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    harness().run(case, Ok((vr, meta))).assert_ok();
    image
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

#[derive(Clone)]
struct Bird {
    species: &'static str,
    mass_g: f64,
    span_cm: f64,
}

/// Eleven species, each a cluster of 16 birds around a typical body mass
/// and wingspan.
fn birds() -> Vec<Bird> {
    const SPECIES: [(&str, f64, f64); 11] = [
        ("Wren", 10.0, 15.0),
        ("Sparrow", 30.0, 23.0),
        ("Starling", 80.0, 39.0),
        ("Kestrel", 200.0, 75.0),
        ("Pigeon", 320.0, 66.0),
        ("Crow", 520.0, 96.0),
        ("Buzzard", 900.0, 122.0),
        ("Mallard", 1_150.0, 88.0),
        ("Gull", 800.0, 145.0),
        ("Heron", 1_600.0, 178.0),
        ("Swan", 10_000.0, 215.0),
    ];
    (0..16u32)
        .flat_map(|i| {
            SPECIES.iter().map(move |&(species, mass, span)| Bird {
                species,
                mass_g: mass * (0.35 * (halton(i + 1, 2) - 0.5)).exp(),
                span_cm: span * (1.0 + 0.12 * (halton(i + 1, 3) - 0.5)),
            })
        })
        .collect()
}

/// AC2: a scatter of eleven species (more than Okabe-Ito's 8 colours)
/// coloured by a dictionary-encoded key, with a swatch legend in
/// first-seen order. Every species has its own colour, and each swatch is
/// exactly the colour its species is drawn in.
#[test]
fn categorical_scatter_with_eleven_keys_and_a_legend() {
    let cx = Context::new_blocking().unwrap();
    let rows = birds();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Log::new()), plot.y(Linear::new()));
    let colour = ScaleRef::new(Categorical::okabe_ito());
    plot.title("Body mass (g) and wingspan (cm) of eleven birds")
        .add(Selection::<Bird, Circle>::new(rows.clone()))
        .attr(Circle::X, x.encode(|b: &Bird| b.mass_g))
        .attr(Circle::Y, y.encode(|b: &Bird| b.span_cm))
        .attr(Circle::RADIUS, Px(4.5))
        .attr(Circle::FILL, colour.encode_key(|b: &Bird| b.species));
    let (mut scene, layout) = resolve(&cx, &mut plot, 110.0);
    let swatches = colour.read().legend().swatches().to_vec();
    assert_eq!(swatches.len(), 11);
    assert_eq!(&*swatches[10].label, "Swan");
    for (s, c) in swatches.iter().zip(OKABE_ITO) {
        assert_eq!(s.color, c, "{}: the first 8 are Okabe-Ito", s.label);
    }
    let placed = legend::swatches(
        &mut scene,
        &swatches,
        geom::Point::new(layout.plot.right() + 24.0, layout.plot.top()),
    );
    let mut meta = legend::with_legend(layout_metadata(&layout), &placed);
    for s in &swatches {
        meta = meta.with_expected_color(format!("{} swatch", s.label), rgba8(s.color));
    }
    let image = check(&cx, "gup_core/categorical_species", &scene, meta);

    // Each species' last bird (drawn after the rest of its cluster) has
    // its centre drawn in its swatch's colour, and the swatch itself, as
    // drawn, is that colour too.
    let r = 4.5;
    let centres: Vec<(f64, f64)> = rows
        .iter()
        .map(|b| (x.read().eval(b.mass_g), y.read().eval(b.span_cm)))
        .collect();
    for (s, rect) in swatches.iter().zip(&placed.guides) {
        let last = rows.iter().rposition(|b| b.species == &*s.label).unwrap();
        let (px, py) = centres[last];
        let covered = centres[last + 1..]
            .iter()
            .any(|&(qx, qy)| (qx - px).hypot(qy - py) < r + 1.0);
        let drawn = image.get_pixel(px as u32, py as u32).0;
        let swatch = image
            .get_pixel(
                (rect.x + rect.width / 2.0) as u32,
                (rect.y + rect.height / 2.0) as u32,
            )
            .0;
        assert_eq!(swatch, s.color.to_rgba8(), "{} swatch", s.label);
        if !covered {
            assert_eq!(drawn, swatch, "{}: data and legend differ", s.label);
        }
    }
}

/// Global temperature anomaly (synthetic): a warming trend with
/// year-to-year wobble, in °C against a 1951–1980 baseline.
fn anomalies() -> Vec<(f64, f64)> {
    (1950..=2024)
        .map(|year| {
            let t = f64::from(year - 1950);
            let trend = -0.05 + 0.0002 * t * t;
            let wobble = 0.12 * (t * 0.9).sin() + 0.08 * (t * 2.3).cos();
            (f64::from(year), trend + wobble)
        })
        .collect()
}

/// AC3: a signed quantity on a diverging scale: cold years blue, warm
/// years red, the midpoint (0 °C) near-white, with a ramp legend whose
/// ends are the colours the data's extremes are drawn in.
#[test]
fn diverging_temperature_anomaly_with_a_ramp_legend() {
    let cx = Context::new_blocking().unwrap();
    let rows = anomalies();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let colour = ScaleRef::new(Diverging::blue_red());
    plot.title("Temperature anomaly (°C), 1950–2024")
        .add(Selection::<(f64, f64), Circle>::new(rows.clone()))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Circle::RADIUS, Px(5.5))
        .attr(Circle::FILL, colour.encode(|r: &(f64, f64)| r.1));
    let (mut scene, layout) = resolve(&cx, &mut plot, 70.0);
    let [lo, mid, hi] = colour.read().current_domain().unwrap();
    assert_eq!(mid, 0.0);
    assert_eq!(lo, -hi, "the fitted domain is symmetric");
    let ramp = colour.read().ramp();
    let bar = Rect::from_edges(
        layout.plot.right() + 20.0,
        layout.plot.top(),
        layout.plot.right() + 34.0,
        layout.plot.bottom(),
    );
    let placed = legend::ramp(&mut scene, &ramp, bar);
    // Near-midpoint colours are near-white: a tinted plot background keeps
    // them visible (circles have no stroke until RFC-001 S6).
    let clip = scene.add_clip(layout.plot);
    scene.push(Item {
        z: Z_GRID,
        clip: Some(clip),
        kind: ItemKind::Rects(vec![RectPrim {
            rect: layout.plot,
            color: PLOT_TINT,
        }]),
    });
    let (coldest, warmest) = rows.iter().fold((rows[0], rows[0]), |(c, w), &r| {
        (if r.1 < c.1 { r } else { c }, if r.1 > w.1 { r } else { w })
    });
    let meta = legend::with_legend(layout_metadata(&layout), &placed)
        .with_expected_color("plot background", rgba8(PLOT_TINT))
        .with_expected_color("coldest year", rgba8(colour.read().eval(coldest.1)))
        .with_expected_color("warmest year", rgba8(colour.read().eval(warmest.1)));
    let image = check(&cx, "gup_core/diverging_anomaly", &scene, meta);

    // The bar's ends are the domain's ends, which the warmest year (or,
    // on the cold side, the coldest) reaches; and the data's extremes
    // are drawn in the colour the bar shows at their value.
    let bar_at = |t: f64| {
        let py = bar.bottom() - t as f32 * bar.height;
        image
            .get_pixel(
                (bar.x + bar.width / 2.0) as u32,
                py.clamp(bar.top(), bar.bottom() - 1.0) as u32,
            )
            .0
    };
    let drawn = |r: (f64, f64)| {
        image
            .get_pixel(x.read().eval(r.0) as u32, y.read().eval(r.1) as u32)
            .0
    };
    for r in [coldest, warmest] {
        let (want, bar_colour) = (drawn(r), bar_at(ramp.t(r.1)));
        for k in 0..3 {
            assert!(
                want[k].abs_diff(bar_colour[k]) <= 3,
                "{r:?}: drawn {want:?}, legend {bar_colour:?}"
            );
        }
    }
    let (top, bottom) = (bar_at(1.0), bar_at(0.0));
    // The edge rows sample half a pixel inside the ends.
    for (got, want, end) in [
        (top, colour.read().eval(hi), "top"),
        (bottom, colour.read().eval(lo), "bottom"),
    ] {
        let want = want.to_rgba8();
        assert!(
            (0..3).all(|k| got[k].abs_diff(want[k]) <= 2),
            "bar {end}: {got:?} vs {want:?}"
        );
    }
    assert!(
        top[0] > top[2] && bottom[2] > bottom[0],
        "red top, blue bottom"
    );
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum Month {
    Jan,
    Feb,
    Mar,
    Apr,
    May,
    Jun,
    Jul,
    Aug,
    Sep,
    Oct,
    Nov,
    Dec,
}

impl fmt::Display for Month {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

const MONTHS: [Month; 12] = [
    Month::Jan,
    Month::Feb,
    Month::Mar,
    Month::Apr,
    Month::May,
    Month::Jun,
    Month::Jul,
    Month::Aug,
    Month::Sep,
    Month::Oct,
    Month::Nov,
    Month::Dec,
];

/// A day's high temperature in a temperate southern-hemisphere city, or a
/// day whose month was not recorded (`None`).
#[derive(Clone)]
struct Day {
    month: Option<Month>,
    high_c: f64,
}

fn days() -> Vec<Day> {
    (0..12 * 14u32)
        .map(|i| {
            let m = (i % 12) as usize;
            let season = 7.0 * (2.0 * std::f64::consts::PI * (m as f64 + 0.5) / 12.0).cos();
            Day {
                // Every 41st day lost its month.
                month: (i % 41 != 40).then_some(MONTHS[m]),
                high_c: 20.0 + season + 6.0 * (halton(i / 12 + 1, 2) - 0.5),
            }
        })
        .collect()
}

/// AC4 and AC5: a band x axis over an enum's keys (`encode_owned_key`),
/// labelled by `Display` in first-seen order, with each band shaded and
/// every day's high a dot at its band's centre, coloured by its anomaly
/// from the yearly mean on the diverging scale. Days with no month are
/// not drawn.
#[test]
fn band_axis_labelled_by_enum_keys() {
    let cx = Context::new_blocking().unwrap();
    let rows = days();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new().padding(0.15)), plot.y(Linear::new()));
    plot.title("Daily highs by month (°C)")
        .add(Selection::<Day, Circle>::new(rows.clone()))
        .attr(
            Circle::X,
            x.encode_nullable_owned_key(|d: &Day| d.month)
                .domain(MONTHS),
        )
        .attr(Circle::Y, y.encode(|d: &Day| d.high_c))
        .attr(Circle::RADIUS, Px(4.0))
        .attr(
            Circle::FILL,
            Diverging::blue_red()
                .midpoint(20.0)
                .encode(|d: &Day| d.high_c),
        );
    let resolved = plot.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
    let (mut scene, layout) = (resolved.scene, resolved.layout);

    // The axis is the keys, in first-seen order, at the bands' centres.
    let labels: Vec<&str> = layout
        .texts
        .iter()
        .filter(|t| t.run.role == gup_core::scene::TextRole::TickLabel)
        .map(|t| &*t.run.text)
        .take(12)
        .collect();
    let names: Vec<String> = MONTHS.iter().map(|m| m.to_string()).collect();
    assert_eq!(labels, names);
    let band = x.read().clone();
    for (k, (code, px)) in layout.x_ticks.iter().enumerate() {
        assert_eq!(*code, k as f64);
        assert!((f64::from(*px) - band.eval(*code)).abs() < 1e-3);
    }

    // Shade each band, under the data.
    let clip = scene.add_clip(layout.plot);
    let shades = (0..12)
        .map(|k| {
            let c = band.eval(f64::from(k)) as f32;
            let half = band.band_width() as f32 / 2.0;
            RectPrim {
                rect: Rect::from_edges(c - half, layout.plot.top(), c + half, layout.plot.bottom()),
                color: PLOT_TINT,
            }
        })
        .collect();
    scene.push(Item {
        z: Z_GRID,
        clip: Some(clip),
        kind: ItemKind::Rects(shades),
    });
    let meta = layout_metadata(&layout).with_expected_color("band shade", rgba8(PLOT_TINT));
    let image = check(&cx, "gup_core/band_months", &scene, meta);

    // Every day with a month is drawn at its band's centre; the days
    // without one are not drawn anywhere (their y is still on-screen).
    let shade = PLOT_TINT.to_rgba8();
    let drawn = |d: &Day, m: Month| {
        let px = band.eval(f64::from(
            MONTHS.iter().position(|&n| n == m).unwrap() as u32
        ));
        image.get_pixel(px as u32, y.read().eval(d.high_c) as u32).0
    };
    for d in &rows {
        match d.month {
            Some(m) => assert_ne!(drawn(d, m), shade, "a day in {m} is missing"),
            None => {
                // Not at any band's centre at its height (unless another
                // day's dot happens to be there).
                let row = y.read().eval(d.high_c) as u32;
                let near = |m: Month| {
                    rows.iter().any(|o| {
                        o.month == Some(m) && (y.read().eval(o.high_c) - f64::from(row)).abs() < 6.0
                    })
                };
                for m in MONTHS {
                    if !near(m) {
                        assert_eq!(drawn(d, m), shade, "a day without a month drawn in {m}");
                    }
                }
            }
        }
    }
    assert!(rows.iter().filter(|d| d.month.is_none()).count() >= 4);
}
