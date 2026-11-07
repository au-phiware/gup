// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-467: a golden scene's SVG, rasterised with resvg, against the
//! image the GPU drew (the one the golden test has just compared with its
//! PNG golden). Each `*_png.rs` golden test calls [`assert_matches`] with
//! its scene, so every golden scene is checked.

#![allow(dead_code)] // each includer uses a different subset

use gup_core::{Scene, SvgTarget, VectorTarget};
use gup_visual_regression::diff::{DiffStats, perceptual_diff};
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{DiffTolerance, LayoutMetadata, RgbaImage};
use std::path::Path;

/// Rasterise `svg` at its own size, with only the bundled Inter loaded.
pub fn rasterise(svg: &str) -> RgbaImage {
    rasterise_with(svg, &[])
}

/// [`rasterise`] with `fonts` (font files, e.g. a scene's fallbacks) loaded
/// as well.
pub fn rasterise_with(svg: &str, fonts: &[&[u8]]) -> RgbaImage {
    use resvg::{tiny_skia, usvg};
    let mut opt = usvg::Options::default();
    opt.fontdb_mut()
        .load_font_data(gup_text::INTER_REGULAR.to_vec());
    for font in fonts {
        opt.fontdb_mut().load_font_data(font.to_vec());
    }
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

/// Block statistics of the rasterised `svg` against `gpu`, with the text
/// regions of `meta` (plus one pixel) made equal first. Also returns the
/// per-pixel statistics, for the log.
pub fn compare(
    svg: &str,
    gpu: &RgbaImage,
    meta: &LayoutMetadata,
) -> (DiffStats, DiffStats, RgbaImage) {
    compare_with(svg, gpu, meta, &[])
}

/// [`compare`] with `fonts` loaded into the rasteriser as well.
pub fn compare_with(
    svg: &str,
    gpu: &RgbaImage,
    meta: &LayoutMetadata,
    fonts: &[&[u8]],
) -> (DiffStats, DiffStats, RgbaImage) {
    let raster = rasterise_with(svg, fonts);
    assert_eq!(
        (raster.width(), raster.height()),
        (gpu.width(), gpu.height()),
        "the SVG is not the PNG's size"
    );
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
    let whole = perceptual_diff(gpu, &masked, &tol).unwrap();
    let (a, b) = (blocks(gpu, BLOCK), blocks(&masked, BLOCK));
    (whole, perceptual_diff(&a, &b, &tol).unwrap(), masked)
}

/// The largest block ΔE a correct SVG may have: resvg's quarter-step
/// vertical coverage moves a block by up to 4.7 (measured over every
/// golden scene). Recalibrated 5.0 -> 5.5 on 2026-11-01 (GUP-526): two
/// independent scenes reached 5.4 from the same antialiasing on hairlines
/// and steep lines: the heatmap's 168 cell gaps (GUP-549, which skipped this
/// check; it measures 4.1 on current main) and the dual-axis line after a
/// sub-pixel shift (`secondary_y`, 5.4). No other case exceeds 4.7.
pub const MAX_BLOCK_DE: f32 = 5.5;

/// Whether block statistics are within the harness tolerance
/// ([`DiffTolerance::default`]) and `MAX_BLOCK_DE`.
pub fn within(stats: &DiffStats) -> bool {
    stats.differing_fraction() <= DiffTolerance::default().max_differing_fraction
        && stats.max_delta_e <= MAX_BLOCK_DE
}

/// Per-case allowance of differing blocks (more than ΔE 3), with
/// `MAX_BLOCK_DE` unchanged. Only `gup_core/histogram`: its bars are
/// separated by 1 px gaps, which resvg antialiases differently from the GPU,
/// and how many 4x4 blocks pass ΔE 3 depends on which fill colours border
/// the gaps. Measured with the GUP-658 palette (blue and vermillion): 51
/// blocks, max ΔE 3.4, where the default allows 40 (the heatmap check is
/// skipped for the same class of difference). Allowed: 64.
fn within_case_override(case: &str, stats: &DiffStats) -> bool {
    case == "gup_core/histogram"
        && stats.differing_pixels <= 64
        && stats.max_delta_e <= MAX_BLOCK_DE
}

/// Write `scene` as SVG, rasterise it and compare it with `gpu` (the
/// scene as the GPU drew it): both are averaged over 4×4 blocks (resvg
/// quantises vertical coverage to quarter steps, which moves single edge
/// pixels past the per-pixel ΔE 3 while the geometry is right), then
/// compared within the harness's default tolerance and `MAX_BLOCK_DE`.
/// Glyph pixels are not compared: the GPU draws bitmaps and resvg
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
    // The scene's fallback fonts are loaded too, as a viewer with them
    // installed (or the embedded copies) would draw.
    let fonts: Vec<&[u8]> = scene.fallback_fonts.iter().map(|f| f.data()).collect();
    let (whole, stats, masked) = compare_with(&svg, gpu, meta, &fonts);
    masked
        .save_png(artifacts.with_file_name(format!(
            "{}_svg.png",
            artifacts.file_name().unwrap().to_string_lossy()
        )))
        .unwrap();
    eprintln!(
        "{case}: SVG raster vs GPU image: per pixel {whole:?}; in {BLOCK}x{BLOCK} blocks {stats:?}"
    );
    assert!(
        within(&stats) || within_case_override(case, &stats),
        "{case}: {} of {} blocks differ by more than ΔE 3 (max ΔE {:.1}); see {}",
        stats.differing_pixels,
        stats.total_pixels,
        stats.max_delta_e,
        artifacts.display()
    );
    svg
}

/// Side of the blocks both images are averaged over before comparing.
const BLOCK: u32 = 4;

/// `img` with each `k × k` block of pixels averaged into one.
fn blocks(img: &RgbaImage, k: u32) -> RgbaImage {
    let (w, h) = (img.width() / k, img.height() / k);
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for by in 0..h {
        for bx in 0..w {
            let mut sum = [0u32; 4];
            for y in by * k..(by + 1) * k {
                for x in bx * k..(bx + 1) * k {
                    let p = img.pixel(x, y);
                    for (s, c) in sum.iter_mut().zip([p.r, p.g, p.b, p.a]) {
                        *s += u32::from(c);
                    }
                }
            }
            data.extend(sum.map(|s| ((s + k * k / 2) / (k * k)) as u8));
        }
    }
    RgbaImage::new(w, h, data).unwrap()
}
