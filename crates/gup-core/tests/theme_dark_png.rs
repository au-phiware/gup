// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-553: `Theme::dark()` end to end: the nine-category scatter (so the
//! ninth, generated colour shows) with legend, grid, title, tooltip and a
//! brush, and the theme palette's upload behaviour.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::scene::ItemKind;
use gup_core::{Button, Chart as _, Input, RowId};
use gup_visual_regression::RgbaImage;

const W: u32 = 640;
const H: u32 = 400;

#[derive(Clone)]
struct Obs {
    group: &'static str,
    x: f64,
    y: f64,
}

const GROUPS: [&str; 9] = [
    "Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta", "Iota",
];

/// Nine clusters of eight points, laid out deterministically.
fn observations() -> Vec<Obs> {
    let mut rows = Vec::new();
    for (g, group) in GROUPS.iter().enumerate() {
        let (cx, cy) = ((g % 3) as f64 * 4.0 + 2.0, (g / 3) as f64 * 4.0 + 2.0);
        for k in 0..8 {
            let a = f64::from(k) * std::f64::consts::TAU / 8.0 + g as f64;
            let r = 0.8 + 0.3 * f64::from(k % 3);
            rows.push(Obs {
                group,
                x: cx + r * a.cos(),
                y: cy + r * a.sin(),
            });
        }
    }
    rows
}

type Chart = gup_core::ScatterChart<Obs>;

fn chart() -> Chart {
    gup_core::scatter(observations())
        .x(|o| o.x)
        .y(|o| o.y)
        .color(|o| o.group)
        .radius(6.0)
        .title("Nine groups, dark")
        .x_title("x")
        .y_title("y")
        .legend_title("Group")
        .grid(Axes::Both)
        .theme(Theme::dark())
        .tooltip(|o| format!("{}\n({:.1}, {:.1})", o.group, o.x, o.y))
}

fn render_golden(name: &str, chart: &mut Chart, cx: &Context, svg: bool) {
    let resolved = chart.resolve(cx, W as f32, H as f32).unwrap().clone();
    let image = gup_core::ImageTarget::new(cx, W, H)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(W, H, image.into_raw()).unwrap();
    let dark = Theme::dark();
    let mut meta = layout_metadata(&resolved.layout)
        .with_background(rgba8(dark.background))
        .with_expected_color("first series", rgba8(dark.palette[0]));
    // A tooltip's text is not in the layout: its box is a text region.
    for item in &resolved.scene.items {
        if let ItemKind::Rects(rects) = &item.kind {
            for r in rects.iter().filter(|r| r.color == dark.tooltip_background) {
                meta = meta.with_text(
                    gup_visual_regression::TextRole::Other,
                    "tooltip",
                    common::vr::rect(r.rect),
                    rgba8(dark.tooltip_text),
                );
            }
        }
    }
    harness()
        .run(name, Ok((vr.clone(), meta.clone())))
        .assert_ok();
    // The SVG target does not dim unbrushed rows (a GPU-side effect).
    if svg {
        svg_marks::assert_matches(name, &resolved.scene, &vr, &meta);
    }
}

/// A hovered point's tooltip, drawn dark.
#[test]
fn dark_theme_golden() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = chart();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    let anchor = chart
        .plot_mut()
        .pick_anchor(RowId { layer: 0, row: 36 })
        .expect("an anchor");
    assert!(chart.handle(Input::PointerMoved { at: anchor }).redraw);
    let resolved = chart.resolve(&cx, W as f32, H as f32).unwrap();
    let shown = resolved.scene.items.iter().any(|i| {
        matches!(&i.kind, ItemKind::Text(runs) if runs.iter().any(|r| r.text.starts_with("Epsilon")))
    });
    assert!(shown, "the tooltip of row 36");
    render_golden("gup_core/theme_dark", &mut chart, &cx, true);
}

/// A brush over part of the chart: its fill and border are the theme's,
/// and the rows outside it fade.
#[test]
fn dark_theme_brush_golden() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = chart();
    chart.plot_mut().drag(Drag::Brush);
    let resolved = chart.resolve(&cx, W as f32, H as f32).unwrap();
    let r = resolved.layout.plot;
    let (a, b) = (
        Point::new(r.x + 0.30 * r.width, r.y + 0.15 * r.height),
        Point::new(r.x + 0.75 * r.width, r.y + 0.65 * r.height),
    );
    let button = Button::Primary;
    let _ = chart.handle(Input::PointerDown { at: a, button });
    let _ = chart.handle(Input::PointerMoved {
        at: Point::new(a.x + 20.0, a.y + 20.0),
    });
    let _ = chart.handle(Input::PointerMoved { at: b });
    let _ = chart.handle(Input::PointerUp { at: b, button });
    assert!(chart.plot_mut().brush().is_some_and(|b| !b.rows.is_empty()));
    render_golden("gup_core/theme_dark_brush", &mut chart, &cx, false);
}

/// AC2: applying the same theme again rebuilds nothing and writes no
/// texture; a changed theme writes the new palette once.
#[test]
fn the_same_theme_again_writes_no_texture() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = chart();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    let before = cx.upload_stats();
    chart.plot_mut().theme(Theme::dark());
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    let delta = cx.upload_stats() - before;
    assert_eq!(delta.textures, Default::default(), "{delta:?}");

    // A different background changes the generated colours: one rewrite.
    chart.plot_mut().theme(Theme {
        background: Color::hex(0x000000),
        ..Theme::dark()
    });
    let before = cx.upload_stats();
    chart.resolve(&cx, W as f32, H as f32).unwrap();
    let delta = cx.upload_stats() - before;
    assert_ne!(delta.textures, Default::default(), "{delta:?}");
}
