// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CPU side of the glyph atlas: shelf packing, growth and the dirty
//! rectangle that the next upload copies.

use crate::error::{Error, Result};
use crate::font::{Face, FaceId};
use std::collections::HashMap;

/// Initial atlas width and height.
pub(crate) const INITIAL_SIZE: u32 = 1024;
/// Empty pixels around each glyph, so neighbours never bleed.
const PAD: u32 = 1;

/// A glyph's rectangle in the atlas, in texels.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct AtlasRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Texels `x0..x1` × `y0..y1` changed since the last upload.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Dirty {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl Dirty {
    fn union(self, other: Dirty) -> Dirty {
        Dirty {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }
}

/// R8 coverage bitmaps of rasterised glyphs, packed into rows ("shelves").
/// The atlas is square; when it is full it doubles, up to `max_size`.
pub(crate) struct GlyphAtlas {
    size: u32,
    max_size: u32,
    pixels: Vec<u8>,
    /// By face, glyph index, size in physical pixels and halo radius in
    /// physical pixels (`f32` bits; 0 for the glyph itself).
    glyphs: HashMap<(FaceId, u16, u32, u32), AtlasRect>,
    cursor: (u32, u32),
    row_height: u32,
    dirty: Option<Dirty>,
}

impl GlyphAtlas {
    /// An empty atlas that can grow to `max_size` (the device's largest
    /// 2D texture).
    pub(crate) fn new(max_size: u32) -> Self {
        Self::with_sizes(INITIAL_SIZE, max_size)
    }

    fn with_sizes(initial: u32, max_size: u32) -> Self {
        let size = initial.min(max_size);
        Self {
            size,
            max_size,
            pixels: vec![0; (size * size) as usize],
            glyphs: HashMap::new(),
            cursor: (PAD, PAD),
            row_height: 0,
            // wgpu zero-initialises textures, so nothing is dirty yet.
            dirty: None,
        }
    }

    /// Width and height in texels.
    pub(crate) fn size(&self) -> u32 {
        self.size
    }

    /// Distinct glyphs (face, glyph index and size) rasterised so far.
    pub(crate) fn len(&self) -> usize {
        self.glyphs.len()
    }

    pub(crate) fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// Take the region changed since the last call.
    pub(crate) fn take_dirty(&mut self) -> Option<Dirty> {
        self.dirty.take()
    }

    /// The atlas rectangle of `glyph` of `face` at `px` physical pixels,
    /// rasterising it on first use. `ch` is the character the glyph draws,
    /// for [`Error::AtlasFull`]: every glyph comes from one character,
    /// because there are no ligatures.
    pub(crate) fn glyph(
        &mut self,
        face: &Face,
        glyph: u16,
        ch: char,
        px: f32,
    ) -> Result<AtlasRect> {
        let key = (face.id, glyph, px.to_bits(), 0);
        if let Some(r) = self.glyphs.get(&key) {
            return Ok(*r);
        }
        let (metrics, bitmap) = face.glyphs.rasterize_indexed(glyph, px);
        self.insert(
            key,
            metrics.width as u32,
            metrics.height as u32,
            &bitmap,
            ch,
            px,
        )
    }

    /// The atlas rectangle of `glyph`'s halo: its coverage dilated by
    /// `radius` physical pixels ([`dilate`]), rasterised on first use. The
    /// rectangle is [`margin`]`(radius)` texels larger than the glyph's on
    /// every side.
    pub(crate) fn halo(
        &mut self,
        face: &Face,
        glyph: u16,
        ch: char,
        px: f32,
        radius: f32,
    ) -> Result<AtlasRect> {
        let key = (face.id, glyph, px.to_bits(), radius.to_bits());
        if let Some(r) = self.glyphs.get(&key) {
            return Ok(*r);
        }
        let (metrics, bitmap) = face.glyphs.rasterize_indexed(glyph, px);
        let (w, h) = (metrics.width, metrics.height);
        let halo = dilate(&bitmap, w, h, radius);
        let m = margin(radius) as usize;
        self.insert(key, (w + 2 * m) as u32, (h + 2 * m) as u32, &halo, ch, px)
    }

    /// Copy a `w` × `h` bitmap into newly allocated room and remember it
    /// under `key`.
    fn insert(
        &mut self,
        key: (FaceId, u16, u32, u32),
        w: u32,
        h: u32,
        bitmap: &[u8],
        ch: char,
        px: f32,
    ) -> Result<AtlasRect> {
        let (x, y) = self.allocate(w, h).ok_or(Error::AtlasFull {
            ch,
            px,
            size: self.size,
            glyphs: self.glyphs.len(),
        })?;
        for row in 0..h {
            let dst = ((y + row) * self.size + x) as usize;
            let src = (row * w) as usize;
            self.pixels[dst..dst + w as usize].copy_from_slice(&bitmap[src..src + w as usize]);
        }
        self.mark_dirty(Dirty {
            x0: x,
            y0: y,
            x1: x + w,
            y1: y + h,
        });
        let r = AtlasRect {
            x,
            y,
            width: w,
            height: h,
        };
        self.glyphs.insert(key, r);
        Ok(r)
    }

    /// Find room for a `w` × `h` bitmap, growing the atlas if needed.
    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        loop {
            let (mut x, mut y) = self.cursor;
            let mut row_height = self.row_height;
            if x + w + PAD > self.size {
                (x, y) = (PAD, y + row_height + PAD);
                row_height = 0;
            }
            if x + w + PAD <= self.size && y + h + PAD <= self.size {
                self.cursor = (x + w + PAD, y);
                self.row_height = row_height.max(h);
                return Some((x, y));
            }
            if !self.grow() {
                return None;
            }
        }
    }

    /// Double the atlas, keeping every glyph where it is. The whole atlas
    /// is dirty afterwards, because the texture is recreated.
    fn grow(&mut self) -> bool {
        let new = (self.size * 2).min(self.max_size);
        if new <= self.size {
            return false;
        }
        let mut pixels = vec![0; (new * new) as usize];
        for row in 0..self.size {
            let src = (row * self.size) as usize;
            let dst = (row * new) as usize;
            pixels[dst..dst + self.size as usize]
                .copy_from_slice(&self.pixels[src..src + self.size as usize]);
        }
        self.pixels = pixels;
        self.size = new;
        self.dirty = Some(Dirty {
            x0: 0,
            y0: 0,
            x1: new,
            y1: new,
        });
        true
    }

    fn mark_dirty(&mut self, d: Dirty) {
        self.dirty = Some(self.dirty.map_or(d, |old| old.union(d)));
    }
}

/// How many texels a halo of `radius` physical pixels adds on each side
/// of its glyph: the dilation reaches pixels nearer than `radius + 1`.
pub(crate) fn margin(radius: f32) -> u32 {
    radius.ceil() as u32
}

/// `bitmap` (`w` × `h` coverage) dilated by a disc of `radius` pixels, with
/// a one-pixel antialiased edge, on a bitmap [`margin`]`(radius)` larger
/// on each side: each pixel is `clamp(max_q c(q) + radius − |p − q|, 0, 1)`
/// over the source pixels `q`. A pixel's coverage stands for where the
/// outline crosses it, so this is the outside half of a round-joined
/// stroke `2 × radius` wide, as an SVG stroke draws a halo.
pub(crate) fn dilate(bitmap: &[u8], w: usize, h: usize, radius: f32) -> Vec<u8> {
    let m = margin(radius) as usize;
    let (dw, dh) = (w + 2 * m, h + 2 * m);
    // Each offset within reach (from the source pixel's place in the
    // output), with what it adds to that pixel's coverage.
    let mut offsets = Vec::new();
    for oy in 0..=2 * m {
        for ox in 0..=2 * m {
            let (dx, dy) = (ox as f32 - m as f32, oy as f32 - m as f32);
            let gain = radius - (dx * dx + dy * dy).sqrt();
            if gain > -1.0 {
                offsets.push((ox, oy, gain));
            }
        }
    }
    let mut out = vec![0.0f32; dw * dh];
    for y in 0..h {
        for x in 0..w {
            let c = f32::from(bitmap[y * w + x]) / 255.0;
            if c == 0.0 {
                continue;
            }
            for &(ox, oy, gain) in &offsets {
                let i = (y + oy) * dw + x + ox;
                out[i] = out[i].max(c + gain);
            }
        }
    }
    out.into_iter()
        .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::Font;

    /// The atlas rectangle of the glyph `font` draws for `c`.
    fn put(atlas: &mut GlyphAtlas, font: &Font, c: char, px: f32) -> Result<AtlasRect> {
        let face = font.face_for(c);
        atlas.glyph(face, face.glyphs.lookup_glyph_index(c), c, px)
    }

    #[test]
    fn a_glyph_is_rasterised_once_per_size() {
        let font = Font::inter();
        let mut atlas = GlyphAtlas::new(4096);
        let a = put(&mut atlas, &font, '1', 12.0).unwrap();
        let b = put(&mut atlas, &font, '1', 12.0).unwrap();
        let c = put(&mut atlas, &font, '1', 24.0).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(atlas.len(), 2);
        assert!(c.height > a.height);
    }

    /// A halo is a disc dilation: one full pixel grows into a disc of the
    /// radius, with a one-pixel ramp beyond it; a half-covered pixel's
    /// disc is half a pixel smaller. The source pixel stays covered.
    #[test]
    fn a_halo_dilates_coverage_by_a_disc() {
        let r = 2.0;
        assert_eq!(margin(r), 2);
        let out = dilate(&[255], 1, 1, r);
        assert_eq!(out.len(), 5 * 5);
        let at = |x: usize, y: usize| out[y * 5 + x];
        assert_eq!(at(2, 2), 255);
        // Two pixels away along an axis: on the disc's edge, still full.
        assert_eq!(
            (at(0, 2), at(4, 2), at(2, 0), at(2, 4)),
            (255, 255, 255, 255)
        );
        // The corner, √8 ≈ 2.83 px away: 0.17 of a pixel inside the ramp.
        assert_eq!(at(0, 0), ((1.0f32 + r - 8f32.sqrt()) * 255.0).round() as u8);
        // A fractional radius reaches one more pixel, faintly.
        let out = dilate(&[255], 1, 1, 2.5);
        assert_eq!(out.len(), 7 * 7);
        assert_eq!(out[3 * 7], 128, "3 px left: half of the ramp");
        // Half coverage: the disc's edge moves in by half a pixel.
        let out = dilate(&[128], 1, 1, 2.0);
        assert!((i32::from(out[2 * 5]) - 128).abs() <= 1, "{}", out[2 * 5]);
    }

    /// A glyph's halo has its own atlas rectangle, `margin` larger on each
    /// side, rasterised once per radius; the glyph itself is untouched.
    #[test]
    fn a_halo_is_cached_beside_its_glyph() {
        let font = Font::inter();
        let face = font.face_for('A');
        let g = face.glyphs.lookup_glyph_index('A');
        let mut atlas = GlyphAtlas::new(4096);
        let plain = atlas.glyph(face, g, 'A', 16.0).unwrap();
        let halo = atlas.halo(face, g, 'A', 16.0, 2.0).unwrap();
        assert_eq!(
            (halo.width, halo.height),
            (plain.width + 4, plain.height + 4)
        );
        assert_eq!(atlas.halo(face, g, 'A', 16.0, 2.0).unwrap(), halo);
        assert_ne!(atlas.halo(face, g, 'A', 16.0, 3.0).unwrap(), halo);
        assert_eq!(atlas.glyph(face, g, 'A', 16.0).unwrap(), plain);
        assert_eq!(atlas.len(), 3);
    }

    #[test]
    fn the_dirty_rectangle_covers_only_new_glyphs() {
        let font = Font::inter();
        let mut atlas = GlyphAtlas::new(4096);
        assert_eq!(atlas.take_dirty(), None);
        let a = put(&mut atlas, &font, 'A', 16.0).unwrap();
        let b = put(&mut atlas, &font, 'B', 16.0).unwrap();
        let d = atlas.take_dirty().unwrap();
        assert_eq!((d.x0, d.y0), (a.x, a.y.min(b.y)));
        assert_eq!(d.x1, b.x + b.width);
        assert!(d.y1 - d.y0 <= a.height.max(b.height));
        assert!((d.x1 - d.x0) < 64, "a dirty rect, not whole rows: {d:?}");
        // Cached glyphs dirty nothing.
        put(&mut atlas, &font, 'A', 16.0).unwrap();
        assert_eq!(atlas.take_dirty(), None);
        // Coverage landed in the atlas.
        let row = ((a.y + a.height / 2) * atlas.size()) as usize;
        let span = &atlas.pixels()[row + a.x as usize..row + (a.x + a.width) as usize];
        assert!(span.iter().any(|&p| p > 200), "{span:?}");
    }

    #[test]
    fn a_full_atlas_grows_keeping_its_glyphs_then_errors_at_the_maximum() {
        let font = Font::inter();
        let texels = |a: &GlyphAtlas, r: AtlasRect| -> Vec<u8> {
            (r.y..r.y + r.height)
                .flat_map(|y| {
                    let row = (y * a.size() + r.x) as usize;
                    a.pixels()[row..row + r.width as usize].to_vec()
                })
                .collect()
        };
        let mut atlas = GlyphAtlas::with_sizes(128, 256);
        let first = put(&mut atlas, &font, 'W', 40.0).unwrap();
        let before = texels(&atlas, first);
        atlas.take_dirty();
        let mut px = 41;
        while atlas.size() == 128 {
            put(&mut atlas, &font, 'W', px as f32).unwrap();
            px += 1;
        }
        assert_eq!(atlas.size(), 256);
        assert_eq!(put(&mut atlas, &font, 'W', 40.0).unwrap(), first);
        assert_eq!(texels(&atlas, first), before);
        assert_eq!(
            atlas.take_dirty(),
            Some(Dirty {
                x0: 0,
                y0: 0,
                x1: 256,
                y1: 256
            })
        );
        let err = loop {
            match put(&mut atlas, &font, 'W', px as f32) {
                Ok(_) => px += 1,
                Err(e) => break e,
            }
        };
        assert!(
            matches!(
                err,
                Error::AtlasFull {
                    size: 256,
                    ch: 'W',
                    ..
                }
            ),
            "{err}"
        );
    }
}
