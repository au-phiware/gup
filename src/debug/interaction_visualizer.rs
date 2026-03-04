// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interactive GPU debugging visualization tool for interaction system.
//!
//! Provides visual debugging of GPU hit testing by rendering:
//! - Element positions and boundaries
//! - Query locations and regions
//! - Hit test results with color coding
//! - Buffer contents inspection
//!
//! # Examples
//!
//! ```rust,ignore
//! use gup::debug::InteractionDebugVisualizer;
//! use gup::interaction::{ElementData, GpuInteractionQuery, InteractionResult};
//! use gup::RenderContext;
//! use std::sync::Arc;
//!
//! async fn debug_interaction() -> Result<(), Box<dyn std::error::Error>> {
//!     let context = Arc::new(RenderContext::new().await?);
//!     let mut visualizer = InteractionDebugVisualizer::new(context.clone());
//!
//!     // Update with current interaction data
//!     visualizer.update(&elements, &queries, &results);
//!
//!     // Export visualization or show in debug window
//!     visualizer.export_json("debug_interaction.json")?;
//!
//!     Ok(())
//! }
//! ```

use crate::RenderContext;
use crate::error::{GupError, GupResult};
use crate::interaction::{ElementData, GpuInteractionQuery, InteractionResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Color coding for visualization
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DebugColor {
    /// Red channel value.
    pub r: f32,
    /// Green channel value.
    pub g: f32,
    /// Blue channel value.
    pub b: f32,
    /// Alpha channel value.
    pub a: f32,
}

impl DebugColor {
    /// Create a new debug color from RGBA components.
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Blue for Circle marks
    pub const CIRCLE: Self = Self::new(0.2, 0.4, 0.8, 1.0);
    /// Green for Rectangle marks
    pub const RECTANGLE: Self = Self::new(0.2, 0.8, 0.4, 1.0);
    /// Red for Line marks
    pub const LINE: Self = Self::new(0.8, 0.2, 0.2, 1.0);
    /// Yellow for query points
    pub const QUERY: Self = Self::new(1.0, 0.9, 0.0, 1.0);
    /// Semi-transparent query region
    pub const QUERY_REGION: Self = Self::new(1.0, 0.9, 0.0, 0.3);
    /// Green for hits
    pub const HIT: Self = Self::new(0.0, 1.0, 0.0, 0.8);
    /// Red for misses
    pub const MISS: Self = Self::new(1.0, 0.0, 0.0, 0.3);
    /// Orange for highlighted elements
    pub const HIGHLIGHT: Self = Self::new(1.0, 0.6, 0.0, 1.0);
}

/// Visualization representation of an element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualElement {
    /// Screen position of the element.
    pub position: [f32; 2],
    /// Size of the element bounding box.
    pub size: [f32; 2],
    /// Numeric mark type identifier.
    pub mark_type: u32,
    /// Human-readable mark type name.
    pub mark_type_name: String,
    /// Selection group identifier.
    pub selection_id: u32,
    /// Unique element identifier.
    pub element_id: u32,
    /// Display color for this element.
    pub color: DebugColor,
    /// Whether this element is highlighted as a hit.
    pub is_highlighted: bool,
}

impl VisualElement {
    fn from_element_data(data: &ElementData, element_id: u32) -> Self {
        let (mark_type_name, color) = match data.mark_type {
            0 => ("Circle", DebugColor::CIRCLE),
            1 => ("Rectangle", DebugColor::RECTANGLE),
            2 => ("Line", DebugColor::LINE),
            _ => ("Unknown", DebugColor::new(0.5, 0.5, 0.5, 1.0)),
        };

        Self {
            position: data.position,
            size: data.size,
            mark_type: data.mark_type,
            mark_type_name: mark_type_name.to_string(),
            selection_id: data.selection_id,
            element_id,
            color,
            is_highlighted: false,
        }
    }
}

/// Visualization representation of a query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualQuery {
    /// Screen position of the query.
    pub position: [f32; 2],
    /// Region size for area queries.
    pub region_size: [f32; 2],
    /// Numeric query type identifier.
    pub query_type_id: u32,
    /// Human-readable query type name.
    pub query_type_name: String,
    /// Maximum number of results for this query.
    pub max_results: u32,
    /// Display color for this query.
    pub color: DebugColor,
}

impl VisualQuery {
    fn from_gpu_query(query: &GpuInteractionQuery) -> Self {
        let query_type_name = match query.query_type {
            0 => "Point",
            1 => "Region",
            _ => "Unknown",
        };

        Self {
            position: query.position,
            region_size: query.region_size,
            query_type_id: query.query_type,
            query_type_name: query_type_name.to_string(),
            max_results: query.max_results,
            color: DebugColor::QUERY,
        }
    }
}

/// Visualization representation of a hit result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualResult {
    /// Element that was tested.
    pub element_id: u32,
    /// Selection group of the element.
    pub selection_id: u32,
    /// Distance from query point to element.
    pub distance: f32,
    /// Whether the element was hit.
    pub is_hit: bool,
    /// Point of intersection between query and element.
    pub intersection_point: [f32; 2],
    /// Display color for this result.
    pub color: DebugColor,
}

impl VisualResult {
    fn from_interaction_result(result: &InteractionResult) -> Self {
        let is_hit = result.is_hit != 0;
        let color = if is_hit {
            DebugColor::HIT
        } else {
            DebugColor::MISS
        };

        Self {
            element_id: result.element_id,
            selection_id: result.selection_id,
            distance: result.distance,
            is_hit,
            intersection_point: result.intersection_point,
            color,
        }
    }
}

/// Summary statistics for the debug view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugSummary {
    /// Total number of elements in the scene.
    pub total_elements: usize,
    /// Total number of queries executed.
    pub total_queries: usize,
    /// Total number of results returned.
    pub total_results: usize,
    /// Number of successful hits.
    pub total_hits: usize,
    /// Number of misses.
    pub total_misses: usize,
    /// Element counts grouped by mark type.
    pub elements_by_mark_type: Vec<(String, usize)>,
    /// Hit rate as a percentage.
    pub hit_rate_percent: f32,
}

impl DebugSummary {
    fn new(elements: &[VisualElement], queries: &[VisualQuery], results: &[VisualResult]) -> Self {
        let total_hits = results.iter().filter(|r| r.is_hit).count();
        let total_misses = results.len() - total_hits;
        let hit_rate_percent = if !results.is_empty() {
            (total_hits as f32 / results.len() as f32) * 100.0
        } else {
            0.0
        };

        // Count elements by mark type
        let mut mark_type_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for element in elements {
            *mark_type_counts
                .entry(element.mark_type_name.clone())
                .or_insert(0) += 1;
        }

        let mut elements_by_mark_type: Vec<(String, usize)> =
            mark_type_counts.into_iter().collect();
        elements_by_mark_type.sort_by(|a, b| b.1.cmp(&a.1));

        Self {
            total_elements: elements.len(),
            total_queries: queries.len(),
            total_results: results.len(),
            total_hits,
            total_misses,
            elements_by_mark_type,
            hit_rate_percent,
        }
    }
}

/// Complete debug visualization state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugVisualizationState {
    /// Timestamp when the state was captured.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Visual representations of scene elements.
    pub elements: Vec<VisualElement>,
    /// Visual representations of interaction queries.
    pub queries: Vec<VisualQuery>,
    /// Visual representations of hit test results.
    pub results: Vec<VisualResult>,
    /// Summary statistics for the debug view.
    pub summary: DebugSummary,
}

/// Interactive GPU debugging visualizer for interaction system
pub struct InteractionDebugVisualizer {
    #[allow(dead_code)]
    context: Arc<RenderContext>,
    state: Option<DebugVisualizationState>,
    enabled: bool,
}

impl InteractionDebugVisualizer {
    /// Create a new interaction debug visualizer
    pub fn new(context: Arc<RenderContext>) -> Self {
        Self {
            context,
            state: None,
            enabled: cfg!(debug_assertions),
        }
    }

    /// Enable or disable the visualizer
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if visualizer is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Update visualizer with current interaction data
    pub fn update(
        &mut self,
        elements: &[ElementData],
        queries: &[GpuInteractionQuery],
        results: &[InteractionResult],
    ) {
        if !self.enabled {
            return;
        }

        // Convert to visual representations
        let visual_elements: Vec<VisualElement> = elements
            .iter()
            .enumerate()
            .map(|(i, e)| VisualElement::from_element_data(e, i as u32))
            .collect();

        let visual_queries: Vec<VisualQuery> =
            queries.iter().map(VisualQuery::from_gpu_query).collect();

        let visual_results: Vec<VisualResult> = results
            .iter()
            .map(VisualResult::from_interaction_result)
            .collect();

        // Highlight elements that were hit
        let mut highlighted_elements = visual_elements;
        for result in &visual_results {
            if result.is_hit
                && let Some(elem) = highlighted_elements.get_mut(result.element_id as usize)
            {
                elem.is_highlighted = true;
                elem.color = DebugColor::HIGHLIGHT;
            }
        }

        let summary = DebugSummary::new(&highlighted_elements, &visual_queries, &visual_results);

        self.state = Some(DebugVisualizationState {
            timestamp: chrono::Utc::now(),
            elements: highlighted_elements,
            queries: visual_queries,
            results: visual_results,
            summary,
        });
    }

    /// Get current visualization state
    pub fn state(&self) -> Option<&DebugVisualizationState> {
        self.state.as_ref()
    }

    /// Export current state to JSON file
    pub fn export_json(&self, path: &str) -> GupResult<()> {
        let state = self
            .state
            .as_ref()
            .ok_or_else(|| GupError::validation_error("No visualization state available"))?;

        let json = serde_json::to_string_pretty(state).map_err(|e| {
            GupError::validation_error(format!("Failed to serialize visualization state: {e}"))
        })?;

        std::fs::write(path, json).map_err(|e| {
            GupError::resource_error(format!("Failed to write visualization state: {e}"))
        })?;

        Ok(())
    }

    /// Generate ASCII art visualization for terminal
    pub fn render_ascii(&self, width: usize, height: usize) -> GupResult<String> {
        let state = self
            .state
            .as_ref()
            .ok_or_else(|| GupError::validation_error("No visualization state available"))?;

        let mut output = String::new();
        output.push_str("╔══════════════════════════════════════════════════════════════╗\n");
        output.push_str("║         GPU Interaction Debug Visualization                 ║\n");
        output.push_str("╠══════════════════════════════════════════════════════════════╣\n");
        output.push_str(&format!(
            "║ Timestamp: {:<47}║\n",
            state.timestamp.format("%Y-%m-%d %H:%M:%S")
        ));
        output.push_str(&format!(
            "║ Elements: {:<50}║\n",
            state.summary.total_elements
        ));
        output.push_str(&format!(
            "║ Queries:  {:<50}║\n",
            state.summary.total_queries
        ));
        output.push_str(&format!("║ Hits:     {:<50}║\n", state.summary.total_hits));
        output.push_str(&format!(
            "║ Misses:   {:<50}║\n",
            state.summary.total_misses
        ));
        output.push_str(&format!(
            "║ Hit Rate: {:<47.1}%║\n",
            state.summary.hit_rate_percent
        ));
        output.push_str("╠══════════════════════════════════════════════════════════════╣\n");
        output.push_str("║ Elements by Mark Type:                                      ║\n");

        for (mark_type, count) in &state.summary.elements_by_mark_type {
            output.push_str(&format!("║   {:<10} : {:<46}║\n", mark_type, count));
        }

        output.push_str("╚══════════════════════════════════════════════════════════════╝\n");

        // Simple grid visualization
        if !state.elements.is_empty() {
            output.push_str("\nElement Distribution:\n");

            // Calculate bounds
            let mut min_x = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_y = f32::NEG_INFINITY;

            for elem in &state.elements {
                min_x = min_x.min(elem.position[0]);
                max_x = max_x.max(elem.position[0]);
                min_y = min_y.min(elem.position[1]);
                max_y = max_y.max(elem.position[1]);
            }

            // Add margin
            let margin = 10.0;
            min_x -= margin;
            max_x += margin;
            min_y -= margin;
            max_y += margin;

            // Create grid
            let mut grid = vec![vec![' '; width]; height];

            // Plot elements
            for elem in &state.elements {
                let x = ((elem.position[0] - min_x) / (max_x - min_x) * width as f32) as usize;
                let y = ((elem.position[1] - min_y) / (max_y - min_y) * height as f32) as usize;

                if x < width && y < height {
                    grid[y][x] = if elem.is_highlighted {
                        '*'
                    } else {
                        match elem.mark_type {
                            0 => 'o', // Circle
                            1 => '□', // Rectangle
                            2 => '─', // Line
                            _ => '?',
                        }
                    };
                }
            }

            // Plot queries
            for query in &state.queries {
                let x = ((query.position[0] - min_x) / (max_x - min_x) * width as f32) as usize;
                let y = ((query.position[1] - min_y) / (max_y - min_y) * height as f32) as usize;

                if x < width && y < height {
                    grid[y][x] = '+';
                }
            }

            // Render grid
            for row in grid {
                output.push_str(&row.iter().collect::<String>());
                output.push('\n');
            }

            output.push_str("\nLegend: o=Circle  □=Rectangle  ─=Line  +=Query  *=Hit\n");
        }

        Ok(output)
    }

    /// Get detailed buffer inspection data
    pub fn inspect_buffers(&self) -> GupResult<BufferInspectionData> {
        let state = self
            .state
            .as_ref()
            .ok_or_else(|| GupError::validation_error("No visualization state available"))?;

        Ok(BufferInspectionData {
            element_buffer: state.elements.clone(),
            query_buffer: state.queries.clone(),
            result_buffer: state.results.clone(),
            element_buffer_size_bytes: state.elements.len() * std::mem::size_of::<ElementData>(),
            query_buffer_size_bytes: state.queries.len()
                * std::mem::size_of::<GpuInteractionQuery>(),
            result_buffer_size_bytes: state.results.len()
                * std::mem::size_of::<InteractionResult>(),
        })
    }

    /// Clear current visualization state
    pub fn clear(&mut self) {
        self.state = None;
    }
}

/// Buffer inspection data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferInspectionData {
    /// Visual elements from the element buffer.
    pub element_buffer: Vec<VisualElement>,
    /// Visual queries from the query buffer.
    pub query_buffer: Vec<VisualQuery>,
    /// Visual results from the result buffer.
    pub result_buffer: Vec<VisualResult>,
    /// Size of the element buffer in bytes.
    pub element_buffer_size_bytes: usize,
    /// Size of the query buffer in bytes.
    pub query_buffer_size_bytes: usize,
    /// Size of the result buffer in bytes.
    pub result_buffer_size_bytes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_color_constants() {
        let circle = DebugColor::CIRCLE;
        assert_eq!(circle.r, 0.2);
        assert_eq!(circle.g, 0.4);
        assert_eq!(circle.b, 0.8);
        assert_eq!(circle.a, 1.0);
    }

    #[test]
    fn test_visual_element_from_element_data() {
        let data = ElementData {
            position: [100.0, 200.0],
            size: [10.0, 10.0],
            mark_type: 0, // Circle
            element_id: 0,
            selection_id: 1,
            _padding: 0,
        };

        let visual = VisualElement::from_element_data(&data, 42);

        assert_eq!(visual.position, [100.0, 200.0]);
        assert_eq!(visual.size, [10.0, 10.0]);
        assert_eq!(visual.mark_type, 0);
        assert_eq!(visual.mark_type_name, "Circle");
        assert_eq!(visual.selection_id, 1);
        assert_eq!(visual.element_id, 42);
        assert!(!visual.is_highlighted);
    }

    #[test]
    fn test_debug_summary_calculation() {
        let elements = vec![
            VisualElement {
                position: [0.0, 0.0],
                size: [10.0, 10.0],
                mark_type: 0,
                mark_type_name: "Circle".to_string(),
                selection_id: 0,
                element_id: 0,
                color: DebugColor::CIRCLE,
                is_highlighted: false,
            },
            VisualElement {
                position: [20.0, 20.0],
                size: [10.0, 10.0],
                mark_type: 1,
                mark_type_name: "Rectangle".to_string(),
                selection_id: 0,
                element_id: 1,
                color: DebugColor::RECTANGLE,
                is_highlighted: false,
            },
        ];

        let queries = vec![];
        let results = vec![
            VisualResult {
                element_id: 0,
                selection_id: 0,
                distance: 0.0,
                is_hit: true,
                intersection_point: [0.0, 0.0],
                color: DebugColor::HIT,
            },
            VisualResult {
                element_id: 1,
                selection_id: 0,
                distance: 100.0,
                is_hit: false,
                intersection_point: [0.0, 0.0],
                color: DebugColor::MISS,
            },
        ];

        let summary = DebugSummary::new(&elements, &queries, &results);

        assert_eq!(summary.total_elements, 2);
        assert_eq!(summary.total_queries, 0);
        assert_eq!(summary.total_results, 2);
        assert_eq!(summary.total_hits, 1);
        assert_eq!(summary.total_misses, 1);
        assert_eq!(summary.hit_rate_percent, 50.0);
        assert_eq!(summary.elements_by_mark_type.len(), 2);
    }
}
