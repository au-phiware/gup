// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Antialiasing on Gup-owned targets (RFC-001 §7). One scene with large
//! circles, a fan of diagonal rules and a rect on fractional coordinates,
//! rendered at 1 and 4 samples. Circles, rules and rects all antialias
//! analytically (GUP-421), so one sample looks as smooth as four did when
//! only MSAA smoothed rules and rects: `fixtures/msaa_4x_before_analytic.png`
//! is that 4× render, kept as the reference. Both renders are golden
//! images, and side-by-side, 8× enlarged crops (the reference, 1× and 4×)
//! are written to the artifact directory
//! (`$CARGO_TARGET_DIR/visual-regression/gup_core/msaa_compare*.png`) for
//! looking at.

mod common;

use common::vr::harness;
use gup_core::geom::{Point, Rect};
use gup_core::prelude::*;
use gup_core::scene::{Item, ItemKind, RectPrim, Rule, Z_AXES};
use gup_core::{
    Antialias, Channel, ChannelDesc, ImageTarget, Mark, Scene, SvgTarget, TargetOptions,
    VectorTarget, WgslModule,
};
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{LayoutMetadata, PxRect, Rgba8, RgbaImage};
use std::path::Path;

const W: u32 = 320;
const H: u32 = 200;
const INK: Color = Color::hex(0x1f3a93);

/// Large circles from a plot, plus diagonal rules and a fractional rect.
fn scene(cx: &Context) -> Scene {
    let mut plot = Plot::new();
    let (x, y) = (
        plot.x(Linear::new().domain(0.0, 2.0)),
        plot.y(Linear::new().domain(0.0, 1.0)),
    );

    plot.add(Selection::<(f64, f64), Circle>::new(vec![
        (0.0, 0.0),
        (1.0, 1.0),
        (2.0, 0.4),
    ]))
    .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
    .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
    .attr(Circle::RADIUS, Px(14.0));
    let mut scene = plot.resolve(cx, W as f32, H as f32).unwrap().scene.clone();
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
    ImageTarget::with_options(cx, W, H, TargetOptions::default().with_samples(samples))
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

/// How much ink `area` holds, in pixels' worth of full coverage: each
/// pixel's red channel read as the fraction of the way from the white
/// background to the ink colour (the scene blends in sRGB space).
fn ink(img: &image::RgbaImage, area: [u32; 4]) -> f64 {
    let ink_r = f64::from(INK.to_rgba8()[0]);
    let [x0, y0, x1, y1] = area;
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .map(|(x, y)| (255.0 - f64::from(img.get_pixel(x, y).0[0])) / (255.0 - ink_r))
        .sum()
}

/// Nearest-neighbour enlargement of `crop` from each image, side by side.
fn compare(images: &[&image::RgbaImage], crop: [u32; 4]) -> image::RgbaImage {
    const SCALE: u32 = 8;
    const GAP: u32 = 8;
    let [x0, y0, x1, y1] = crop;
    let (cw, ch) = ((x1 - x0) * SCALE, (y1 - y0) * SCALE);
    let n = images.len() as u32;
    let mut out =
        image::RgbaImage::from_pixel(n * cw + (n - 1) * GAP, ch, image::Rgba([255, 0, 0, 255]));
    for (i, img) in images.iter().enumerate() {
        for y in 0..ch {
            for x in 0..cw {
                let p = *img.get_pixel(x0 + x / SCALE, y0 + y / SCALE);
                out.put_pixel(i as u32 * (cw + GAP) + x, y, p);
            }
        }
    }
    out
}

/// The 4× render of [`scene`] from before rules and rects antialiased
/// analytically, when MSAA alone smoothed them (GUP-401's `msaa_4x`
/// golden, as GUP-417 measured it).
fn reference() -> image::RgbaImage {
    image::load_from_memory(include_bytes!("fixtures/msaa_4x_before_analytic.png"))
        .unwrap()
        .to_rgba8()
}

/// The rule fan, away from the circles and axes.
const FAN: [u32; 4] = [190, 20, 270, 140];
/// The fractional rect's edges.
const RECT: [u32; 4] = [86, 36, 125, 66];

/// One sample now looks like four samples did when only MSAA smoothed
/// rules and rects (GUP-421 AC1, AC2, AC4).
#[test]
fn rules_and_rects_antialias_without_msaa() {
    let cx = Context::new_blocking().unwrap();
    let scene = scene(&cx);
    let one = render(&cx, &scene, 1);
    let four = render(&cx, &scene, 4);
    let before = reference();

    let artifacts =
        default_artifact_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).join("gup_core");
    std::fs::create_dir_all(&artifacts).unwrap();
    for (name, crop) in [
        ("msaa_compare.png", [200, 40, 260, 100]),
        ("msaa_compare_rect.png", RECT),
        ("msaa_compare_circle.png", [30, 128, 82, 178]),
    ] {
        compare(&[&before, &one, &four], crop)
            .save(artifacts.join(name))
            .unwrap();
    }

    // Partially covered pixels: GUP-417's counts for MSAA alone, and now
    // at one sample. Analytic coverage also shades pixels that a sliver
    // of the edge covers but none of MSAA's four sample points hit (the
    // rect's bottom row is 10% covered, below every sample), so it finds
    // more of them: at least as many, and at most half as many again.
    let (fan_before, rect_before) = (partial_pixels(&before, FAN), partial_pixels(&before, RECT));
    assert_eq!(
        (fan_before, rect_before),
        (597, 71),
        "the reference changed"
    );
    let (fan1, rect1) = (partial_pixels(&one, FAN), partial_pixels(&one, RECT));
    let (fan4, rect4) = (partial_pixels(&four, FAN), partial_pixels(&four, RECT));
    eprintln!(
        "partially covered pixels: rule fan {fan_before} before (4×), 1× {fan1}, 4× {fan4}; \
         rect {rect_before} before (4×), 1× {rect1}, 4× {rect4}"
    );
    for (what, now, before) in [("rule fan", fan1, fan_before), ("rect", rect1, rect_before)] {
        assert!(
            now >= before && now * 2 <= before * 3,
            "1× {what}: {now} partial pixels, against {before} at 4× before analytic coverage"
        );
    }

    // The same amount of ink as MSAA's estimate of the fan's area: the
    // ramp moves coverage between edge pixels, it does not thicken or
    // thin the rules.
    let (ink_before, ink1) = (ink(&before, FAN), ink(&one, FAN));
    eprintln!("rule fan ink: {ink_before:.1} before (4×), 1× {ink1:.1}");
    assert!(
        (ink1 / ink_before - 1.0).abs() < 0.02,
        "1× rule fan holds {ink1:.1} pixels of ink, against {ink_before:.1} at 4× before"
    );

    // An axis-aligned rect's coverage is exact: the area of each pixel
    // inside it.
    let overlap = |p: u32, lo: f32, hi: f32| (hi.min(p as f32 + 1.0) - lo.max(p as f32)).max(0.0);
    let ink_rgb = INK.to_rgba8();
    let [x0, y0, x1, y1] = RECT;
    for y in y0..y1 {
        for x in x0..x1 {
            let coverage = overlap(x, 90.3, 120.7) * overlap(y, 40.6, 61.1);
            let got = one.get_pixel(x, y).0;
            for k in 0..3 {
                let want = 255.0 + (f32::from(ink_rgb[k]) - 255.0) * coverage;
                assert!(
                    (f32::from(got[k]) - want).abs() <= 2.0,
                    "rect pixel ({x}, {y}): {got:?}, want {want:.1} in channel {k} for coverage \
                     {coverage:.3}"
                );
            }
        }
    }

    // MSAA no longer changes rules and rects: every sample of a
    // covered pixel is inside the quad, so 4× resolves to the 1× colour.
    for area in [FAN, RECT] {
        let max_diff = max_channel_diff(&one, &four, area);
        assert!(
            max_diff <= 2,
            "{area:?} differs by {max_diff} between 1× and 4×"
        );
    }

    for (case, img) in [("gup_core/msaa_1x", one), ("gup_core/msaa_4x", four)] {
        let meta = LayoutMetadata::new(PxRect::new(0.0, 0.0, W as f32, H as f32))
            .with_background(Rgba8::WHITE)
            .with_expected_color("ink", Rgba8::from_hex(0x1f3a93));
        let vr = RgbaImage::new(W, H, img.into_raw()).unwrap();
        harness().run(case, Ok((vr, meta))).assert_ok();
    }
}

/// The largest difference of any colour channel between `a` and `b` in
/// `area`.
fn max_channel_diff(a: &image::RgbaImage, b: &image::RgbaImage, area: [u32; 4]) -> u8 {
    (area[1]..area[3])
        .flat_map(|y| (area[0]..area[2]).map(move |x| (x, y)))
        .map(|(x, y)| {
            let (a, b) = (a.get_pixel(x, y).0, b.get_pixel(x, y).0);
            (0..3).map(|k| a[k].abs_diff(b[k])).max().unwrap()
        })
        .max()
        .unwrap()
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
    let max_diff = max_channel_diff(&one, &four, area);
    assert!(
        max_diff <= 2,
        "circle edge differs by {max_diff} between 1× and 4×"
    );
}

/// Rules snapped to pixel centres and rects on whole pixels have every
/// edge on a pixel boundary, so analytic coverage leaves them exactly as
/// crisp as before (GUP-421 AC1, AC2): at one sample (dpr 1 and 2) and at
/// four,
/// every pixel is background or ink, and the ink covers exactly the
/// shapes' area.
#[test]
fn snapped_hairlines_and_whole_pixel_rects_stay_crisp() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (120.0, 80.0);
    let rule = |x0, y0, x1, y1, width| Rule {
        p0: Point::new(x0, y0),
        p1: Point::new(x1, y1),
        width: Px(width),
        color: INK,
    };
    let rect = |x, y, w, h| RectPrim {
        rect: Rect::new(x, y, w, h),
        color: INK,
    };
    let mut scene = Scene::new(w, h, Color::WHITE);
    scene.push(Item {
        z: Z_AXES,
        clip: None,
        kind: ItemKind::Rules(vec![
            // Hairlines on pixel centres, as guides draw axes and ticks:
            // 51 × 1 and 1 × 31 pixels with their square caps.
            rule(10.5, 10.5, 60.5, 10.5, 1.0),
            rule(80.5, 5.5, 80.5, 35.5, 1.0),
            // Even widths on pixel boundaries, odd widths on centres:
            // 42 × 2 and 3 × 28.
            rule(10.0, 20.0, 50.0, 20.0, 2.0),
            rule(100.5, 5.5, 100.5, 30.5, 3.0),
        ]),
    });
    scene.push(Item {
        z: Z_AXES,
        clip: None,
        kind: ItemKind::Rects(vec![
            rect(10.0, 40.0, 30.0, 20.0),
            rect(60.0, 45.0, 1.0, 25.0),
        ]),
    });
    let area = 51 + 31 + 84 + 84 + 600 + 25;
    let ink = INK.to_rgba8();
    for (dpr, samples) in [(1u32, 1), (2, 1), (1, 4)] {
        let options = TargetOptions {
            dpr: dpr as f32,
            samples,
        };
        let img = ImageTarget::with_options(&cx, 120 * dpr, 80 * dpr, options)
            .unwrap()
            .render_blocking(&cx, &scene)
            .unwrap();
        let partial = partial_pixels(&img, [0, 0, img.width(), img.height()]);
        let inked = img.pixels().filter(|p| p.0 == ink).count() as u32;
        let case = format!("dpr {dpr}, {samples}×");
        assert_eq!(partial, 0, "{case}: {partial} partially covered pixels");
        assert_eq!(inked, area * dpr * dpr, "{case}: ink pixels");
    }
}

/// A stand-in for an area or polygon mark, none of which exists yet
/// (RFC-001 S6, S11): the circle's WGSL, declared [`Antialias::Msaa`].
enum StandInArea {}

impl StandInArea {
    const X: Channel<StandInArea, Px> = Channel::new(0, "x");
    const Y: Channel<StandInArea, Px> = Channel::new(1, "y");
}

impl Mark for StandInArea {
    const NAME: &'static str = Circle::NAME;
    const MODULE: &'static WgslModule = Circle::MODULE;
    const CHANNELS: &'static [ChannelDesc] = Circle::CHANNELS;
    const VERTICES_PER_INSTANCE: u32 = Circle::VERTICES_PER_INSTANCE;
    const ANTIALIAS: Antialias = Antialias::Msaa;
}

/// A scene with a mark that antialiases with MSAA is drawn with 4 samples
/// even on a single-sampled target (RFC-001 §7, GUP-421 AC3); every
/// other scene keeps the target's count.
#[test]
fn scenes_with_msaa_marks_draw_multisampled() {
    let resolve = |cx: &Context| {
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
        plot.add(Selection::<(f64, f64), StandInArea>::new(vec![
            (0.0, 0.0),
            (1.0, 1.0),
        ]))
        .attr(StandInArea::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(StandInArea::Y, y.encode(|r: &(f64, f64)| r.1));
        plot.resolve(cx, W as f32, H as f32).unwrap().scene.clone()
    };
    let target = |cx: &Context, samples| {
        ImageTarget::with_options(cx, W, H, TargetOptions::default().with_samples(samples)).unwrap()
    };
    let created = |cx: &Context| cx.pipeline_stats().pipelines_created;

    // Pipelines are keyed by sample count, so drawing a scene on a 4×
    // target creates none if it was already drawn with 4 samples.
    let cx = Context::new_blocking().unwrap();
    let area = resolve(&cx);
    assert_eq!(area.min_samples(), 4);
    let on_1x = target(&cx, 1).render_blocking(&cx, &area).unwrap();
    let before = created(&cx);
    let on_4x = target(&cx, 4).render_blocking(&cx, &area).unwrap();
    assert_eq!(
        created(&cx),
        before,
        "the 1× target did not draw with 4 samples"
    );
    assert!(
        on_1x == on_4x,
        "the area scene differs between 1× and 4× targets"
    );

    // Other scenes keep the target's count: the 4× target needs new
    // pipelines.
    let cx = Context::new_blocking().unwrap();
    let circles = scene(&cx);
    assert_eq!(circles.min_samples(), 1);
    target(&cx, 1).render_blocking(&cx, &circles).unwrap();
    let before = created(&cx);
    target(&cx, 4).render_blocking(&cx, &circles).unwrap();
    assert!(
        created(&cx) > before,
        "the 1× target drew the circles with 4 samples"
    );
}

/// Vector output has no sample count and no coverage ramp: the comparison
/// scene's guides (the axes, the rule fan, the fractional rect and the
/// tick labels) are written as exactly the SVG they were before rules and
/// rects gained analytic coverage on the GPU (GUP-421 AC6). The fixture
/// was written by the tree before that change.
#[test]
fn svg_of_the_comparison_scene_is_unchanged() {
    let cx = Context::new_blocking().unwrap();
    let mut target = SvgTarget::new();
    target.render(&scene(&cx).guides()).unwrap();
    let artifacts =
        default_artifact_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).join("gup_core");
    target.save(artifacts.join("msaa_guides.svg")).unwrap();
    assert!(
        target.svg() == include_str!("fixtures/msaa_guides.svg"),
        "the SVG of the comparison scene changed; compare {} with tests/fixtures/msaa_guides.svg",
        artifacts.join("msaa_guides.svg").display()
    );
}
