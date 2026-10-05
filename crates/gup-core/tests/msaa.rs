// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! MSAA on Gup-owned targets (RFC-001 §7, GUP-401 AC3). One scene with
//! large circles (analytic antialiasing), a fan of diagonal rules and a
//! rect on fractional coordinates (geometric edges), rendered at 1 and 4
//! samples. Both renders are golden images, and a side-by-side, 8×
//! enlarged crop is written to the artifact directory
//! (`$CARGO_TARGET_DIR/visual-regression/gup_core/msaa_compare.png`) for
//! looking at.

mod common;

use common::vr::harness;
use gup_core::geom::{Point, Rect};
use gup_core::prelude::*;
use gup_core::scene::{Item, ItemKind, RectPrim, Rule, Z_AXES};
use gup_core::{ImageTarget, Scene, TargetOptions};
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{LayoutMetadata, PxRect, Rgba8, RgbaImage};
use std::path::Path;

const W: u32 = 320;
const H: u32 = 200;
const INK: Color = Color::hex(0x1f3a93);

/// Large circles from a plot, plus diagonal rules and a fractional rect.
fn scene(cx: &Context) -> Scene {
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(Selection::<(f64, f64), Circle>::new(vec![
        (0.0, 0.0),
        (1.0, 1.0),
        (2.0, 0.4),
    ]))
    .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
    .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
    .attr(Circle::RADIUS, Px(14.0));
    let mut scene = plot.resolve(cx, W as f32, H as f32).unwrap().scene;
    let rules = (0..7)
        .map(|i| {
            let a = (5.0 + 12.0 * i as f32).to_radians();
            Rule {
                p0: Point::new(150.0, 150.0),
                p1: Point::new(150.0 + 150.0 * a.cos(), 150.0 - 150.0 * a.sin()),
                width: Px(1.5),
                color: INK,
            }
        })
        .collect();
    scene.push(Item {
        z: Z_AXES,
        clip: None,
        kind: ItemKind::Rules(rules),
    });
    scene.push(Item {
        z: Z_AXES,
        clip: None,
        kind: ItemKind::Rects(vec![RectPrim {
            rect: Rect::new(90.3, 40.6, 30.4, 20.5),
            color: INK,
        }]),
    });
    scene
}

fn render(cx: &Context, scene: &Scene, samples: u32) -> image::RgbaImage {
    let options = TargetOptions {
        samples,
        ..TargetOptions::default()
    };
    ImageTarget::with_options(cx, W, H, options)
        .unwrap()
        .render_blocking(cx, scene)
        .unwrap()
}

/// Pixels in `area` that are neither the background nor the ink colour:
/// partially covered edge pixels.
fn partial_pixels(img: &image::RgbaImage, area: [u32; 4]) -> usize {
    let ink = INK.to_rgba8();
    let [x0, y0, x1, y1] = area;
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let p = img.get_pixel(x, y).0;
            p != [255, 255, 255, 255] && p != ink
        })
        .count()
}

/// Nearest-neighbour enlargement of `crop` from both images, side by side.
fn compare(one: &image::RgbaImage, four: &image::RgbaImage, crop: [u32; 4]) -> image::RgbaImage {
    const SCALE: u32 = 8;
    const GAP: u32 = 8;
    let [x0, y0, x1, y1] = crop;
    let (cw, ch) = ((x1 - x0) * SCALE, (y1 - y0) * SCALE);
    let mut out = image::RgbaImage::from_pixel(2 * cw + GAP, ch, image::Rgba([255, 0, 0, 255]));
    for (i, img) in [one, four].into_iter().enumerate() {
        for y in 0..ch {
            for x in 0..cw {
                let p = *img.get_pixel(x0 + x / SCALE, y0 + y / SCALE);
                out.put_pixel(i as u32 * (cw + GAP) + x, y, p);
            }
        }
    }
    out
}

#[test]
fn four_samples_smooth_geometric_edges() {
    let cx = Context::new_blocking().unwrap();
    let scene = scene(&cx);
    let one = render(&cx, &scene, 1);
    let four = render(&cx, &scene, 4);

    // The rule fan, away from the circles and axes.
    let fan = [190, 20, 270, 140];
    let (fan1, fan4) = (partial_pixels(&one, fan), partial_pixels(&four, fan));
    // The fractional rect's edges.
    let rect = [86, 36, 125, 66];
    let (rect1, rect4) = (partial_pixels(&one, rect), partial_pixels(&four, rect));
    eprintln!(
        "partially covered pixels: rule fan 1× {fan1}, 4× {fan4}; rect 1× {rect1}, 4× {rect4}"
    );
    let artifacts =
        default_artifact_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).join("gup_core");
    std::fs::create_dir_all(&artifacts).unwrap();
    compare(&one, &four, [200, 40, 260, 100])
        .save(artifacts.join("msaa_compare.png"))
        .unwrap();
    compare(&one, &four, [30, 128, 82, 178])
        .save(artifacts.join("msaa_compare_circle.png"))
        .unwrap();
    // At one sample a pixel is either covered or not: rules and rects
    // have hard, stair-stepped edges. At four, edge pixels blend.
    assert_eq!(fan1, 0, "1× rules should have no partial pixels");
    assert_eq!(rect1, 0, "1× rect should have no partial pixels");
    assert!(fan4 > 200, "4× rules: only {fan4} partial pixels");
    assert!(rect4 > 50, "4× rect: only {rect4} partial pixels");

    for (case, img) in [("gup_core/msaa_1x", one), ("gup_core/msaa_4x", four)] {
        let meta = LayoutMetadata::new(PxRect::new(0.0, 0.0, W as f32, H as f32))
            .with_background(Rgba8::WHITE)
            .with_expected_color("ink", Rgba8::from_hex(0x1f3a93));
        let vr = RgbaImage::new(W, H, img.into_raw()).unwrap();
        harness().run(case, Ok((vr, meta))).assert_ok();
    }
}

/// Circles antialias analytically, so MSAA barely changes them: the
/// 1× and 4× renders of a circle's edge agree to within rounding.
#[test]
fn circles_antialias_without_msaa() {
    let cx = Context::new_blocking().unwrap();
    let scene = scene(&cx);
    let (one, four) = (render(&cx, &scene, 1), render(&cx, &scene, 4));
    let area = [20, 120, 80, 190];
    let partial = partial_pixels(&one, area);
    assert!(
        partial > 30,
        "1× circle edge: only {partial} partial pixels"
    );
    let max_diff = (area[1]..area[3])
        .flat_map(|y| (area[0]..area[2]).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (a, b) = (one.get_pixel(x, y).0, four.get_pixel(x, y).0);
            (0..3).map(|k| a[k].abs_diff(b[k])).max().unwrap()
        })
        .max()
        .unwrap();
    assert!(
        max_diff <= 2,
        "circle edge differs by {max_diff} between 1× and 4×"
    );
}
