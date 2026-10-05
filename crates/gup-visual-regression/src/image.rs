// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A plain, renderer-independent RGBA image.

use crate::color::Rgba8;
use crate::layout::PxRect;
use std::fmt;
use std::path::Path;

/// Errors produced while constructing, decoding or encoding an [`RgbaImage`].
#[derive(Debug)]
pub enum ImageError {
    /// The byte buffer length does not equal `width * height * 4`.
    SizeMismatch {
        /// Image width in pixels.
        width: u32,
        /// Image height in pixels.
        height: u32,
        /// Actual buffer length in bytes.
        len: usize,
    },
    /// PNG decoding failed.
    Decode(String),
    /// PNG encoding failed.
    Encode(String),
    /// Reading or writing a file failed.
    Io {
        /// The file involved.
        path: String,
        /// The underlying error.
        error: std::io::Error,
    },
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageError::SizeMismatch { width, height, len } => write!(
                f,
                "RGBA buffer of {len} bytes does not match {width}x{height} (expected {})",
                *width as usize * *height as usize * 4
            ),
            ImageError::Decode(e) => write!(f, "PNG decode failed: {e}"),
            ImageError::Encode(e) => write!(f, "PNG encode failed: {e}"),
            ImageError::Io { path, error } => write!(f, "{path}: {error}"),
        }
    }
}

impl std::error::Error for ImageError {}

/// A tightly packed, row-major, 8-bit RGBA image with the origin at the
/// top-left corner.
///
/// This is the only image type the harness's checks accept. Any renderer can
/// be validated by converting its output to an `RgbaImage`.
#[derive(Clone, PartialEq, Eq)]
pub struct RgbaImage {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

impl fmt::Debug for RgbaImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RgbaImage({}x{})", self.width, self.height)
    }
}

impl RgbaImage {
    /// Wrap an RGBA byte buffer. Fails if `data.len() != width * height * 4`.
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self, ImageError> {
        if data.len() != width as usize * height as usize * 4 {
            return Err(ImageError::SizeMismatch {
                width,
                height,
                len: data.len(),
            });
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    /// Create an image filled with a single colour.
    pub fn filled(width: u32, height: u32, color: Rgba8) -> Self {
        let mut data = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..(width as usize * height as usize) {
            data.extend_from_slice(&[color.r, color.g, color.b, color.a]);
        }
        Self {
            width,
            height,
            data,
        }
    }

    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The raw RGBA bytes.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Total number of pixels.
    pub fn pixel_count(&self) -> usize {
        self.width as usize * self.height as usize
    }

    /// The colour at `(x, y)`. Panics if out of bounds.
    pub fn pixel(&self, x: u32, y: u32) -> Rgba8 {
        assert!(
            x < self.width && y < self.height,
            "pixel ({x}, {y}) out of bounds"
        );
        let i = (y as usize * self.width as usize + x as usize) * 4;
        Rgba8::new(
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        )
    }

    /// Set the colour at `(x, y)`. Panics if out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, c: Rgba8) {
        assert!(
            x < self.width && y < self.height,
            "pixel ({x}, {y}) out of bounds"
        );
        let i = (y as usize * self.width as usize + x as usize) * 4;
        self.data[i..i + 4].copy_from_slice(&[c.r, c.g, c.b, c.a]);
    }

    /// Fill every pixel whose centre lies inside `rect` with `c`.
    pub fn fill_rect(&mut self, rect: PxRect, c: Rgba8) {
        if let Some((x0, y0, x1, y1)) = rect.pixel_span(self.width, self.height) {
            for y in y0..y1 {
                for x in x0..x1 {
                    self.set_pixel(x, y, c);
                }
            }
        }
    }

    /// Iterate over `(x, y, colour)` for every pixel.
    pub fn pixels(&self) -> impl Iterator<Item = (u32, u32, Rgba8)> + '_ {
        let w = self.width;
        self.data
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .map(move |(i, &[r, g, b, a])| (i as u32 % w, i as u32 / w, Rgba8::new(r, g, b, a)))
    }

    /// Decode a PNG (any 8-bit or 16-bit colour type) into RGBA8.
    pub fn from_png(bytes: &[u8]) -> Result<Self, ImageError> {
        let mut decoder = png::Decoder::new(bytes);
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder
            .read_info()
            .map_err(|e| ImageError::Decode(e.to_string()))?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut buf)
            .map_err(|e| ImageError::Decode(e.to_string()))?;
        buf.truncate(info.buffer_size());
        let (w, h) = (info.width, info.height);
        let data = match info.color_type {
            png::ColorType::Rgba => buf,
            png::ColorType::Rgb => buf
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|&[r, g, b]| [r, g, b, 255])
                .collect(),
            png::ColorType::Grayscale => buf.iter().flat_map(|&v| [v, v, v, 255]).collect(),
            png::ColorType::GrayscaleAlpha => buf
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|&[v, a]| [v, v, v, a])
                .collect(),
            png::ColorType::Indexed => {
                return Err(ImageError::Decode(
                    "indexed PNG was not expanded".to_string(),
                ));
            }
        };
        Self::new(w, h, data)
    }

    /// Encode as an RGBA8 PNG.
    pub fn to_png(&self) -> Result<Vec<u8>, ImageError> {
        let mut out = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|e| ImageError::Encode(e.to_string()))?;
            writer
                .write_image_data(&self.data)
                .map_err(|e| ImageError::Encode(e.to_string()))?;
        }
        Ok(out)
    }

    /// Read and decode a PNG file.
    pub fn load_png(path: impl AsRef<Path>) -> Result<Self, ImageError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|error| ImageError::Io {
            path: path.display().to_string(),
            error,
        })?;
        Self::from_png(&bytes)
    }

    /// Encode and write a PNG file, creating parent directories as needed.
    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<(), ImageError> {
        let path = path.as_ref();
        let io = |error| ImageError::Io {
            path: path.display().to_string(),
            error,
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        std::fs::write(path, self.to_png()?).map_err(io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_wrong_buffer_length() {
        assert!(RgbaImage::new(2, 2, vec![0; 15]).is_err());
        assert!(RgbaImage::new(2, 2, vec![0; 16]).is_ok());
    }

    #[test]
    fn png_round_trip_preserves_pixels() {
        let mut img = RgbaImage::filled(5, 3, Rgba8::WHITE);
        img.set_pixel(4, 2, Rgba8::new(10, 20, 30, 128));
        let back = RgbaImage::from_png(&img.to_png().unwrap()).unwrap();
        assert_eq!(back, img);
    }

    #[test]
    fn pixels_iterates_row_major() {
        let mut img = RgbaImage::filled(3, 2, Rgba8::BLACK);
        img.set_pixel(2, 1, Rgba8::WHITE);
        let last = img.pixels().last().unwrap();
        assert_eq!(last, (2, 1, Rgba8::WHITE));
    }

    #[test]
    fn fill_rect_covers_pixel_centres() {
        let mut img = RgbaImage::filled(10, 10, Rgba8::WHITE);
        img.fill_rect(PxRect::new(2.0, 2.0, 3.0, 1.0), Rgba8::BLACK);
        let black = img.pixels().filter(|p| p.2 == Rgba8::BLACK).count();
        assert_eq!(black, 3);
        assert_eq!(img.pixel(2, 2), Rgba8::BLACK);
        assert_eq!(img.pixel(5, 2), Rgba8::WHITE);
    }
}
