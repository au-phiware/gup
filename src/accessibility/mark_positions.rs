// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mark Position Extraction for Accessibility
//!
//! This module provides functionality to extract positions from marks and selections
//! for use with the DOM overlay positioning system.

use crate::accessibility::position_sync::GpuPosition;
use crate::mark::circle::CircleVertex;
use crate::mark::line::LineVertex;
use crate::mark::rectangle::RectangleVertex;
use crate::mark::Mark;
use crate::selection::Selection;

/// Trait for extracting position from vertex data
pub trait PositionExtractor {
    /// Extract a GPU position from this vertex
    fn extract_position(&self) -> GpuPosition;
}

impl PositionExtractor for CircleVertex {
    fn extract_position(&self) -> GpuPosition {
        GpuPosition {
            x: self.position[0],
            y: self.position[1],
        }
    }
}

impl PositionExtractor for LineVertex {
    fn extract_position(&self) -> GpuPosition {
        // Use the start point for line positioning
        GpuPosition {
            x: self.start[0],
            y: self.start[1],
        }
    }
}

impl PositionExtractor for RectangleVertex {
    fn extract_position(&self) -> GpuPosition {
        // Use the center point for rectangle positioning
        GpuPosition {
            x: self.position[0],
            y: self.position[1],
        }
    }
}

/// Extract positions from a selection
///
/// This function queries the cached attribute values from the selection
/// and extracts the position for each data point.
pub fn extract_positions_from_selection<T, M>(selection: &Selection<T, M>) -> Vec<GpuPosition>
where
    M: Mark,
    M::Vertex: PositionExtractor,
{
    selection
        .cached_attributes()
        .iter()
        .map(|attr| {
            let vertex = M::create_vertex(attr);
            vertex.extract_position()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::circle::{Circle, CircleAttributes, CircleVertex};

    #[test]
    fn test_circle_position_extraction() {
        let vertex = CircleVertex {
            position: [0.5, -0.3],
            color: [1.0, 0.0, 0.0, 1.0],
            radius: 5.0,
            _padding: [0.0; 3],
        };

        let pos = vertex.extract_position();
        assert_eq!(pos.x, 0.5);
        assert_eq!(pos.y, -0.3);
    }

    #[test]
    fn test_circle_positions_from_attributes() {
        let attrs = vec![
            CircleAttributes {
                position: [0.0, 0.0],
                color: [1.0, 0.0, 0.0, 1.0],
                radius: 5.0,
            },
            CircleAttributes {
                position: [0.5, 0.5],
                color: [0.0, 1.0, 0.0, 1.0],
                radius: 7.0,
            },
        ];

        let positions: Vec<GpuPosition> = attrs
            .iter()
            .map(|attr| {
                let vertex = Circle::create_vertex(attr);
                vertex.extract_position()
            })
            .collect();

        assert_eq!(positions.len(), 2);
        assert_eq!(positions[0].x, 0.0);
        assert_eq!(positions[0].y, 0.0);
        assert_eq!(positions[1].x, 0.5);
        assert_eq!(positions[1].y, 0.5);
    }
}
