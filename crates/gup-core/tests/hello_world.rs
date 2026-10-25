// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S8e (GUP-448): the chart of `examples/hello_world.rs`, headless,
//! with the pointer on row 30 (mid-plot, so the tooltip stays inside) so the tooltip shows.

mod common;

use common::vr::{harness, layout_metadata};
use gup_core::prelude::*;
use gup_core::scene::ItemKind;
use gup_core::{ImageTarget, Input, RowId};
use gup_visual_regression::RgbaImage;

const W: u32 = 640;
const H: u32 = 400;

#[test]
fn hello_world_golden() {
    let cx = Context::new_blocking().expect("headless GPU context");
    let points = (0..50).map(|i| f64::from(i) / 5.0).map(|x| (x, x.sin()));
    let mut plot = Plot::new();
    plot.circles(points)
        .x(|p| p.0)
        .y(|p| p.1)
        .tooltip(|p| format!("sin({:.1}) = {:.2}", p.0, p.1));
    plot.resolve(&cx, W as f32, H as f32).unwrap();
    let at = plot
        .pick_anchor(RowId { layer: 0, row: 30 })
        .expect("row 30 has an anchor");
    eprintln!("hello_world pointer anchor: {},{}", at.x, at.y);
    assert!(plot.handle(Input::PointerMoved { at }).redraw);
    let resolved = plot.resolve(&cx, W as f32, H as f32).unwrap();
    let has_tooltip = resolved.scene.items.iter().any(|i| {
        matches!(&i.kind, ItemKind::Text(runs) if runs.iter().any(|r| &*r.text == "sin(6.0) = -0.28"))
    });
    assert!(has_tooltip, "the scene has the tooltip text");
    let layout = resolved.layout.clone();
    let image = ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(W, H, image.into_raw()).unwrap();
    harness()
        .run("gup_core/hello_world", Ok((vr, layout_metadata(&layout))))
        .assert_ok();
}
