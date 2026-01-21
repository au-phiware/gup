// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Label positioning with collision detection and automatic layout optimization.

use super::*;
use crate::error::GupResult;
use crate::shader_function::Vec2;
use crate::text::{TextBounds, TextLayoutEngine, TextStyle};
use std::collections::HashSet;

/// Label positioner with intelligent collision detection and layout optimization.
pub struct LabelPositioner {
    /// Text layout engine for measuring and positioning text
    layout_engine: TextLayoutEngine,
    /// Previously placed labels for collision detection
    placed_labels: Vec<TextBounds>,
    /// Grid-based collision detection
    collision_grid: CollisionGrid,
}

/// Grid-based collision detection for efficient overlap checking.
struct CollisionGrid {
    /// Grid cell size in pixels
    cell_size: f32,
    /// Occupied grid cells
    occupied_cells: HashSet<(i32, i32)>,
}

impl LabelPositioner {
    /// Create a new label positioner.
    pub fn new() -> Self {
        Self {
            layout_engine: TextLayoutEngine::new(),
            placed_labels: Vec::new(),
            collision_grid: CollisionGrid::new(32.0),
        }
    }

    /// Layout labels for axis tick positions.
    pub fn layout_labels(
        &mut self,
        tick_positions: &[f64],
        axis_info: &AxisInfo,
        formatter: &dyn LabelFormatter,
        constraints: &LabelConstraints,
    ) -> GupResult<LabelLayout> {
        if tick_positions.is_empty() {
            return Ok(LabelLayout {
                positions: Vec::new(),
                hidden_labels: Vec::new(),
                margin_requirements: Margins::default(),
                rotated: false,
            });
        }

        // Generate initial label positions
        let mut label_positions =
            self.generate_initial_positions(tick_positions, axis_info, formatter, constraints)?;

        // Apply collision detection and resolution
        let mut layout = self.resolve_collisions(&mut label_positions, axis_info, constraints)?;

        // Apply rotation if needed and allowed
        if constraints.allow_rotation && self.labels_still_overlap(&layout.positions) {
            layout = self.apply_rotation(label_positions, axis_info, constraints)?;
        }

        // Hide overlapping labels if still needed
        if constraints.hide_overlapping && self.labels_still_overlap(&layout.positions) {
            self.hide_overlapping_labels(&mut layout, constraints);
        }

        // Calculate margin requirements
        layout.margin_requirements =
            self.calculate_margin_requirements(&layout.positions, axis_info);

        // Update collision grid with placed labels
        for position in &layout.positions {
            self.collision_grid.add_bounds(&position.bounds);
            self.placed_labels.push(position.bounds);
        }

        Ok(layout)
    }

    /// Clear all placed labels and collision data.
    pub fn clear(&mut self) {
        self.layout_engine.clear();
        self.placed_labels.clear();
        self.collision_grid.clear();
    }

    /// Generate initial label positions at tick marks.
    fn generate_initial_positions(
        &mut self,
        tick_positions: &[f64],
        axis_info: &AxisInfo,
        formatter: &dyn LabelFormatter,
        constraints: &LabelConstraints,
    ) -> GupResult<Vec<LabelPosition>> {
        let mut positions = Vec::new();
        let axis_direction = axis_info.direction();
        let axis_normal = axis_info.normal();

        for &tick_pos in tick_positions.iter() {
            // Calculate position along axis
            let axis_position = Vec2 {
                x: axis_info.start.x + axis_direction.x * axis_info.length * tick_pos as f32,
                y: axis_info.start.y + axis_direction.y * axis_info.length * tick_pos as f32,
            };

            // Offset by label spacing based on axis position
            let label_offset = self.calculate_label_offset(axis_info, &constraints.text_style);
            let label_position = Vec2 {
                x: axis_position.x + axis_normal.x * label_offset,
                y: axis_position.y + axis_normal.y * label_offset,
            };

            // Format the value
            let domain_value = tick_pos; // In a real implementation, this would convert from normalized to domain space
            let text = formatter.format_value(domain_value);

            // Determine anchor based on axis position
            let anchor = self.determine_anchor(axis_info.position);

            // Measure text to get bounds
            let text_bounds = self.layout_engine.measure_text(
                &text,
                &constraints.text_style,
                // In a real implementation, we'd have access to the font atlas here
                #[allow(invalid_value)]
                &unsafe { std::mem::zeroed() },
            )?;

            // Adjust position based on anchor
            let adjusted_position =
                self.apply_anchor_adjustment(label_position, &text_bounds, anchor);

            // Create positioned bounds
            let positioned_bounds = TextBounds {
                left: adjusted_position.x,
                top: adjusted_position.y,
                right: adjusted_position.x + text_bounds.width(),
                bottom: adjusted_position.y + text_bounds.height(),
            };

            positions.push(LabelPosition {
                position: adjusted_position,
                rotation: 0.0,
                anchor,
                text,
                style: constraints.text_style.clone(),
                bounds: positioned_bounds,
            });

            // Apply max labels limit
            if let Some(max_labels) = constraints.max_labels
                && positions.len() >= max_labels
            {
                break;
            }
        }

        Ok(positions)
    }

    /// Resolve collisions between labels.
    fn resolve_collisions(
        &mut self,
        label_positions: &mut [LabelPosition],
        axis_info: &AxisInfo,
        constraints: &LabelConstraints,
    ) -> GupResult<LabelLayout> {
        let mut visible_positions = Vec::new();
        let mut hidden_labels = Vec::new();

        for (i, position) in label_positions.iter().enumerate() {
            // Check for collision with previously placed labels
            let has_collision = self.check_collision(&position.bounds, constraints.min_spacing);

            if !has_collision {
                visible_positions.push(position.clone());
                self.collision_grid.add_bounds(&position.bounds);
            } else {
                // Try to find alternative position
                if let Some(alternative) =
                    self.find_alternative_position(position, axis_info, constraints)
                {
                    visible_positions.push(alternative);
                } else {
                    hidden_labels.push(i);
                }
            }
        }

        Ok(LabelLayout {
            positions: visible_positions,
            hidden_labels,
            margin_requirements: Margins::default(),
            rotated: false,
        })
    }

    /// Apply rotation to labels to avoid collisions.
    fn apply_rotation(
        &mut self,
        mut label_positions: Vec<LabelPosition>,
        axis_info: &AxisInfo,
        constraints: &LabelConstraints,
    ) -> GupResult<LabelLayout> {
        let rotation_angles = [
            std::f32::consts::PI / 6.0, // 30 degrees
            std::f32::consts::PI / 4.0, // 45 degrees
            std::f32::consts::PI / 3.0, // 60 degrees
            std::f32::consts::PI / 2.0, // 90 degrees
        ];

        for &rotation in &rotation_angles {
            if rotation > constraints.max_rotation {
                break;
            }

            // Apply rotation to all labels
            for position in &mut label_positions {
                position.rotation = rotation;
                position.style = position.style.clone().with_rotation(rotation);

                // Recalculate bounds with rotation
                position.bounds = self.calculate_rotated_bounds(position);
            }

            // Check if rotation resolved collisions
            self.collision_grid.clear();
            let layout = self.resolve_collisions(&mut label_positions, axis_info, constraints)?;

            if !self.labels_still_overlap(&layout.positions) {
                return Ok(LabelLayout {
                    rotated: true,
                    ..layout
                });
            }
        }

        // If no rotation worked, return the original layout
        self.resolve_collisions(&mut label_positions, axis_info, constraints)
    }

    /// Hide overlapping labels based on priority.
    fn hide_overlapping_labels(&self, layout: &mut LabelLayout, constraints: &LabelConstraints) {
        if !constraints.hide_overlapping {
            return;
        }

        let mut final_positions = Vec::new();
        let mut collision_grid = CollisionGrid::new(32.0);

        // Sort by priority if provided, otherwise keep original order
        let mut indexed_positions: Vec<(usize, &LabelPosition)> =
            layout.positions.iter().enumerate().collect();

        if let Some(ref priorities) = constraints.label_priorities {
            indexed_positions.sort_by(|&(i, _), &(j, _)| {
                let priority_i = priorities.get(i).unwrap_or(&0.0);
                let priority_j = priorities.get(j).unwrap_or(&0.0);
                priority_j
                    .partial_cmp(priority_i)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        for (original_index, position) in indexed_positions {
            if !collision_grid.has_collision(&position.bounds, constraints.min_spacing) {
                final_positions.push(position.clone());
                collision_grid.add_bounds(&position.bounds);
            } else {
                layout.hidden_labels.push(original_index);
            }
        }

        layout.positions = final_positions;
    }

    /// Calculate label offset from axis based on style and axis position.
    fn calculate_label_offset(&self, axis_info: &AxisInfo, style: &TextStyle) -> f32 {
        match axis_info.position {
            AxisPosition::Top => -style.font_size * 1.2,
            AxisPosition::Bottom => style.font_size * 1.2,
            AxisPosition::Left => -style.font_size * 0.5,
            AxisPosition::Right => style.font_size * 0.5,
        }
    }

    /// Determine appropriate text anchor based on axis position.
    fn determine_anchor(&self, position: AxisPosition) -> TextAnchor {
        match position {
            AxisPosition::Top => TextAnchor::BottomCenter,
            AxisPosition::Bottom => TextAnchor::TopCenter,
            AxisPosition::Left => TextAnchor::CenterRight,
            AxisPosition::Right => TextAnchor::CenterLeft,
        }
    }

    /// Apply anchor adjustment to position.
    fn apply_anchor_adjustment(
        &self,
        position: Vec2,
        bounds: &TextBounds,
        anchor: TextAnchor,
    ) -> Vec2 {
        let offset = anchor.offset();
        Vec2 {
            x: position.x - bounds.width() * offset.x,
            y: position.y - bounds.height() * offset.y,
        }
    }

    /// Check for collision with existing labels.
    fn check_collision(&self, bounds: &TextBounds, min_spacing: f32) -> bool {
        // Expand bounds by minimum spacing
        let expanded_bounds = TextBounds {
            left: bounds.left - min_spacing,
            top: bounds.top - min_spacing,
            right: bounds.right + min_spacing,
            bottom: bounds.bottom + min_spacing,
        };

        self.collision_grid.has_collision(&expanded_bounds, 0.0)
    }

    /// Find alternative position to avoid collision.
    fn find_alternative_position(
        &self,
        position: &LabelPosition,
        _axis_info: &AxisInfo,
        constraints: &LabelConstraints,
    ) -> Option<LabelPosition> {
        // Try small offsets from the original position
        let offsets = [
            Vec2 { x: 0.0, y: 4.0 },
            Vec2 { x: 0.0, y: -4.0 },
            Vec2 { x: 4.0, y: 0.0 },
            Vec2 { x: -4.0, y: 0.0 },
        ];

        for offset in &offsets {
            let new_position = Vec2 {
                x: position.position.x + offset.x,
                y: position.position.y + offset.y,
            };

            let new_bounds = TextBounds {
                left: new_position.x,
                top: new_position.y,
                right: new_position.x + position.bounds.width(),
                bottom: new_position.y + position.bounds.height(),
            };

            if !self.check_collision(&new_bounds, constraints.min_spacing) {
                return Some(LabelPosition {
                    position: new_position,
                    bounds: new_bounds,
                    ..position.clone()
                });
            }
        }

        None
    }

    /// Check if labels still overlap.
    fn labels_still_overlap(&self, positions: &[LabelPosition]) -> bool {
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                if positions[i].bounds.intersects(&positions[j].bounds) {
                    return true;
                }
            }
        }
        false
    }

    /// Calculate bounds for rotated text.
    fn calculate_rotated_bounds(&self, position: &LabelPosition) -> TextBounds {
        if position.rotation.abs() < 0.001 {
            return position.bounds;
        }

        let cos_r = position.rotation.cos();
        let sin_r = position.rotation.sin();
        let width = position.bounds.width();
        let height = position.bounds.height();

        // Calculate rotated bounding box
        let corners = [(0.0, 0.0), (width, 0.0), (width, height), (0.0, height)];

        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for (x, y) in &corners {
            let rotated_x = x * cos_r - y * sin_r;
            let rotated_y = x * sin_r + y * cos_r;

            min_x = min_x.min(rotated_x);
            max_x = max_x.max(rotated_x);
            min_y = min_y.min(rotated_y);
            max_y = max_y.max(rotated_y);
        }

        TextBounds {
            left: position.position.x + min_x,
            top: position.position.y + min_y,
            right: position.position.x + max_x,
            bottom: position.position.y + max_y,
        }
    }

    /// Calculate margin requirements for label layout.
    fn calculate_margin_requirements(
        &self,
        positions: &[LabelPosition],
        axis_info: &AxisInfo,
    ) -> Margins {
        if positions.is_empty() {
            return Margins::default();
        }

        let mut margins = Margins::default();

        for position in positions {
            match axis_info.position {
                AxisPosition::Top => {
                    margins.top = margins.top.max(axis_info.start.y - position.bounds.top);
                }
                AxisPosition::Bottom => {
                    margins.bottom = margins
                        .bottom
                        .max(position.bounds.bottom - axis_info.start.y);
                }
                AxisPosition::Left => {
                    margins.left = margins.left.max(axis_info.start.x - position.bounds.left);
                }
                AxisPosition::Right => {
                    margins.right = margins.right.max(position.bounds.right - axis_info.start.x);
                }
            }
        }

        margins
    }
}

impl Default for LabelPositioner {
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
        }
    }

    /// Add bounds to the collision grid.
    fn add_bounds(&mut self, bounds: &TextBounds) {
        let cells = self.get_cells_for_bounds(bounds);
        for cell in cells {
            self.occupied_cells.insert(cell);
        }
    }

    /// Check if bounds collide with existing content.
    fn has_collision(&self, bounds: &TextBounds, padding: f32) -> bool {
        let padded_bounds = TextBounds {
            left: bounds.left - padding,
            top: bounds.top - padding,
            right: bounds.right + padding,
            bottom: bounds.bottom + padding,
        };

        let cells = self.get_cells_for_bounds(&padded_bounds);
        for cell in &cells {
            if self.occupied_cells.contains(cell) {
                return true;
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

    /// Clear all collision data.
    fn clear(&mut self) {
        self.occupied_cells.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_positioner_creation() {
        let positioner = LabelPositioner::new();
        assert_eq!(positioner.placed_labels.len(), 0);
        assert_eq!(positioner.collision_grid.cell_size, 32.0);
    }

    #[test]
    fn test_anchor_determination() {
        let positioner = LabelPositioner::new();

        assert_eq!(
            positioner.determine_anchor(AxisPosition::Top),
            TextAnchor::BottomCenter
        );
        assert_eq!(
            positioner.determine_anchor(AxisPosition::Bottom),
            TextAnchor::TopCenter
        );
        assert_eq!(
            positioner.determine_anchor(AxisPosition::Left),
            TextAnchor::CenterRight
        );
        assert_eq!(
            positioner.determine_anchor(AxisPosition::Right),
            TextAnchor::CenterLeft
        );
    }

    #[test]
    fn test_label_offset_calculation() {
        let positioner = LabelPositioner::new();
        let axis_info = AxisInfo::horizontal(800.0);
        let style = TextStyle::new(16.0);

        let offset = positioner.calculate_label_offset(&axis_info, &style);
        assert!(offset > 0.0); // Bottom axis should have positive offset
    }

    #[test]
    fn test_collision_grid() {
        let mut grid = CollisionGrid::new(10.0);

        let bounds1 = TextBounds::new(0.0, 0.0, 5.0, 5.0);
        let bounds2 = TextBounds::new(50.0, 50.0, 55.0, 55.0); // Far away
        let bounds3 = TextBounds::new(2.0, 2.0, 7.0, 7.0); // Overlaps with bounds1

        // Add first bounds
        grid.add_bounds(&bounds1);
        assert!(!grid.has_collision(&bounds2, 0.0)); // Should not collide
        assert!(grid.has_collision(&bounds1, 0.0)); // Should collide with itself
        assert!(grid.has_collision(&bounds3, 0.0)); // Should collide with bounds1

        // Add second bounds
        grid.add_bounds(&bounds2);
        assert!(grid.has_collision(&bounds3, 0.0)); // Should collide with both now
    }

    #[test]
    fn test_rotated_bounds_calculation() {
        let positioner = LabelPositioner::new();
        let position = LabelPosition {
            position: Vec2 { x: 0.0, y: 0.0 },
            rotation: std::f32::consts::PI / 2.0, // 90 degrees
            anchor: TextAnchor::TopLeft,
            text: "Test".to_string(),
            style: TextStyle::default(),
            bounds: TextBounds::new(0.0, 0.0, 10.0, 5.0),
        };

        let rotated_bounds = positioner.calculate_rotated_bounds(&position);

        // 90-degree rotation should swap width and height dimensions
        assert!((rotated_bounds.width() - 5.0).abs() < 0.1);
        assert!((rotated_bounds.height() - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_margin_calculation() {
        let positioner = LabelPositioner::new();
        let axis_info = AxisInfo::horizontal(800.0);

        let positions = vec![LabelPosition {
            position: Vec2 { x: 100.0, y: 20.0 },
            rotation: 0.0,
            anchor: TextAnchor::TopCenter,
            text: "Label".to_string(),
            style: TextStyle::default(),
            bounds: TextBounds::new(95.0, 20.0, 105.0, 35.0),
        }];

        let margins = positioner.calculate_margin_requirements(&positions, &axis_info);
        assert!(margins.bottom > 0.0);
    }

    #[test]
    fn test_label_overlap_detection() {
        let positioner = LabelPositioner::new();

        let positions = vec![
            LabelPosition {
                position: Vec2 { x: 0.0, y: 0.0 },
                rotation: 0.0,
                anchor: TextAnchor::TopLeft,
                text: "Label1".to_string(),
                style: TextStyle::default(),
                bounds: TextBounds::new(0.0, 0.0, 20.0, 10.0),
            },
            LabelPosition {
                position: Vec2 { x: 10.0, y: 5.0 },
                rotation: 0.0,
                anchor: TextAnchor::TopLeft,
                text: "Label2".to_string(),
                style: TextStyle::default(),
                bounds: TextBounds::new(10.0, 5.0, 30.0, 15.0),
            },
        ];

        assert!(positioner.labels_still_overlap(&positions));
    }
}
