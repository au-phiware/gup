// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-527: legible labels over dense marks. `TextLayer::halo` draws each
//! label over an outline in the theme's background colour, on the GPU and
//! in SVG; `TextLayer::hide_overlaps` keeps a label only if its box
//! overlaps no label kept before it. Plus the `gup_core/text_halo` golden.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata};
use gup_core::geom::Rect as Box2;
use gup_core::prelude::*;
use gup_core::scene::{Halo, ItemKind, TextRole};
use gup_core::{ImageTarget, Layout, PlacedText, SvgTarget, VectorTarget};
use gup_visual_regression::RgbaImage;

const W: u32 = 480;
const H: u32 = 320;

/// The annotation labels of a layout, in draw order.
fn labels(layout: &Layout) -> Vec<&PlacedText> {
    layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::Annotation)
        .collect()
}

/// A thick line across y = 5 in `line`, and "HH" centred on it at x = 5,
/// with a halo of `halo` px (none if 0) and `theme`.
fn label_on_a_line(theme: Theme, line: Color, halo: Option<f32>) -> Plot {
    let mut plot = Plot::new();
    plot.legends(false);
    plot.theme(theme);
    plot.x(Linear::new().domain(0.0, 10.0));
    plot.y(Linear::new().domain(0.0, 10.0));
    plot.line([(0.0, 5.0), (10.0, 5.0)])
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .color(line)
        .width(Px(24.0));
    let layer = plot
        .text([(5.0, 5.0)])
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|_: &(f64, f64)| "HH")
        .size(14.0)
        .color(Color::hex(0xd55e00));
    if let Some(px) = halo {
        layer.halo(Px(px));
    }
    plot
}

fn render(cx: &Context, plot: &mut Plot) -> (image::RgbaImage, Layout) {
    let resolved = plot.resolve(cx, W as f32, H as f32).unwrap().clone();
    let image = ImageTarget::new(cx, W, H)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap();
    (image, resolved.layout)
}

fn close(a: [u8; 4], b: Color, tol: i32) -> bool {
    let b = b.to_rgba8();
    (0..3).all(|i| (i32::from(a[i]) - i32::from(b[i])).abs() <= tol)
}

/// The pixels left and right of the label's ink, 1 and 2 px out, at the
/// middle of its height: inside a 3 px halo, beside the H's stems.
fn beside(image: &image::RgbaImage, t: &PlacedText) -> Vec<[u8; 4]> {
    let y = (t.bounds.top() + t.bounds.height / 2.0) as u32;
    let (l, r) = (
        t.bounds.left().floor() as u32,
        t.bounds.right().ceil() as u32,
    );
    [l - 2, l - 1, r, r + 1]
        .into_iter()
        .map(|x| image.get_pixel(x, y).0)
        .collect()
}

/// AC1: a halo paints the background colour around the glyphs over a
/// mark, light on the default theme and dark on the dark theme; without
/// one the mark shows right up to the ink. The text keeps its colour.
#[test]
fn a_halo_is_the_background_colour_around_the_glyphs() {
    let cx = Context::new_blocking().unwrap();
    for (theme, line) in [
        (Theme::default(), Color::hex(0x0072b2)),
        (Theme::dark(), Color::hex(0xf0e442)),
    ] {
        let background = theme.background;
        let (plain, layout) = render(&cx, &mut label_on_a_line(theme.clone(), line, None));
        let t = labels(&layout)[0];
        for p in beside(&plain, t) {
            assert!(close(p, line, 8), "no halo: the line, not {p:?}");
        }
        let (haloed, layout) = render(&cx, &mut label_on_a_line(theme, line, Some(3.0)));
        let h = labels(&layout)[0];
        assert_eq!(h.bounds, t.bounds, "a halo does not move the ink box");
        assert_eq!(
            h.run.style.halo,
            Some(Halo {
                width: Px(3.0),
                color: background
            })
        );
        for p in beside(&haloed, h) {
            assert!(
                close(p, background, 8),
                "halo pixel {p:?}, want {background:?}"
            );
        }
        // Text pixels in the label colour are still there.
        let orange = (h.bounds.top() as u32..h.bounds.bottom() as u32)
            .flat_map(|y| (h.bounds.left() as u32..h.bounds.right() as u32).map(move |x| (x, y)))
            .filter(|&(x, y)| close(haloed.get_pixel(x, y).0, Color::hex(0xd55e00), 24))
            .count();
        assert!(orange >= 10, "{orange} text pixels");
        // The halo stays near the glyphs: 5 px out is the line again.
        let y = (h.bounds.top() + h.bounds.height / 2.0) as u32;
        let p = haloed.get_pixel(h.bounds.left() as u32 - 5, y).0;
        assert!(close(p, line, 8), "5 px out: {p:?}");
    }
}

/// Risk mitigation: `halo(Px(0.0))` draws exactly what no halo draws.
#[test]
fn a_zero_halo_changes_nothing() {
    let cx = Context::new_blocking().unwrap();
    let line = Color::hex(0x0072b2);
    let (a, la) = render(&cx, &mut label_on_a_line(Theme::default(), line, None));
    let (b, lb) = render(&cx, &mut label_on_a_line(Theme::default(), line, Some(0.0)));
    assert_eq!(labels(&lb)[0].run.style.halo, None);
    assert_eq!(la.texts, lb.texts);
    assert!(a.as_raw() == b.as_raw(), "the images differ");
}

/// AC1: SVG strokes a haloed label twice the width, round-joined, in the
/// background colour, under its fill; other text has no stroke.
#[test]
fn svg_strokes_the_halo_under_the_fill() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = label_on_a_line(Theme::dark(), Color::hex(0xf0e442), Some(1.5));
    plot.title("Halo");
    let scene = plot.resolve(&cx, W as f32, H as f32).unwrap().scene.clone();
    let mut svg = SvgTarget::new();
    svg.render(&scene).unwrap();
    let svg = svg.into_string();
    let bg = Theme::dark().background.to_rgba8();
    let stroke = format!(
        r##" stroke="#{:02x}{:02x}{:02x}" stroke-width="3" stroke-linejoin="round" paint-order="stroke""##,
        bg[0], bg[1], bg[2]
    );
    let label = svg.lines().find(|l| l.ends_with(">HH</text>")).unwrap();
    assert!(label.contains(&stroke), "{label}");
    let title = svg.lines().find(|l| l.ends_with(">Halo</text>")).unwrap();
    assert!(!title.contains("stroke"), "{title}");

    // resvg draws the stroke under the fill (a stroke on top would cover
    // the 1.6 px stems), and the halo beside the stems as the GPU does.
    let raster = svg_marks::rasterise(&svg);
    let raster =
        image::RgbaImage::from_raw(raster.width(), raster.height(), raster.data().to_vec())
            .unwrap();
    let layout = plot
        .resolve(&cx, W as f32, H as f32)
        .unwrap()
        .layout
        .clone();
    let t = labels(&layout)[0];
    let orange = (t.bounds.top() as u32..t.bounds.bottom() as u32)
        .flat_map(|y| (t.bounds.left() as u32..t.bounds.right() as u32).map(move |x| (x, y)))
        .filter(|&(x, y)| close(raster.get_pixel(x, y).0, Color::hex(0xd55e00), 24))
        .count();
    assert!(orange >= 10, "{orange} text pixels in the SVG raster");
    let y = (t.bounds.top() + t.bounds.height / 2.0) as u32;
    let p = raster.get_pixel(t.bounds.left() as u32 - 1, y).0;
    assert!(
        close(p, Theme::dark().background, 8),
        "SVG halo pixel {p:?}"
    );
}

/// A scatter of `n` points spread evenly (a golden-ratio sequence) over
/// 0..100 × 0..100.
fn scatter(n: u32) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let t = f64::from(i);
            (
                (t * 0.618_034).fract() * 100.0,
                (t * 0.414_214).fract() * 100.0,
            )
        })
        .collect()
}

fn labelled_scatter(rows: &[(f64, f64)], halo: f32, hide: bool) -> Plot {
    let mut plot = Plot::new();
    plot.legends(false);
    plot.circles(rows.to_vec())
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1);
    plot.text(rows.to_vec())
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|r: &(f64, f64)| format!("{:.1}", r.0))
        .anchor(Anchor::BOTTOM)
        .dy(-4.0)
        .halo(Px(halo))
        .hide_overlaps(hide);
    plot
}

/// AC2: with `hide_overlaps`, no two kept boxes (grown by the halo)
/// overlap, and every label that was dropped overlaps a label kept before
/// it in row order: the greedy rule, checked against the labels drawn
/// without culling.
#[test]
fn hide_overlaps_keeps_labels_that_overlap_no_earlier_kept_label() {
    let cx = Context::new_blocking().unwrap();
    let rows = scatter(1000);
    for halo in [0.0, 2.0] {
        let all = labelled_scatter(&rows, halo, false)
            .resolve(&cx, 800.0, 600.0)
            .unwrap()
            .layout
            .clone();
        let kept = labelled_scatter(&rows, halo, true)
            .resolve(&cx, 800.0, 600.0)
            .unwrap()
            .layout
            .clone();
        let (all, kept) = (labels(&all), labels(&kept));
        let overlap = |a: &PlacedText, b: &PlacedText| {
            let (a, b) = (a.bounds.inflate(halo), b.bounds.inflate(halo));
            a.left() < b.right()
                && b.left() < a.right()
                && a.top() < b.bottom()
                && b.top() < a.bottom()
        };
        assert!(all.len() > 900, "{}", all.len());
        assert!(
            kept.len() > 100 && kept.len() < all.len() * 3 / 4,
            "{} of {} kept",
            kept.len(),
            all.len()
        );
        for (i, a) in kept.iter().enumerate() {
            for b in &kept[..i] {
                assert!(!overlap(a, b), "{a:?} overlaps {b:?}");
            }
        }
        // Replay the rule over every drawn label, in row order.
        let mut want: Vec<&PlacedText> = Vec::new();
        for t in &all {
            if !want.iter().any(|k| overlap(k, t)) {
                want.push(t);
            }
        }
        assert_eq!(
            kept.iter().map(|t| &t.run).collect::<Vec<_>>(),
            want.iter().map(|t| &t.run).collect::<Vec<_>>()
        );
    }
}

/// AC2: culling runs again on a resize: a larger plot keeps more labels.
#[test]
fn hide_overlaps_keeps_more_labels_on_a_larger_plot() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = labelled_scatter(&scatter(400), 0.0, true);
    let small = labels(&plot.resolve(&cx, 400.0, 300.0).unwrap().layout).len();
    let large = labels(&plot.resolve(&cx, 1600.0, 1200.0).unwrap().layout).len();
    assert!(large > small * 2, "{small} then {large}");
}

/// AC3: the `gup_core/text_halo` golden: 300 labelled discs with 2 px
/// halos and overlapping labels hidden, a line through them, and the SVG
/// raster matching.
#[test]
fn text_halo_golden() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (640u32, 400u32);
    let rows = scatter(300);
    let mut plot = Plot::new();
    plot.legends(false);
    plot.title("Labels with halos, overlaps hidden");
    plot.x(Linear::new().domain(0.0, 100.0));
    plot.y(Linear::new().domain(0.0, 100.0));
    plot.circles(rows.clone())
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .radius(Px(4.0));
    plot.line([(0.0, 20.0), (100.0, 80.0)])
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .color(Color::hex(0xd55e00))
        .width(Px(2.0));
    plot.text(rows)
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|r: &(f64, f64)| format!("{:.1}", r.1))
        .anchor(Anchor::BOTTOM)
        .dy(-6.0)
        .halo(Px(2.0))
        .hide_overlaps(true);
    let resolved = plot.resolve(&cx, w as f32, h as f32).unwrap().clone();
    let kept = labels(&resolved.layout).len();
    assert!((40..300).contains(&kept), "{kept} labels kept");
    // Every annotation run in the scene carries the halo.
    let runs = resolved
        .scene
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            ItemKind::Text(runs) => Some(runs),
            _ => None,
        })
        .flatten()
        .filter(|r| r.role == TextRole::Annotation);
    for r in runs {
        assert_eq!(r.style.halo.map(|h| h.width), Some(Px(2.0)));
    }
    let image = ImageTarget::new(&cx, w, h)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let meta = layout_metadata(&resolved.layout);
    harness()
        .run("gup_core/text_halo", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    svg_marks::assert_matches("gup_core/text_halo", &resolved.scene, &vr, &meta);
}

// GUP-611: a label that collides tries other anchors before it is hidden.

/// The fallbacks the tests use: right of the point, left of it, below it.
const FALLBACKS: [(Anchor, f32, f32); 3] = [
    (Anchor::LEFT, 5.0, 0.0),
    (Anchor::RIGHT, -5.0, 0.0),
    (Anchor::TOP, 0.0, 5.0),
];

/// The scatter test's halo (the cost test's scatter has none).
const HALO: f32 = 0.0;

fn overlap(a: &PlacedText, b: &PlacedText, halo: f32) -> bool {
    let (a, b) = (a.bounds.inflate(halo), b.bounds.inflate(halo));
    a.left() < b.right() && b.left() < a.right() && a.top() < b.bottom() && b.top() < a.bottom()
}

/// The 1,000-point scatter of `labelled_scatter` with 2 px halos, culled,
/// with `fallbacks` (and culling off if `hide` is false).
fn placed_scatter(rows: &[(f64, f64)], fallbacks: &[(Anchor, f32, f32)], hide: bool) -> Plot {
    let mut plot = Plot::new();
    plot.legends(false);
    plot.circles(rows.to_vec())
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1);
    plot.text(rows.to_vec())
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|r: &(f64, f64)| format!("{:.1}", r.0))
        .anchor(Anchor::BOTTOM)
        .dy(-4.0)
        .halo(Px(HALO))
        .fallbacks(fallbacks.iter().copied())
        .hide_overlaps(hide);
    plot
}

/// With no fallbacks (or with culling off) the output is today's, byte for
/// byte; with fallbacks no two kept boxes overlap, more labels are kept,
/// and the result is the same on every resolve.
#[test]
fn fallbacks_keep_more_labels_and_none_overlap() {
    let cx = Context::new_blocking().unwrap();
    let rows = scatter(1000);
    let halo = HALO;
    let resolve = |plot: &mut Plot| plot.resolve(&cx, 800.0, 600.0).unwrap().layout.clone();
    // No fallbacks: the layer `labelled_scatter` builds, as before.
    let base = resolve(&mut placed_scatter(&rows, &[], true));
    let single = resolve(&mut labelled_scatter(&rows, halo, true));
    assert_eq!(labels(&base), labels(&single));
    // Culling off: the fallbacks are ignored.
    let all = resolve(&mut placed_scatter(&rows, &[], false));
    let ignored = resolve(&mut placed_scatter(&rows, &FALLBACKS, false));
    assert_eq!(labels(&all), labels(&ignored));

    let placed = resolve(&mut placed_scatter(&rows, &FALLBACKS, true));
    let (base, placed) = (labels(&base), labels(&placed));
    println!("kept {} then {} with fallbacks", base.len(), placed.len());
    assert!(placed.len() > base.len() * 11 / 10);
    for (i, a) in placed.iter().enumerate() {
        for b in &placed[..i] {
            assert!(!overlap(a, b, halo), "{a:?} overlaps {b:?}");
        }
    }
    // Some labels sit at a fallback, and only at the listed anchors.
    assert!(placed.iter().any(|t| t.run.anchor != Anchor::BOTTOM));
    for t in &placed {
        let a = t.run.anchor;
        assert!(a == Anchor::BOTTOM || FALLBACKS.iter().any(|f| f.0 == a));
    }
    // Deterministic.
    let again = resolve(&mut placed_scatter(&rows, &FALLBACKS, true));
    assert_eq!(placed, labels(&again));
}

/// Two labels on one point: the second, hidden at the layer's anchor, is
/// kept at the first fallback that fits (right of the point before below
/// it).
#[test]
fn a_hidden_label_is_kept_at_its_second_candidate() {
    let cx = Context::new_blocking().unwrap();
    let rows = vec![(5.0, 5.0), (5.0, 5.0)];
    let make = |fallbacks: &[(Anchor, f32, f32)]| {
        let mut plot = Plot::new();
        plot.x(Linear::new().domain(0.0, 10.0));
        plot.y(Linear::new().domain(0.0, 10.0));
        plot.circles(vec![(5.0, 5.0)])
            .x(|r: &(f64, f64)| r.0)
            .y(|r: &(f64, f64)| r.1);
        plot.text(rows.clone())
            .x(|r: &(f64, f64)| r.0)
            .y(|r: &(f64, f64)| r.1)
            .text(|r: &(f64, f64)| format!("{:.1}", r.0))
            .anchor(Anchor::BOTTOM)
            .dy(-4.0)
            .fallbacks(fallbacks.iter().copied())
            .hide_overlaps(true);
        plot
    };
    let hidden = make(&[]).resolve(&cx, 480.0, 320.0).unwrap().layout.clone();
    assert_eq!(labels(&hidden).len(), 1);
    let placed = make(&FALLBACKS)
        .resolve(&cx, 480.0, 320.0)
        .unwrap()
        .layout
        .clone();
    let l = labels(&placed);
    assert_eq!(l.len(), 2);
    assert_eq!(
        (l[0].run.anchor, l[1].run.anchor),
        (Anchor::BOTTOM, Anchor::LEFT)
    );
    assert_eq!(l[0].run.at.x + 5.0, l[1].run.at.x);
    assert!(!overlap(l[0], l[1], 0.0));
}

/// Edge room covers every candidate's reach: a fallback that reaches past
/// the plot edge insets the axis, and only when culling uses it.
#[test]
fn edge_room_covers_every_candidate() {
    let cx = Context::new_blocking().unwrap();
    let make = |fallbacks: &[(Anchor, f32, f32)], hide: bool| {
        let mut plot = Plot::new();
        let x = plot.x(Linear::new().domain(0.0, 10.0));
        plot.y(Linear::new().domain(0.0, 10.0));
        plot.circles(vec![(5.0, 5.0)])
            .x(|r: &(f64, f64)| r.0)
            .y(|r: &(f64, f64)| r.1);
        plot.text(vec![(10.0, 5.0)])
            .x(|r: &(f64, f64)| r.0)
            .y(|r: &(f64, f64)| r.1)
            .text(|_: &(f64, f64)| "label")
            .anchor(Anchor::BOTTOM)
            .dy(-4.0)
            .fallbacks(fallbacks.iter().copied())
            .hide_overlaps(hide);
        (plot, x)
    };
    let edge = |(mut plot, x): (Plot, ScaleRef<Linear>)| {
        plot.resolve(&cx, 480.0, 320.0).unwrap();

        x.read().eval(10.0)
    };
    let far = [(Anchor::LEFT, 40.0, 0.0)];
    let base = edge(make(&[], true));
    let with = edge(make(&far, true));
    assert!(with < base - 20.0, "{base} then {with}");
    assert_eq!(base, edge(make(&far, false)));
}

// GUP-656: labels keep off the points.

/// The radius of the discs and of the boxes `avoid_points` seeds.
const DISC: f32 = 4.0;
/// Offsets that clear a disc: the first choice above it, then the sides and
/// below.
const CLEAR: [(Anchor, f32, f32); 4] = [
    (Anchor::BOTTOM, 0.0, -5.0),
    (Anchor::LEFT, 6.0, 0.0),
    (Anchor::RIGHT, -6.0, 0.0),
    (Anchor::TOP, 0.0, 6.0),
];

/// The 1,000-point scatter with `DISC`-radius circles and every candidate of
/// `CLEAR`, culled; `avoid` is `avoid_points`' argument (0 for off).
fn avoiding_scatter(rows: &[(f64, f64)], avoid: f32) -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let mut plot = Plot::new();
    plot.legends(false);
    let x = plot.x(Linear::new());
    let y = plot.y(Linear::new());
    plot.circles(rows.to_vec())
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .radius(Px(DISC));
    plot.text(rows.to_vec())
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|r: &(f64, f64)| format!("{:.1}", r.0))
        .anchor(CLEAR[0].0)
        .dy(CLEAR[0].2)
        .fallbacks(CLEAR[1..].iter().copied())
        .hide_overlaps(true)
        .avoid_points(Px(avoid));
    (plot, x, y)
}

fn disc_boxes(rows: &[(f64, f64)], x: &ScaleRef<Linear>, y: &ScaleRef<Linear>) -> Vec<Box2> {
    rows.iter()
        .map(|r| {
            let (px, py) = (x.read().eval(r.0) as f32, y.read().eval(r.1) as f32);
            Box2::from_edges(px - DISC, py - DISC, px + DISC, py + DISC)
        })
        .collect()
}

fn touches(b: Box2, d: Box2) -> bool {
    b.left() < d.right() && d.left() < b.right() && b.top() < d.bottom() && d.top() < b.bottom()
}

/// The distance from a point to a box (0 inside).
fn distance(b: Box2, p: (f32, f32)) -> f32 {
    let dx = (b.left() - p.0).max(p.0 - b.right()).max(0.0);
    let dy = (b.top() - p.1).max(p.1 - b.bottom()).max(0.0);
    dx.hypot(dy)
}

/// How many kept labels have a box nearer another row's point than their
/// own (the own point is the one whose candidate position the label sits
/// at), and how many are kept.
fn farther_from_own(layout: &Layout, points: &[(f32, f32)]) -> (usize, usize) {
    let kept = labels(layout);
    let mut wrong = 0;
    for t in &kept {
        let own = points
            .iter()
            .position(|p| {
                CLEAR.iter().any(|&(a, dx, dy)| {
                    a == t.run.anchor
                        && (t.run.at.x - p.0 - dx).abs() < 1e-3
                        && (t.run.at.y - p.1 - dy).abs() < 1e-3
                })
            })
            .expect("a label sits at a candidate of some row");
        let d_own = distance(t.bounds, points[own]);
        if points
            .iter()
            .enumerate()
            .any(|(i, &p)| i != own && distance(t.bounds, p) < d_own)
        {
            wrong += 1;
        }
    }
    (wrong, kept.len())
}

/// No kept label touches a disc, the output is deterministic, and the
/// nearest-point readability holds for nearly every label.
#[test]
fn avoid_points_keeps_labels_off_the_discs() {
    let cx = Context::new_blocking().unwrap();
    let rows = scatter(1000);
    let run = |avoid: f32| {
        let (mut plot, x, y) = avoiding_scatter(&rows, avoid);
        let layout = plot.resolve(&cx, 800.0, 600.0).unwrap().layout.clone();
        let discs = disc_boxes(&rows, &x, &y);
        (layout, discs)
    };
    let (without, discs) = run(0.0);
    let (with, _) = run(DISC);
    let covering = |l: &Layout| {
        labels(l)
            .iter()
            .filter(|t| discs.iter().any(|&d| touches(t.bounds, d)))
            .count()
    };
    println!(
        "labels touching a disc: without {}, with {}",
        covering(&without),
        covering(&with)
    );
    assert!(covering(&without) > 0);
    assert_eq!(covering(&with), 0);
    let kept = labels(&with);
    for (i, a) in kept.iter().enumerate() {
        for b in &kept[..i] {
            assert!(!overlap(a, b, 0.0), "{a:?} overlaps {b:?}");
        }
    }
    println!(
        "avoid_points kept {} (without {}, GUP-611 599)",
        kept.len(),
        labels(&without).len()
    );
    assert!(kept.len() >= 250, "{} kept of 1000", kept.len());
    let points: Vec<(f32, f32)> = discs
        .iter()
        .map(|d| ((d.left() + d.right()) / 2.0, (d.top() + d.bottom()) / 2.0))
        .collect();
    let (bad_without, n_without) = farther_from_own(&without, &points);
    let (bad, n) = farther_from_own(&with, &points);
    println!(
        "nearer another point: without {bad_without}/{n_without} ({:.1}%), with {bad}/{n} ({:.1}%)",
        100.0 * bad_without as f64 / n_without as f64,
        100.0 * bad as f64 / n as f64
    );
    assert_eq!(bad, 0, "{bad} of {n} nearer another point");
    let (again, _) = run(DISC);
    assert_eq!(labels(&with), labels(&again));
}

/// A row without a label is an obstacle too, and a non-finite row is not.
#[test]
fn unlabelled_rows_take_room_and_non_finite_rows_do_not() {
    let cx = Context::new_blocking().unwrap();
    let resolve = |rows: Vec<(f64, f64)>, avoid: f32| {
        let mut plot = Plot::new();
        plot.legends(false);
        plot.x(Linear::new().domain(0.0, 10.0));
        plot.y(Linear::new().domain(0.0, 10.0));
        plot.circles(rows.clone())
            .x(|r: &(f64, f64)| r.0)
            .y(|r: &(f64, f64)| r.1);
        plot.text(rows)
            .x(|r: &(f64, f64)| r.0)
            .y(|r: &(f64, f64)| r.1)
            .text(|r: &(f64, f64)| {
                if r.0 == 5.0 {
                    "x".to_string()
                } else {
                    String::new()
                }
            })
            .anchor(Anchor::BOTTOM)
            .dy(-8.0)
            .hide_overlaps(true)
            .avoid_points(Px(avoid));
        labels(&plot.resolve(&cx, 480.0, 300.0).unwrap().layout).len()
    };
    // An unlabelled point just above the first row's point: its disc
    // covers the label.
    let near = vec![(5.0, 5.0), (5.01, 5.8)];
    assert_eq!(resolve(near.clone(), 0.0), 1);
    assert_eq!(resolve(near, 6.0), 0);
    let nan = vec![(5.0, 5.0), (f64::NAN, 5.8)];
    assert_eq!(resolve(nan, 6.0), 1);
}

/// The `gup_core/text_fallbacks` golden: the `text_halo` scatter with
/// fallbacks, so labels the single anchor hid sit beside or below their
/// points.
#[test]
fn text_fallbacks_golden() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (640u32, 400u32);
    let rows = scatter(300);
    let mut plot = Plot::new();
    plot.legends(false);
    plot.title("Labels try other anchors and keep off the points");
    plot.x(Linear::new().domain(0.0, 100.0));
    plot.y(Linear::new().domain(0.0, 100.0));
    plot.circles(rows.clone())
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .radius(Px(4.0));
    plot.text(rows)
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|r: &(f64, f64)| format!("{:.1}", r.1))
        .anchor(Anchor::BOTTOM)
        .dy(-6.0)
        .fallbacks([
            (Anchor::LEFT, 7.0, 0.0),
            (Anchor::RIGHT, -7.0, 0.0),
            (Anchor::TOP, 0.0, 7.0),
        ])
        .halo(Px(2.0))
        .hide_overlaps(true)
        .avoid_points(Px(4.0));
    let resolved = plot.resolve(&cx, w as f32, h as f32).unwrap().clone();
    println!("golden kept {}", labels(&resolved.layout).len());
    let image = ImageTarget::new(&cx, w, h)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let meta = layout_metadata(&resolved.layout);
    harness()
        .run("gup_core/text_fallbacks", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    svg_marks::assert_matches("gup_core/text_fallbacks", &resolved.scene, &vr, &meta);
}
