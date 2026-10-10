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
    Anchor, GradientBar, GradientDirection, HAlign, Item, ItemKind, RectPrim, Rule, TextRole,
    TextRun, TextStyle, VAlign, Z_GRID, Z_TITLE,
};
use gup_core::{Layout, Scene};
use gup_core::{Ramp, Swatch};
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

/// Colour legends drawn from a scale's `Legend` (RFC-001 S5b), in the
/// scene items gup-core has today (guides placing them are RFC-001 S7):
/// what each piece is, so harness metadata can describe it.
pub struct Placed {
    /// Swatches, bars and tick rules: guide regions for the harness.
    pub guides: Vec<Rect>,
    /// Labels, with their roles and colours.
    pub labels: Vec<TextRun>,
}

const LEGEND_INK: Color = Color::hex(0x333333);
const SWATCH: f32 = 12.0;
const SWATCH_ROW: f32 = 20.0;

fn legend_label(text: &str, at: Point, anchor: Anchor) -> TextRun {
    TextRun {
        text: text.into(),
        at,
        anchor,
        style: TextStyle {
            size: Px(LEGEND_SIZE),
            color: LEGEND_INK,
        },
        role: TextRole::Legend,
    }
}

/// A swatch legend: one `SWATCH`-pixel square per key, top-down from
/// `at`, with the key's label to its right.
pub fn swatches(scene: &mut Scene, swatches: &[Swatch], at: Point) -> Placed {
    let mut rects = Vec::new();
    let mut labels = Vec::new();
    for (i, s) in swatches.iter().enumerate() {
        let top = at.y + i as f32 * SWATCH_ROW;
        let rect = Rect::new(at.x, top, SWATCH, SWATCH);
        rects.push(RectPrim {
            rect,
            color: s.color,
        });
        labels.push(legend_label(
            &s.label,
            Point::new(at.x + SWATCH + 6.0, top + SWATCH / 2.0),
            Anchor::new(HAlign::Start, VAlign::Middle),
        ));
    }
    let guides = rects.iter().map(|r| r.rect).collect();
    scene.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Rects(rects),
    });
    scene.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Text(labels.clone()),
    });
    Placed { guides, labels }
}

/// A vertical ramp legend in `bar`: the gradient (low end at the bottom),
/// a short tick to the right of the bar at each of the ramp's ticks, and
/// its label.
pub fn ramp(scene: &mut Scene, ramp: &Ramp, bar: Rect) -> Placed {
    scene.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Gradient(GradientBar::new(ramp, bar, GradientDirection::Vertical)),
    });
    let mut rules = Vec::new();
    let mut labels = Vec::new();
    for tick in ramp.ticks() {
        let y = bar.bottom() - tick.t as f32 * bar.height;
        let y = y.floor() + 0.5;
        rules.push(Rule {
            p0: Point::new(bar.right(), y),
            p1: Point::new(bar.right() + 4.0, y),
            width: Px(1.0),
            color: LEGEND_INK,
        });
        labels.push(legend_label(
            &tick.label,
            Point::new(bar.right() + 7.0, y),
            Anchor::new(HAlign::Start, VAlign::Middle),
        ));
    }
    let mut guides = vec![bar];
    guides.extend(rules.iter().map(Rule::bounds));
    scene.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Rules(rules),
    });
    scene.push(Item {
        z: Z_TITLE,
        clip: None,
        kind: ItemKind::Text(labels.clone()),
    });
    Placed { guides, labels }
}

/// `meta` plus a placed legend's guide regions and labels.
pub fn with_legend(mut meta: LayoutMetadata, placed: &Placed) -> LayoutMetadata {
    let font = gup_text::Font::inter();
    for g in &placed.guides {
        meta = meta.with_guide(rect(*g).inflate(0.5));
    }
    for run in &placed.labels {
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
