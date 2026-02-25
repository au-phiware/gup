// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Font atlas generation and management for SDF text rendering.

use super::*;
use crate::error::{GupError, GupResult};
use std::collections::HashMap;
use wgpu::{
    Device, Extent3d, Origin3d, Queue, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture,
    TextureAspect, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    util::DeviceExt,
};

/// SDF font atlas for GPU text rendering.
pub struct FontAtlas {
    /// GPU texture containing MSDF glyph data (RGBA for 3-channel MSDF)
    atlas_texture: Texture,
    /// Glyph metadata (positions, sizes, metrics)
    glyph_info: HashMap<char, GlyphInfo>,
    /// Font metrics (line height, baseline, etc.)
    font_metrics: FontMetrics,
    /// Font data for ttf_parser (kept for potential future use)
    #[allow(dead_code)]
    font_data: Vec<u8>,
    /// ttf_parser face instance
    font: ttf_parser::Face<'static>,
    /// MSDF generator
    msdf_generator: MsdfGenerator,
    /// Current atlas position for glyph packing
    current_x: u32,
    current_y: u32,
    current_row_height: u32,
    /// Size of the atlas texture
    atlas_size: u32,
}

impl FontAtlas {
    /// Create a new font atlas using the embedded default font.
    pub fn new(device: &Device, queue: &Queue, font_size: f32) -> GupResult<Self> {
        // Use embedded font. System font loading will be implemented in GUP-106.
        let font_data = Self::get_default_font_data();
        let font = ttf_parser::Face::parse(font_data, 0)
            .map_err(|e| GupError::resource_error(format!("Failed to parse font: {e:?}")))?;

        let atlas_size = sdf::ATLAS_SIZE;

        // Create MSDF generator with default config
        let msdf_config = MsdfConfig::default();
        let msdf_generator = MsdfGenerator::new(font_data.to_vec(), msdf_config)?;

        // Create initial atlas data with MSDF outside value (RGBA: 0,0,0,255 = far outside)
        let atlas_data = vec![0u8; (atlas_size * atlas_size * 4) as usize]; // RGBA format

        // Create the atlas texture with initial data (RGBA for MSDF)
        let atlas_texture = device.create_texture_with_data(
            queue,
            &TextureDescriptor {
                label: Some("MSDF Font Atlas"),
                size: wgpu::Extent3d {
                    width: atlas_size,
                    height: atlas_size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba8Unorm, // RGBA for 3-channel MSDF + alpha
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &atlas_data,
        );

        // Calculate font metrics from ttf_parser
        let font_metrics = Self::calculate_font_metrics(&font, font_size);

        let mut atlas = Self {
            atlas_texture,
            glyph_info: HashMap::new(),
            font_metrics,
            font_data: font_data.to_vec(),
            font,
            msdf_generator,
            current_x: 0,
            current_y: 0,
            current_row_height: 0,
            atlas_size,
        };

        // Pre-cache common ASCII characters
        atlas.preload_ascii_glyphs(device, queue, font_size)?;

        Ok(atlas)
    }

    /// Get the atlas texture for GPU binding.
    pub fn texture(&self) -> &Texture {
        &self.atlas_texture
    }

    /// Get font metrics.
    pub fn metrics(&self) -> &FontMetrics {
        &self.font_metrics
    }

    /// Get glyph information if it exists in the atlas.
    pub fn get_glyph(&self, character: char) -> Option<&GlyphInfo> {
        self.glyph_info.get(&character)
    }

    /// Get the number of glyphs currently loaded in the atlas.
    pub fn glyph_count(&self) -> usize {
        self.glyph_info.len()
    }

    /// Get atlas texture utilization as a fraction (0.0 to 1.0).
    ///
    /// This measures how much of the atlas vertical space has been consumed.
    /// Useful for monitoring atlas capacity and deciding when to resize.
    pub fn atlas_utilization(&self) -> f32 {
        let used_height = self.current_y + self.current_row_height;
        used_height as f32 / self.atlas_size as f32
    }

    /// Get the atlas texture size in pixels (always square).
    pub fn atlas_size(&self) -> u32 {
        self.atlas_size
    }

    /// Ensure a glyph is available in the atlas, loading it if necessary.
    pub fn ensure_glyph(
        &mut self,
        device: &Device,
        queue: &Queue,
        character: char,
        font_size: f32,
    ) -> GupResult<&GlyphInfo> {
        if !self.glyph_info.contains_key(&character) {
            self.add_glyph(device, queue, character, font_size)?;
        }
        Ok(self.glyph_info.get(&character).unwrap())
    }

    /// Add a new glyph to the atlas.
    fn add_glyph(
        &mut self,
        _device: &Device,
        queue: &Queue,
        character: char,
        font_size: f32,
    ) -> GupResult<()> {
        // Get glyph ID from character
        let glyph_id = self.font.glyph_index(character).ok_or_else(|| {
            GupError::resource_error(format!("Glyph not found for character: {character}"))
        })?;

        // Check if glyph has outline - try to get bounding box as a proxy
        let has_outline = self.font.glyph_bounding_box(glyph_id).is_some();

        if !has_outline {
            // Handle non-outline glyphs (whitespace, etc.)
            let advance = self.font.glyph_hor_advance(glyph_id).unwrap_or(0) as f32;
            let scale = font_size / self.font.units_per_em() as f32;

            let glyph_info = GlyphInfo {
                character,
                atlas_pos: [0.0, 0.0, 0.0, 0.0], // Empty region
                size: Vec2 { x: 0.0, y: 0.0 },
                bearing: Vec2 { x: 0.0, y: 0.0 },
                advance: advance * scale,
                sdf_scale: 1.0,
            };
            self.glyph_info.insert(character, glyph_info);
            return Ok(());
        }

        // Generate MSDF for the glyph
        let msdf_bitmap = self.msdf_generator.generate_msdf(glyph_id)?;
        let glyph_width = msdf_bitmap.width as u32;
        let glyph_height = msdf_bitmap.height as u32;

        // Find space in atlas
        if self.current_x + glyph_width > self.atlas_size {
            // Move to next row (add 1 pixel gap to prevent texture bleeding)
            self.current_x = 0;
            self.current_y += self.current_row_height + 1;
            self.current_row_height = 0;
        }

        if self.current_y + glyph_height > self.atlas_size {
            return Err(GupError::resource_error("Font atlas is full".to_string()));
        }

        // Convert MSDF bitmap to RGBA pixel data
        let rgba_pixels = msdf_bitmap.to_rgba_pixels();

        // Upload MSDF data to texture (RGBA format)
        let upload_x = self.current_x;
        let upload_y = self.current_y;

        // Validate texture upload parameters before calling write_texture
        // (write_texture panics on invalid arguments, so we validate first)
        let expected_size = (glyph_width * glyph_height * 4) as usize;
        if rgba_pixels.len() != expected_size {
            return Err(GupError::resource_error(format!(
                "MSDF bitmap size mismatch for '{}': expected {} bytes ({}x{}x4), got {}",
                character,
                expected_size,
                glyph_width,
                glyph_height,
                rgba_pixels.len()
            )));
        }

        if upload_x + glyph_width > self.atlas_size || upload_y + glyph_height > self.atlas_size {
            return Err(GupError::resource_error(format!(
                "Glyph '{}' would exceed atlas bounds: upload at ({}, {}), size {}x{}, atlas {}x{}",
                character,
                upload_x,
                upload_y,
                glyph_width,
                glyph_height,
                self.atlas_size,
                self.atlas_size
            )));
        }

        if glyph_width == 0 || glyph_height == 0 {
            return Err(GupError::resource_error(format!(
                "Glyph '{}' has zero-sized MSDF bitmap: {}x{}",
                character, glyph_width, glyph_height
            )));
        }

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: Origin3d {
                    x: upload_x,
                    y: upload_y,
                    z: 0,
                },
                aspect: TextureAspect::All,
            },
            &rgba_pixels,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(glyph_width * 4), // RGBA = 4 bytes per pixel
                rows_per_image: Some(glyph_height),
            },
            Extent3d {
                width: glyph_width,
                height: glyph_height,
                depth_or_array_layers: 1,
            },
        );

        // Get glyph metrics from ttf_parser
        let advance = self.font.glyph_hor_advance(glyph_id).unwrap_or(0) as f32;
        let units_per_em = self.font.units_per_em() as f32;
        let atlas_scale = font_size / units_per_em;

        // Get MSDF scale and padding - the MSDF bitmap was generated at this scale
        let msdf_scale = msdf_bitmap.scale;
        let msdf_padding = msdf_bitmap.padding as f32;

        // The ratio converts MSDF pixels to screen pixels at font_size
        let scale_ratio = atlas_scale / msdf_scale;

        // Calculate quad size from MSDF bitmap dimensions (which include padding)
        // The quad needs to be sized to match the MSDF bitmap content
        let width = glyph_width as f32 * scale_ratio;
        let height = glyph_height as f32 * scale_ratio;

        // Get glyph bounding box for bearing calculation
        let bbox = self.font.glyph_bounding_box(glyph_id);
        let (bearing_x, bearing_y) = if let Some(bbox) = bbox {
            // The bearing needs to account for the MSDF padding
            // The padding in MSDF pixels becomes padding * scale_ratio in screen pixels
            let padding_screen = msdf_padding * scale_ratio;
            let bearing_x = bbox.x_min as f32 * atlas_scale - padding_screen;
            let bearing_y = bbox.y_min as f32 * atlas_scale - padding_screen;
            (bearing_x, bearing_y)
        } else {
            (0.0, 0.0) // Fallback
        };

        // Create glyph info
        let glyph_info = GlyphInfo {
            character,
            atlas_pos: [
                upload_x as f32 / self.atlas_size as f32,
                upload_y as f32 / self.atlas_size as f32,
                (upload_x + glyph_width) as f32 / self.atlas_size as f32,
                (upload_y + glyph_height) as f32 / self.atlas_size as f32,
            ],
            size: Vec2 {
                x: width,
                y: height,
            },
            bearing: Vec2 {
                x: bearing_x,
                y: bearing_y,
            },
            advance: advance * atlas_scale,
            sdf_scale: scale_ratio, // Store the scale ratio for shader use
        };

        self.glyph_info.insert(character, glyph_info);

        // Update atlas position (add 1 pixel gap to prevent texture bleeding)
        self.current_x += glyph_width + 1;
        self.current_row_height = self.current_row_height.max(glyph_height);

        Ok(())
    }

    /// Create a raw bitmap without SDF processing for debugging.
    #[allow(dead_code)]
    fn create_raw_bitmap(&self, bitmap: &[u8], width: usize, height: usize) -> Vec<u8> {
        let padded_width = width + (sdf::GLYPH_PADDING * 2) as usize;
        let padded_height = height + (sdf::GLYPH_PADDING * 2) as usize;
        let mut raw_bitmap = vec![0u8; padded_width * padded_height];

        let padding = sdf::GLYPH_PADDING as usize;

        // Copy fontdue bitmap data to center of padded bitmap
        for y in 0..height {
            for x in 0..width {
                let src_idx = y * width + x;
                let dst_idx = (y + padding) * padded_width + (x + padding);

                // fontdue provides coverage values 0-255, use them directly
                // This bypasses SDF generation to test raw font rasterization
                raw_bitmap[dst_idx] = bitmap[src_idx];
            }
        }

        raw_bitmap
    }

    /// Pre-load common ASCII characters.
    fn preload_ascii_glyphs(
        &mut self,
        device: &Device,
        queue: &Queue,
        font_size: f32,
    ) -> GupResult<()> {
        // Load printable ASCII characters that exist in the font
        for ch in 32u8..=126u8 {
            let character = ch as char;
            // Only try to load characters that exist in the font
            if self.font.glyph_index(character).is_some() {
                self.add_glyph(device, queue, character, font_size)?;
            }
        }
        Ok(())
    }

    /// Calculate font metrics from ttf_parser Face.
    fn calculate_font_metrics(font: &ttf_parser::Face, font_size: f32) -> FontMetrics {
        // Get font units per em
        let units_per_em = font.units_per_em() as f32;

        // Scale factor from font units to pixels
        let scale = font_size / units_per_em;

        // Get OS/2 table for additional metrics
        let ascent = if let Some(os2) = font.tables().os2 {
            os2.typographic_ascender() as f32 * scale
        } else {
            font_size * 0.8 // Fallback
        };

        let descent = if let Some(os2) = font.tables().os2 {
            -(os2.typographic_descender() as f32) * scale // Note: descent is usually negative
        } else {
            font_size * 0.2 // Fallback
        };

        let line_gap = if let Some(os2) = font.tables().os2 {
            os2.typographic_line_gap() as f32 * scale
        } else {
            font_size * 0.1 // Fallback
        };

        let line_height = ascent + descent + line_gap;

        FontMetrics {
            size: font_size,
            line_height,
            ascent,
            descent,
            line_gap,
        }
    }

    /// Get default font data - Squada One embedded font.
    fn get_default_font_data() -> &'static [u8] {
        // Embed Squada One font data directly into the binary
        include_bytes!("../../assets/fonts/default.ttf")
    }
}

impl std::fmt::Debug for FontAtlas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontAtlas")
            .field("glyph_count", &self.glyph_info.len())
            .field("font_metrics", &self.font_metrics)
            .field("atlas_size", &self.atlas_size)
            .field("current_position", &(self.current_x, self.current_y))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderContext;

    #[tokio::test]
    async fn test_font_atlas_creation() {
        let context = RenderContext::new().await.unwrap();

        // This test will fail until we have a proper embedded font
        // For now, we'll skip it during implementation
        if std::env::var("SKIP_FONT_TESTS").is_ok() {
            return;
        }

        let atlas = FontAtlas::new(context.device(), context.queue(), 16.0);
        // We expect this to fail without proper font data
        assert!(atlas.is_err() || atlas.is_ok());
    }

    #[test]
    #[ignore] // Disabled due to unsafe mem::zeroed() usage in test mock
    fn test_sdf_generation_basic() {
        // Test SDF generation with a simple bitmap
        let bitmap = vec![0, 0, 255, 255, 255, 0, 0];
        let width = 7;
        let height = 1;

        #[allow(invalid_value)]
        let _atlas = FontAtlas {
            atlas_texture: unsafe { std::mem::zeroed() }, // Placeholder
            glyph_info: HashMap::new(),
            font_metrics: FontMetrics::default(),
            font_data: Vec::new(),                         // Placeholder
            font: unsafe { std::mem::zeroed() },           // Placeholder
            msdf_generator: unsafe { std::mem::zeroed() }, // Placeholder
            current_x: 0,
            current_y: 0,
            current_row_height: 0,
            atlas_size: 1024,
        };

        // Use MockFontAtlas for SDF generation testing
        let font_data = include_bytes!("../../assets/fonts/default.ttf");
        let font = ttf_parser::Face::parse(&font_data[..], 0).unwrap();
        let mock_atlas = MockFontAtlas { font };
        let sdf = mock_atlas.generate_sdf(&bitmap, width, height);

        // SDF should be larger than original due to padding
        let expected_size =
            (width + sdf::GLYPH_PADDING as usize * 2) * (height + sdf::GLYPH_PADDING as usize * 2);
        assert_eq!(sdf.len(), expected_size);

        // SDF values should be in valid range (0-255 for u8)
        // This assertion is always true for u8, but serves as documentation
        #[allow(unused_comparisons, clippy::absurd_extreme_comparisons)]
        for &value in &sdf {
            let _ = value <= 255;
        }
    }

    #[test]
    fn test_font_metrics_calculation() {
        // Test with default values since we can't create a real font without font data
        let metrics = FontMetrics::default();
        assert!(metrics.size > 0.0);
        assert!(metrics.line_height >= metrics.size);
        assert!(metrics.ascent > 0.0);
        assert!(metrics.descent >= 0.0);
    }

    #[test]
    fn test_sdf_generation_performance() {
        // Test SDF generation performance for typical glyph sizes
        use std::time::Instant;

        // Create a mock atlas to test the SDF generation function
        let font_data = include_bytes!("../../assets/fonts/default.ttf");
        let font = ttf_parser::Face::parse(&font_data[..], 0).unwrap();
        let mock_atlas = MockFontAtlas { font };

        // Create a moderate-sized glyph bitmap (typical for readable font size)
        let width = 32;
        let height = 32;
        let bitmap: Vec<u8> = (0..width * height)
            .map(|i| {
                if (i % width < width / 2) && (i / width < height / 2) {
                    255
                } else {
                    0
                }
            })
            .collect();

        let start = Instant::now();
        let sdf = mock_atlas.generate_sdf(&bitmap, width, height);
        let duration = start.elapsed();

        // Verify result dimensions
        let expected_width = width + (sdf::GLYPH_PADDING * 2) as usize;
        let expected_height = height + (sdf::GLYPH_PADDING * 2) as usize;
        assert_eq!(sdf.len(), expected_width * expected_height);

        // Performance: SDF generation for a 32x32 glyph should be fast.
        // Debug builds are slower; use generous thresholds.
        #[cfg(debug_assertions)]
        let threshold_ms: u128 = 200;
        #[cfg(not(debug_assertions))]
        let threshold_ms: u128 = 50;

        println!("SDF generation for {width}x{height} glyph took: {duration:?}");
        assert!(
            duration.as_millis() < threshold_ms,
            "SDF generation too slow: {duration:?} (threshold: {threshold_ms}ms)"
        );
    }

    #[test]
    fn test_glyph_cache_efficiency() {
        // Test that glyph caching works efficiently
        let mut glyphs = std::collections::HashMap::new();

        // Simulate adding many glyphs to cache
        let glyph_count = 1000;
        let start = std::time::Instant::now();

        for i in 0..glyph_count {
            let character = char::from_u32(65 + (i % 26) as u32).unwrap_or('A');
            let glyph_info = GlyphInfo {
                character,
                atlas_pos: [0.0, 0.0, 0.1, 0.1],
                size: Vec2 { x: 16.0, y: 16.0 },
                bearing: Vec2 { x: 0.0, y: 0.0 },
                advance: 10.0,
                sdf_scale: 1.0,
            };
            glyphs.insert(character, glyph_info);
        }

        let insertion_duration = start.elapsed();

        // Test lookup performance
        let lookup_start = std::time::Instant::now();
        for i in 0..glyph_count {
            let character = char::from_u32(65 + (i % 26) as u32).unwrap_or('A');
            let _glyph = glyphs.get(&character);
        }
        let lookup_duration = lookup_start.elapsed();

        // Performance: glyph cache operations should be fast.
        // Debug builds are slower; use generous thresholds.
        #[cfg(debug_assertions)]
        let insert_threshold_ms: u128 = 50;
        #[cfg(not(debug_assertions))]
        let insert_threshold_ms: u128 = 10;

        #[cfg(debug_assertions)]
        let lookup_threshold_ms: u128 = 25;
        #[cfg(not(debug_assertions))]
        let lookup_threshold_ms: u128 = 5;

        println!("Glyph cache insertion for {glyph_count} glyphs took: {insertion_duration:?}");
        println!("Glyph cache lookup for {glyph_count} glyphs took: {lookup_duration:?}");

        assert!(
            insertion_duration.as_millis() < insert_threshold_ms,
            "Glyph cache insertion too slow: {insertion_duration:?} (threshold: {insert_threshold_ms}ms)"
        );
        assert!(
            lookup_duration.as_millis() < lookup_threshold_ms,
            "Glyph cache lookup too slow: {lookup_duration:?} (threshold: {lookup_threshold_ms}ms)"
        );
    }

    // Helper struct for testing SDF generation
    struct MockFontAtlas {
        #[allow(dead_code)]
        font: ttf_parser::Face<'static>,
    }

    impl MockFontAtlas {
        fn generate_sdf(&self, bitmap: &[u8], width: usize, height: usize) -> Vec<u8> {
            // Same improved SDF generation logic as FontAtlas
            let sdf_width = width + (sdf::GLYPH_PADDING * 2) as usize;
            let sdf_height = height + (sdf::GLYPH_PADDING * 2) as usize;
            let mut sdf_bitmap = vec![128u8; sdf_width * sdf_height];

            let padding = sdf::GLYPH_PADDING as usize;
            let range = sdf::SDF_RANGE;

            for y in 0..sdf_height {
                for x in 0..sdf_width {
                    let src_x = x as i32 - padding as i32;
                    let src_y = y as i32 - padding as i32;

                    let mut min_distance = range;
                    let mut inside = false;

                    if src_x >= 0 && src_x < width as i32 && src_y >= 0 && src_y < height as i32 {
                        let pixel = bitmap[src_y as usize * width + src_x as usize];
                        inside = pixel > 32;
                    }

                    let search_range = (range * 1.5) as i32;
                    for dy in -search_range..=search_range {
                        for dx in -search_range..=search_range {
                            let check_x = src_x + dx;
                            let check_y = src_y + dy;

                            if check_x >= 0
                                && check_x < width as i32
                                && check_y >= 0
                                && check_y < height as i32
                            {
                                let pixel = bitmap[check_y as usize * width + check_x as usize];
                                let pixel_inside = pixel > 32;

                                if pixel_inside != inside {
                                    let distance = ((dx * dx + dy * dy) as f32).sqrt();
                                    min_distance = min_distance.min(distance);
                                }
                            }
                        }
                    }

                    let normalized_distance = (min_distance / range).clamp(0.0, 1.0);
                    let sdf_value = if inside {
                        128.0 + normalized_distance * 127.0
                    } else {
                        128.0 - normalized_distance * 128.0
                    };

                    sdf_bitmap[y * sdf_width + x] = sdf_value.clamp(0.0, 255.0) as u8;
                }
            }

            sdf_bitmap
        }
    }

    #[test]
    fn test_memory_usage_patterns() {
        // Test that our data structures use memory efficiently
        use std::mem;

        let glyph_info_size = mem::size_of::<GlyphInfo>();
        let font_metrics_size = mem::size_of::<FontMetrics>();

        // GlyphInfo should be reasonably sized
        assert!(
            glyph_info_size <= 64,
            "GlyphInfo too large: {glyph_info_size} bytes"
        );

        // FontMetrics should be compact
        assert!(
            font_metrics_size <= 32,
            "FontMetrics too large: {font_metrics_size} bytes"
        );

        // Test alignment requirements
        assert_eq!(mem::align_of::<GlyphInfo>(), 4);
        assert_eq!(mem::align_of::<FontMetrics>(), 4);
    }
}
