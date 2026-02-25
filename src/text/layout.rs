// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text layout and positioning with collision detection.

use super::*;
use crate::error::GupResult;
use std::collections::HashSet;
use unicode_normalization::UnicodeNormalization;

/// Viewport boundaries for text clipping detection.
#[derive(Debug, Clone)]
pub struct ViewportBounds {
    /// Visible area coordinates
    pub viewport_rect: TextBounds,
    /// Container-specific bounds (optional)
    pub container_bounds: Option<TextBounds>,
    /// Margin requirements for text padding
    pub text_margins: TextMargins,
}

/// Text margin requirements around container boundaries.
#[derive(Debug, Clone)]
pub struct TextMargins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Result of clipping detection analysis.
#[derive(Debug, Clone)]
pub enum ClippingResult {
    NoClipping,
    PartialClipping {
        clipped_edges: Vec<ClippedEdge>,
        visible_percentage: f32, // 0.0-1.0
    },
    CompletelyClipped,
}

impl ClippingResult {
    /// Returns `true` if any clipping was detected.
    pub fn is_clipped(&self) -> bool {
        !matches!(self, ClippingResult::NoClipping)
    }

    /// Returns `true` if the text is completely invisible.
    pub fn is_completely_clipped(&self) -> bool {
        matches!(self, ClippingResult::CompletelyClipped)
    }

    /// Returns the visible percentage (1.0 = fully visible, 0.0 = invisible).
    pub fn visible_percentage(&self) -> f32 {
        match self {
            ClippingResult::NoClipping => 1.0,
            ClippingResult::PartialClipping {
                visible_percentage, ..
            } => *visible_percentage,
            ClippingResult::CompletelyClipped => 0.0,
        }
    }

    /// Returns `true` if clipping occurs on the right edge.
    pub fn is_clipped_right(&self) -> bool {
        match self {
            ClippingResult::PartialClipping { clipped_edges, .. } => {
                clipped_edges.iter().any(|e| matches!(e, ClippedEdge::Right { .. }))
            }
            _ => false,
        }
    }
}

/// Specific edge where text is clipped.
#[derive(Debug, Clone)]
pub enum ClippedEdge {
    Top { overflow_pixels: f32 },
    Right { overflow_pixels: f32 },
    Bottom { overflow_pixels: f32 },
    Left { overflow_pixels: f32 },
}

/// Configuration for different text clipping strategies.
#[derive(Debug, Clone)]
pub struct ClippingStrategyConfig {
    pub primary_strategy: ClippingStrategy,
    pub fallback_strategies: Vec<ClippingStrategy>,
    pub minimum_visible_percentage: f32, // Don't render if less than X% visible
    pub enable_hover_reveal: bool,
}

/// Available strategies for handling clipped text.
#[derive(Debug, Clone)]
pub enum ClippingStrategy {
    /// Truncate text with ellipsis
    TruncateWithEllipsis {
        ellipsis_text: String, // Default: "..."
        preserve_words: bool,  // Try to break at word boundaries
    },
    /// Reduce font size to fit
    DynamicFontScaling {
        min_font_size: f32,
        scale_factor: f32, // How aggressively to scale (0.1 = 10% reduction per step)
    },
    /// Move text to stay within bounds
    RepositionText {
        prefer_directions: Vec<Vec2>, // Preferred offset directions
        max_offset_distance: f32,
    },
    /// Hide text completely if it doesn't fit
    HideIfClipped {
        min_visible_threshold: f32, // Hide if less than X% visible
    },
}

impl Default for TextMargins {
    fn default() -> Self {
        Self {
            top: 4.0,
            right: 4.0,
            bottom: 4.0,
            left: 4.0,
        }
    }
}

impl Default for ClippingStrategyConfig {
    fn default() -> Self {
        Self {
            primary_strategy: ClippingStrategy::TruncateWithEllipsis {
                ellipsis_text: "...".to_string(),
                preserve_words: true,
            },
            fallback_strategies: vec![
                ClippingStrategy::DynamicFontScaling {
                    min_font_size: 8.0,
                    scale_factor: 0.1,
                },
                ClippingStrategy::HideIfClipped {
                    min_visible_threshold: 0.3,
                },
            ],
            minimum_visible_percentage: 0.5,
            enable_hover_reveal: false,
        }
    }
}

impl ViewportBounds {
    /// Create viewport bounds from screen dimensions.
    pub fn from_screen(width: f32, height: f32) -> Self {
        Self {
            viewport_rect: TextBounds::new(0.0, 0.0, width, height),
            container_bounds: None,
            text_margins: TextMargins::default(),
        }
    }

    /// Create viewport bounds with a specific container.
    pub fn from_container(container: TextBounds) -> Self {
        Self {
            viewport_rect: container,
            container_bounds: Some(container),
            text_margins: TextMargins::default(),
        }
    }

    /// Set custom text margins.
    pub fn with_margins(mut self, margins: TextMargins) -> Self {
        self.text_margins = margins;
        self
    }

    /// Get the effective bounding area accounting for margins.
    pub fn effective_bounds(&self) -> TextBounds {
        let base = self.container_bounds.unwrap_or(self.viewport_rect);
        TextBounds::new(
            base.left + self.text_margins.left,
            base.top + self.text_margins.top,
            base.right - self.text_margins.right,
            base.bottom - self.text_margins.bottom,
        )
    }

    /// Detect whether text extends beyond the effective bounds.
    pub fn detect_clipping(&self, text_bounds: &TextBounds) -> ClippingResult {
        let container = self.effective_bounds();

        // Degenerate container → everything is clipped.
        if container.width() <= 0.0 || container.height() <= 0.0 {
            return ClippingResult::CompletelyClipped;
        }

        let mut clipped_edges = Vec::new();

        if text_bounds.left < container.left {
            clipped_edges.push(ClippedEdge::Left {
                overflow_pixels: container.left - text_bounds.left,
            });
        }
        if text_bounds.right > container.right {
            clipped_edges.push(ClippedEdge::Right {
                overflow_pixels: text_bounds.right - container.right,
            });
        }
        if text_bounds.top < container.top {
            clipped_edges.push(ClippedEdge::Top {
                overflow_pixels: container.top - text_bounds.top,
            });
        }
        if text_bounds.bottom > container.bottom {
            clipped_edges.push(ClippedEdge::Bottom {
                overflow_pixels: text_bounds.bottom - container.bottom,
            });
        }

        if clipped_edges.is_empty() {
            return ClippingResult::NoClipping;
        }

        // Calculate visible percentage from the intersection area.
        let total_area = text_bounds.width() * text_bounds.height();
        if total_area <= 0.0 {
            return ClippingResult::CompletelyClipped;
        }

        let visible_left = text_bounds.left.max(container.left);
        let visible_right = text_bounds.right.min(container.right);
        let visible_top = text_bounds.top.max(container.top);
        let visible_bottom = text_bounds.bottom.min(container.bottom);

        let visible_width = (visible_right - visible_left).max(0.0);
        let visible_height = (visible_bottom - visible_top).max(0.0);
        let visible_area = visible_width * visible_height;

        if visible_area <= 0.0 {
            ClippingResult::CompletelyClipped
        } else {
            ClippingResult::PartialClipping {
                clipped_edges,
                visible_percentage: visible_area / total_area,
            }
        }
    }

    /// Calculate the available width at a given position.
    pub fn available_width_at(&self, x: f32) -> f32 {
        let container = self.effective_bounds();
        (container.right - x).max(0.0)
    }

    /// Calculate the available height at a given position.
    pub fn available_height_at(&self, y: f32) -> f32 {
        let container = self.effective_bounds();
        (container.bottom - y).max(0.0)
    }
}

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

        // Add to collision grid for collision detection (functionality will be enhanced in future text layout stories)
        self.collision_grid.add_bounds(&final_bounds);

        Ok(LayoutResult {
            glyphs,
            bounds: final_bounds,
            clipped: false, // TODO: Clipping detection will be added in a future text layout story
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
                        y: baseline_y - (glyph_info.size.y + glyph_info.bearing.y) * scale,
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

                    // Create scaled glyph info
                    let scaled_glyph = GlyphInfo {
                        character: glyph_info.character,
                        atlas_pos: glyph_info.atlas_pos,
                        size: Vec2 {
                            x: glyph_info.size.x * scale,
                            y: glyph_info.size.y * scale,
                        },
                        bearing: Vec2 {
                            x: glyph_info.bearing.x * scale,
                            y: glyph_info.bearing.y * scale,
                        },
                        advance: glyph_info.advance * scale,
                        sdf_scale: glyph_info.sdf_scale,
                    };

                    glyphs.push(PositionedGlyph {
                        glyph: scaled_glyph,
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
    // Future text layout stories may add public testing methods or constructors if needed

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

    #[test]
    fn test_glyph_positioning_performance() {
        // Test glyph positioning performance - a core part of text layout
        use std::time::Instant;

        let style = TextStyle::default();
        let mock_atlas = MockFontAtlas::new();

        // Test positioning of many glyphs
        let text =
            "Hello World! This is a performance test for glyph positioning with many characters.";
        let position = Vec2 { x: 100.0, y: 100.0 };

        let start = Instant::now();

        // Simulate the core glyph positioning logic
        let mut glyphs = Vec::new();
        let metrics = mock_atlas.metrics();
        let scale = style.font_size / metrics.size;
        let mut cursor_x = position.x;
        let baseline_y = position.y + metrics.ascent * scale;

        for ch in text.chars() {
            if let Some(glyph_info) = mock_atlas.get_glyph(ch) {
                if glyph_info.size.x > 0.0 && glyph_info.size.y > 0.0 {
                    let glyph_position = Vec2 {
                        x: cursor_x + glyph_info.bearing.x * scale,
                        y: baseline_y - (glyph_info.size.y + glyph_info.bearing.y) * scale,
                    };

                    glyphs.push(PositionedGlyph {
                        glyph: *glyph_info,
                        position: glyph_position,
                        color: Vec4 {
                            x: 1.0,
                            y: 1.0,
                            z: 1.0,
                            w: 1.0,
                        },
                    });
                }
                cursor_x += glyph_info.advance * scale;
            }
        }

        let duration = start.elapsed();

        // Should create many glyphs for typical text
        assert!(
            glyphs.len() > 50,
            "Should generate substantial number of glyphs: {}",
            glyphs.len()
        );

        // Performance: glyph positioning should be well under 5ms for typical text.
        // Debug builds are significantly slower; use generous thresholds to avoid flaky failures.
        #[cfg(debug_assertions)]
        let threshold_ms: u128 = 50;
        #[cfg(not(debug_assertions))]
        let threshold_ms: u128 = 10;

        println!(
            "Glyph positioning for {} chars took: {:?} (threshold: {}ms)",
            text.len(),
            duration,
            threshold_ms
        );
        assert!(
            duration.as_millis() < threshold_ms,
            "Glyph positioning too slow: {duration:?} (threshold: {threshold_ms}ms)"
        );
    }

    #[test]
    fn test_collision_detection_performance() {
        // Test collision detection performance with many existing labels
        use std::time::Instant;

        let mut engine = TextLayoutEngine::new();

        // Add many existing bounds to simulate crowded text
        let bounds_count = 100;
        for i in 0..bounds_count {
            let x = (i % 10) as f32 * 50.0;
            let y = (i / 10) as f32 * 20.0;
            let bounds = TextBounds::new(x, y, x + 40.0, y + 15.0);
            engine.collision_grid.add_bounds(&bounds);
        }

        // Test collision detection for new text placement
        let test_bounds = TextBounds::new(125.0, 50.0, 175.0, 65.0);

        let start = Instant::now();
        let has_collision = engine.collision_grid.has_collision(&test_bounds);
        let duration = start.elapsed();

        // Should detect collision with existing bounds
        assert!(
            has_collision,
            "Should detect collision with existing bounds"
        );

        // Performance: collision detection should be well under 1ms.
        // Debug builds are slower; use generous thresholds.
        #[cfg(debug_assertions)]
        let threshold_ms: u128 = 25;
        #[cfg(not(debug_assertions))]
        let threshold_ms: u128 = 5;

        println!("Collision detection against {bounds_count} bounds took: {duration:?}");
        assert!(
            duration.as_millis() < threshold_ms,
            "Collision detection too slow: {duration:?} (threshold: {threshold_ms}ms)"
        );
    }

    #[test]
    fn test_collision_grid_efficiency() {
        // Test that collision grid scales well with many bounds
        use std::time::Instant;

        let mut grid = CollisionGrid::new(32.0);

        // Add many bounds to stress test the grid
        let bounds_count = 1000;
        let start = Instant::now();

        for i in 0..bounds_count {
            let x = (i % 100) as f32 * 10.0;
            let y = (i / 100) as f32 * 10.0;
            let bounds = TextBounds::new(x, y, x + 8.0, y + 8.0);
            grid.add_bounds(&bounds);
        }

        let insertion_duration = start.elapsed();

        // Test lookup performance
        let lookup_start = Instant::now();
        for i in 0..100 {
            let x = (i % 10) as f32 * 50.0;
            let y = (i / 10) as f32 * 50.0;
            let test_bounds = TextBounds::new(x, y, x + 15.0, y + 15.0);
            let _collision = grid.has_collision(&test_bounds);
        }
        let lookup_duration = lookup_start.elapsed();

        // Performance: grid operations should scale well.
        // Debug builds are slower; use generous thresholds.
        #[cfg(debug_assertions)]
        let insert_threshold_ms: u128 = 200;
        #[cfg(not(debug_assertions))]
        let insert_threshold_ms: u128 = 50;

        #[cfg(debug_assertions)]
        let lookup_threshold_ms: u128 = 50;
        #[cfg(not(debug_assertions))]
        let lookup_threshold_ms: u128 = 10;

        println!("Grid insertion for {bounds_count} bounds took: {insertion_duration:?}");
        println!("Grid lookup for 100 queries took: {lookup_duration:?}");

        assert!(
            insertion_duration.as_millis() < insert_threshold_ms,
            "Grid insertion too slow: {insertion_duration:?} (threshold: {insert_threshold_ms}ms)"
        );
        assert!(
            lookup_duration.as_millis() < lookup_threshold_ms,
            "Grid lookup too slow: {lookup_duration:?} (threshold: {lookup_threshold_ms}ms)"
        );
    }

    #[test]
    fn test_memory_usage_collision_grid() {
        // Test that collision grid uses memory efficiently
        use std::mem;

        let grid_size = mem::size_of::<CollisionGrid>();
        let bounds_size = mem::size_of::<TextBounds>();

        // CollisionGrid should be reasonably sized
        assert!(
            grid_size <= 128,
            "CollisionGrid too large: {grid_size} bytes"
        );
        assert!(
            bounds_size <= 32,
            "TextBounds too large: {bounds_size} bytes"
        );

        // Test that grid doesn't grow excessively with many small bounds
        let mut grid = CollisionGrid::new(32.0);

        // Add 100 bounds and measure memory growth
        for i in 0..100 {
            let bounds = TextBounds::new(i as f32, i as f32, i as f32 + 5.0, i as f32 + 5.0);
            grid.add_bounds(&bounds);
        }

        // Grid should not consume excessive memory even with many bounds
        let final_cell_count = grid.occupied_cells.len();
        let final_bounds_count = grid.placed_bounds.len();

        assert!(
            final_cell_count <= 500,
            "Too many occupied cells: {final_cell_count}"
        );
        assert_eq!(final_bounds_count, 100, "Should track all added bounds");
    }

    // Helper struct for testing text layout
    struct MockFontAtlas {
        glyph_info: std::collections::HashMap<char, GlyphInfo>,
        font_metrics: FontMetrics,
    }

    impl MockFontAtlas {
        fn new() -> Self {
            let mut glyph_info = std::collections::HashMap::new();

            // Pre-populate with common characters
            for i in 32u32..127u32 {
                if let Some(ch) = char::from_u32(i) {
                    glyph_info.insert(
                        ch,
                        GlyphInfo {
                            character: ch,
                            atlas_pos: [0.0, 0.0, 0.1, 0.1],
                            size: Vec2 { x: 8.0, y: 12.0 },
                            bearing: Vec2 { x: 0.0, y: 10.0 },
                            advance: 9.0,
                            sdf_scale: 1.0,
                        },
                    );
                }
            }

            Self {
                glyph_info,
                font_metrics: FontMetrics::default(),
            }
        }

        fn get_glyph(&self, character: char) -> Option<&GlyphInfo> {
            self.glyph_info.get(&character)
        }

        fn metrics(&self) -> &FontMetrics {
            &self.font_metrics
        }
    }

    // === Viewport Bounds & Clipping Detection Tests ===

    #[test]
    fn test_viewport_bounds_from_screen() {
        let vb = ViewportBounds::from_screen(800.0, 600.0);
        assert_eq!(vb.viewport_rect, TextBounds::new(0.0, 0.0, 800.0, 600.0));
        assert!(vb.container_bounds.is_none());
    }

    #[test]
    fn test_viewport_bounds_from_container() {
        let container = TextBounds::new(50.0, 50.0, 400.0, 300.0);
        let vb = ViewportBounds::from_container(container);
        assert_eq!(vb.container_bounds, Some(container));
    }

    #[test]
    fn test_effective_bounds_with_margins() {
        let vb = ViewportBounds::from_screen(800.0, 600.0)
            .with_margins(TextMargins {
                top: 10.0,
                right: 20.0,
                bottom: 10.0,
                left: 20.0,
            });
        let eff = vb.effective_bounds();
        assert_eq!(eff.left, 20.0);
        assert_eq!(eff.top, 10.0);
        assert_eq!(eff.right, 780.0);
        assert_eq!(eff.bottom, 590.0);
    }

    #[test]
    fn test_detect_clipping_no_clip() {
        let vb = ViewportBounds::from_screen(800.0, 600.0);
        let text = TextBounds::new(100.0, 100.0, 200.0, 130.0);
        let result = vb.detect_clipping(&text);
        assert!(!result.is_clipped());
        assert!((result.visible_percentage() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_detect_clipping_right_edge() {
        let vb = ViewportBounds::from_screen(800.0, 600.0)
            .with_margins(TextMargins::default());
        // Text extends past right margin (effective right = 796.0)
        let text = TextBounds::new(750.0, 100.0, 850.0, 130.0);
        let result = vb.detect_clipping(&text);
        assert!(result.is_clipped());
        assert!(result.is_clipped_right());
        assert!(!result.is_completely_clipped());
    }

    #[test]
    fn test_detect_clipping_completely_outside() {
        let vb = ViewportBounds::from_screen(800.0, 600.0);
        let text = TextBounds::new(900.0, 700.0, 1000.0, 730.0);
        let result = vb.detect_clipping(&text);
        assert!(result.is_completely_clipped());
        assert!((result.visible_percentage()).abs() < f32::EPSILON);
    }

    #[test]
    fn test_detect_clipping_partial_visible_percentage() {
        let vb = ViewportBounds::from_screen(100.0, 100.0)
            .with_margins(TextMargins {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            });
        // Text: 100px wide, 10px tall. 50px overflows right.
        let text = TextBounds::new(50.0, 10.0, 150.0, 20.0);
        let result = vb.detect_clipping(&text);
        match result {
            ClippingResult::PartialClipping {
                visible_percentage, ..
            } => {
                assert!((visible_percentage - 0.5).abs() < 0.01);
            }
            other => panic!("Expected PartialClipping, got {:?}", other),
        }
    }

    #[test]
    fn test_detect_clipping_multiple_edges() {
        let vb = ViewportBounds::from_container(TextBounds::new(10.0, 10.0, 90.0, 90.0))
            .with_margins(TextMargins {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            });
        // Text extends past both left and right
        let text = TextBounds::new(0.0, 20.0, 100.0, 40.0);
        let result = vb.detect_clipping(&text);
        match result {
            ClippingResult::PartialClipping { clipped_edges, .. } => {
                assert_eq!(clipped_edges.len(), 2);
            }
            other => panic!("Expected PartialClipping, got {:?}", other),
        }
    }

    #[test]
    fn test_available_width_and_height() {
        let vb = ViewportBounds::from_screen(800.0, 600.0)
            .with_margins(TextMargins {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            });
        assert!((vb.available_width_at(100.0) - 700.0).abs() < f32::EPSILON);
        assert!((vb.available_height_at(100.0) - 500.0).abs() < f32::EPSILON);
        // Beyond edge
        assert!((vb.available_width_at(900.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_degenerate_container() {
        let vb = ViewportBounds::from_screen(0.0, 0.0);
        let text = TextBounds::new(0.0, 0.0, 10.0, 10.0);
        let result = vb.detect_clipping(&text);
        assert!(result.is_completely_clipped());
    }
}
