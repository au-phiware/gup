// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-580: fallback fonts through `Context::add_fallback_font`: the
//! `gup_core/fallback_title` golden (Cyrillic, CJK and Latin in one title),
//! laziness (a fallback no text needs is never parsed), and a fallback
//! added after a resolve taking effect on the next one. Each test has its
//! own context, never `Context::shared()`, so no other test sees the
//! fallbacks.

mod common;

use common::scatter::{self, HEIGHT, WIDTH};
use common::svg_marks;
use common::vr::{harness, metadata};
use gup_core::prelude::*;
use gup_core::scene::TextRole;
use gup_core::{Font, Layout};
use gup_visual_regression::RgbaImage;

/// A Noto Sans JP subset with "東" and "京" (see its README).
const JP: &[u8] = include_bytes!("../../gup-text/tests/fonts/NotoSansJP-Test.ttf");

fn jp() -> Font {
    Font::from_bytes("Noto Sans JP", JP).unwrap()
}

const TITLE: &str = "Київ — 東京 — Zürich";

#[test]
fn a_mixed_script_title_draws_with_fallbacks_on_screen_and_in_the_svg() {
    // The primary lacks six characters, so the golden shows the fallbacks.
    assert_eq!(
        Font::inter().missing_glyphs(TITLE),
        ['К', 'и', 'ї', 'в', '東', '京']
    );
    let cx = Context::new_blocking().expect("headless GPU context");
    cx.add_fallback_font(Font::inter_full());
    cx.add_fallback_font(jp());
    assert!(
        Font::inter()
            .with_fallbacks(&cx.fallback_fonts())
            .missing_glyphs(TITLE)
            .is_empty()
    );
    let mut plot = scatter::plot();
    plot.title(TITLE);
    let (image, layout) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    let scene = plot
        .resolve(&cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .scene
        .clone();
    assert_eq!(scene.fallback_fonts.len(), 2);

    let vr = RgbaImage::new(WIDTH, HEIGHT, image.into_raw()).unwrap();
    let meta = metadata(&layout, &scatter::fill());
    harness()
        .run("gup_core/fallback_title", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    let svg = svg_marks::assert_matches("gup_core/fallback_title", &scene, &vr, &meta);
    assert!(
        svg.contains(
            r#"font-family="Inter, &quot;Inter 1&quot;, &quot;Noto Sans JP&quot;, sans-serif""#
        ),
        "{svg}"
    );
}

/// AC2: covered text never parses the fallback; text it lacks does.
#[test]
fn a_fallback_is_parsed_only_when_a_character_needs_it() {
    let cx = Context::new_blocking().unwrap();
    let jp = jp();
    cx.add_fallback_font(jp.clone());
    let mut plot = scatter::plot();
    plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    assert!(!jp.is_parsed(), "parsed for text Inter covers");
    plot.title("東京");
    plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    assert!(jp.is_parsed());
}

fn ink(layout: &Layout, role: TextRole) -> (f32, f32) {
    let t = layout
        .texts
        .iter()
        .find(|t| t.run.role == role && &*t.run.text == "東京")
        .unwrap_or_else(|| panic!("no {role:?} text 東京"));
    (t.bounds.width, t.run.style.size.0)
}

/// AC1: a fallback added after a resolve applies to the next one, with
/// nothing else changed: the title's and a label's runs go from two
/// missing-glyph boxes to two glyphs of about one em each.
#[test]
fn a_fallback_added_after_a_resolve_applies_to_the_next() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.legends(false);
    plot.title("東京");
    plot.x(Linear::new().domain(0.0, 10.0));
    plot.y(Linear::new().domain(0.0, 10.0));
    plot.circles(vec![(5.0, 5.0)])
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1);
    plot.text(vec![(5.0, 5.0)])
        .x(|r: &(f64, f64)| r.0)
        .y(|r: &(f64, f64)| r.1)
        .text(|_: &(f64, f64)| "東京".to_string());
    plot.resolve(&cx, 480.0, 320.0).unwrap();
    let before = plot.layout().unwrap().clone();
    let (title0, label0) = (
        ink(&before, TextRole::Title),
        ink(&before, TextRole::Annotation),
    );

    cx.add_fallback_font(jp());
    plot.resolve(&cx, 480.0, 320.0).unwrap();
    let after = plot.layout().unwrap().clone();
    for (role, was) in [(TextRole::Title, title0), (TextRole::Annotation, label0)] {
        let (width, size) = ink(&after, role);
        assert_ne!(width, was.0, "{role:?} kept its boxes");
        assert!(
            (width - 2.0 * size).abs() <= 0.12 * 2.0 * size,
            "{role:?}: ink {width} for size {size}"
        );
    }
}
