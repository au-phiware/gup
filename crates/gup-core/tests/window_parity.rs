// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Window/PNG parity (RFC-001 S0 exit criterion 2): the reference scatter
//! drawn by `gup_core::show` into a real window, read back from the
//! window's surface texture via `GUP_SCREENSHOT_PATH`, must be within
//! CIEDE2000 ΔE < 2 of the same `Plot` rendered by `ImageTarget`, pixel by
//! pixel, and of the checked-in golden PNG.
//!
//! Needs a display (Wayland or X11), so it is ignored by default:
//!
//! ```text
//! cargo test -p gup-core --test window_parity -- --ignored --nocapture
//! ```
//!
//! winit allows one event loop per process, so this file holds one test.

mod common;

use common::scatter::{self, HEIGHT, WIDTH};
use gup_core::Context;
use gup_visual_regression::golden::default_artifact_dir;
use gup_visual_regression::{Rgba8, RgbaImage};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn vr(image: image::RgbaImage) -> RgbaImage {
    let (w, h) = image.dimensions();
    RgbaImage::new(w, h, image.into_raw()).expect("RGBA8 image")
}

/// Per-pixel CIEDE2000 statistics.
#[derive(Debug)]
struct DeltaE {
    max: f32,
    mean: f64,
    /// Pixels with ΔE > 0, > 0.5, > 1 and ≥ 2.
    above: [usize; 4],
    total: usize,
    worst: (u32, u32, Rgba8, Rgba8),
}

fn delta_e(expected: &RgbaImage, actual: &RgbaImage) -> DeltaE {
    assert_eq!(
        (expected.width(), expected.height()),
        (actual.width(), actual.height()),
        "image sizes differ"
    );
    let mut d = DeltaE {
        max: 0.0,
        mean: 0.0,
        above: [0; 4],
        total: 0,
        worst: (0, 0, Rgba8::WHITE, Rgba8::WHITE),
    };
    let mut sum = 0.0;
    for ((x, y, e), (_, _, a)) in expected.pixels().zip(actual.pixels()) {
        let de = e.delta_e(a);
        sum += f64::from(de);
        d.total += 1;
        for (count, threshold) in d.above.iter_mut().zip([0.0, 0.5, 1.0]) {
            if de > threshold {
                *count += 1;
            }
        }
        if de >= 2.0 {
            d.above[3] += 1;
        }
        if de > d.max {
            d.max = de;
            d.worst = (x, y, e, a);
        }
    }
    d.mean = sum / d.total as f64;
    d
}

#[test]
#[ignore = "opens a window: needs a display (run with --ignored)"]
fn window_screenshot_matches_the_png_within_delta_e_2() {
    let shot = default_artifact_dir(&root()).join("gup_core/window_scatter.png");
    let _ = std::fs::remove_file(&shot);
    // SAFETY: this test binary has a single test; nothing else reads the
    // environment concurrently.
    unsafe {
        std::env::set_var("GUP_SCREENSHOT_PATH", &shot);
        std::env::set_var("GUP_SCREENSHOT_WIDTH", WIDTH.to_string());
        std::env::set_var("GUP_SCREENSHOT_HEIGHT", HEIGHT.to_string());
    }
    gup_core::show(scatter::plot()).expect("show the scatter in a window");
    let window = vr(image::open(&shot)
        .unwrap_or_else(|e| panic!("{}: {e}", shot.display()))
        .to_rgba8());
    assert_eq!(
        (window.width(), window.height()),
        (WIDTH, HEIGHT),
        "the window was not given the requested size"
    );

    // The same Plot through ImageTarget, in this process.
    let cx = Context::new_blocking().expect("headless context");
    let (png, _) = scatter::plot()
        .render_resolved(&cx, WIDTH, HEIGHT)
        .expect("render to an image");
    let png = vr(png);
    let golden_path = root().join("tests/golden/gup_core/scatter.png");
    let golden = RgbaImage::load_png(&golden_path).expect("golden PNG");

    for (name, reference) in [("ImageTarget", &png), ("golden PNG", &golden)] {
        let d = delta_e(reference, &window);
        eprintln!(
            "window vs {name} ({}): {} px; ΔE max {:.4}, mean {:.6}; \
             >0: {}, >0.5: {}, >1: {}, ≥2: {}; worst at {:?}",
            shot.display(),
            d.total,
            d.max,
            d.mean,
            d.above[0],
            d.above[1],
            d.above[2],
            d.above[3],
            d.worst
        );
        assert!(
            d.max < 2.0,
            "window differs from the {name} by ΔE {:.3} at {:?}",
            d.max,
            d.worst
        );
    }

    // Negative control: the failure the non-sRGB view prevents. Rendering
    // sRGB-encoded colours through the surface's *Srgb view encodes them a
    // second time; that image must fail the same threshold.
    let double = double_encoded(&window);
    let d = delta_e(&png, &double);
    eprintln!(
        "control, double sRGB encoding: ΔE max {:.2}, mean {:.3}; ≥2: {} of {} px",
        d.max, d.mean, d.above[3], d.total
    );
    assert!(d.max >= 2.0, "the ΔE check cannot see a double encoding");
}

/// `image` as it would look had each sRGB-encoded channel been encoded
/// again (an sRGB render-target view over values that are already sRGB).
fn double_encoded(image: &RgbaImage) -> RgbaImage {
    let encode = |c: u8| {
        let v = f64::from(c) / 255.0;
        let s = if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0).round() as u8
    };
    let mut out = image.clone();
    for (x, y, p) in image.pixels() {
        out.set_pixel(
            x,
            y,
            Rgba8 {
                r: encode(p.r),
                g: encode(p.g),
                b: encode(p.b),
                a: p.a,
            },
        );
    }
    out
}
