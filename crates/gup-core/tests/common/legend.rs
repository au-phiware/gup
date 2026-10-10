// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The reference scatter with a tinted plot background (a clipped
//! `ItemKind::Rects`) and a vertical colour legend for its fill scale
//! (`ItemKind::Gradient` plus two `TextRole::Legend` labels) to its right.
//! Shared by the PNG golden (`tests/scene_items.rs`) and the SVG test
//! (`tests/svg.rs`), so both targets draw the same scene.

#![allow(dead_code)] // each includer uses a different subset

use super::scatter::{self, HEIGHT, WIDTH};
use super::vr::{metadata, rect, rgba8, role};
use gup_core::geom::{Point, Rect};
use gup_core::prelude::*;
use gup_core::scene::{
    Anchor, GradientBar, GradientDirection, HAlign, Item, ItemKind, RectPrim, TextRole, TextRun,
    TextStyle, VAlign, Z_GRID, Z_TITLE,
};
use gup_core::{Layout, Scene};
use gup_visual_regression::LayoutMetadata;

/// Logical width left for the plot; the legend takes the rest.
pub const PLOT_WIDTH: f32 = 650.0;
/// The plot background's colour.
pub const PLOT_BACKGROUND: Color = Color::hex(0xeef1f6);
const LEGEND_SIZE: f32 = 12.0;

/// The scene and where its parts went.
pub struct LegendScene {
    pub scene: Scene,
    /// The scatter's own layout (plot rect, ticks, title).
    pub layout: Layout,
    /// The legend bar.
    pub bar: Rect,
    /// The legend's end labels (domain max at the top, min at the bottom).
    pub labels: Vec<TextRun>,
}

/// The scatter, resolved narrower than the image, with the background
/// under the plot and the legend in the strip to its right.
pub fn scene(cx: &Context) -> LegendScene {
    let resolved = scatter::plot()
        .resolve(cx, PLOT_WIDTH, HEIGHT as f32)
        .unwrap();
    let (mut scene, layout) = (resolved.scene, resolved.layout);
    scene.width = WIDTH as f32;
    let clip = scene.add_clip(layout.plot);
    scene.push(Item {
        z: Z_GRID,
        clip: Some(clip),
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
        kind: ItemKind::Gradient(GradientBar::new(
            &fill.ramp(),
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
    LegendScene {
        scene,
        layout,
        bar,
        labels,
    }
}

/// Harness metadata: the scatter's layout plus the legend bar (as a guide
/// region), its labels and the background colour.
pub fn legend_metadata(s: &LegendScene) -> LayoutMetadata {
    let font = gup_text::Font::inter();
    let mut meta = metadata(&s.layout, &scatter::fill())
        .with_guide(rect(s.bar).inflate(0.5))
        .with_expected_color("plot background", rgba8(PLOT_BACKGROUND));
    for run in &s.labels {
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
    meta
}
