// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `SvgTarget` for guides (RFC-001 §7, GUP-401 AC5). The legend scene
//! (scatter guides, a clipped plot background, a gradient legend) is
//! written as SVG, its elements are read back, and the SVG is rasterised
//! with resvg (with only the bundled Inter loaded, so the `Inter` family
//! name must resolve) and compared with the GPU render of the same
//! guides-only scene: the GUP-388 structural checks with gup-core's own
//! layout, a golden of the rasterised SVG, and a perceptual diff against
//! the PNG.

mod common;

use common::legend::{self, legend_metadata};
use common::scatter::{HEIGHT, WIDTH};
use common::vr::harness;
use gup_core::scene::{ItemKind, TextRole};
use gup_core::{Context, ImageTarget, SvgTarget, VectorTarget};
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{DiffTolerance, RgbaImage, diff::perceptual_diff};
use std::path::Path;

fn rasterise(svg: &str) -> RgbaImage {
    use resvg::{tiny_skia, usvg};
    let mut opt = usvg::Options::default();
    opt.fontdb_mut()
        .load_font_data(gup_text::INTER_REGULAR.to_vec());
    let tree = usvg::Tree::from_str(svg, &opt).expect("SvgTarget output parses");
    let size = tree.size().to_int_size();
    assert_eq!((size.width(), size.height()), (WIDTH, HEIGHT));
    let mut pixmap = tiny_skia::Pixmap::new(WIDTH, HEIGHT).unwrap();
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    let data = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    RgbaImage::new(WIDTH, HEIGHT, data).unwrap()
}

/// Every `<tag …>…</tag>` text content, in order.
fn contents<'a>(svg: &'a str, tag: &str) -> Vec<&'a str> {
    svg.match_indices(&format!("<{tag} "))
        .map(|(i, _)| {
            let rest = &svg[i..];
            let start = rest.find('>').unwrap() + 1;
            let end = rest.find(&format!("</{tag}>")).unwrap();
            &rest[start..end]
        })
        .collect()
}

#[test]
fn guides_svg_matches_the_png() {
    let cx = Context::new_blocking().unwrap();
    let s = legend::scene(&cx);
    let mut target = SvgTarget::new();

    // Marks are refused, not dropped.
    let err = target.render(&s.scene).unwrap_err().to_string();
    assert!(
        err.contains("1 data layer") && err.contains("scene.guides()"),
        "{err}"
    );
    assert!(target.svg().is_empty(), "a refused scene wrote something");

    let guides = s.scene.guides();
    target.render(&guides).unwrap();
    let svg = target.svg();
    let artifacts =
        default_artifact_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).join("gup_core");
    target.save(artifacts.join("scene_guides.svg")).unwrap();

    // Every rule is a <line>, every text run a <text> with its string.
    let rules = guides.rules().count();
    assert_eq!(svg.matches("<line ").count(), rules);
    let texts: Vec<String> = guides.text_runs().map(|r| r.text.to_string()).collect();
    assert_eq!(contents(svg, "text"), texts);
    assert!(texts.iter().any(|t| t == common::scatter::TITLE));
    let count = |role| guides.text_runs().filter(|r| r.role == role).count();
    assert_eq!(count(TextRole::Legend), 2);
    assert!(count(TextRole::TickLabel) >= 8);
    assert!(svg.contains(r#"font-family="Inter, sans-serif""#));
    // The clipped background and the legend.
    assert!(svg.contains(r#"<g clip-path="url(#clip1)">"#), "{svg}");
    assert_eq!(svg.matches("<linearGradient ").count(), 1);
    assert!(
        guides
            .items
            .iter()
            .all(|i| !matches!(i.kind, ItemKind::Marks(_)))
    );

    // The rasterised SVG passes the structural checks against gup-core's
    // own layout (text where layout put it, nothing stray outside the
    // plot, background and viridis legend colours present).
    let raster = rasterise(svg);
    let png = ImageTarget::new(&cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(&cx, &guides)
        .unwrap();
    let png = RgbaImage::new(WIDTH, HEIGHT, png.into_raw()).unwrap();
    png.save_png(artifacts.join("scene_guides_gpu.png"))
        .unwrap();
    // The SVG adapter's text regions are gup-core's ink boxes plus one
    // pixel: the GPU draws fontdue bitmaps placed on whole pixels (the
    // ink boxes are exactly those bitmaps), while resvg rasterises the
    // outlines at their sub-pixel positions, which can add one faint
    // anti-aliased column or row. Both snap the pen start and baseline
    // identically (without that snap, 151 pixels fell outside; with it,
    // 23, all within one pixel of a box).
    let mut meta = legend_metadata(&s);
    for t in &mut meta.text_regions {
        t.rect = t.rect.inflate(1.0);
    }
    harness()
        .run(
            "gup_core/scene_guides_svg",
            Ok((raster.clone(), meta.clone())),
        )
        .assert_ok();

    // And it looks like the GPU render of the same scene. Whole image:
    // only glyph pixels differ (two rasterisers), so outside the text
    // regions every pixel must match, rules, rects, clip and the 64-stop
    // gradient included...
    let whole = perceptual_diff(&raster, &png, &DiffTolerance::default()).unwrap();
    eprintln!("SVG raster vs GPU PNG, whole image: {whole:?}");
    let in_text = |x: u32, y: u32| {
        meta.text_regions
            .iter()
            .any(|t| t.rect.contains_pixel(x, y))
    };
    let (mut outside, mut worst) = (0usize, 0f32);
    for (x, y, p) in png.pixels() {
        if !in_text(x, y) {
            let de = p.delta_e(raster.pixel(x, y));
            worst = worst.max(de);
            outside += usize::from(de > 3.0);
        }
    }
    eprintln!("outside text: {outside} pixels over ΔE 3, max ΔE {worst:.2}");
    assert_eq!(
        outside, 0,
        "SVG and PNG differ outside text (max ΔE {worst:.2})"
    );
    // ...and each label carries about the same ink in both.
    let ink = |img: &RgbaImage, r: &gup_visual_regression::PxRect| -> f64 {
        img.pixels()
            .filter(|&(x, y, _)| r.contains_pixel(x, y))
            .map(|(_, _, p)| 255.0 - (f64::from(p.r) + f64::from(p.g) + f64::from(p.b)) / 3.0)
            .sum()
    };
    let mut ratios = Vec::new();
    for t in &meta.text_regions {
        let (a, b) = (ink(&png, &t.rect), ink(&raster, &t.rect));
        let ratio = b / a;
        ratios.push(ratio);
        assert!(
            (0.8..=1.25).contains(&ratio),
            "{:?}: SVG ink {b:.0} vs PNG {a:.0} (×{ratio:.2})",
            t.text
        );
    }
    ratios.sort_by(f64::total_cmp);
    eprintln!(
        "SVG/PNG ink per label: min ×{:.3}, max ×{:.3} over {} labels",
        ratios[0],
        ratios[ratios.len() - 1],
        ratios.len()
    );
}
