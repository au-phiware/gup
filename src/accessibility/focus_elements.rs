// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Focus elements for data points.
//!
//! This module provides integration between marks and the focus management system,
//! enabling keyboard navigation of individual data points in visualizations.

use super::keyboard::{ElementType, FocusManager, FocusableElement};
use crate::interaction::{Rect, Vec2};

/// Configuration for focus element generation from marks.
#[derive(Debug, Clone)]
pub struct FocusElementConfig {
    /// Size of the focus target (width and height in pixels)
    pub target_size: f32,
    
    /// Maximum number of focus elements to create
    /// (to avoid performance issues with large datasets)
    pub max_elements: usize,
    
    /// Whether to create focus elements for off-screen marks
    pub include_offscreen: bool,
}

impl Default for FocusElementConfig {
    fn default() -> Self {
        Self {
            target_size: 20.0,
            max_elements: 1000,
            include_offscreen: false,
        }
    }
}

/// Helper for creating focusable elements from mark data.
#[derive(Debug)]
pub struct MarkFocusHelper {
    config: FocusElementConfig,
}

impl MarkFocusHelper {
    /// Create a new mark focus helper with default configuration.
    pub fn new() -> Self {
        Self {
            config: FocusElementConfig::default(),
        }
    }
    
    /// Create a new mark focus helper with custom configuration.
    pub fn with_config(config: FocusElementConfig) -> Self {
        Self { config }
    }
    
    /// Register mark positions as focusable elements.
    ///
    /// Takes a list of mark positions and descriptions, creates focusable elements,
    /// and registers them with the provided FocusManager.
    ///
    /// # Arguments
    ///
    /// * `focus_manager` - The focus manager to register elements with
    /// * `positions` - List of (position, data_index, description) tuples
    ///
    /// # Returns
    ///
    /// The number of elements registered
    pub fn register_mark_positions(
        &self,
        focus_manager: &mut FocusManager,
        positions: &[(Vec2, usize, String)],
    ) -> usize {
        let mut registered = 0;
        let count = positions.len().min(self.config.max_elements);
        
        for (pos, data_index, description) in positions.iter().take(count) {
            let bounds = Rect::from_center_size(*pos, Vec2::new(self.config.target_size, self.config.target_size));
            
            let element = FocusableElement::new(
                ElementType::DataPoint,
                bounds,
                description.clone(),
            )
            .with_data_index(*data_index);
            
            focus_manager.add_focusable_element(element);
            registered += 1;
        }
        
        registered
    }
    
    /// Create focus elements from mark centers without registering.
    ///
    /// This is useful for batch operations or custom registration logic.
    pub fn create_focus_elements(
        &self,
        positions: &[(Vec2, usize, String)],
    ) -> Vec<FocusableElement> {
        positions
            .iter()
            .take(self.config.max_elements)
            .map(|(pos, data_index, description)| {
                let bounds = Rect::from_center_size(
                    *pos,
                    Vec2::new(self.config.target_size, self.config.target_size),
                );
                
                FocusableElement::new(ElementType::DataPoint, bounds, description.clone())
                    .with_data_index(*data_index)
            })
            .collect()
    }
}

impl Default for MarkFocusHelper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = FocusElementConfig::default();
        assert_eq!(config.target_size, 20.0);
        assert_eq!(config.max_elements, 1000);
        assert!(!config.include_offscreen);
    }

    #[test]
    fn test_create_focus_elements() {
        let helper = MarkFocusHelper::new();
        let positions = vec![
            (Vec2::new(100.0, 100.0), 0, "Point 1".to_string()),
            (Vec2::new(200.0, 200.0), 1, "Point 2".to_string()),
        ];
        
        let elements = helper.create_focus_elements(&positions);
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].data_index, Some(0));
        assert_eq!(elements[1].data_index, Some(1));
    }

    #[test]
    fn test_max_elements_limit() {
        let config = FocusElementConfig {
            max_elements: 2,
            ..Default::default()
        };
        let helper = MarkFocusHelper::with_config(config);
        
        let positions = vec![
            (Vec2::new(100.0, 100.0), 0, "Point 1".to_string()),
            (Vec2::new(200.0, 200.0), 1, "Point 2".to_string()),
            (Vec2::new(300.0, 300.0), 2, "Point 3".to_string()),
        ];
        
        let elements = helper.create_focus_elements(&positions);
        assert_eq!(elements.len(), 2, "Should respect max_elements limit");
    }

    #[test]
    fn test_register_mark_positions() {
        let helper = MarkFocusHelper::new();
        let mut focus_manager = FocusManager::new();
        
        let positions = vec![
            (Vec2::new(100.0, 100.0), 0, "Point 1".to_string()),
            (Vec2::new(200.0, 200.0), 1, "Point 2".to_string()),
        ];
        
        let count = helper.register_mark_positions(&mut focus_manager, &positions);
        assert_eq!(count, 2);
    }
}
