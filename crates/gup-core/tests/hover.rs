// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-676: the row under the pointer is ringed.

mod common;

use common::vr::{harness, layout_metadata};
use gup_core::prelude::*;
use gup_core::scene::{ItemKind, Z_FOCUS, Z_HOVER, Z_TOOLTIP};
use gup_core::{ImageTarget, Input, Key, RowId};
use gup_visual_regression::RgbaImage;

const W: f32 = 480.0;
const H: f32 = 320.0;

fn row(row: usize) -> RowId {
    RowId { layer: 0, row }
}

/// Five points; rows 1 and 2 are close together.
const XS: [f64; 5] = [10.0, 30.0, 33.0, 50.0, 70.0];
const YS: [f64; 5] = [3.0, 7.0, 6.0, 4.0, 8.0];

fn scatter() -> (Context, Plot) {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut plot = Plot::new();
    plot.circles((0..5usize).collect::<Vec<_>>())
        .x(|r| XS[*r])
        .y(|r| YS[*r])
        .radius(Px(6.0))
        .tooltip(|r| format!("row {r}"));
    plot.resolve(&cx, W, H).unwrap();
    (cx, plot)
}

fn render(cx: &Context, plot: &mut Plot) -> (image::RgbaImage, gup_core::Layout) {
    let resolved = plot.resolve(cx, W, H).unwrap();
    let layout = resolved.layout.clone();
    let image = ImageTarget::new(cx, W as u32, H as u32)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap();
    (image, layout)
}

fn golden(name: &str, cx: &Context, plot: &mut Plot) {
    let (image, layout) = render(cx, plot);
    let vr = RgbaImage::new(W as u32, H as u32, image.into_raw()).unwrap();
    harness()
        .run(name, Ok((vr, layout_metadata(&layout))))
        .assert_ok();
}

#[test]
fn a_hovered_row_is_ringed_at_its_anchor_and_leaving_clears_it() {
    let (cx, mut plot) = scatter();
    let a = plot.pick_anchor(row(1)).unwrap();
    assert!(plot.handle(Input::pointer_moved(a)).redraw);
    assert_eq!(plot.hovered(), Some(row(1)));
    let resolved = plot.resolve(&cx, W, H).unwrap();
    let ring = resolved.layout.hover.expect("a hover ring");
    assert!(resolved.layout.focus.is_none() && resolved.layout.tooltip.is_some());
    assert_eq!((ring.width, ring.height), (16.0, 16.0));
    assert!((ring.left() + 8.0 - a.x).abs() <= 1.0);
    assert!((ring.top() + 8.0 - a.y).abs() <= 1.0);
    let z: Vec<i32> = resolved.scene.items.iter().map(|i| i.z).collect();
    let at = z.iter().position(|&z| z == Z_HOVER).expect("a ring item");
    const { assert!(Z_HOVER < Z_FOCUS && Z_FOCUS < Z_TOOLTIP) };
    let ItemKind::Rects(rects) = &resolved.scene.items[at].kind else {
        panic!("the ring is rectangles");
    };
    assert_eq!(rects.len(), 8);
    assert!(rects.iter().all(|r| r.rect.x.fract() == 0.0));
    // Off the rows: no ring.
    let _ = plot.handle(Input::pointer_moved((1.0, 1.0)));
    assert_eq!(plot.hovered(), None);
    let resolved = plot.resolve(&cx, W, H).unwrap();
    assert!(resolved.layout.hover.is_none());
    assert!(resolved.scene.items.iter().all(|i| i.z != Z_HOVER));
}

#[test]
fn keyboard_focus_draws_only_the_focus_ring() {
    let (cx, mut plot) = scatter();
    let a = plot.pick_anchor(row(1)).unwrap();
    let _ = plot.handle(Input::pointer_moved(a));
    assert!(plot.resolve(&cx, W, H).unwrap().layout.hover.is_some());
    assert!(plot.handle(Input::key(Key::Right)).redraw);
    assert!(plot.focused().is_some());
    let resolved = plot.resolve(&cx, W, H).unwrap();
    assert!(resolved.layout.focus.is_some() && resolved.layout.hover.is_none());
    assert!(resolved.scene.items.iter().all(|i| i.z != Z_HOVER));
}

#[test]
fn off_is_byte_identical_to_no_hover_ring() {
    let (cx, mut plot) = scatter();
    plot.hover_highlight(false);
    let a = plot.pick_anchor(row(1)).unwrap();
    let _ = plot.handle(Input::pointer_moved(a));
    let off = plot.resolve(&cx, W, H).unwrap().clone();
    assert!(off.layout.hover.is_none());
    assert!(off.scene.items.iter().all(|i| i.z != Z_HOVER));
    // The same scene as the default's, minus exactly the ring item.
    plot.hover_highlight(true);
    let on = plot.resolve(&cx, W, H).unwrap().clone();
    let strip = |r: &gup_core::Resolved| {
        r.scene
            .items
            .iter()
            .filter(|i| i.z != Z_HOVER)
            .map(|i| format!("{i:?}"))
            .collect::<Vec<_>>()
    };
    assert_eq!(on.scene.items.len(), off.scene.items.len() + 1);
    assert_eq!(strip(&on), strip(&off));
}

#[test]
fn the_setter_is_on_the_builders() {
    let cx = Context::new_blocking().expect("headless GPU context");
    let mut chart = gup_core::scatter(vec![(1.0, 2.0), (3.0, 4.0)])
        .x(|p| p.0)
        .y(|p| p.1)
        .tooltip(|p| format!("{p:?}"))
        .hover_highlight(false);
    let plot = chart.plot_mut();
    plot.resolve(&cx, W, H).unwrap();
    let a = plot.pick_anchor(row(0)).unwrap();
    let _ = plot.handle(Input::pointer_moved(a));
    assert_eq!(plot.hovered(), Some(row(0)));
    assert!(plot.resolve(&cx, W, H).unwrap().layout.hover.is_none());
    plot.hover_highlight(true);
    assert!(plot.resolve(&cx, W, H).unwrap().layout.hover.is_some());
}

fn luminance(c: Color) -> f64 {
    let [r, g, b, _] = c.to_rgba8();
    let lin = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn contrast(a: Color, b: Color) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// The ring's ink is the title colour and its edge the background colour, in
/// the dark and the high-contrast themes, and the ink holds 3:1.
#[test]
fn the_ring_colours_hold_3_to_1_in_dark_and_high_contrast() {
    for theme in [Theme::dark(), Theme::high_contrast()] {
        let (cx, mut plot) = scatter();
        plot.theme(theme.clone());
        let a = plot.pick_anchor(row(1)).unwrap();
        let _ = plot.handle(Input::pointer_moved(a));
        let (image, layout) = render(&cx, &mut plot);
        let ring = layout.hover.unwrap();
        let px = |x: f32, y: f32| image.get_pixel(x as u32, y as u32).0;
        let mid = ring.top() + 8.0;
        assert_eq!(px(ring.left() + 1.0, mid), theme.title.to_rgba8());
        assert_eq!(px(ring.left(), mid), theme.background.to_rgba8());
        let c = contrast(theme.title, theme.background);
        assert!(c >= 3.0, "{c}");
    }
}

#[test]
fn hover_highlight_scatter_golden() {
    let (cx, mut plot) = scatter();
    let a = plot.pick_anchor(row(2)).unwrap();
    let _ = plot.handle(Input::pointer_moved(a));
    assert_eq!(plot.hovered(), Some(row(2)));
    golden("gup_core/hover_highlight_scatter", &cx, &mut plot);
}

#[test]
fn hover_highlight_bar_golden() {
    let cx = Context::new_blocking().expect("headless GPU context");
    let rows = vec![("Q1", 12.0), ("Q2", 7.0), ("Q3", 3.0), ("Q4", 9.0)];
    let mut chart = gup_core::bar(rows)
        .x(|r| r.0)
        .y(|r| r.1)
        .tooltip(|r| format!("{} {}", r.0, r.1));
    let plot = chart.plot_mut();
    plot.resolve(&cx, W, H).unwrap();
    let a = plot.pick_anchor(row(1)).unwrap();
    let _ = plot.handle(Input::pointer_moved(a));
    assert_eq!(plot.hovered(), Some(row(1)));
    golden("gup_core/hover_highlight_bar", &cx, plot);
}
