// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! **Temporary, internal-only** text path for the S0a spike.
//!
//! Just enough to measure and draw titles and tick labels: one embedded
//! font, `fontdue` bitmap rasterisation into one R8 glyph atlas, and
//! straight-line horizontal layout with kerning. No shaping, no system font
//! discovery, no MSDF, no rotation. It has no public API; RFC-001 S2
//! replaces it with the extracted `gup-text` crate (one `TextSystem` per
//! `Context`, Inter bundled).

use crate::geom::Rect;
use crate::scene::{HAlign, TextRun, VAlign};
use std::collections::HashMap;

/// The embedded default font (the same file the old path embeds). Inter
/// replaces it with GUP-392/S2.
const FONT: &[u8] = include_bytes!("../../../assets/fonts/default.ttf");

const ATLAS_SIZE: u32 = 1024;
/// Empty pixels around each glyph in the atlas, so neighbours never bleed.
const ATLAS_PAD: u32 = 1;

/// Line metrics of a run, in logical pixels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct TextMetrics {
    /// Advance width of the whole run.
    pub width: f32,
    /// Height of a capital letter above the baseline.
    pub cap_height: f32,
    /// Descent below the baseline (positive).
    pub descent: f32,
}

/// One glyph quad: rect in logical pixels, uv in the atlas.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct PlacedGlyph {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
}

#[derive(Copy, Clone, Debug)]
struct AtlasGlyph {
    /// x, y, width, height in atlas pixels.
    rect: [u32; 4],
}

/// The font, its atlas and the atlas texture.
pub(crate) struct TextSystem {
    font: fontdue::Font,
    pixels: Vec<u8>,
    glyphs: HashMap<(char, u32), AtlasGlyph>,
    cursor: (u32, u32),
    row_height: u32,
    dirty: bool,
    texture: Option<(wgpu::Texture, wgpu::TextureView)>,
}

impl TextSystem {
    pub(crate) fn new() -> Self {
        let font = fontdue::Font::from_bytes(FONT, fontdue::FontSettings::default())
            .expect("embedded font parses");
        Self {
            font,
            pixels: vec![0; (ATLAS_SIZE * ATLAS_SIZE) as usize],
            glyphs: HashMap::new(),
            cursor: (ATLAS_PAD, ATLAS_PAD),
            row_height: 0,
            dirty: true,
            texture: None,
        }
    }

    /// Measure `text` at `size` pixels.
    pub(crate) fn measure(&self, text: &str, size: f32) -> TextMetrics {
        let mut width = 0.0;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                width += self.font.horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            width += self.font.metrics(c, size).advance_width;
            prev = Some(c);
        }
        let cap = self.font.metrics('H', size);
        let descent = self
            .font
            .horizontal_line_metrics(size)
            .map_or(size * 0.2, |m| -m.descent);
        TextMetrics {
            width,
            cap_height: cap.height as f32 + cap.ymin as f32,
            descent,
        }
    }

    /// The baseline origin of `run` after applying its anchor.
    fn origin(&self, run: &TextRun) -> (f32, f32) {
        let m = self.measure(&run.text, run.style.size.0);
        let x = match run.anchor.h {
            HAlign::Start => run.at.x,
            HAlign::Middle => run.at.x - m.width / 2.0,
            HAlign::End => run.at.x - m.width,
        };
        let y = match run.anchor.v {
            VAlign::Top => run.at.y + m.cap_height,
            VAlign::Middle => run.at.y + m.cap_height / 2.0,
            VAlign::Baseline => run.at.y,
            VAlign::Bottom => run.at.y - m.descent,
        };
        // Whole pixels: bitmap glyphs are drawn 1:1.
        (x.round(), y.round())
    }

    /// Walk the glyphs of `run`: `(char, metrics, top-left px)`.
    fn walk(&self, run: &TextRun, mut f: impl FnMut(char, fontdue::Metrics, f32, f32)) {
        let size = run.style.size.0;
        let (mut pen, baseline) = self.origin(run);
        let mut prev = None;
        for c in run.text.chars() {
            if let Some(p) = prev {
                pen += self.font.horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            let m = self.font.metrics(c, size);
            let x = (pen + m.xmin as f32).round();
            let y = baseline - (m.ymin as f32 + m.height as f32);
            f(c, m, x, y);
            pen += m.advance_width;
            prev = Some(c);
        }
    }

    /// The ink box of `run` (union of its glyph bitmaps).
    pub(crate) fn ink_bounds(&self, run: &TextRun) -> Rect {
        let mut bounds: Option<(f32, f32, f32, f32)> = None;
        self.walk(run, |_, m, x, y| {
            if m.width == 0 || m.height == 0 {
                return;
            }
            let (x1, y1) = (x + m.width as f32, y + m.height as f32);
            let b = bounds.get_or_insert((x, y, x1, y1));
            *b = (b.0.min(x), b.1.min(y), b.2.max(x1), b.3.max(y1));
        });
        bounds.map_or(
            Rect::new(run.at.x, run.at.y, 0.0, 0.0),
            |(x0, y0, x1, y1)| Rect::new(x0, y0, x1 - x0, y1 - y0),
        )
    }

    /// Glyph quads for `run`, rasterising new glyphs into the atlas.
    pub(crate) fn layout(&mut self, run: &TextRun) -> Vec<PlacedGlyph> {
        let size_key = run.style.size.0.to_bits();
        let mut placed = Vec::new();
        let mut needed = Vec::new();
        self.walk(run, |c, m, x, y| needed.push((c, m, x, y)));
        for (c, m, x, y) in needed {
            if m.width == 0 || m.height == 0 {
                continue;
            }
            let Some(g) = self.glyph(c, size_key, run.style.size.0) else {
                continue;
            };
            let s = ATLAS_SIZE as f32;
            let [gx, gy, gw, gh] = g.rect.map(|v| v as f32);
            placed.push(PlacedGlyph {
                rect: [x, y, gw, gh],
                uv: [gx / s, gy / s, (gx + gw) / s, (gy + gh) / s],
            });
        }
        placed
    }

    /// The atlas entry for `c`, rasterising it if needed. `None` if the
    /// atlas is full (S2's atlas grows; this one is sized for labels).
    fn glyph(&mut self, c: char, size_key: u32, size: f32) -> Option<AtlasGlyph> {
        if let Some(g) = self.glyphs.get(&(c, size_key)) {
            return Some(*g);
        }
        let (metrics, bitmap) = self.font.rasterize(c, size);
        let (w, h) = (metrics.width as u32, metrics.height as u32);
        if self.cursor.0 + w + ATLAS_PAD > ATLAS_SIZE {
            self.cursor = (ATLAS_PAD, self.cursor.1 + self.row_height + ATLAS_PAD);
            self.row_height = 0;
        }
        if self.cursor.1 + h + ATLAS_PAD > ATLAS_SIZE {
            return None;
        }
        let (x0, y0) = self.cursor;
        for row in 0..h {
            let dst = ((y0 + row) * ATLAS_SIZE + x0) as usize;
            let src = (row * w) as usize;
            self.pixels[dst..dst + w as usize].copy_from_slice(&bitmap[src..src + w as usize]);
        }
        self.cursor.0 += w + ATLAS_PAD;
        self.row_height = self.row_height.max(h);
        self.dirty = true;
        let g = AtlasGlyph {
            rect: [x0, y0, w, h],
        };
        self.glyphs.insert((c, size_key), g);
        Some(g)
    }

    /// The atlas texture view, uploading new glyphs first.
    pub(crate) fn atlas_view(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> wgpu::TextureView {
        let (texture, view) = self.texture.get_or_insert_with(|| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("gup glyph atlas"),
                size: wgpu::Extent3d {
                    width: ATLAS_SIZE,
                    height: ATLAS_SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            (texture, view)
        });
        if self.dirty {
            queue.write_texture(
                texture.as_image_copy(),
                &self.pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(ATLAS_SIZE),
                    rows_per_image: None,
                },
                texture.size(),
            );
            self.dirty = false;
        }
        view.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::{Color, Px};
    use crate::geom::Point;
    use crate::scene::Anchor;
    use crate::scene::{TextRole, TextStyle};

    fn run(text: &str, anchor: Anchor) -> TextRun {
        TextRun {
            text: text.into(),
            at: Point::new(100.0, 50.0),
            anchor,
            style: TextStyle {
                size: Px(16.0),
                color: Color::BLACK,
            },
            role: TextRole::Title,
        }
    }

    #[test]
    fn measure_grows_with_text_and_size() {
        let t = TextSystem::new();
        let a = t.measure("10", 12.0);
        let b = t.measure("1000", 12.0);
        let c = t.measure("10", 24.0);
        assert!(a.width > 0.0 && b.width > a.width && c.width > a.width);
        assert!(a.cap_height > 5.0 && a.cap_height < 12.0, "{a:?}");
    }

    #[test]
    fn anchors_place_the_ink_box() {
        let t = TextSystem::new();
        let middle_top = t.ink_bounds(&run("Title", Anchor::new(HAlign::Middle, VAlign::Top)));
        // Centred on x = 100 (within rounding), starting at y = 50.
        let centre = middle_top.x + middle_top.width / 2.0;
        assert!((centre - 100.0).abs() <= 1.5, "{middle_top:?}");
        assert!((middle_top.y - 50.0).abs() <= 1.0, "{middle_top:?}");
        let end = t.ink_bounds(&run("42", Anchor::new(HAlign::End, VAlign::Middle)));
        assert!(
            end.x + end.width <= 101.0 && end.x + end.width >= 97.0,
            "{end:?}"
        );
    }

    #[test]
    fn layout_rasterises_into_the_atlas_once() {
        let mut t = TextSystem::new();
        let glyphs = t.layout(&run("1001", Anchor::new(HAlign::Start, VAlign::Baseline)));
        assert_eq!(glyphs.len(), 4);
        // Only two distinct glyphs were rasterised.
        assert_eq!(t.glyphs.len(), 2);
        assert_eq!(glyphs[0].uv, glyphs[3].uv);
        assert!(t.pixels.iter().any(|&p| p > 200));
    }
}
