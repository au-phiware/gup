// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The gup-core adapter for the GUP-388 visual regression harness: layout
//! metadata built from gup-core's own `Layout`, so the checks compare the
//! image with what gup-core intended to draw.

#![allow(dead_code)] // each includer uses a different subset

use gup_core::geom::Rect;
use gup_core::prelude::*;
use gup_core::{Layout, scene::TextRole};
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{
    ExpectedFailures, GoldenStore, Harness, LayoutMetadata, PxRect, Rgba8, TextRole as VrTextRole,
};
use std::path::Path;

pub fn rgba8(c: Color) -> Rgba8 {
    let [r, g, b, _] = c.to_rgba8();
    Rgba8::rgb(r, g, b)
}

pub fn rect(r: Rect) -> PxRect {
    PxRect::new(r.x, r.y, r.width, r.height)
}

pub fn role(role: TextRole) -> VrTextRole {
    match role {
        TextRole::Title => VrTextRole::Title,
        TextRole::TickLabel => VrTextRole::TickLabel,
        TextRole::Legend => VrTextRole::Legend,
    }
}

/// Everything from gup-core's own `Layout`: plot rect, background, text
/// and guides.
pub fn layout_metadata(layout: &Layout) -> LayoutMetadata {
    let mut meta = LayoutMetadata::new(rect(layout.plot)).with_background(Rgba8::WHITE);
    for t in &layout.texts {
        meta = meta.with_text(
            role(t.run.role),
            t.run.text.to_string(),
            rect(t.bounds),
            rgba8(t.run.style.color),
        );
    }
    for g in &layout.guides {
        // Pad by half a pixel for the edge pixels of snapped hairlines.
        meta = meta.with_guide(rect(*g).inflate(0.5));
    }
    meta
}

/// [`layout_metadata`] plus the extremes of the sequential fill: the
/// darkest and brightest points are drawn with exactly these colours at
/// their centres.
pub fn metadata(layout: &Layout, fill: &Sequential) -> LayoutMetadata {
    layout_metadata(layout)
        .with_expected_color(
            "fill at domain min (viridis start)",
            rgba8(fill.eval(f64::MIN)),
        )
        .with_expected_color(
            "fill at domain max (viridis end)",
            rgba8(fill.eval(f64::MAX)),
        )
}

/// The harness over the workspace's golden images and expected failures.
pub fn harness() -> Harness {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden = GoldenStore::new(root.join("tests/golden"), default_artifact_dir(&root));
    let expected =
        ExpectedFailures::load(root.join("tests/visual_regression/expected_failures.toml"))
            .expect("expected-failure list parses");
    Harness::new(golden, expected)
}
