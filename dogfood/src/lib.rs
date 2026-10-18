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

/// Minimal egui embedding of any [`gup_core::Chart`]: renders on gup's own
/// context and uploads to an egui texture when dirty. (`gup-egui` is RFC-001
/// S13.) The chart stays owned by the caller so it can be mutated.
pub struct ChartTexture {
    cx: gup_core::Context,
    tex: Option<eframe::egui::TextureHandle>,
    size: (u32, u32),
    pub dirty: bool,
    pub last_render: std::time::Duration,
}

impl ChartTexture {
    pub fn new() -> Self {
        Self {
            cx: gup_core::Context::new_blocking().expect("gpu context"),
            tex: None,
            size: (0, 0),
            dirty: true,
            last_render: Default::default(),
        }
    }

    pub fn context(&self) -> &gup_core::Context {
        &self.cx
    }

    /// Draw `chart` filling `rect`; re-render when dirty or resized.
    pub fn show(
        &mut self,
        ui: &mut eframe::egui::Ui,
        rect: eframe::egui::Rect,
        chart: &mut dyn gup_core::Chart,
    ) {
        use gup_core::ChartExt;
        let (w, h) = (rect.width() as u32, rect.height() as u32);
        if self.dirty || self.tex.is_none() || self.size != (w, h) {
            let t0 = std::time::Instant::now();
            let rgba = chart.render(&self.cx, w, h).expect("render");
            let img = eframe::egui::ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                rgba.as_raw(),
            );
            match &mut self.tex {
                Some(t) => t.set(img, Default::default()),
                None => {
                    self.tex = Some(ui.ctx().load_texture("gup-chart", img, Default::default()))
                }
            }
            self.dirty = false;
            self.size = (w, h);
            self.last_render = t0.elapsed();
        }
        if let Some(t) = &self.tex {
            ui.put(
                rect,
                eframe::egui::Image::new(t).fit_to_exact_size(rect.size()),
            );
        }
    }
}

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
