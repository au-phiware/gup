// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-467: a golden scene's SVG, rasterised with resvg, against the
//! image the GPU drew (the one the golden test has just compared with its
//! PNG golden). Each `*_png.rs` golden test calls [`assert_matches`] with
//! its scene, so every golden scene is checked.

#![allow(dead_code)] // each includer uses a different subset

use gup_core::{Scene, SvgTarget, VectorTarget};
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{DiffTolerance, LayoutMetadata, RgbaImage, diff::perceptual_diff};
use std::path::Path;

/// Rasterise `svg` at its own size, with only the bundled Inter loaded.
pub fn rasterise(svg: &str) -> RgbaImage {
    use resvg::{tiny_skia, usvg};
    let mut opt = usvg::Options::default();
    opt.fontdb_mut()
        .load_font_data(gup_text::INTER_REGULAR.to_vec());
    let tree = usvg::Tree::from_str(svg, &opt).expect("SvgTarget output parses");
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).unwrap();
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    let data = pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    RgbaImage::new(size.width(), size.height(), data).unwrap()
}

/// Write `scene` as SVG, rasterise it and compare it with `gpu` (the
/// scene as the GPU drew it) within the harness's default tolerance
/// ([`DiffTolerance::default`]). Glyph pixels (the text regions of `meta`,
/// plus one pixel) are not compared: the GPU draws bitmaps and resvg
/// outlines, and `tests/svg.rs` checks text by its ink. Writes
/// `<case>.svg` and `<case>_svg.png` to the artifact directory. Returns
/// the SVG.
pub fn assert_matches(case: &str, scene: &Scene, gpu: &RgbaImage, meta: &LayoutMetadata) -> String {
    let mut target = SvgTarget::new();
    target.render(scene).unwrap();
    let svg = target.svg().to_owned();
    let artifacts =
        default_artifact_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).join(case);
    target.save(artifacts.with_extension("svg")).unwrap();
    let raster = rasterise(&svg);
    raster
        .save_png(artifacts.with_file_name(format!(
            "{}_svg.png",
            artifacts.file_name().unwrap().to_string_lossy()
        )))
        .unwrap();
    assert_eq!(
        (raster.width(), raster.height()),
        (gpu.width(), gpu.height()),
        "{case}: the SVG is not the PNG's size"
    );
    // Make the text regions equal, then compare everything else.
    let regions: Vec<_> = meta
        .text_regions
        .iter()
        .map(|t| t.rect.inflate(1.0))
        .collect();
    let mut data = Vec::with_capacity(raster.width() as usize * raster.height() as usize * 4);
    for (x, y, p) in raster.pixels() {
        let p = if regions.iter().any(|r| r.contains_pixel(x, y)) {
            gpu.pixel(x, y)
        } else {
            p
        };
        data.extend_from_slice(&[p.r, p.g, p.b, p.a]);
    }
    let masked = RgbaImage::new(raster.width(), raster.height(), data).unwrap();
    let tol = DiffTolerance::default();
    let stats = perceptual_diff(gpu, &masked, &tol).unwrap();
    eprintln!("{case}: SVG raster vs GPU image {stats:?}");
    assert!(
        stats.differing_fraction() <= tol.max_differing_fraction,
        "{case}: {} of {} pixels differ by more than ΔE {} (max ΔE {:.1}); see {}",
        stats.differing_pixels,
        stats.total_pixels,
        tol.pixel_delta_e,
        stats.max_delta_e,
        artifacts.display()
    );
    svg
}
