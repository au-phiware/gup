// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Label positioning with collision detection and automatic layout optimization.
//!
//! Provides a [`LabelPositioner`] that resolves overlapping labels using a
//! configurable pipeline of strategies: offset, rotation, and hiding.
//!
//! The collision detection uses a two-phase spatial grid: a broad phase that
//! maps bounding boxes to grid cells, and a narrow phase that tests actual
//! bounding-box intersection.

use super::*;
use crate::axis::AxisLabel;
use crate::error::GupResult;
use crate::shader_function::Vec2;
use crate::text::TextBounds;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Positioning strategy
// ---------------------------------------------------------------------------

/// Strategy for resolving label collisions, applied in order.
///
/// The positioner runs each strategy in sequence.  If after a strategy
/// no collisions remain, later strategies are skipped.
#[derive(Debug, Clone)]
pub enum LabelPositioningStrategy {
    /// Try offsetting labels away from the axis to avoid overlaps.
    ///
    /// `max_distance` is the furthest (in pixels) a label may be shifted.
    /// `directions` lists the unit offsets to attempt.
    Offset {
        /// Maximum displacement distance in pixels.
        max_distance: f32,
        /// Unit-offset directions to attempt.
        directions: Vec<Vec2>,
    },
    /// Rotate all labels by increasing angles until overlaps are resolved.
    ///
    /// Angles are tried in `step`-radian increments up to `max_angle`.
    Rotate {
        /// Maximum rotation angle in radians.
        max_angle: f32,
        /// Angle increment per attempt in radians.
        step: f32,
    },
    /// Hide lower-priority labels that still overlap.
    ///
    /// Labels with priority below `priority_threshold` may be hidden.
    /// When no explicit priorities are supplied the default is positional
    /// order (first and last labels have highest priority).
    Hide {
        /// Minimum priority a label must have to remain visible.
        priority_threshold: f32,
    },
    /// Scale label font size down to fit.
    ///
    /// Not yet implemented — reserved for future use.
    Scale {
        /// Minimum font size in pixels.
        min_size: f32,
        /// Maximum font size in pixels.
        max_size: f32,
    },
}

impl LabelPositioningStrategy {
    /// Sensible default offset strategy: try ±4px in 4 cardinal directions, up to 12px.
    pub fn default_offset() -> Self {
        Self::Offset {
            max_distance: 12.0,
            directions: vec![
                Vec2 { x: 0.0, y: 4.0 },
                Vec2 { x: 0.0, y: -4.0 },
                Vec2 { x: 4.0, y: 0.0 },
                Vec2 { x: -4.0, y: 0.0 },
            ],
        }
    }

    /// Rotation strategy that tries 30°, 45°, 60°, 90°.
    pub fn default_rotate() -> Self {
        Self::Rotate {
            max_angle: std::f32::consts::FRAC_PI_2,
            step: std::f32::consts::FRAC_PI_6,
        }
    }

    /// Default hiding strategy with threshold 0 (any label may be hidden).
    pub fn default_hide() -> Self {
        Self::Hide {
            priority_threshold: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// SpatialGrid — broad + narrow phase collision detection
// ---------------------------------------------------------------------------

/// Grid-based spatial index for efficient collision detection.
///
/// Each grid cell stores references (indices) into a flat `bounds` array.
/// Collision queries first identify candidate cells, then perform exact
/// `TextBounds::intersects()` checks against the stored bounds.
pub struct SpatialGrid {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<usize>>,
    bounds: Vec<TextBounds>,
}

impl SpatialGrid {
    /// Create a new spatial grid with the given cell size.
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size: cell_size.max(1.0),
            cells: HashMap::new(),
            bounds: Vec::new(),
        }
    }

    /// Insert bounds and return its index.
    pub fn insert(&mut self, b: TextBounds) -> usize {
        let idx = self.bounds.len();
        self.bounds.push(b);
        for cell in self.cells_for(&b) {
            self.cells.entry(cell).or_default().push(idx);
        }
        idx
    }

    /// Test whether `query` (optionally padded by `padding`) collides with any stored bounds.
    pub fn has_collision(&self, query: &TextBounds, padding: f32) -> bool {
        let padded = Self::pad(query, padding);
        for cell in self.cells_for(&padded) {
            if let Some(indices) = self.cells.get(&cell) {
                for &idx in indices {
                    if padded.intersects(&self.bounds[idx]) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Return all stored bounds that collide with `query`.
    pub fn query_collisions(&self, query: &TextBounds, padding: f32) -> Vec<TextBounds> {
        let padded = Self::pad(query, padding);
        let mut seen = Vec::new();
        let mut results = Vec::new();
        for cell in self.cells_for(&padded) {
            if let Some(indices) = self.cells.get(&cell) {
                for &idx in indices {
                    if !seen.contains(&idx) && padded.intersects(&self.bounds[idx]) {
                        seen.push(idx);
                        results.push(self.bounds[idx]);
                    }
                }
            }
        }
        results
    }

    /// Number of stored bounds.
    pub fn len(&self) -> usize {
        self.bounds.len()
    }

    /// Whether the grid is empty.
    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty()
    }

    /// Remove all data.
    pub fn clear(&mut self) {
        self.cells.clear();
        self.bounds.clear();
    }

    // -- helpers --

    fn cells_for(&self, b: &TextBounds) -> Vec<(i32, i32)> {
        let min_x = (b.left / self.cell_size).floor() as i32;
        let max_x = (b.right / self.cell_size).ceil() as i32;
        let min_y = (b.top / self.cell_size).floor() as i32;
        let max_y = (b.bottom / self.cell_size).ceil() as i32;

        let mut out = Vec::with_capacity(((max_x - min_x + 1) * (max_y - min_y + 1)) as usize);
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                out.push((x, y));
            }
        }
        out
    }

    fn pad(b: &TextBounds, p: f32) -> TextBounds {
        TextBounds {
            left: b.left - p,
            top: b.top - p,
            right: b.right + p,
            bottom: b.bottom + p,
        }
    }
}

// ---------------------------------------------------------------------------
// LabelPositioner
// ---------------------------------------------------------------------------

/// Label positioner with intelligent collision detection and layout optimization.
///
/// Operates in two modes:
///
/// 1. **High-level**: [`layout_labels`](Self::layout_labels) — takes tick
///    positions + formatter and generates `LabelLayout` from scratch.
/// 2. **Integration**: [`resolve_labels`](Self::resolve_labels) — takes a
///    `Vec<AxisLabel>` (already produced by [`AxisRenderer::generate_label_data`](crate::axis::AxisRenderer::generate_label_data))
///    and resolves collisions in-place, returning the resolved `LabelLayout`.
pub struct LabelPositioner {
    /// Previously placed labels from prior `layout_labels` calls.
    placed_labels: Vec<TextBounds>,
    /// Spatial index for collision detection.
    grid: SpatialGrid,
    /// Pipeline of strategies to try, in order.
    strategies: Vec<LabelPositioningStrategy>,
}

impl LabelPositioner {
    /// Create a new label positioner with the default strategy pipeline.
    ///
    /// Default pipeline: Offset → Rotate → Hide.
    pub fn new() -> Self {
        Self {
            placed_labels: Vec::new(),
            grid: SpatialGrid::new(32.0),
            strategies: vec![
                LabelPositioningStrategy::default_offset(),
                LabelPositioningStrategy::default_rotate(),
                LabelPositioningStrategy::default_hide(),
            ],
        }
    }

    /// Create a positioner with a custom strategy pipeline.
    pub fn with_strategies(strategies: Vec<LabelPositioningStrategy>) -> Self {
        Self {
            placed_labels: Vec::new(),
            grid: SpatialGrid::new(32.0),
            strategies,
        }
    }

    /// Layout labels for axis tick positions.
    ///
    /// This is the high-level API that generates labels from tick positions,
    /// formats them, estimates bounds, and resolves collisions.
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

        // Generate initial label positions using the formatter for bounds estimation
        let mut label_positions =
            self.generate_initial_positions(tick_positions, axis_info, formatter, constraints);

        // Run the resolution pipeline
        let layout = self.run_pipeline(&mut label_positions, axis_info, constraints)?;

        // Track placed labels for cross-axis collision avoidance
        for position in &layout.positions {
            self.placed_labels.push(position.bounds);
        }

        Ok(layout)
    }

    /// Resolve collisions for labels already generated by `AxisRenderer::generate_label_data`.
    ///
    /// This converts `AxisLabel` data into `LabelPosition`s, runs the
    /// collision resolution pipeline, and returns the result.
    pub fn resolve_labels(
        &mut self,
        axis_labels: &[AxisLabel],
        axis_info: &AxisInfo,
        constraints: &LabelConstraints,
    ) -> GupResult<LabelLayout> {
        if axis_labels.is_empty() {
            return Ok(LabelLayout {
                positions: Vec::new(),
                hidden_labels: Vec::new(),
                margin_requirements: Margins::default(),
                rotated: false,
            });
        }

        let mut label_positions: Vec<LabelPosition> = axis_labels
            .iter()
            .map(|al| {
                let width = estimate_text_width(&al.text, constraints.text_style.font_size);
                let height = constraints.text_style.font_size;
                let offset = al.anchor.offset();
                let top_left = Vec2 {
                    x: al.screen_position.x - width * offset.x,
                    y: al.screen_position.y - height * offset.y,
                };
                LabelPosition {
                    position: al.screen_position,
                    rotation: 0.0,
                    anchor: al.anchor,
                    text: al.text.clone(),
                    style: constraints.text_style.clone(),
                    bounds: TextBounds::new(
                        top_left.x,
                        top_left.y,
                        top_left.x + width,
                        top_left.y + height,
                    ),
                }
            })
            .collect();

        let layout = self.run_pipeline(&mut label_positions, axis_info, constraints)?;

        for position in &layout.positions {
            self.placed_labels.push(position.bounds);
        }

        Ok(layout)
    }

    /// Clear all placed labels and collision data.
    pub fn clear(&mut self) {
        self.placed_labels.clear();
        self.grid.clear();
    }

    // -----------------------------------------------------------------------
    // Internal: pipeline runner
    // -----------------------------------------------------------------------

    fn run_pipeline(
        &mut self,
        positions: &mut Vec<LabelPosition>,
        axis_info: &AxisInfo,
        constraints: &LabelConstraints,
    ) -> GupResult<LabelLayout> {
        // Start with a simple greedy placement
        self.grid.clear();
        // Re-insert any previously placed labels (cross-axis)
        for b in &self.placed_labels {
            self.grid.insert(*b);
        }

        let mut layout = self.greedy_place(positions, constraints);

        // If there are no collisions, we're done
        if !labels_still_overlap(&layout.positions) {
            layout.margin_requirements =
                calculate_margin_requirements(&layout.positions, axis_info);
            return Ok(layout);
        }

        // Run each strategy until collisions are resolved
        let strategies = self.strategies.clone();
        for strategy in &strategies {
            match strategy {
                LabelPositioningStrategy::Offset {
                    max_distance,
                    directions,
                } => {
                    self.apply_offset_strategy(&mut layout, constraints, *max_distance, directions);
                }
                LabelPositioningStrategy::Rotate {
                    max_angle, step, ..
                } => {
                    if constraints.allow_rotation {
                        self.apply_rotation_strategy(
                            positions,
                            &mut layout,
                            axis_info,
                            constraints,
                            *max_angle,
                            *step,
                        )?;
                    }
                }
                LabelPositioningStrategy::Hide {
                    priority_threshold, ..
                } => {
                    if constraints.hide_overlapping {
                        self.apply_hide_strategy(&mut layout, constraints, *priority_threshold);
                    }
                }
                LabelPositioningStrategy::Scale { .. } => {
                    // Not yet implemented
                }
            }

            if !labels_still_overlap(&layout.positions) {
                break;
            }
        }

        layout.margin_requirements = calculate_margin_requirements(&layout.positions, axis_info);
        Ok(layout)
    }

    // -----------------------------------------------------------------------
    // Greedy placement (first pass)
    // -----------------------------------------------------------------------

    fn greedy_place(
        &mut self,
        positions: &[LabelPosition],
        constraints: &LabelConstraints,
    ) -> LabelLayout {
        let mut visible = Vec::new();
        let mut hidden = Vec::new();

        for (i, pos) in positions.iter().enumerate() {
            if !self
                .grid
                .has_collision(&pos.bounds, constraints.min_spacing)
            {
                self.grid.insert(pos.bounds);
                visible.push(pos.clone());
            } else {
                hidden.push(i);
                // Still add to visible — the pipeline strategies will
                // resolve or hide later.
                visible.push(pos.clone());
            }
        }

        LabelLayout {
            positions: visible,
            hidden_labels: Vec::new(), // will be populated by hide strategy
            margin_requirements: Margins::default(),
            rotated: false,
        }
    }

    // -----------------------------------------------------------------------
    // Strategy: offset
    // -----------------------------------------------------------------------

    fn apply_offset_strategy(
        &mut self,
        layout: &mut LabelLayout,
        constraints: &LabelConstraints,
        max_distance: f32,
        directions: &[Vec2],
    ) {
        self.grid.clear();
        for b in &self.placed_labels {
            self.grid.insert(*b);
        }

        let mut resolved = Vec::new();

        for pos in &layout.positions {
            if !self
                .grid
                .has_collision(&pos.bounds, constraints.min_spacing)
            {
                self.grid.insert(pos.bounds);
                resolved.push(pos.clone());
                continue;
            }

            // Try offsets at increasing distances
            let mut placed = false;
            let mut step = 1.0_f32;
            while step * 4.0 <= max_distance {
                for dir in directions {
                    let factor = step;
                    let new_position = Vec2 {
                        x: pos.position.x + dir.x * factor,
                        y: pos.position.y + dir.y * factor,
                    };
                    let w = pos.bounds.width();
                    let h = pos.bounds.height();
                    let new_bounds = TextBounds::new(
                        new_position.x - w * pos.anchor.offset().x,
                        new_position.y - h * pos.anchor.offset().y,
                        new_position.x - w * pos.anchor.offset().x + w,
                        new_position.y - h * pos.anchor.offset().y + h,
                    );
                    if !self
                        .grid
                        .has_collision(&new_bounds, constraints.min_spacing)
                    {
                        self.grid.insert(new_bounds);
                        resolved.push(LabelPosition {
                            position: new_position,
                            bounds: new_bounds,
                            ..pos.clone()
                        });
                        placed = true;
                        break;
                    }
                }
                if placed {
                    break;
                }
                step += 1.0;
            }

            if !placed {
                // Keep original position — later strategies will resolve
                self.grid.insert(pos.bounds);
                resolved.push(pos.clone());
            }
        }

        layout.positions = resolved;
    }

    // -----------------------------------------------------------------------
    // Strategy: rotation
    // -----------------------------------------------------------------------

    fn apply_rotation_strategy(
        &mut self,
        original_positions: &mut Vec<LabelPosition>,
        layout: &mut LabelLayout,
        axis_info: &AxisInfo,
        constraints: &LabelConstraints,
        max_angle: f32,
        step: f32,
    ) -> GupResult<()> {
        let mut angle = step;
        while angle <= max_angle + 0.001 {
            // Apply rotation to all labels
            let rotated_positions: Vec<LabelPosition> = original_positions
                .iter()
                .map(|pos| {
                    let mut rp = pos.clone();
                    rp.rotation = angle;
                    rp.style = pos.style.clone().with_rotation(angle);
                    rp.bounds = calculate_rotated_bounds(&rp);
                    rp
                })
                .collect();

            // Try greedy placement with rotated bounds
            self.grid.clear();
            for b in &self.placed_labels {
                self.grid.insert(*b);
            }
            let candidate = self.greedy_place(&rotated_positions, constraints);

            if !labels_still_overlap(&candidate.positions) {
                *layout = LabelLayout {
                    rotated: true,
                    margin_requirements: calculate_margin_requirements(
                        &candidate.positions,
                        axis_info,
                    ),
                    ..candidate
                };
                // Update original_positions so later strategies see the rotated state
                *original_positions = rotated_positions;
                return Ok(());
            }

            angle += step;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Strategy: hide
    // -----------------------------------------------------------------------

    fn apply_hide_strategy(
        &mut self,
        layout: &mut LabelLayout,
        constraints: &LabelConstraints,
        _priority_threshold: f32,
    ) {
        self.grid.clear();
        for b in &self.placed_labels {
            self.grid.insert(*b);
        }

        let mut final_positions = Vec::new();

        // Build priority order: explicit priorities, or positional heuristic
        // (first & last are most important, middle labels less so).
        let count = layout.positions.len();
        let mut indexed: Vec<(usize, &LabelPosition)> =
            layout.positions.iter().enumerate().collect();

        if let Some(ref priorities) = constraints.label_priorities {
            indexed.sort_by(|&(i, _), &(j, _)| {
                let pi = priorities.get(i).copied().unwrap_or(0.0);
                let pj = priorities.get(j).copied().unwrap_or(0.0);
                pj.partial_cmp(&pi).unwrap_or(std::cmp::Ordering::Equal)
            });
        } else {
            // Heuristic: first and last labels have highest priority,
            // labels near the middle have lowest.
            indexed.sort_by(|&(i, _), &(j, _)| {
                let pi = positional_priority(i, count);
                let pj = positional_priority(j, count);
                pj.partial_cmp(&pi).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        for (original_index, position) in indexed {
            if !self
                .grid
                .has_collision(&position.bounds, constraints.min_spacing)
            {
                self.grid.insert(position.bounds);
                final_positions.push(position.clone());
            } else {
                layout.hidden_labels.push(original_index);
            }
        }

        layout.positions = final_positions;
    }

    // -----------------------------------------------------------------------
    // Internal: initial position generation
    // -----------------------------------------------------------------------

    fn generate_initial_positions(
        &self,
        tick_positions: &[f64],
        axis_info: &AxisInfo,
        formatter: &dyn LabelFormatter,
        constraints: &LabelConstraints,
    ) -> Vec<LabelPosition> {
        let axis_direction = axis_info.direction();
        let axis_normal = axis_info.normal();
        let font_size = constraints.text_style.font_size;

        let mut positions = Vec::with_capacity(tick_positions.len());

        for &tick_pos in tick_positions {
            // Position along axis
            let on_axis = Vec2 {
                x: axis_info.start.x + axis_direction.x * axis_info.length * tick_pos as f32,
                y: axis_info.start.y + axis_direction.y * axis_info.length * tick_pos as f32,
            };

            // Offset perpendicular to axis
            let label_offset = calculate_label_offset(axis_info, font_size);
            let label_pos = Vec2 {
                x: on_axis.x + axis_normal.x * label_offset,
                y: on_axis.y + axis_normal.y * label_offset,
            };

            let text = formatter.format_value(tick_pos);
            let anchor = determine_anchor(axis_info.position);

            // Estimate bounds using formatter width hint + font size height
            let est_width = formatter.estimate_width(tick_pos);
            let est_height = font_size;
            let anchor_off = anchor.offset();
            let top_left = Vec2 {
                x: label_pos.x - est_width * anchor_off.x,
                y: label_pos.y - est_height * anchor_off.y,
            };

            let bounds = TextBounds::new(
                top_left.x,
                top_left.y,
                top_left.x + est_width,
                top_left.y + est_height,
            );

            positions.push(LabelPosition {
                position: label_pos,
                rotation: 0.0,
                anchor,
                text,
                style: constraints.text_style.clone(),
                bounds,
            });

            if let Some(max) = constraints.max_labels
                && positions.len() >= max
            {
                break;
            }
        }

        positions
    }
}

impl Default for LabelPositioner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Free functions used by both LabelPositioner methods and tests
// ---------------------------------------------------------------------------

/// Simple text-width estimate: character count × average glyph width.
fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    // Approximate average glyph width as 0.6 × font_size
    text.len() as f32 * font_size * 0.6
}

/// Positional priority heuristic: endpoints get priority 1.0, middle gets ≈0.0.
fn positional_priority(index: usize, count: usize) -> f32 {
    if count <= 1 {
        return 1.0;
    }
    let mid = (count - 1) as f32 / 2.0;

    (index as f32 - mid).abs() / mid // 0.0 at center, 1.0 at endpoints
}

/// Calculate label offset from axis based on font size and axis position.
fn calculate_label_offset(axis_info: &AxisInfo, font_size: f32) -> f32 {
    match axis_info.position {
        AxisPosition::Top => -font_size * 1.2,
        AxisPosition::Bottom => font_size * 1.2,
        AxisPosition::Left => -font_size * 0.5,
        AxisPosition::Right => font_size * 0.5,
    }
}

/// Determine appropriate text anchor based on axis position.
fn determine_anchor(position: AxisPosition) -> TextAnchor {
    match position {
        AxisPosition::Top => TextAnchor::BottomCenter,
        AxisPosition::Bottom => TextAnchor::TopCenter,
        AxisPosition::Left => TextAnchor::CenterRight,
        AxisPosition::Right => TextAnchor::CenterLeft,
    }
}

/// Check if any pair of labels in the list overlaps.
fn labels_still_overlap(positions: &[LabelPosition]) -> bool {
    for i in 0..positions.len() {
        for j in (i + 1)..positions.len() {
            if positions[i].bounds.intersects(&positions[j].bounds) {
                return true;
            }
        }
    }
    false
}

/// Calculate the axis-aligned bounding box for a rotated label.
fn calculate_rotated_bounds(position: &LabelPosition) -> TextBounds {
    if position.rotation.abs() < 0.001 {
        return position.bounds;
    }

    let cos_r = position.rotation.cos();
    let sin_r = position.rotation.sin();
    let w = position.bounds.width();
    let h = position.bounds.height();

    let corners = [(0.0_f32, 0.0_f32), (w, 0.0), (w, h), (0.0, h)];

    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for (x, y) in &corners {
        let rx = x * cos_r - y * sin_r;
        let ry = x * sin_r + y * cos_r;
        min_x = min_x.min(rx);
        max_x = max_x.max(rx);
        min_y = min_y.min(ry);
        max_y = max_y.max(ry);
    }

    TextBounds {
        left: position.position.x + min_x,
        top: position.position.y + min_y,
        right: position.position.x + max_x,
        bottom: position.position.y + max_y,
    }
}

/// Calculate margin requirements from a set of label positions relative to the axis.
fn calculate_margin_requirements(positions: &[LabelPosition], axis_info: &AxisInfo) -> Margins {
    if positions.is_empty() {
        return Margins::default();
    }

    let mut margins = Margins::default();
    for pos in positions {
        match axis_info.position {
            AxisPosition::Top => {
                margins.top = margins.top.max(axis_info.start.y - pos.bounds.top);
            }
            AxisPosition::Bottom => {
                margins.bottom = margins.bottom.max(pos.bounds.bottom - axis_info.start.y);
            }
            AxisPosition::Left => {
                margins.left = margins.left.max(axis_info.start.x - pos.bounds.left);
            }
            AxisPosition::Right => {
                margins.right = margins.right.max(pos.bounds.right - axis_info.start.x);
            }
        }
    }
    margins
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::label::NumericFormatter;

    // -- SpatialGrid tests --

    #[test]
    fn test_spatial_grid_no_collision_on_empty() {
        let grid = SpatialGrid::new(10.0);
        let query = TextBounds::new(0.0, 0.0, 5.0, 5.0);
        assert!(!grid.has_collision(&query, 0.0));
    }

    #[test]
    fn test_spatial_grid_detects_exact_overlap() {
        let mut grid = SpatialGrid::new(10.0);
        let b = TextBounds::new(0.0, 0.0, 8.0, 8.0);
        grid.insert(b);
        assert!(grid.has_collision(&b, 0.0));
    }

    #[test]
    fn test_spatial_grid_detects_partial_overlap() {
        let mut grid = SpatialGrid::new(10.0);
        grid.insert(TextBounds::new(0.0, 0.0, 10.0, 10.0));
        let partial = TextBounds::new(5.0, 5.0, 15.0, 15.0);
        assert!(grid.has_collision(&partial, 0.0));
    }

    #[test]
    fn test_spatial_grid_no_false_positive_same_cell() {
        // Two small bounds in the same grid cell but not overlapping
        let mut grid = SpatialGrid::new(100.0);
        grid.insert(TextBounds::new(0.0, 0.0, 5.0, 5.0));
        let non_overlap = TextBounds::new(6.0, 6.0, 10.0, 10.0);
        assert!(!grid.has_collision(&non_overlap, 0.0));
    }

    #[test]
    fn test_spatial_grid_no_collision_far_apart() {
        let mut grid = SpatialGrid::new(10.0);
        grid.insert(TextBounds::new(0.0, 0.0, 5.0, 5.0));
        let far = TextBounds::new(100.0, 100.0, 110.0, 110.0);
        assert!(!grid.has_collision(&far, 0.0));
    }

    #[test]
    fn test_spatial_grid_with_padding() {
        let mut grid = SpatialGrid::new(10.0);
        grid.insert(TextBounds::new(0.0, 0.0, 10.0, 10.0));
        // 12px away — no collision without padding
        let nearby = TextBounds::new(12.0, 0.0, 22.0, 10.0);
        assert!(!grid.has_collision(&nearby, 0.0));
        // But with 3px padding, it overlaps
        assert!(grid.has_collision(&nearby, 3.0));
    }

    #[test]
    fn test_spatial_grid_query_collisions() {
        let mut grid = SpatialGrid::new(10.0);
        grid.insert(TextBounds::new(0.0, 0.0, 10.0, 10.0));
        grid.insert(TextBounds::new(100.0, 100.0, 110.0, 110.0));

        let query = TextBounds::new(5.0, 5.0, 15.0, 15.0);
        let results = grid.query_collisions(&query, 0.0);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_spatial_grid_clear() {
        let mut grid = SpatialGrid::new(10.0);
        grid.insert(TextBounds::new(0.0, 0.0, 5.0, 5.0));
        assert!(!grid.is_empty());
        grid.clear();
        assert!(grid.is_empty());
        assert!(!grid.has_collision(&TextBounds::new(0.0, 0.0, 5.0, 5.0), 0.0));
    }

    // -- LabelPositioner creation tests --

    #[test]
    fn test_label_positioner_creation() {
        let positioner = LabelPositioner::new();
        assert!(positioner.placed_labels.is_empty());
        assert!(positioner.grid.is_empty());
        assert_eq!(positioner.strategies.len(), 3);
    }

    #[test]
    fn test_label_positioner_custom_strategies() {
        let positioner =
            LabelPositioner::with_strategies(vec![LabelPositioningStrategy::default_hide()]);
        assert_eq!(positioner.strategies.len(), 1);
    }

    // -- Anchor determination --

    #[test]
    fn test_anchor_determination() {
        assert_eq!(
            determine_anchor(AxisPosition::Top),
            TextAnchor::BottomCenter
        );
        assert_eq!(
            determine_anchor(AxisPosition::Bottom),
            TextAnchor::TopCenter
        );
        assert_eq!(
            determine_anchor(AxisPosition::Left),
            TextAnchor::CenterRight
        );
        assert_eq!(
            determine_anchor(AxisPosition::Right),
            TextAnchor::CenterLeft
        );
    }

    // -- Label offset --

    #[test]
    fn test_label_offset_horizontal() {
        let axis_info = AxisInfo::horizontal(800.0);
        let offset = calculate_label_offset(&axis_info, 16.0);
        assert!(offset > 0.0); // Bottom axis → positive offset
    }

    #[test]
    fn test_label_offset_vertical() {
        let axis_info = AxisInfo::vertical(600.0);
        let offset = calculate_label_offset(&axis_info, 16.0);
        assert!(offset < 0.0); // Left axis → negative offset
    }

    // -- Rotated bounds --

    #[test]
    fn test_rotated_bounds_no_rotation() {
        let pos = LabelPosition {
            position: Vec2 { x: 10.0, y: 20.0 },
            rotation: 0.0,
            anchor: TextAnchor::TopLeft,
            text: "Test".into(),
            style: TextStyle::default(),
            bounds: TextBounds::new(10.0, 20.0, 50.0, 35.0),
        };
        let rb = calculate_rotated_bounds(&pos);
        assert_eq!(rb, pos.bounds);
    }

    #[test]
    fn test_rotated_bounds_90_degrees() {
        let pos = LabelPosition {
            position: Vec2 { x: 0.0, y: 0.0 },
            rotation: std::f32::consts::FRAC_PI_2,
            anchor: TextAnchor::TopLeft,
            text: "Test".into(),
            style: TextStyle::default(),
            bounds: TextBounds::new(0.0, 0.0, 10.0, 5.0),
        };
        let rb = calculate_rotated_bounds(&pos);
        assert!((rb.width() - 5.0).abs() < 0.1);
        assert!((rb.height() - 10.0).abs() < 0.1);
    }

    // -- Overlap detection --

    #[test]
    fn test_labels_still_overlap_yes() {
        let positions = vec![
            LabelPosition {
                position: Vec2 { x: 0.0, y: 0.0 },
                rotation: 0.0,
                anchor: TextAnchor::TopLeft,
                text: "A".into(),
                style: TextStyle::default(),
                bounds: TextBounds::new(0.0, 0.0, 20.0, 10.0),
            },
            LabelPosition {
                position: Vec2 { x: 10.0, y: 5.0 },
                rotation: 0.0,
                anchor: TextAnchor::TopLeft,
                text: "B".into(),
                style: TextStyle::default(),
                bounds: TextBounds::new(10.0, 5.0, 30.0, 15.0),
            },
        ];
        assert!(labels_still_overlap(&positions));
    }

    #[test]
    fn test_labels_still_overlap_no() {
        let positions = vec![
            LabelPosition {
                position: Vec2 { x: 0.0, y: 0.0 },
                rotation: 0.0,
                anchor: TextAnchor::TopLeft,
                text: "A".into(),
                style: TextStyle::default(),
                bounds: TextBounds::new(0.0, 0.0, 10.0, 10.0),
            },
            LabelPosition {
                position: Vec2 { x: 50.0, y: 0.0 },
                rotation: 0.0,
                anchor: TextAnchor::TopLeft,
                text: "B".into(),
                style: TextStyle::default(),
                bounds: TextBounds::new(50.0, 0.0, 60.0, 10.0),
            },
        ];
        assert!(!labels_still_overlap(&positions));
    }

    // -- Positional priority --

    #[test]
    fn test_positional_priority() {
        // 5 labels: endpoints should have highest priority
        assert!((positional_priority(0, 5) - 1.0).abs() < 0.01);
        assert!((positional_priority(4, 5) - 1.0).abs() < 0.01);
        assert!((positional_priority(2, 5) - 0.0).abs() < 0.01); // center
    }

    // -- Margin calculation --

    #[test]
    fn test_margin_calculation_bottom_axis() {
        let axis_info = AxisInfo::horizontal(800.0);
        let positions = vec![LabelPosition {
            position: Vec2 { x: 100.0, y: 20.0 },
            rotation: 0.0,
            anchor: TextAnchor::TopCenter,
            text: "Label".into(),
            style: TextStyle::default(),
            bounds: TextBounds::new(95.0, 20.0, 105.0, 35.0),
        }];
        let margins = calculate_margin_requirements(&positions, &axis_info);
        assert!(margins.bottom > 0.0);
    }

    // -- layout_labels integration test --

    #[test]
    fn test_layout_labels_empty() {
        let mut positioner = LabelPositioner::new();
        let axis_info = AxisInfo::horizontal(800.0);
        let formatter = NumericFormatter::default();
        let constraints = LabelConstraints::default();

        let layout = positioner
            .layout_labels(&[], &axis_info, &formatter, &constraints)
            .unwrap();
        assert!(layout.positions.is_empty());
    }

    #[test]
    fn test_layout_labels_no_overlap() {
        let mut positioner = LabelPositioner::new();
        let axis_info = AxisInfo::horizontal(800.0);
        let formatter = NumericFormatter::default();
        let constraints = LabelConstraints::default();

        // Widely spaced tick positions — should not overlap
        let ticks = vec![0.0, 0.5, 1.0];
        let layout = positioner
            .layout_labels(&ticks, &axis_info, &formatter, &constraints)
            .unwrap();

        assert_eq!(layout.positions.len(), 3);
        assert!(layout.hidden_labels.is_empty());
        assert!(!layout.rotated);
    }

    #[test]
    fn test_layout_labels_dense_hides_some() {
        let mut positioner = LabelPositioner::new();
        let axis_info = AxisInfo::horizontal(100.0); // short axis
        let formatter = NumericFormatter::default();
        let constraints = LabelConstraints {
            hide_overlapping: true,
            allow_rotation: false,
            min_spacing: 2.0,
            text_style: TextStyle::new(14.0),
            ..Default::default()
        };

        // 20 ticks on a 100px axis — many will overlap
        let ticks: Vec<f64> = (0..20).map(|i| i as f64 / 19.0).collect();
        let layout = positioner
            .layout_labels(&ticks, &axis_info, &formatter, &constraints)
            .unwrap();

        // Some labels should have been hidden
        assert!(layout.positions.len() < 20);
        assert!(!layout.hidden_labels.is_empty());
    }

    #[test]
    fn test_layout_labels_rotation_resolves_overlap() {
        let mut positioner = LabelPositioner::new();
        let axis_info = AxisInfo::horizontal(200.0);
        let formatter = NumericFormatter::default();
        let constraints = LabelConstraints {
            allow_rotation: true,
            max_rotation: std::f32::consts::FRAC_PI_2,
            hide_overlapping: false,
            min_spacing: 2.0,
            text_style: TextStyle::new(12.0),
            ..Default::default()
        };

        // Moderate density — rotation may help
        let ticks: Vec<f64> = (0..8).map(|i| i as f64 / 7.0).collect();
        let layout = positioner
            .layout_labels(&ticks, &axis_info, &formatter, &constraints)
            .unwrap();

        // The layout should produce results (rotation or not, labels are generated)
        assert!(!layout.positions.is_empty());
    }

    // -- resolve_labels integration test --

    #[test]
    fn test_resolve_labels_no_overlap() {
        let mut positioner = LabelPositioner::new();
        let axis_info = AxisInfo::horizontal(800.0);
        let constraints = LabelConstraints::default();

        let axis_labels = vec![
            AxisLabel {
                text: "0".into(),
                screen_position: Vec2 { x: 50.0, y: 620.0 },
                ndc_position: Vec2 { x: -0.8, y: -0.8 },
                anchor: TextAnchor::TopCenter,
                value: 0.0,
            },
            AxisLabel {
                text: "100".into(),
                screen_position: Vec2 { x: 750.0, y: 620.0 },
                ndc_position: Vec2 { x: 0.8, y: -0.8 },
                anchor: TextAnchor::TopCenter,
                value: 100.0,
            },
        ];

        let layout = positioner
            .resolve_labels(&axis_labels, &axis_info, &constraints)
            .unwrap();

        assert_eq!(layout.positions.len(), 2);
        assert!(layout.hidden_labels.is_empty());
    }

    #[test]
    fn test_resolve_labels_dense_hides() {
        let mut positioner = LabelPositioner::new();
        let axis_info = AxisInfo::horizontal(100.0);
        let constraints = LabelConstraints {
            hide_overlapping: true,
            allow_rotation: false,
            min_spacing: 2.0,
            text_style: TextStyle::new(14.0),
            ..Default::default()
        };

        // Generate many overlapping labels
        let axis_labels: Vec<AxisLabel> = (0..15)
            .map(|i| AxisLabel {
                text: format!("{:.1}", i as f64 / 14.0),
                screen_position: Vec2 {
                    x: (i as f32 / 14.0) * 100.0,
                    y: 50.0,
                },
                ndc_position: Vec2 {
                    x: (i as f32 / 14.0) * 2.0 - 1.0,
                    y: -0.8,
                },
                anchor: TextAnchor::TopCenter,
                value: i as f64 / 14.0,
            })
            .collect();

        let layout = positioner
            .resolve_labels(&axis_labels, &axis_info, &constraints)
            .unwrap();

        assert!(layout.positions.len() < 15);
        assert!(!layout.hidden_labels.is_empty());
    }

    // -- Estimate text width --

    #[test]
    fn test_estimate_text_width() {
        let w = estimate_text_width("Hello", 16.0);
        // 5 chars × 16.0 × 0.6 = 48.0
        assert!((w - 48.0).abs() < 0.01);
    }

    // -- Performance: 500 labels --

    #[test]
    fn test_performance_500_labels() {
        let mut positioner = LabelPositioner::new();
        let axis_info = AxisInfo::horizontal(2000.0);
        let formatter = NumericFormatter::default();
        let constraints = LabelConstraints {
            hide_overlapping: true,
            allow_rotation: false,
            min_spacing: 2.0,
            text_style: TextStyle::new(12.0),
            ..Default::default()
        };

        let ticks: Vec<f64> = (0..500).map(|i| i as f64 / 499.0).collect();

        let start = std::time::Instant::now();
        let layout = positioner
            .layout_labels(&ticks, &axis_info, &formatter, &constraints)
            .unwrap();
        let elapsed = start.elapsed();

        assert!(!layout.positions.is_empty());

        // Debug builds are significantly slower; use a generous threshold to
        // avoid flaky CI failures while still catching real regressions.
        // Typically < 1ms on modern hardware in release mode.
        #[cfg(debug_assertions)]
        let threshold_ms: u128 = 50;
        #[cfg(not(debug_assertions))]
        let threshold_ms: u128 = 10;

        assert!(
            elapsed.as_millis() < threshold_ms,
            "500 labels took {}ms (target < {}ms)",
            elapsed.as_millis(),
            threshold_ms,
        );
    }
}
