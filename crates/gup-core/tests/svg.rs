// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `SvgTarget` for guides (RFC-001 §7, GUP-401 AC5). The legend scene
//! (scatter guides, a clipped plot background, a gradient legend) is
//! written as SVG, its elements are read back, and the SVG is rasterised
//! with resvg (with only the bundled Inter loaded, so the `Inter` family
//! name must resolve) and compared with the GPU render of the same
//! guides-only scene: the GUP-388 structural checks with gup-core's own
//! layout, a golden of the rasterised SVG, and a perceptual diff against
//! the PNG. A second test embeds the font and checks the `@font-face`.

mod common;

use common::legend::{self, legend_metadata};
use common::scatter::{HEIGHT, WIDTH};
use common::vr::harness;
use gup_core::geom::Point;
use gup_core::scene::{
    Anchor, HAlign, Item, ItemKind, TextRole, TextRun, TextStyle, VAlign, Weight,
};
use gup_core::{Color, Context, ImageTarget, Px, Scene, SvgOptions, SvgTarget, VectorTarget};
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{DiffTolerance, RgbaImage, diff::perceptual_diff};
use std::path::Path;

fn rasterise(svg: &str) -> RgbaImage {
    rasterise_with(svg, gup_text::INTER_REGULAR)
}

/// Rasterise with only `font` loaded.
fn rasterise_with(svg: &str, font: &[u8]) -> RgbaImage {
    use resvg::{tiny_skia, usvg};
    let mut opt = usvg::Options::default();
    opt.fontdb_mut().load_font_data(font.to_vec());
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

    // The whole scene writes its data too (GUP-467); this test is about
    // the guides, which `Scene::guides` writes alone, so the GPU render
    // below, of the guides only, is what the SVG is compared with.
    let mut full = SvgTarget::new();
    full.render(&s.scene).unwrap();
    assert!(full.svg().contains("<circle "), "the marks are written");

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

/// GUP-407: with `embed_font`, the document declares the bundled Inter
/// subset as an `@font-face` data URL and is otherwise the same document.
/// resvg ignores web fonts, so the round trip decodes the embedded file,
/// gives resvg only that, and expects the unembedded rasterisation.
#[test]
fn embedded_font_is_the_bundled_subset() {
    use base64::Engine as _;
    let cx = Context::new_blocking().unwrap();
    let guides = legend::scene(&cx).scene.guides();
    let mut plain = SvgTarget::new();
    plain.render(&guides).unwrap();
    let mut embedded = SvgTarget::with_options(SvgOptions {
        embed_font: true,
        ..SvgOptions::default()
    });
    embedded.render(&guides).unwrap();
    let svg = embedded.svg();
    let artifacts =
        default_artifact_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).join("gup_core");
    embedded
        .save(artifacts.join("scene_guides_embedded.svg"))
        .unwrap();

    // One declaration, of the family the text names, inside <defs>.
    let prefix =
        r#"<style>@font-face{font-family:"Inter";font-weight:400;src:url(data:font/ttf;base64,"#;
    assert_eq!(svg.matches("@font-face").count(), 1);
    let start = svg.find(prefix).expect("an @font-face for Inter") + prefix.len();
    let defs = (svg.find("<defs>").unwrap(), svg.find("</defs>").unwrap());
    assert!(
        defs.0 < start && start < defs.1,
        "the face is outside <defs>"
    );
    let len = svg[start..].find(')').unwrap();
    let data = &svg[start..start + len];
    assert!(svg[start + len..].starts_with(r#") format("truetype")}</style>"#));
    let font = base64::engine::general_purpose::STANDARD
        .decode(data)
        .expect("valid base64");
    assert_eq!(
        font,
        gup_text::INTER_REGULAR,
        "the embedded font is not the subset"
    );
    assert!(svg.contains(r#"font-family="Inter, sans-serif""#));

    // Everything else is the unembedded document.
    let line_start = svg[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = start + svg[start..].find('\n').unwrap() + 1;
    let without = format!("{}{}", &svg[..line_start], &svg[line_end..]);
    assert_eq!(without, plain.svg());
    eprintln!(
        "embedding the subset adds {} bytes (the font is {} bytes)",
        svg.len() - plain.svg().len(),
        gup_text::INTER_REGULAR.len()
    );

    // It parses, and the decoded font draws the same pixels.
    let a = rasterise_with(svg, &font);
    let b = rasterise(plain.svg());
    assert!(a == b, "the embedded font rasterises differently");
}

/// GUP-436: a semibold run is written with `font-weight="600"`, and with
/// `embed_font` the SemiBold subset follows the Regular one as a second
/// `@font-face`; a scene with no semibold run embeds one face only.
#[test]
fn semibold_runs_set_the_weight_and_embed_the_second_face() {
    use base64::Engine as _;
    let run = |text: &str, weight| TextRun {
        text: text.into(),
        at: Point::new(50.0, 20.0),
        anchor: Anchor::new(HAlign::Middle, VAlign::Top),
        style: TextStyle {
            size: Px(12.0),
            color: Color::hex(0x333333),
            weight,
        },
        role: TextRole::TickLabel,
    };
    let scene = |runs| {
        let mut scene = Scene::new(100.0, 40.0, Color::WHITE);
        scene.push(Item {
            z: 0,
            clip: None,
            kind: ItemKind::Text(runs),
        });
        scene
    };
    let mut svg = SvgTarget::with_options(SvgOptions {
        embed_font: true,
        ..SvgOptions::default()
    });
    svg.render(&scene(vec![
        run("2021", Weight::SemiBold),
        run("Jul", Weight::Regular),
    ]))
    .unwrap();
    let doc = svg.svg();
    assert_eq!(doc.matches("@font-face").count(), 2, "{doc}");
    assert_eq!(doc.matches(r#"font-weight="600""#).count(), 1, "{doc}");
    assert!(doc.contains(r##"font-weight="600" text-anchor="middle" fill="#333333">2021<"##));
    let prefix = r#"font-family:"Inter";font-weight:600;src:url(data:font/ttf;base64,"#;
    let start = doc.find(prefix).expect("a 600 @font-face") + prefix.len();
    let len = doc[start..].find(')').unwrap();
    let font = base64::engine::general_purpose::STANDARD
        .decode(&doc[start..start + len])
        .unwrap();
    assert_eq!(font, gup_text::INTER_SEMIBOLD);

    let mut regular = SvgTarget::with_options(SvgOptions {
        embed_font: true,
        ..SvgOptions::default()
    });
    regular
        .render(&scene(vec![run("Jul", Weight::Regular)]))
        .unwrap();
    assert_eq!(regular.svg().matches("@font-face").count(), 1);
    assert!(!regular.svg().contains("font-weight=\"600\""));
}
