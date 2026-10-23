// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers for the gup dogfood binaries. Everything at this level is
//! code a user had to write because gup does not (publicly) provide it.
//!
//! The suite itself (task intents, expected outputs, known gaps) lives in
//! [`suite`] and uses only the gup-independent [`pixels`] measurements, so
//! the task binaries can be re-pointed at a new API without touching it.

pub mod pixels;
pub mod suite;

#[derive(Debug, Clone)]
pub struct Pt {
    pub id: u32,
    pub income: f32,
    pub spend: f32,
    pub segment: String,
    pub weight: f32,
}

pub fn load_points() -> Result<Vec<Pt>, Box<dyn std::error::Error>> {
    let mut rdr = csv::Reader::from_path("/tmp/gup-dogfood/points.csv")?;
    let mut out = Vec::with_capacity(200_000);
    for rec in rdr.records() {
        let r = rec?;
        out.push(Pt {
            id: r[0].parse()?,
            income: r[1].parse()?,
            spend: r[2].parse()?,
            segment: r[3].into(),
            weight: r[4].parse()?,
        });
    }
    Ok(out)
}

/// The 10-colour palette the manifest checks (sRGB hex).
pub const PALETTE_HEX: [u32; 5] = [0x1f77b4, 0xff7f0e, 0x2ca02c, 0xd62728, 0x9467bd];

/// Save an egui screenshot event (if present this frame) and return true.
pub fn save_screenshot(ctx: &eframe::egui::Context, path: &str) -> bool {
    let shot = ctx.input(|i| {
        i.raw.events.iter().find_map(|e| match e {
            eframe::egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    if let Some(img) = shot {
        let [w, h] = img.size;
        let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        image::save_buffer(path, &bytes, w as u32, h as u32, image::ColorType::Rgba8).unwrap();
        println!("saved {path} ({w}x{h})");
        return true;
    }
    false
}
