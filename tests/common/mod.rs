// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Helpers shared by the golden-image test files (`tutorials.rs`,
//! `gallery.rs`): render a chart, run the harness's structural checks and
//! compare with the golden.

#![allow(dead_code)]

use gup::prelude::*;
use gup::scene::TextRole;
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{
    ExpectedFailures, GoldenStore, Harness, LayoutMetadata, PxRect, Rgba8, RgbaImage,
    TextRole as VrTextRole,
};
use std::path::Path;

fn rect(r: gup::geom::Rect) -> PxRect {
    PxRect::new(r.x, r.y, r.width, r.height)
}

/// Layout metadata from gup's own `Layout` (a copy of the adapter in
/// `crates/gup-core/tests/common/vr.rs`, which this crate cannot import).
pub fn layout_metadata(layout: &gup::Layout, background: Rgba8) -> LayoutMetadata {
    let mut meta = LayoutMetadata::new(rect(layout.plot)).with_background(background);
    for t in &layout.texts {
        let role = match t.run.role {
            TextRole::Title => VrTextRole::Title,
            TextRole::TickLabel => VrTextRole::TickLabel,
            TextRole::AxisTitle => VrTextRole::AxisTitle,
            TextRole::Legend => VrTextRole::Legend,
            TextRole::Subtitle | TextRole::Tooltip | TextRole::Annotation => VrTextRole::Other,
        };
        let [r, g, b, _] = t.run.style.color.to_rgba8();
        meta = meta.with_text(
            role,
            t.run.text.to_string(),
            rect(t.bounds),
            Rgba8::rgb(r, g, b),
        );
    }
    for g in &layout.guides {
        meta = meta.with_guide(rect(*g).inflate(0.5));
    }
    meta
}

pub fn harness() -> Harness {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let golden = GoldenStore::new(root.join("tests/golden"), default_artifact_dir(root));
    let expected =
        ExpectedFailures::load(root.join("tests/visual_regression/expected_failures.toml"))
            .expect("expected-failure list parses");
    Harness::new(golden, expected)
}

/// Render `chart` at `width` x `height` on white and check it against the
/// golden `name`.
pub fn check(name: &str, chart: &mut impl Chart, width: u32, height: u32) {
    check_on(name, chart, width, height, Rgba8::WHITE);
}

/// Like [`check`], for a chart drawn on `background`.
pub fn check_on(name: &str, chart: &mut impl Chart, width: u32, height: u32, background: Rgba8) {
    let cx = Context::shared().unwrap();
    let (image, layout) = chart.render_resolved(&cx, width, height).unwrap();
    assert_eq!(image.dimensions(), (width, height));
    let image = RgbaImage::new(width, height, image.into_raw()).unwrap();
    harness()
        .run(name, Ok((image, layout_metadata(&layout, background))))
        .assert_ok();
}
