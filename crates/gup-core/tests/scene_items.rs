// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `ItemKind::Rects` and `ItemKind::Gradient` (RFC-001 §7, GUP-401 AC4):
//! the reference scatter with a tinted plot background behind the marks
//! and a vertical colour-legend bar for its fill scale beside the plot,
//! drawn in the same single pass and checked by the GUP-388 harness.

mod common;

use common::legend::{self, PLOT_BACKGROUND, legend_metadata};
use common::scatter::{self, HEIGHT, WIDTH};
use common::vr::{harness, rgba8};
use gup_core::ImageTarget;
use gup_core::geom::Rect;
use gup_core::prelude::*;
use gup_core::scene::{GradientBar, GradientDirection};
use gup_visual_regression::{Rgba8, RgbaImage};

#[test]
fn background_rect_and_gradient_legend() {
    let cx = Context::new_blocking().unwrap();
    let s = legend::scene(&cx);
    let image = ImageTarget::new(&cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(&cx, &s.scene)
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
    let p = s.layout.plot;
    close(
        "plot background",
        px(p.left() + 2.0, p.top() + 2.0),
        rgba8(PLOT_BACKGROUND),
    );
    close("outside the plot", px(p.left() - 30.0, 3.0), Rgba8::WHITE);

    // The legend's ends are the fill scale's domain extremes: viridis
    // start at the bottom (domain min), end at the top (domain max).
    let bar = s.bar;
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
    let meta = legend_metadata(&s);
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
        GradientBar::new(
            &s.ramp(),
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
