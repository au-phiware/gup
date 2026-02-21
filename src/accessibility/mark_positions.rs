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
        // Use the position from the vertex
        GpuPosition {
            x: self.position[0],
            y: self.position[1],
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

// NOTE: The following function is currently disabled as it depends on APIs that
// don't exist yet (cached_attributes() and create_vertex()). This will be
// re-enabled when those APIs are implemented.
/*
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
*/

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::circle::CircleVertex;

    #[test]
    fn test_circle_position_extraction() {
        let vertex = CircleVertex {
            position: [0.5, -0.3],
        };

        let pos = vertex.extract_position();
        assert_eq!(pos.x, 0.5);
        assert_eq!(pos.y, -0.3);
    }
}
