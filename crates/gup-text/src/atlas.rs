// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The CPU side of the glyph atlas: shelf packing, growth and the dirty
//! rectangle that the next upload copies.

use crate::error::{Error, Result};
use crate::font::Font;
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
    /// By character and size in physical pixels (`f32` bits).
    glyphs: HashMap<(char, u32), AtlasRect>,
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

    /// Distinct glyphs (character and size) rasterised so far.
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

    /// The atlas rectangle of `c` at `px` physical pixels, rasterising it
    /// on first use.
    pub(crate) fn glyph(&mut self, font: &Font, c: char, px: f32) -> Result<AtlasRect> {
        if let Some(r) = self.glyphs.get(&(c, px.to_bits())) {
            return Ok(*r);
        }
        let (metrics, bitmap) = font.face().rasterize(c, px);
        let (w, h) = (metrics.width as u32, metrics.height as u32);
        let (x, y) = self.allocate(w, h).ok_or(Error::AtlasFull {
            ch: c,
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
        self.glyphs.insert((c, px.to_bits()), r);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_glyph_is_rasterised_once_per_size() {
        let font = Font::inter();
        let mut atlas = GlyphAtlas::new(4096);
        let a = atlas.glyph(&font, '1', 12.0).unwrap();
        let b = atlas.glyph(&font, '1', 12.0).unwrap();
        let c = atlas.glyph(&font, '1', 24.0).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(atlas.len(), 2);
        assert!(c.height > a.height);
    }

    #[test]
    fn the_dirty_rectangle_covers_only_new_glyphs() {
        let font = Font::inter();
        let mut atlas = GlyphAtlas::new(4096);
        assert_eq!(atlas.take_dirty(), None);
        let a = atlas.glyph(&font, 'A', 16.0).unwrap();
        let b = atlas.glyph(&font, 'B', 16.0).unwrap();
        let d = atlas.take_dirty().unwrap();
        assert_eq!((d.x0, d.y0), (a.x, a.y.min(b.y)));
        assert_eq!(d.x1, b.x + b.width);
        assert!(d.y1 - d.y0 <= a.height.max(b.height));
        assert!((d.x1 - d.x0) < 64, "a dirty rect, not whole rows: {d:?}");
        // Cached glyphs dirty nothing.
        atlas.glyph(&font, 'A', 16.0).unwrap();
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
        let first = atlas.glyph(&font, 'W', 40.0).unwrap();
        let before = texels(&atlas, first);
        atlas.take_dirty();
        let mut px = 41;
        while atlas.size() == 128 {
            atlas.glyph(&font, 'W', px as f32).unwrap();
            px += 1;
        }
        assert_eq!(atlas.size(), 256);
        assert_eq!(atlas.glyph(&font, 'W', 40.0).unwrap(), first);
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
            match atlas.glyph(&font, 'W', px as f32) {
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
