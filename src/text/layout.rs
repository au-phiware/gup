// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text layout and positioning with collision detection.

use super::*;
use crate::error::GupResult;
use std::collections::HashSet;
use unicode_normalization::UnicodeNormalization;

/// Text layout engine for positioning and collision detection.
pub struct TextLayoutEngine {
    /// Collision detection grid for performance
    collision_grid: CollisionGrid,
}

/// Grid-based collision detection for efficient overlap checking.
struct CollisionGrid {
    /// Grid cell size in pixels
    cell_size: f32,
    /// Occupied grid cells
    occupied_cells: HashSet<(i32, i32)>,
    /// All placed text bounds for detailed collision checking
    placed_bounds: Vec<TextBounds>,
}

/// Result of text layout operation.
#[derive(Debug, Clone)]
pub struct LayoutResult {
    /// Final positioned glyphs ready for rendering
    pub glyphs: GlyphBatch,
    /// Bounding rectangle of the entire text
    pub bounds: TextBounds,
    /// Whether any text was clipped or hidden
    pub clipped: bool,
}

impl TextLayoutEngine {
    /// Create a new text layout engine.
    pub fn new() -> Self {
        Self {
            collision_grid: CollisionGrid::new(32.0), // 32px grid cells
        }
    }

    /// Layout text with the given style and constraints.
    pub fn layout_text(
        &mut self,
        text: &str,
        position: Vec2,
        style: &TextStyle,
        font_atlas: &FontAtlas,
        constraints: Option<&LayoutConstraints>,
    ) -> GupResult<LayoutResult> {
        // Normalize Unicode text
        let normalized_text: String = text.nfc().collect();

        // Measure text to get basic bounds
        let text_bounds = self.measure_text(&normalized_text, style, font_atlas)?;

        // Adjust position based on anchor
        let adjusted_position = self.apply_anchor(position, &text_bounds, style.anchor);

        // Check for collisions if enabled
        let final_position = if let Some(constraints) = constraints {
            if constraints.avoid_collisions {
                self.find_collision_free_position(adjusted_position, &text_bounds, constraints)?
            } else {
                adjusted_position
            }
        } else {
            adjusted_position
        };

        // Generate positioned glyphs
        let glyphs = self.position_glyphs(&normalized_text, final_position, style, font_atlas)?;

        // Calculate final bounds
        let final_bounds = self.calculate_glyph_bounds(&glyphs);

        // Add to collision grid for future collision detection
        self.collision_grid.add_bounds(&final_bounds);

        Ok(LayoutResult {
            glyphs,
            bounds: final_bounds,
            clipped: false, // TODO: Implement clipping detection
        })
    }

    /// Measure text without rendering to get bounds.
    pub fn measure_text(
        &self,
        text: &str,
        style: &TextStyle,
        font_atlas: &FontAtlas,
    ) -> GupResult<TextBounds> {
        let metrics = font_atlas.metrics();
        let scale = style.font_size / metrics.size;

        let mut width = 0.0f32;
        let mut min_y = 0.0f32;
        let mut max_y = metrics.line_height * scale;

        for ch in text.chars() {
            if let Some(glyph) = font_atlas.get_glyph(ch) {
                width += glyph.advance * scale * style.letter_spacing;

                // Update vertical bounds based on glyph bearing
                let glyph_top = glyph.bearing.y * scale;
                let glyph_bottom = glyph_top + glyph.size.y * scale;
                min_y = min_y.min(glyph_top);
                max_y = max_y.max(glyph_bottom);
            } else {
                // Fallback for missing glyphs - use average character width
                width += metrics.size * 0.5 * scale * style.letter_spacing;
            }
        }

        Ok(TextBounds {
            left: 0.0,
            top: min_y,
            right: width,
            bottom: max_y,
        })
    }

    /// Apply anchor positioning to adjust text position.
    fn apply_anchor(&self, position: Vec2, bounds: &TextBounds, anchor: TextAnchor) -> Vec2 {
        let offset = anchor.offset();
        Vec2 {
            x: position.x - bounds.width() * offset.x,
            y: position.y - bounds.height() * offset.y,
        }
    }

    /// Find a collision-free position for text.
    fn find_collision_free_position(
        &self,
        preferred_position: Vec2,
        bounds: &TextBounds,
        constraints: &LayoutConstraints,
    ) -> GupResult<Vec2> {
        // Create bounds at preferred position
        let mut test_bounds = *bounds;
        test_bounds.left += preferred_position.x;
        test_bounds.right += preferred_position.x;
        test_bounds.top += preferred_position.y;
        test_bounds.bottom += preferred_position.y;

        // Check if preferred position is free
        if !self.collision_grid.has_collision(&test_bounds) {
            return Ok(preferred_position);
        }

        // Try nearby positions in a spiral pattern
        let max_offset = constraints.max_collision_offset.unwrap_or(50.0);
        let step_size = constraints.collision_step_size.unwrap_or(4.0);

        for radius in (step_size as i32..=max_offset as i32).step_by(step_size as usize) {
            for angle_steps in 0..(radius * 2) {
                let angle = (angle_steps as f32 / (radius * 2) as f32) * 2.0 * std::f32::consts::PI;
                let offset_x = radius as f32 * angle.cos();
                let offset_y = radius as f32 * angle.sin();

                let test_position = Vec2 {
                    x: preferred_position.x + offset_x,
                    y: preferred_position.y + offset_y,
                };

                // Update test bounds
                test_bounds.left = bounds.left + test_position.x;
                test_bounds.right = bounds.right + test_position.x;
                test_bounds.top = bounds.top + test_position.y;
                test_bounds.bottom = bounds.bottom + test_position.y;

                if !self.collision_grid.has_collision(&test_bounds) {
                    return Ok(test_position);
                }
            }
        }

        // If no collision-free position found, return preferred position
        // In a more sophisticated implementation, we might hide the text or
        // apply other strategies like rotation or size reduction
        Ok(preferred_position)
    }

    /// Generate positioned glyphs for rendering.
    fn position_glyphs(
        &self,
        text: &str,
        position: Vec2,
        style: &TextStyle,
        font_atlas: &FontAtlas,
    ) -> GupResult<GlyphBatch> {
        let metrics = font_atlas.metrics();
        let scale = style.font_size / metrics.size;
        let mut glyphs = Vec::new();

        let mut cursor_x = position.x;
        let baseline_y = position.y + metrics.ascent * scale;

        for ch in text.chars() {
            if let Some(glyph_info) = font_atlas.get_glyph(ch) {
                // Skip rendering for whitespace characters
                if glyph_info.size.x > 0.0 && glyph_info.size.y > 0.0 {
                    let glyph_position = Vec2 {
                        x: cursor_x + glyph_info.bearing.x * scale,
                        y: baseline_y - glyph_info.bearing.y * scale,
                    };

                    // Apply rotation if needed
                    let final_position = if style.is_rotated() {
                        let relative_pos = Vec2 {
                            x: glyph_position.x - position.x,
                            y: glyph_position.y - position.y,
                        };
                        let rotated_relative = style.rotate_point(relative_pos);
                        Vec2 {
                            x: position.x + rotated_relative.x,
                            y: position.y + rotated_relative.y,
                        }
                    } else {
                        glyph_position
                    };

                    glyphs.push(PositionedGlyph {
                        glyph: *glyph_info,
                        position: final_position,
                        color: style.color,
                    });
                }

                cursor_x += glyph_info.advance * scale * style.letter_spacing;
            } else {
                // Handle missing glyph
                cursor_x += metrics.size * 0.5 * scale * style.letter_spacing;
            }
        }

        Ok(glyphs)
    }

    /// Calculate bounding box from positioned glyphs.
    fn calculate_glyph_bounds(&self, glyphs: &GlyphBatch) -> TextBounds {
        if glyphs.is_empty() {
            return TextBounds::default();
        }

        let mut bounds = TextBounds {
            left: f32::INFINITY,
            top: f32::INFINITY,
            right: f32::NEG_INFINITY,
            bottom: f32::NEG_INFINITY,
        };

        for glyph in glyphs {
            let glyph_left = glyph.position.x;
            let glyph_right = glyph.position.x + glyph.glyph.size.x;
            let glyph_top = glyph.position.y;
            let glyph_bottom = glyph.position.y + glyph.glyph.size.y;

            bounds.left = bounds.left.min(glyph_left);
            bounds.right = bounds.right.max(glyph_right);
            bounds.top = bounds.top.min(glyph_top);
            bounds.bottom = bounds.bottom.max(glyph_bottom);
        }

        bounds
    }

    /// Clear all collision data.
    pub fn clear(&mut self) {
        self.collision_grid.clear();
    }
}

impl Default for TextLayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CollisionGrid {
    /// Create a new collision grid.
    fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            occupied_cells: HashSet::new(),
            placed_bounds: Vec::new(),
        }
    }

    /// Add bounds to the collision grid.
    fn add_bounds(&mut self, bounds: &TextBounds) {
        // Add to detailed bounds list
        self.placed_bounds.push(*bounds);

        // Mark grid cells as occupied
        let cells = self.get_cells_for_bounds(bounds);
        for cell in cells {
            self.occupied_cells.insert(cell);
        }
    }

    /// Check if bounds collide with existing text.
    fn has_collision(&self, bounds: &TextBounds) -> bool {
        // First, quick check using grid cells
        let cells = self.get_cells_for_bounds(bounds);
        for cell in &cells {
            if self.occupied_cells.contains(cell) {
                // Found potential collision, do detailed check
                return self.detailed_collision_check(bounds);
            }
        }
        false
    }

    /// Get grid cells that bounds intersect.
    fn get_cells_for_bounds(&self, bounds: &TextBounds) -> Vec<(i32, i32)> {
        let mut cells = Vec::new();

        let min_x = (bounds.left / self.cell_size).floor() as i32;
        let max_x = (bounds.right / self.cell_size).ceil() as i32;
        let min_y = (bounds.top / self.cell_size).floor() as i32;
        let max_y = (bounds.bottom / self.cell_size).ceil() as i32;

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                cells.push((x, y));
            }
        }

        cells
    }

    /// Perform detailed collision check against all placed bounds.
    fn detailed_collision_check(&self, bounds: &TextBounds) -> bool {
        for placed_bounds in &self.placed_bounds {
            if bounds.intersects(placed_bounds) {
                return true;
            }
        }
        false
    }

    /// Clear all collision data.
    fn clear(&mut self) {
        self.occupied_cells.clear();
        self.placed_bounds.clear();
    }
}

/// Layout constraints for text positioning.
#[derive(Debug, Clone)]
pub struct LayoutConstraints {
    /// Whether to avoid collisions with existing text
    pub avoid_collisions: bool,
    /// Maximum offset to try when avoiding collisions
    pub max_collision_offset: Option<f32>,
    /// Step size for collision avoidance search
    pub collision_step_size: Option<f32>,
    /// Available area for text placement
    pub clip_area: Option<TextBounds>,
    /// Whether to allow text rotation for collision avoidance
    pub allow_rotation: bool,
    /// Maximum rotation angle in radians
    pub max_rotation: f32,
}

impl Default for LayoutConstraints {
    fn default() -> Self {
        Self {
            avoid_collisions: true,
            max_collision_offset: Some(50.0),
            collision_step_size: Some(4.0),
            clip_area: None,
            allow_rotation: false,
            max_rotation: std::f32::consts::PI / 4.0, // 45 degrees
        }
    }
}

impl LayoutConstraints {
    /// Create constraints for axis labels.
    pub fn axis_labels() -> Self {
        Self {
            avoid_collisions: true,
            max_collision_offset: Some(30.0),
            collision_step_size: Some(2.0),
            allow_rotation: true,
            max_rotation: std::f32::consts::PI / 2.0, // 90 degrees
            ..Default::default()
        }
    }

    /// Create constraints for dense label placement.
    pub fn dense_labels() -> Self {
        Self {
            avoid_collisions: true,
            max_collision_offset: Some(20.0),
            collision_step_size: Some(1.0),
            allow_rotation: true,
            max_rotation: std::f32::consts::PI / 4.0, // 45 degrees
            ..Default::default()
        }
    }

    /// Create constraints for overlapping text (no collision avoidance).
    pub fn allow_overlap() -> Self {
        Self {
            avoid_collisions: false,
            allow_rotation: false,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_engine_creation() {
        let engine = TextLayoutEngine::new();
        assert_eq!(engine.collision_grid.cell_size, 32.0);
        assert!(engine.collision_grid.occupied_cells.is_empty());
    }

    // Note: Tests requiring FontAtlas are disabled since FontAtlas fields are private
    // In a full implementation, we would add public testing methods or constructors

    #[test]
    fn test_anchor_positioning() {
        let engine = TextLayoutEngine::new();
        let position = Vec2 { x: 100.0, y: 50.0 };
        let bounds = TextBounds::new(0.0, 0.0, 40.0, 20.0);

        // Top-left anchor should not change position
        let adjusted = engine.apply_anchor(position, &bounds, TextAnchor::TopLeft);
        assert_eq!(adjusted.x, 100.0);
        assert_eq!(adjusted.y, 50.0);

        // Center anchor should offset by half width/height
        let adjusted = engine.apply_anchor(position, &bounds, TextAnchor::Center);
        assert_eq!(adjusted.x, 80.0); // 100 - 40/2
        assert_eq!(adjusted.y, 40.0); // 50 - 20/2

        // Bottom-right anchor should offset by full width/height
        let adjusted = engine.apply_anchor(position, &bounds, TextAnchor::BottomRight);
        assert_eq!(adjusted.x, 60.0); // 100 - 40
        assert_eq!(adjusted.y, 30.0); // 50 - 20
    }

    #[test]
    fn test_collision_grid() {
        let mut grid = CollisionGrid::new(10.0);

        let bounds1 = TextBounds::new(5.0, 5.0, 15.0, 15.0);
        let bounds2 = TextBounds::new(20.0, 20.0, 30.0, 30.0);
        let bounds3 = TextBounds::new(10.0, 10.0, 25.0, 25.0); // Overlaps with both

        // Add first bounds
        grid.add_bounds(&bounds1);
        assert!(!grid.has_collision(&bounds2)); // Should not collide
        assert!(grid.has_collision(&bounds1)); // Should collide with itself
        assert!(grid.has_collision(&bounds3)); // Should collide with bounds1

        // Add second bounds
        grid.add_bounds(&bounds2);
        assert!(grid.has_collision(&bounds3)); // Should collide with both now
    }

    #[test]
    fn test_layout_constraints_presets() {
        let axis_constraints = LayoutConstraints::axis_labels();
        assert!(axis_constraints.avoid_collisions);
        assert!(axis_constraints.allow_rotation);
        assert_eq!(axis_constraints.max_rotation, std::f32::consts::PI / 2.0);

        let dense_constraints = LayoutConstraints::dense_labels();
        assert!(dense_constraints.avoid_collisions);
        assert_eq!(dense_constraints.max_collision_offset, Some(20.0));

        let overlap_constraints = LayoutConstraints::allow_overlap();
        assert!(!overlap_constraints.avoid_collisions);
        assert!(!overlap_constraints.allow_rotation);
    }

    #[test]
    fn test_bounds_intersection() {
        let bounds1 = TextBounds::new(0.0, 0.0, 10.0, 10.0);
        let bounds2 = TextBounds::new(5.0, 5.0, 15.0, 15.0);
        let bounds3 = TextBounds::new(20.0, 20.0, 30.0, 30.0);

        assert!(bounds1.intersects(&bounds2));
        assert!(bounds2.intersects(&bounds1));
        assert!(!bounds1.intersects(&bounds3));
        assert!(!bounds3.intersects(&bounds1));
    }

    #[test]
    fn test_bounds_union() {
        let mut bounds1 = TextBounds::new(0.0, 0.0, 10.0, 10.0);
        let bounds2 = TextBounds::new(5.0, 5.0, 20.0, 15.0);

        bounds1.union(&bounds2);
        assert_eq!(bounds1.left, 0.0);
        assert_eq!(bounds1.top, 0.0);
        assert_eq!(bounds1.right, 20.0);
        assert_eq!(bounds1.bottom, 15.0);
    }
}
