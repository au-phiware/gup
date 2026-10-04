// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers for the gup dogfood binaries. Everything here is code a
//! user had to write because gup does not (publicly) provide it.

use gup::chart_builder::ComposedChart;
use gup::mark::Mark;
use std::fmt::Debug;

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

pub const SEGMENTS: [&str; 5] = ["retail", "wholesale", "online", "partner", "other"];
/// Same as gup's private DEFAULT_PALETTE (tab10), copied so legends match.
pub const PALETTE: [[f32; 3]; 8] = [
    [0.122, 0.467, 0.706],
    [1.000, 0.498, 0.055],
    [0.173, 0.627, 0.173],
    [0.839, 0.153, 0.157],
    [0.580, 0.404, 0.741],
    [0.549, 0.337, 0.294],
    [0.890, 0.467, 0.761],
    [0.498, 0.498, 0.498],
];

pub fn seg_index(s: &str) -> usize {
    SEGMENTS.iter().position(|x| *x == s).unwrap_or(4)
}

pub fn rgba(i: usize, a: f32) -> [f32; 4] {
    let [r, g, b] = PALETTE[i % PALETTE.len()];
    [r, g, b, a]
}

/// Plot area as fractions of the chart's configured size, replicating the
/// `pub(crate)` `ComposedChart::calculate_chart_area`.
#[derive(Debug, Clone, Copy)]
pub struct PlotFrac {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

pub fn plot_frac<T, M>(chart: &ComposedChart<T, M>) -> PlotFrac
where
    T: Clone + Send + Sync + Debug + 'static,
    M: Mark,
{
    let c = &chart.config;
    let mut m = c.margins;
    if let Some(a) = &chart.bottom_axis {
        m.bottom += a.calculate_margin(None);
    }
    if let Some(a) = &chart.left_axis {
        m.left += a.calculate_margin(None);
    }
    if let Some(a) = &chart.top_axis {
        m.top += a.calculate_margin(None);
    }
    if let Some(a) = &chart.right_axis {
        m.right += a.calculate_margin(None);
    }
    PlotFrac {
        left: m.left / c.width,
        top: m.top / c.height,
        right: 1.0 - m.right / c.width,
        bottom: 1.0 - m.bottom / c.height,
    }
}

impl PlotFrac {
    /// Map a position inside a widget of size (w, h) to unit plot coords
    /// (0..1, 0..1 with y up). Returns None outside the plot area.
    pub fn unit(&self, px: f32, py: f32, w: f32, h: f32) -> Option<(f32, f32)> {
        let fx = px / w;
        let fy = py / h;
        let ux = (fx - self.left) / (self.right - self.left);
        let uy = (self.bottom - fy) / (self.bottom - self.top);
        ((0.0..=1.0).contains(&ux) && (0.0..=1.0).contains(&uy)).then_some((ux, uy))
    }

    /// Unclamped variant of [`unit`](Self::unit).
    pub fn unit_unclamped(&self, px: f32, py: f32, w: f32, h: f32) -> (f32, f32) {
        let ux = (px / w - self.left) / (self.right - self.left);
        let uy = (self.bottom - py / h) / (self.bottom - self.top);
        (ux, uy)
    }
}

/// Minimal egui embedding, written because `gup-egui` does not compile
/// against current gup (E0277: `M: MarkInstanceBuilder` missing on its
/// `DynChart` impl). Renders via `render_to_rgba` on gup's own device and
/// uploads to an egui texture when dirty. The chart stays owned by the
/// caller so it can be mutated (set_data, attr re-binding).
pub struct ChartTexture {
    tex: Option<eframe::egui::TextureHandle>,
    pub dirty: bool,
    pub last_render: std::time::Duration,
}

impl Default for ChartTexture {
    fn default() -> Self {
        Self { tex: None, dirty: true, last_render: Default::default() }
    }
}

impl ChartTexture {
    pub fn show<T, M>(
        &mut self,
        ui: &mut eframe::egui::Ui,
        rect: eframe::egui::Rect,
        chart: &mut ComposedChart<T, M>,
    ) where
        T: Clone + Send + Sync + Debug + 'static,
        M: Mark,
    {
        let (w, h) = (rect.width() as u32, rect.height() as u32);
        if self.dirty || self.tex.is_none() {
            let t0 = std::time::Instant::now();
            let rgba = chart.render_to_rgba(w, h).expect("render_to_rgba");
            let img = eframe::egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            match &mut self.tex {
                Some(t) => t.set(img, Default::default()),
                None => {
                    self.tex = Some(ui.ctx().load_texture("gup-chart", img, Default::default()))
                }
            }
            self.dirty = false;
            self.last_render = t0.elapsed();
        }
        if let Some(t) = &self.tex {
            ui.put(rect, eframe::egui::Image::new(t).fit_to_exact_size(rect.size()));
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
