// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S6d (GUP-430): fill opacity and an inside stroke on `Circle`
//! and `Rect`, rendered headlessly. Compositing is checked against a CPU
//! "over" on known pixels, and a bubble chart is read by the golden.

mod common;

use common::vr::{harness, layout_metadata};
use gup_core::ImageTarget;
use gup_core::prelude::*;
use gup_visual_regression::RgbaImage;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 450;

const A: Color = Color::hex(0x0072b2);
const B: Color = Color::hex(0xd55e00);
const STROKE: Color = Color::hex(0x000000);

fn render(cx: &Context, plot: &mut Plot, width: u32, height: u32) -> image::RgbaImage {
    let resolved = plot.resolve(cx, width as f32, height as f32).unwrap();
    ImageTarget::new(cx, width, height)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap()
}

/// `fill` at `opacity` over `under`, in sRGB, per channel in 0..=255.
fn over(fill: Color, opacity: f32, under: [f32; 3]) -> [f32; 3] {
    let f = [fill.r, fill.g, fill.b];
    std::array::from_fn(|k| (f[k] * opacity * 255.0) + under[k] * (1.0 - opacity))
}

fn assert_close(image: &image::RgbaImage, x: u32, y: u32, want: [f32; 3]) {
    let got = image.get_pixel(x, y).0;
    for k in 0..3 {
        assert!(
            (f32::from(got[k]) - want[k]).abs() <= 3.0,
            "({x}, {y}): {got:?}, wanted {want:?}"
        );
    }
}

/// Two translucent, stroked marks, 60 px apart; each is 120 px across
/// with its centre at `(130, 150)` and `(190, 150)`.
fn two_marks(cx: &Context, configure: impl Fn(&mut Plot, [f32; 2], Color)) -> image::RgbaImage {
    let mut plot = Plot::new();
    plot.x(Linear::new().domain(0.0, 1.0));
    plot.y(Linear::new().domain(0.0, 1.0));
    configure(&mut plot, [130.0, 150.0], A);
    configure(&mut plot, [190.0, 150.0], B);
    render(cx, &mut plot, 400, 300)
}

fn check_compositing(image: &image::RgbaImage) {
    // The midpoint is inside both discs, away from both rings: B over A
    // over white.
    let a = over(A, 0.5, [255.0; 3]);
    assert_close(image, 160, 150, over(B, 0.5, a));
    // The pixel 1.5 px inside A's leftmost edge (x = 70) is the middle
    // of the ring; 1.5 px outside is white.
    assert_close(image, 71, 150, [0.0; 3]);
    assert_close(image, 68, 150, [255.0; 3]);
}

#[test]
fn translucent_stroked_circles_composite_exactly() {
    let cx = Context::new_blocking().unwrap();
    let image = two_marks(&cx, |plot, [x, y], fill| {
        plot.add(Selection::<(), Circle>::new(vec![()]))
            .attr(Circle::X, Px(x))
            .attr(Circle::Y, Px(y))
            .attr(Circle::RADIUS, Px(60.0))
            .attr(Circle::FILL, fill)
            .attr(Circle::FILL_OPACITY, Unit(0.5))
            .attr(Circle::STROKE, STROKE)
            .attr(Circle::STROKE_WIDTH, Px(3.0));
    });
    check_compositing(&image);
}

#[test]
fn translucent_stroked_rects_composite_exactly() {
    let cx = Context::new_blocking().unwrap();
    let image = two_marks(&cx, |plot, [x, y], fill| {
        plot.add(Selection::<(), Rect>::new(vec![()]))
            .attr(Rect::X, Px(x))
            .attr(Rect::Y, Px(y))
            .attr(Rect::WIDTH, Px(120.0))
            .attr(Rect::HEIGHT, Px(120.0))
            .attr(Rect::FILL, fill)
            .attr(Rect::FILL_OPACITY, Unit(0.5))
            .attr(Rect::STROKE, STROKE)
            .attr(Rect::STROKE_WIDTH, Px(3.0));
    });
    check_compositing(&image);
}

/// Defaults draw no stroke and an opaque fill: `Unit` is the f32 uniform.
#[test]
fn unit_is_an_f32_constant() {
    assert_eq!(Unit(0.25).const_value(), gup_core::ConstValue::F32(0.25));
    assert_eq!(<Unit as gup_core::GpuType>::WGSL, "f32");
}

/// AC3: twelve overlapping translucent bubbles with a thin stroke, in
/// one layer whose radius is a column, so they draw largest first
/// (GUP-431): every small bubble's outline is whole, on top.
#[test]
fn bubbles_with_translucent_fills_and_outlines() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.x(Linear::new().domain(0.0, 1.0));
    plot.y(Linear::new().domain(0.0, 1.0));
    plot.title("Overlapping bubbles");
    // The fill is a stand-in colour per bubble, not a quantity to read off a
    // legend (GUP-426 turns legends on for any colour scale).
    plot.legends(false);
    // (x, y, radius, colour), in pixels.
    let bubbles: Vec<[f64; 4]> = (0..12u32)
        .map(|i| {
            [
                130.0 + 460.0 * halton(i + 1, 2),
                130.0 + 220.0 * halton(i + 1, 3),
                20.0 + 40.0 * f64::from((i * 7) % 12) / 11.0,
                f64::from(i) / 11.0,
            ]
        })
        .collect();
    let pixels = || Linear::new().domain(0.0, 1.0).range(Px(0.0), Px(1.0));
    plot.add(Selection::<[f64; 4], Circle>::new(bubbles))
        .attr(Circle::X, pixels().encode(|b: &[f64; 4]| b[0]))
        .attr(Circle::Y, pixels().encode(|b: &[f64; 4]| b[1]))
        .attr(Circle::RADIUS, pixels().encode(|b: &[f64; 4]| b[2]))
        .attr(
            Circle::FILL,
            Sequential::viridis()
                .domain(0.0, 1.0)
                .encode(|b: &[f64; 4]| b[3]),
        )
        .attr(Circle::FILL_OPACITY, Unit(0.6))
        .attr(Circle::STROKE, Color::hex(0x333333))
        .attr(Circle::STROKE_WIDTH, Px(1.5));
    let resolved = plot.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
    let image = ImageTarget::new(&cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    harness()
        .run(
            "gup_core/bubble_stroke",
            Ok((vr, layout_metadata(&resolved.layout))),
        )
        .assert_ok();
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
