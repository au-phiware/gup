// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reference: PNG export of a titled, gridded scatter plot at three sizes
//! (1x, a 2x device-pixel-ratio render and a large one), built externally on
//! `gup`.

use gup::prelude::*;
use gup::{ImageTarget, TargetOptions};

/// A data point for the scatter plot.
#[derive(Debug, Clone)]
struct DataPoint {
    x: f64,
    y: f64,
}

/// Sample data: study hours against test score.
fn sample_data() -> Vec<DataPoint> {
    [
        (1.0, 45.0),
        (2.0, 55.0),
        (3.0, 60.0),
        (4.0, 65.0),
        (5.0, 72.0),
        (6.0, 78.0),
        (7.0, 82.0),
        (8.0, 88.0),
        (9.0, 92.0),
        (10.0, 95.0),
    ]
    .into_iter()
    .map(|(x, y)| DataPoint { x, y })
    .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("PNG Export Example");
    println!("==================");

    let cx = Context::new_blocking()?;
    let mut plot = Plot::new();
    plot.x(Linear::new().domain(0.0, 11.0));
    plot.y(Linear::new().domain(30.0, 100.0));
    plot.title("Study Hours vs Test Score").grid(Axes::Both);
    plot.circles(sample_data())
        .x(|d: &DataPoint| d.x)
        .y(|d: &DataPoint| d.y)
        .radius(Px(8.0))
        .fill(Color::rgb(0.22, 0.46, 0.82))
        .stroke(Color::hex(0x1a1a1a))
        .stroke_width(Px(2.0));

    let output_1x = "/tmp/gup-dogfood/ref_export_png.png";
    plot.save_png(&cx, output_1x, 800, 600)?;
    println!(
        "Wrote {output_1x} ({} bytes)",
        std::fs::metadata(output_1x)?.len()
    );

    // 1600x1200 physical pixels at logical 800x600.
    let output_2x = "/tmp/gup-dogfood/ref_export_png@2x.png";
    let scene = plot.resolve(&cx, 800.0, 600.0)?.scene.clone();
    let image = ImageTarget::with_options(
        &cx,
        1600,
        1200,
        TargetOptions {
            dpr: 2.0,
            ..Default::default()
        },
    )?
    .render_blocking(&cx, &scene)?;
    image.save(output_2x)?;
    println!(
        "Wrote {output_2x} ({} bytes)",
        std::fs::metadata(output_2x)?.len()
    );

    let output_large = "/tmp/gup-dogfood/ref_export_large.png";
    plot.save_png(&cx, output_large, 2400, 1600)?;
    println!(
        "Wrote {output_large} ({} bytes)",
        std::fs::metadata(output_large)?.len()
    );
    Ok(())
}
