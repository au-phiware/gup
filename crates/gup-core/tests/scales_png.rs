// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S5a (GUP-418): the numeric scale family end to end, rendered
//! headlessly and checked by the GUP-388 visual regression harness with
//! layout metadata from gup-core's own layout, plus pixel-level checks
//! that do not depend on a golden: a square-root radius grows as the
//! square root, a symmetric log axis is continuous through zero, a time
//! axis ticks on year boundaries, and a time axis zoomed to a millisecond
//! inside a million rows draws each point within a quarter pixel of its
//! CPU mirror.

mod common;

use common::scatter::{Country, countries_n};
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::prelude::*;
use gup_core::{Layout, ScaleRef};
use gup_visual_regression::RgbaImage;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 450;

fn check(case: &str, image: image::RgbaImage, layout: &Layout, colors: &[(&str, Color)]) {
    let mut meta = layout_metadata(layout);
    for (label, c) in colors {
        meta = meta.with_expected_color(*label, rgba8(*c));
    }
    let vr = RgbaImage::new(image.width(), image.height(), image.into_raw()).unwrap();
    harness().run(case, Ok((vr, meta))).assert_ok();
}

/// The coverage-weighted centre of the ink of a disc drawn in opaque
/// `ink` on white, inside the box of half-size `r` around `(x, y)`.
fn ink_centroid(image: &image::RgbaImage, (x, y): (f64, f64), r: f64, ink: Color) -> (f64, f64) {
    let [ir, ..] = ink.to_rgba8();
    let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
    for py in (y - r).floor().max(0.0) as u32..=((y + r).ceil() as u32).min(image.height() - 1) {
        for px in (x - r).floor().max(0.0) as u32..=((x + r).ceil() as u32).min(image.width() - 1) {
            // Coverage from the red channel: white is 255, ink is `ir`.
            let c = f64::from(image.get_pixel(px, py).0[0]);
            let w = ((255.0 - c) / (255.0 - f64::from(ir))).clamp(0.0, 1.0);
            sx += w * (f64::from(px) + 0.5);
            sy += w * (f64::from(py) + 0.5);
            sw += w;
        }
    }
    assert!(sw > 0.0, "no ink near ({x}, {y})");
    (sx / sw, sy / sw)
}

/// AC2: the RFC's north-star example — GDP per capita against life
/// expectancy, each country's area proportional to its population (a
/// square-root radius scale from 0), coloured by life expectancy.
#[test]
fn sqrt_radius_scatter() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<Country> = countries_n(60);
    let max_pop = rows.iter().map(|c| c.population).fold(0.0, f64::max);
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let area = Pow::sqrt().domain(0.0, max_pop).range(Px(0.0), Px(28.0));
    plot.title("Wealth and health, sized by population")
        .add(Selection::<Country, Circle>::new(rows.clone()))
        .attr(Circle::X, x.encode(|c: &Country| c.gdp_per_capita))
        .attr(Circle::Y, y.encode(|c: &Country| c.life_expectancy))
        .attr(Circle::RADIUS, area.encode(|c: &Country| c.population))
        .attr(
            Circle::FILL,
            Sequential::viridis().encode(|c: &Country| c.life_expectancy),
        );
    let (image, layout) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    // The ranges are inset by the largest radius, so every disc fits.
    for c in &rows {
        let r = area.eval(c.population) as f32;
        let (px, py) = (
            x.read().eval(c.gdp_per_capita) as f32,
            y.read().eval(c.life_expectancy) as f32,
        );
        let p = layout.plot;
        assert!(
            px - r >= p.left() && px + r <= p.right() && py - r >= p.top() && py + r <= p.bottom(),
            "disc at ({px}, {py}) r {r} leaves {p:?}"
        );
    }
    check("gup_core/sqrt_radius", image, &layout, &[]);
}

/// AC2: radii grow as the square root. Five discs on one row with
/// populations 0, 1/4, 1/2, 3/4 and 1 of the domain; each disc's ink area
/// is π r², so area grows linearly with the value (radius as its square
/// root), not quadratically as a linear radius scale would make it.
#[test]
fn sqrt_radius_grows_as_the_square_root() {
    let cx = Context::new_blocking().unwrap();
    let fractions = [0.0625, 0.25, 0.5, 0.75, 1.0];
    let rows: Vec<(f64, f64)> = fractions
        .iter()
        .enumerate()
        .map(|(i, &f)| (i as f64, f * 1e8))
        .collect();
    let mut plot = Plot::new();
    let (x, y) = (
        plot.x(Linear::new().domain(-0.6, 4.6)),
        plot.y(Linear::new().domain(-1.0, 1.0)),
    );
    let black = Color::BLACK;
    plot.add(Selection::<(f64, f64), Circle>::new(rows.clone()))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|_: &(f64, f64)| 0.0))
        .attr(
            Circle::RADIUS,
            Pow::sqrt()
                .domain(0.0, 1e8)
                .range(Px(0.0), Px(40.0))
                .encode(|r: &(f64, f64)| r.1),
        )
        .attr(Circle::FILL, black);
    let (image, _) = plot.render_resolved(&cx, 720, 200).unwrap();
    let cy = y.read().eval(0.0);
    let areas: Vec<f64> = rows
        .iter()
        .map(|r| {
            let cx_ = x.read().eval(r.0);
            let mut ink = 0.0;
            for py in (cy - 45.0) as u32..(cy + 45.0) as u32 {
                for px in (cx_ - 45.0) as u32..(cx_ + 45.0) as u32 {
                    ink += 1.0 - f64::from(image.get_pixel(px, py).0[0]) / 255.0;
                }
            }
            ink
        })
        .collect();
    eprintln!("ink areas: {areas:?}");
    for (f, a) in fractions.iter().zip(&areas) {
        let r = 40.0 * f.sqrt();
        let want = std::f64::consts::PI * r * r;
        assert!(
            (a - want).abs() / want < 0.03,
            "fraction {f}: area {a}, want {want}"
        );
    }
    // A linear radius would make the 1/4 disc's area 1/16 of the largest.
    assert!(areas[1] / areas[4] > 0.24, "{areas:?}");
}

/// AC3: a signed quantity spanning six decades each way on a symmetric
/// log y axis: a smooth S through zero, no gap and no jump at the
/// constant.
#[test]
fn symlog_signed_scatter() {
    let cx = Context::new_blocking().unwrap();
    // y = i³ / 8, from −216,000 to 216,000: spaced linearly near zero
    // (|y| below the constant 1), by decades beyond.
    let rows: Vec<(f64, f64)> = (-120..=120)
        .map(|i| {
            let i = f64::from(i);
            (i, i * i * i / 8.0)
        })
        .collect();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Symlog::new().constant(1.0)));
    let ink = Color::hex(0x0072b2);
    plot.title("A signed quantity on a symmetric log scale")
        .add(Selection::<(f64, f64), Circle>::new(rows.clone()))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Circle::RADIUS, Px(3.0))
        .attr(Circle::FILL, ink);
    let (image, layout) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    // Ticks either side of zero, and zero itself.
    let labels: Vec<&str> = layout.texts.iter().map(|t| &*t.run.text).collect();
    for want in ["0", "10", "−10", "1k", "−1k", "100k", "−100k"] {
        assert!(labels.contains(&want), "no {want:?} tick in {labels:?}");
    }
    // No jump at zero or at the constant (±1), and every point is drawn
    // where the mirror puts it.
    for at in [-1.0, 0.0, 1.0] {
        let (a, b) = (y.read().eval(at - 1e-9), y.read().eval(at + 1e-9));
        assert!((a - b).abs() < 1e-6, "jump of {} px at {at}", (a - b).abs());
    }
    for r in &rows {
        let (px, py) = (x.read().eval(r.0), y.read().eval(r.1));
        let got = image.get_pixel(px as u32, py as u32).0;
        let want = ink.to_rgba8();
        assert!(
            got[..3]
                .iter()
                .zip(&want[..3])
                .all(|(g, w)| g.abs_diff(*w) <= 2),
            "point {r:?} not drawn at ({px}, {py}): {got:?}"
        );
    }
    check("gup_core/symlog_signed", image, &layout, &[("points", ink)]);
}

/// Seconds since the epoch at midnight UTC on 1 January of `year`.
fn new_year(year: i64) -> f64 {
    // Days from 1970 to `year`, counting leap years.
    let days: i64 = (1970..year)
        .map(|y| {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                366
            } else {
                365
            }
        })
        .sum();
    days as f64 * 86_400.0
}

/// AC4: a daily series across four and a half years on a `Time` x axis:
/// calendar ticks on year boundaries, labelled with the year.
#[test]
fn time_axis_ticks_on_years() {
    let cx = Context::new_blocking().unwrap();
    let t0 = new_year(2019) + 120.0 * 86_400.0;
    let rows: Vec<(f64, f64)> = (0..1_650)
        .map(|d| {
            let d = f64::from(d);
            let t = t0 + d * 86_400.0;
            (
                t,
                50.0 + 20.0 * (d / 58.0).sin() + d / 60.0 + 6.0 * (d / 7.0).sin(),
            )
        })
        .collect();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Time::new()), plot.y(Linear::new()));
    let ink = Color::hex(0xd55e00);
    plot.title("A daily series, 2019 to 2023")
        .add(Selection::<(f64, f64), Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Circle::RADIUS, Px(1.6))
        .attr(Circle::FILL, ink);
    let (image, layout) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    let ticks: Vec<f64> = layout.x_ticks.iter().map(|t| t.0).collect();
    let labels: Vec<&str> = layout.texts.iter().map(|t| &*t.run.text).collect();
    eprintln!("x ticks {ticks:?}, labels {labels:?}");
    for year in 2020..=2023 {
        assert!(
            ticks.contains(&new_year(year)),
            "no tick on 1 January {year}: {ticks:?}"
        );
        assert!(
            labels.contains(&year.to_string().as_str()),
            "no {year} label: {labels:?}"
        );
    }
    check("gup_core/time_years", image, &layout, &[("points", ink)]);
}

/// AC4: a million one-per-second rows (twelve days) on a `Time` x axis
/// zoomed to one millisecond across the plot, near the end of the chunk
/// (a million seconds from its origin). Each of the window's points is
/// drawn with its disc's centre within 0.25 px of the CPU mirror, measured
/// from the rendered pixels. The same plot through `Linear`'s
/// `F32Relative` column misses by hundreds of pixels.
#[test]
fn time_zoomed_to_a_millisecond_draws_within_a_quarter_pixel() {
    let cx = Context::new_blocking().unwrap();
    let end = 1.7e9;
    let n = 1u32 << 20;
    let window = (end - 0.4e-3, end + 0.6e-3);
    let burst = 8u32;
    // One-per-second rows, then `burst` points across the window.
    let mut rows: Vec<(f64, f64)> = (0..n - burst)
        .map(|i| (end - f64::from(n - i) + 0.37, 0.0))
        .collect();
    let burst_rows: Vec<(f64, f64)> = (0..burst)
        .map(|k| {
            let t = window.0 + (window.1 - window.0) * (f64::from(k) + 0.5) / f64::from(burst);
            (t, if k % 2 == 0 { 0.25 } else { 0.75 })
        })
        .collect();
    rows.extend(&burst_rows);

    fn render<S: PositionScale<In = f32>>(
        cx: &Context,
        rows: &[(f64, f64)],
        xs: S,
    ) -> (image::RgbaImage, ScaleRef<S>, ScaleRef<Linear>) {
        let mut plot = Plot::new();
        let (x, y) = (plot.x(xs), plot.y(Linear::new().domain(0.0, 1.0)));
        plot.add(Selection::<(f64, f64), Circle>::new(rows.to_vec()))
            .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
            .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
            .attr(Circle::RADIUS, Px(5.0))
            .attr(Circle::FILL, Color::BLACK);
        let (image, _) = plot.render_resolved(cx, WIDTH, 300).unwrap();
        (image, x, y)
    }

    let worst = |image: &image::RgbaImage, x: &dyn Fn(f64) -> f64, y: &ScaleRef<Linear>| {
        burst_rows
            .iter()
            .map(|&(t, v)| {
                let want = (x(t), y.read().eval(v));
                let got = ink_centroid(image, want, 9.0, Color::BLACK);
                ((got.0 - want.0).abs()).max((got.1 - want.1).abs())
            })
            .fold(0.0, f64::max)
    };

    let (image, x, y) = render(&cx, &rows, Time::new().domain(window.0, window.1));
    let err = worst(&image, &|t| x.read().eval(t), &y);
    eprintln!("Time (hi/lo) at 1 ms: max |rendered − mirror| = {err:.3} px");
    assert!(err <= 0.25, "{err} px");

    // Negative control: Linear's F32Relative column. Its points land far
    // from the mirror's positions, so the centroids find little or no ink.
    let (image, x, y) = render(&cx, &rows, Linear::new().domain(window.0, window.1));
    let drawn = burst_rows
        .iter()
        .filter(|&&(t, v)| {
            let (px, py) = (x.read().eval(t), y.read().eval(v));
            image.get_pixel(px as u32, py as u32).0[0] < 128
        })
        .count();
    eprintln!("Linear (F32Relative) at 1 ms: {drawn} of {burst} points at their mirror position");
    assert!(
        drawn < burst as usize / 2,
        "{drawn} of {burst} drawn in place"
    );
}
