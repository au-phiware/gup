// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ItemKind::Rects` and `ItemKind::Gradient` (RFC-001 §7, GUP-401 AC4):
//! the reference scatter with a tinted plot background behind the marks
//! and a vertical colour-legend bar for its fill scale beside the plot,
//! drawn in the same single pass and checked by the GUP-388 harness.

mod common;

use common::scatter::{self, HEIGHT, WIDTH};
use common::vr::{harness, metadata, rect, rgba8, role};
use gup_core::geom::{Point, Rect};
use gup_core::prelude::*;
use gup_core::scene::{
    Anchor, GradientBar, GradientDirection, HAlign, Item, ItemKind, RectPrim, TextRole, TextRun,
    TextStyle, VAlign, Z_GRID, Z_TITLE,
};
use gup_core::{ImageTarget, Layout, Scene};
use gup_visual_regression::{LayoutMetadata, Rgba8, RgbaImage};

/// Logical width left for the plot; the legend takes the rest.
const PLOT_WIDTH: f32 = 650.0;
const PLOT_BACKGROUND: Color = Color::hex(0xeef1f6);
const LEGEND_SIZE: f32 = 12.0;

/// The scatter, resolved narrower than the image, with a background rect
/// under the plot and a legend bar (plus its two end labels) to its right.
fn scene(cx: &Context) -> (Scene, Layout, Rect, Vec<TextRun>) {
    let resolved = scatter::plot()
        .resolve(cx, PLOT_WIDTH, HEIGHT as f32)
        .unwrap();
    let (mut scene, layout) = (resolved.scene, resolved.layout);
    scene.width = WIDTH as f32;
    scene.push(Item {
        z: Z_GRID,
        clip: None,
        kind: ItemKind::Rects(vec![RectPrim {
            rect: layout.plot,
            color: PLOT_BACKGROUND,
        }]),
    });

    let fill = scatter::fill();
    let bar = Rect::from_edges(
        PLOT_WIDTH + 6.0,
        layout.plot.top(),
        PLOT_WIDTH + 20.0,
        layout.plot.bottom(),
    );
    scene.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Gradient(GradientBar::sequential(
            &fill,
            bar,
            GradientDirection::Vertical,
        )),
    });
    let (lo, hi) = fill.current_domain().unwrap();
    let label = |value: f64, y: f32, v: VAlign| TextRun {
        text: format!("{value:.0}").into(),
        at: Point::new(bar.right() + 4.0, y),
        anchor: Anchor::new(HAlign::Start, v),
        style: TextStyle {
            size: Px(LEGEND_SIZE),
            color: Color::hex(0x333333),
        },
        role: TextRole::Legend,
    };
    let labels = vec![
        label(hi, bar.top(), VAlign::Top),
        label(lo, bar.bottom(), VAlign::Baseline),
    ];
    scene.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Text(labels.clone()),
    });
    (scene, layout, bar, labels)
}

#[test]
fn background_rect_and_gradient_legend() {
    let cx = Context::new_blocking().unwrap();
    let (scene, layout, bar, labels) = scene(&cx);
    let image = ImageTarget::new(&cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(&cx, &scene)
        .unwrap();
    let fill = scatter::fill();
    let px = |x: f32, y: f32| {
        let p = image.get_pixel(x as u32, y as u32).0;
        Rgba8::rgb(p[0], p[1], p[2])
    };
    let close = |what: &str, got: Rgba8, want: Rgba8| {
        let de = got.delta_e(want);
        assert!(de < 2.0, "{what}: got {got:?}, want {want:?} (ΔE {de:.2})");
    };

    // The background is drawn under the plot, and only there.
    let p = layout.plot;
    close(
        "plot background",
        px(p.left() + 2.0, p.top() + 2.0),
        rgba8(PLOT_BACKGROUND),
    );
    close("outside the plot", px(p.left() - 30.0, 3.0), Rgba8::WHITE);

    // The legend's ends are the fill scale's domain extremes: viridis
    // start at the bottom (domain min), end at the top (domain max).
    let mid = bar.x + bar.width / 2.0;
    let (lo, hi) = fill.current_domain().unwrap();
    close(
        "legend bottom (domain min)",
        px(mid, bar.bottom() - 0.5),
        rgba8(fill.eval(lo)),
    );
    close(
        "legend top (domain max)",
        px(mid, bar.top() + 0.5),
        rgba8(fill.eval(hi)),
    );
    close(
        "legend middle",
        px(mid, bar.y + bar.height / 2.0),
        rgba8(fill.eval((lo + hi) / 2.0)),
    );

    // The harness: legend bar and labels are declared, so nothing drawn
    // outside the plot is unaccounted for.
    let font = gup_text::Font::inter();
    let mut meta: LayoutMetadata = metadata(&layout, &fill)
        .with_guide(rect(bar).inflate(0.5))
        .with_expected_color("plot background", rgba8(PLOT_BACKGROUND));
    for run in &labels {
        let ink = font.ink_bounds(&gup_text::Run {
            text: &run.text,
            size: run.style.size.0,
            at: [run.at.x, run.at.y],
            anchor: run.anchor,
        });
        meta = meta.with_text(
            role(run.role),
            run.text.to_string(),
            rect(Rect::new(ink.x, ink.y, ink.width, ink.height)),
            rgba8(run.style.color),
        );
    }
    let vr = RgbaImage::new(WIDTH, HEIGHT, image.into_raw()).unwrap();
    harness()
        .run("gup_core/scene_items", Ok((vr, meta)))
        .assert_ok();
}

/// The bar samples the scale's own LUT, so a reversed scale reverses the
/// legend too.
#[test]
fn reversed_scale_reverses_the_legend() {
    let fill = scatter::fill();
    let bar = |s: &Sequential| {
        GradientBar::sequential(
            s,
            Rect::new(0.0, 0.0, 10.0, 100.0),
            GradientDirection::Vertical,
        )
    };
    let (lo, hi) = fill.current_domain().unwrap();
    let forward = bar(&fill);
    assert_eq!(forward.color_at(0.0), fill.eval(lo));
    assert_eq!(forward.color_at(1.0), fill.eval(hi));
    let reversed = fill.clone().reversed();
    let backward = bar(&reversed);
    assert_eq!(backward.color_at(0.0), reversed.eval(lo));
    assert_eq!(backward.color_at(0.0), fill.eval(hi));
}
