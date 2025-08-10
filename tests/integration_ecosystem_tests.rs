// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive integration tests for the Mixable trait ecosystem integration.
//!
//! These tests validate all components of the GUP-023 story:
//! - Integration helper utilities
//! - Plugin system functionality
//! - External wrapper system
//! - Cross-platform compatibility
//! - Performance characteristics

use gup::integration::*;
use gup::plugins::development::*;
use gup::plugins::*;
use gup::{Mixable, MixableExt, RenderContext, register_mixable_plugin};
use std::any::{Any, TypeId};

// Test data structures representing external visualization libraries

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MockExternalChart {
    data: Vec<(f32, f32)>,
    chart_type: MockChartType,
    metadata: ChartMetadata,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum MockChartType {
    Scatter,
    Line,
    Bar,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ChartMetadata {
    title: String,
    color: [f32; 4],
    size: f32,
}

impl MockExternalChart {
    fn new_scatter(data: Vec<(f32, f32)>, title: &str, color: [f32; 4]) -> Self {
        Self {
            data,
            chart_type: MockChartType::Scatter,
            metadata: ChartMetadata {
                title: title.to_string(),
                color,
                size: 5.0,
            },
        }
    }

    fn new_line(data: Vec<(f32, f32)>, title: &str, color: [f32; 4]) -> Self {
        Self {
            data,
            chart_type: MockChartType::Line,
            metadata: ChartMetadata {
                title: title.to_string(),
                color,
                size: 2.0,
            },
        }
    }

    fn point_count(&self) -> usize {
        self.data.len()
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct MockTimeSeriesData {
    timestamps: Vec<i64>,
    values: Vec<f64>,
    name: String,
}

impl MockTimeSeriesData {
    fn new(name: &str, data: Vec<(i64, f64)>) -> Self {
        let (timestamps, values) = data.into_iter().unzip();
        Self {
            timestamps,
            values,
            name: name.to_string(),
        }
    }

    fn to_normalized_points(&self) -> Vec<[f32; 2]> {
        if self.timestamps.is_empty() {
            return Vec::new();
        }

        let min_time = *self.timestamps.iter().min().unwrap() as f64;
        let max_time = *self.timestamps.iter().max().unwrap() as f64;
        let time_range = max_time - min_time;

        self.timestamps
            .iter()
            .zip(&self.values)
            .map(|(&timestamp, &value)| {
                let normalized_time = if time_range > 0.0 {
                    ((timestamp as f64 - min_time) / time_range) as f32
                } else {
                    0.0
                };
                [normalized_time, value as f32]
            })
            .collect()
    }
}

// Plugin implementations for testing

#[derive(Debug)]
struct MockChartPlugin;

impl MixablePlugin for MockChartPlugin {
    fn name(&self) -> &str {
        "mock_chart_plugin"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn can_handle(&self, type_id: TypeId) -> bool {
        type_id == TypeId::of::<MockExternalChart>()
    }

    fn create_mixable(&self, object: Box<dyn Any + Send + Sync>) -> Box<dyn Mixable<Output = ()>> {
        if let Ok(chart) = object.downcast::<MockExternalChart>() {
            let wrapped = wrap_point_data(*chart, |chart| {
                chart.data.iter().map(|&(x, y)| [x, y]).collect()
            });
            Box::new(wrapped)
        } else {
            panic!("MockChartPlugin received unexpected type");
        }
    }

    fn validate(&self) -> Result<(), String> {
        Ok(())
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: "Mock plugin for testing external chart integration".to_string(),
            author: "Gup Test Suite".to_string(),
            supported_types: vec!["MockExternalChart".to_string()],
        }
    }
}

#[derive(Debug)]
struct MockTimeSeriesPlugin {
    should_fail_validation: bool,
}

impl MockTimeSeriesPlugin {
    fn new() -> Self {
        Self {
            should_fail_validation: false,
        }
    }

    fn with_validation_failure() -> Self {
        Self {
            should_fail_validation: true,
        }
    }
}

impl MixablePlugin for MockTimeSeriesPlugin {
    fn name(&self) -> &str {
        "mock_timeseries_plugin"
    }

    fn version(&self) -> &str {
        "2.1.0"
    }

    fn can_handle(&self, type_id: TypeId) -> bool {
        type_id == TypeId::of::<MockTimeSeriesData>()
    }

    fn create_mixable(&self, object: Box<dyn Any + Send + Sync>) -> Box<dyn Mixable<Output = ()>> {
        if let Ok(timeseries) = object.downcast::<MockTimeSeriesData>() {
            let wrapped = wrap_point_data(*timeseries, |data| data.to_normalized_points());
            Box::new(wrapped)
        } else {
            panic!("MockTimeSeriesPlugin received unexpected type");
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.should_fail_validation {
            Err("Mock time series plugin configured to fail validation".to_string())
        } else {
            Ok(())
        }
    }

    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: "Mock plugin for testing time series data integration".to_string(),
            author: "Gup Test Suite".to_string(),
            supported_types: vec!["MockTimeSeriesData".to_string()],
        }
    }
}

// Integration tests

#[tokio::test]
async fn test_external_wrapper_basic_functionality() {
    let mut context = RenderContext::new().await.unwrap();

    let external_chart = MockExternalChart::new_scatter(
        vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)],
        "Test Scatter",
        [1.0, 0.0, 0.0, 1.0],
    );

    let mut wrapped = wrap_point_data(external_chart.clone(), |chart| {
        chart.data.iter().map(|&(x, y)| [x, y]).collect()
    });

    assert!(wrapped.is_valid());
    assert!(wrapped.description().contains("ExternalPointVisualization"));
    assert_eq!(wrapped.inner().point_count(), 3);

    let result = wrapped.render(&mut context);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_external_wrapper_empty_data() {
    let mut context = RenderContext::new().await.unwrap();

    let empty_chart = MockExternalChart::new_scatter(vec![], "Empty Chart", [0.0, 1.0, 0.0, 1.0]);

    let mut wrapped = wrap_point_data(empty_chart, |chart| {
        chart.data.iter().map(|&(x, y)| [x, y]).collect()
    });

    assert!(!wrapped.is_valid()); // Empty data should be invalid

    // Should still render without error (no-op)
    let result = wrapped.render(&mut context);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_custom_render_wrapper() {
    let mut context = RenderContext::new().await.unwrap();

    let chart = MockExternalChart::new_line(
        vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.5)],
        "Custom Line",
        [0.0, 0.0, 1.0, 1.0],
    );

    let mut wrapped = wrap_with_custom_render(chart, |chart, _context| {
        // Custom rendering logic
        let points: Vec<[f32; 2]> = chart.data.iter().map(|&(x, y)| [x, y]).collect();
        if !points.is_empty() {
            // TODO: Use context to render points with GPU pipeline
            let _ = points; // Suppress unused warning
        }
        Ok(())
    });

    assert!(wrapped.is_valid());

    let result = wrapped.render(&mut context);
    assert!(result.is_ok());
}

#[test]
fn test_plugin_registry_basic_operations() {
    let mut registry = MixablePluginRegistry::new();

    // Initially empty
    assert_eq!(registry.list_plugins().len(), 0);
    assert!(!registry.has_plugin("mock_chart_plugin"));

    // Register plugin
    let plugin = MockChartPlugin;
    assert!(registry.register_plugin(Box::new(plugin)).is_ok());

    // Verify registration
    assert_eq!(registry.list_plugins().len(), 1);
    assert!(registry.has_plugin("mock_chart_plugin"));
    assert!(registry.can_handle_type(TypeId::of::<MockExternalChart>()));
    assert!(!registry.can_handle_type(TypeId::of::<String>()));
}

#[test]
fn test_plugin_registry_multiple_plugins() {
    let mut registry = MixablePluginRegistry::new();

    // Register multiple plugins
    let chart_plugin = MockChartPlugin;
    let timeseries_plugin = MockTimeSeriesPlugin::new();

    assert!(registry.register_plugin(Box::new(chart_plugin)).is_ok());
    assert!(
        registry
            .register_plugin(Box::new(timeseries_plugin))
            .is_ok()
    );

    // Verify both are registered
    assert_eq!(registry.list_plugins().len(), 2);
    assert!(registry.has_plugin("mock_chart_plugin"));
    assert!(registry.has_plugin("mock_timeseries_plugin"));

    // Verify type handling
    assert!(registry.can_handle_type(TypeId::of::<MockExternalChart>()));
    assert!(registry.can_handle_type(TypeId::of::<MockTimeSeriesData>()));
}

#[test]
fn test_plugin_registration_conflicts() {
    let mut registry = MixablePluginRegistry::new();

    let plugin1 = MockChartPlugin;
    let plugin2 = MockChartPlugin;

    // First registration should succeed
    assert!(registry.register_plugin(Box::new(plugin1)).is_ok());

    // Second registration with same name should fail
    let result = registry.register_plugin(Box::new(plugin2));
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already registered"));
}

#[test]
fn test_plugin_validation_failure() {
    let mut registry = MixablePluginRegistry::new();

    // Create plugin configured to fail validation
    let plugin = MockTimeSeriesPlugin::with_validation_failure();
    let result = registry.register_plugin(Box::new(plugin));

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .contains("configured to fail validation")
    );
}

#[test]
fn test_plugin_create_mixable() {
    let mut registry = MixablePluginRegistry::new();

    let plugin = MockChartPlugin;
    registry.register_plugin(Box::new(plugin)).unwrap();

    let chart = MockExternalChart::new_scatter(
        vec![(1.0, 2.0), (3.0, 4.0)],
        "Plugin Test",
        [0.5, 0.5, 0.5, 1.0],
    );

    let mixable = registry.create_mixable(chart);
    assert!(mixable.is_some());

    let mixable = mixable.unwrap();
    assert!(mixable.is_valid());
    assert!(mixable.description().contains("ExternalPointVisualization"));
}

#[test]
fn test_plugin_create_mixable_no_handler() {
    let registry = MixablePluginRegistry::new(); // Empty registry

    let chart =
        MockExternalChart::new_scatter(vec![(1.0, 1.0)], "No Handler", [1.0, 1.0, 1.0, 1.0]);

    let mixable = registry.create_mixable(chart);
    assert!(mixable.is_none());
}

#[test]
fn test_plugin_metadata() {
    let mut registry = MixablePluginRegistry::new();

    let plugin = MockChartPlugin;
    registry.register_plugin(Box::new(plugin)).unwrap();

    let metadata = registry.get_plugin_metadata("mock_chart_plugin");
    assert!(metadata.is_some());

    let metadata = metadata.unwrap();
    assert_eq!(metadata.name, "mock_chart_plugin");
    assert_eq!(metadata.version, "1.0.0");
    assert!(metadata.description.contains("Mock plugin"));
    assert_eq!(metadata.author, "Gup Test Suite");
    assert!(
        metadata
            .supported_types
            .contains(&"MockExternalChart".to_string())
    );

    let all_metadata = registry.get_all_metadata();
    assert_eq!(all_metadata.len(), 1);
}

#[test]
fn test_plugin_unregister() {
    let mut registry = MixablePluginRegistry::new();

    let plugin = MockChartPlugin;
    registry.register_plugin(Box::new(plugin)).unwrap();

    assert!(registry.has_plugin("mock_chart_plugin"));
    assert!(registry.unregister_plugin("mock_chart_plugin"));
    assert!(!registry.has_plugin("mock_chart_plugin"));

    // Try to unregister again
    assert!(!registry.unregister_plugin("mock_chart_plugin"));
}

#[test]
fn test_plugin_clear() {
    let mut registry = MixablePluginRegistry::new();

    let chart_plugin = MockChartPlugin;
    let timeseries_plugin = MockTimeSeriesPlugin::new();

    registry.register_plugin(Box::new(chart_plugin)).unwrap();
    registry
        .register_plugin(Box::new(timeseries_plugin))
        .unwrap();

    assert_eq!(registry.list_plugins().len(), 2);

    registry.clear();
    assert_eq!(registry.list_plugins().len(), 0);
}

#[test]
fn test_point_based_plugin_builder() {
    let plugin = PointBasedPluginBuilder::new(
        "test_builder_plugin".to_string(),
        "0.1.0".to_string(),
        |chart: &MockExternalChart| chart.data.iter().map(|&(x, y)| [x, y]).collect(),
    )
    .with_validator(|| Ok(()))
    .build();

    assert_eq!(plugin.name(), "test_builder_plugin");
    assert_eq!(plugin.version(), "0.1.0");
    assert!(plugin.can_handle(TypeId::of::<MockExternalChart>()));
    assert!(!plugin.can_handle(TypeId::of::<String>()));
    assert!(plugin.validate().is_ok());

    let metadata = plugin.metadata();
    assert!(metadata.description.contains("Point-based plugin"));
}

#[test]
fn test_global_registry() {
    // Clear global registry for clean test
    global_registry().lock().unwrap().clear();

    let plugin = MockChartPlugin;
    register_mixable_plugin!(plugin);

    {
        let binding = global_registry().lock().unwrap();
        let plugins = binding.list_plugins();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].0, "mock_chart_plugin");
    } // Lock is dropped here

    // Test try_make_mixable
    let chart = MockExternalChart::new_scatter(
        vec![(0.0, 0.0), (1.0, 1.0)],
        "Global Test",
        [1.0, 0.0, 0.0, 1.0],
    );

    let mixable = try_make_mixable(chart);
    assert!(mixable.is_some());

    let mixable = mixable.unwrap();
    assert!(mixable.is_valid());

    // Clean up for other tests
    global_registry().lock().unwrap().clear();
}

#[test]
fn test_conversion_utilities() {
    use gup::integration::conversion::*;

    // Test tuple conversion
    let tuples = vec![(0.0, 1.0), (2.0, 3.0), (4.0, 5.0)];
    let points = tuples_to_points(&tuples);
    assert_eq!(points, vec![[0.0, 1.0], [2.0, 3.0], [4.0, 5.0]]);

    // Test flat coordinates conversion
    let flat = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
    let points = flat_coords_to_points(&flat);
    assert_eq!(points, vec![[0.0, 1.0], [2.0, 3.0], [4.0, 5.0]]);

    // Test separate coordinates conversion
    let x_coords = vec![0.0, 2.0, 4.0];
    let y_coords = vec![1.0, 3.0, 5.0];
    let points = separate_coords_to_points(&x_coords, &y_coords);
    assert_eq!(points, vec![[0.0, 1.0], [2.0, 3.0], [4.0, 5.0]]);
}

#[test]
#[should_panic(expected = "Coordinate vector must have an even number of elements")]
fn test_flat_coords_odd_length_panic() {
    use gup::integration::conversion::*;
    let odd_coords = vec![0.0, 1.0, 2.0]; // Odd number
    flat_coords_to_points(&odd_coords);
}

#[test]
#[should_panic(expected = "X and Y coordinate vectors must have the same length")]
fn test_separate_coords_length_mismatch_panic() {
    use gup::integration::conversion::*;
    let x_coords = vec![0.0, 1.0];
    let y_coords = vec![2.0]; // Different length
    separate_coords_to_points(&x_coords, &y_coords);
}

#[test]
fn test_chart_adapter() {
    use gup::integration::adapters::*;

    let chart = MockExternalChart::new_scatter(
        vec![(1.0, 2.0), (3.0, 4.0)],
        "Adapter Test",
        [0.0, 1.0, 0.0, 1.0],
    );

    let adapter = ChartAdapter::new(chart, |chart| {
        chart.data.iter().map(|&(x, y)| [x, y]).collect()
    });

    let wrapped = adapter.into_mixable();
    assert!(wrapped.is_valid());
    assert_eq!(wrapped.inner().chart.point_count(), 2);
}

#[tokio::test]
async fn test_timeseries_integration() {
    let mut context = RenderContext::new().await.unwrap();

    let timeseries = MockTimeSeriesData::new(
        "Test Series",
        vec![(1000, 10.0), (2000, 20.0), (3000, 15.0), (4000, 25.0)],
    );

    let points = timeseries.to_normalized_points();
    assert_eq!(points.len(), 4);

    // First point should be at x=0 (normalized)
    assert_eq!(points[0][0], 0.0);
    // Last point should be at x=1 (normalized)
    assert_eq!(points[3][0], 1.0);

    let wrapped = wrap_point_data(timeseries, |data| data.to_normalized_points());
    assert!(wrapped.is_valid());

    let mut wrapped = wrapped;
    let result = wrapped.render(&mut context);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_composition_with_external_types() {
    let mut context = RenderContext::new().await.unwrap();

    let chart1 = MockExternalChart::new_scatter(
        vec![(0.0, 0.0), (1.0, 1.0)],
        "Chart 1",
        [1.0, 0.0, 0.0, 1.0],
    );

    let chart2 = MockExternalChart::new_line(
        vec![(0.5, 0.5), (1.5, 1.5)],
        "Chart 2",
        [0.0, 1.0, 0.0, 1.0],
    );

    let wrapped1 = wrap_point_data(chart1, |c| c.data.iter().map(|&(x, y)| [x, y]).collect());
    let wrapped2 = wrap_point_data(chart2, |c| c.data.iter().map(|&(x, y)| [x, y]).collect());

    // Test composition
    let composed = wrapped1.mix(wrapped2);
    assert!(composed.is_valid());

    let mut composed = composed;
    let result = composed.render(&mut context);
    assert!(result.is_ok());

    // Test different composition modes
    let chart3 = MockExternalChart::new_scatter(vec![(2.0, 2.0)], "Chart 3", [0.0, 0.0, 1.0, 1.0]);
    let wrapped3 = wrap_point_data(chart3, |c| c.data.iter().map(|&(x, y)| [x, y]).collect());

    let overlay = composed.overlay(wrapped3);
    assert!(overlay.is_valid());
}

#[tokio::test]
async fn test_plugin_with_composition() {
    let mut context = RenderContext::new().await.unwrap();
    let mut registry = MixablePluginRegistry::new();

    // Register plugins
    let chart_plugin = MockChartPlugin;
    let timeseries_plugin = MockTimeSeriesPlugin::new();
    registry.register_plugin(Box::new(chart_plugin)).unwrap();
    registry
        .register_plugin(Box::new(timeseries_plugin))
        .unwrap();

    // Create data
    let chart = MockExternalChart::new_scatter(
        vec![(0.0, 0.0), (1.0, 1.0)],
        "Plugin Chart",
        [1.0, 0.0, 0.0, 1.0],
    );

    let timeseries = MockTimeSeriesData::new("Plugin Series", vec![(1000, 5.0), (2000, 10.0)]);

    // Create mixables via plugins
    let mixable1 = registry.create_mixable(chart).unwrap();
    let mixable2 = registry.create_mixable(timeseries).unwrap();

    assert!(mixable1.is_valid());
    assert!(mixable2.is_valid());

    // Test individual rendering (can't compose trait objects directly)
    let mut mixable1_mut = mixable1;
    let mut mixable2_mut = mixable2;

    let result1 = mixable1_mut.render(&mut context);
    let result2 = mixable2_mut.render(&mut context);
    assert!(result1.is_ok());
    assert!(result2.is_ok());
}

#[test]
fn test_builder_pattern_fluent_api() {
    let chart = MockExternalChart::new_scatter(
        vec![(0.0, 1.0), (2.0, 3.0)],
        "Builder Test",
        [1.0, 1.0, 0.0, 1.0],
    );

    let wrapped = ExternalVisualizationBuilder::new()
        .with_point_renderer()
        .with_point_extractor(|chart: &MockExternalChart| {
            chart.data.iter().map(|&(x, y)| [x, y]).collect()
        })(chart);

    assert!(wrapped.is_valid());
    assert!(wrapped.description().contains("ExternalPointVisualization"));
}

#[tokio::test]
async fn test_performance_characteristics() {
    let mut context = RenderContext::new().await.unwrap();

    // Test with larger datasets to ensure reasonable performance
    let large_dataset: Vec<(f32, f32)> = (0..1000).map(|i| (i as f32, (i as f32).sin())).collect();

    let start = std::time::Instant::now();

    let chart =
        MockExternalChart::new_scatter(large_dataset, "Performance Test", [1.0, 0.0, 0.0, 1.0]);

    let wrapped = wrap_point_data(chart, |chart| {
        chart.data.iter().map(|&(x, y)| [x, y]).collect()
    });

    assert!(wrapped.is_valid());

    let mut wrapped = wrapped;
    let result = wrapped.render(&mut context);
    assert!(result.is_ok());

    let elapsed = start.elapsed();

    // Performance should be reasonable (less than 100ms for 1000 points)
    assert!(
        elapsed.as_millis() < 100,
        "Integration took too long: {elapsed:?}"
    );
}

#[tokio::test]
async fn test_error_handling_integration() {
    let mut context = RenderContext::new().await.unwrap();

    // Test error handling in custom render functions
    let error_chart =
        MockExternalChart::new_scatter(vec![(0.0, 0.0)], "Error Test", [1.0, 0.0, 0.0, 1.0]);

    let wrapped = wrap_with_custom_render(error_chart, |_chart, _context| {
        Err(gup::GupError::render_error(
            "Intentional test error".to_string(),
        ))
    });

    assert!(wrapped.is_valid()); // The wrapper itself is valid

    let mut wrapped = wrapped;
    let result = wrapped.render(&mut context);
    assert!(result.is_err());

    if let Err(gup::GupError::RenderError { message }) = result {
        assert!(message.contains("Intentional test error"));
    } else {
        panic!("Expected RenderError");
    }
}

#[test]
fn test_integration_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    let registry = Arc::new(std::sync::Mutex::new(MixablePluginRegistry::new()));

    // Spawn multiple threads to register plugins concurrently
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let registry = registry.clone();
            thread::spawn(move || {
                // Create unique plugin names
                let plugin_name = format!("thread_plugin_{i}");

                #[derive(Debug)]
                struct ThreadPlugin {
                    name: String,
                }

                impl MixablePlugin for ThreadPlugin {
                    fn name(&self) -> &str {
                        &self.name
                    }

                    fn version(&self) -> &str {
                        "1.0.0"
                    }

                    fn can_handle(&self, _type_id: TypeId) -> bool {
                        false
                    }

                    fn create_mixable(
                        &self,
                        _object: Box<dyn Any + Send + Sync>,
                    ) -> Box<dyn Mixable<Output = ()>> {
                        panic!("Not implemented for thread test");
                    }

                    fn validate(&self) -> Result<(), String> {
                        Ok(())
                    }
                }

                let plugin = ThreadPlugin {
                    name: plugin_name.clone(),
                };
                let result = registry.lock().unwrap().register_plugin(Box::new(plugin));
                assert!(result.is_ok());

                plugin_name
            })
        })
        .collect();

    let plugin_names: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Verify all plugins were registered
    let registry = registry.lock().unwrap();
    assert_eq!(registry.list_plugins().len(), 4);

    for name in plugin_names {
        assert!(registry.has_plugin(&name));
    }
}

#[tokio::test]
async fn test_cross_platform_compatibility() {
    // Test that integration works on both native and WASM targets
    // This test focuses on ensuring no platform-specific dependencies

    let mut context = RenderContext::new().await.unwrap();

    let chart = MockExternalChart::new_scatter(
        vec![(0.0, 0.0), (1.0, 1.0)],
        "Cross Platform",
        [1.0, 0.0, 0.0, 1.0],
    );

    let wrapped = wrap_point_data(chart, |chart| {
        chart.data.iter().map(|&(x, y)| [x, y]).collect()
    });

    assert!(wrapped.is_valid());

    let mut wrapped = wrapped;
    let result = wrapped.render(&mut context);
    assert!(result.is_ok());

    // Test plugin system
    let mut registry = MixablePluginRegistry::new();
    let plugin = MockChartPlugin;
    assert!(registry.register_plugin(Box::new(plugin)).is_ok());

    let chart2 = MockExternalChart::new_line(
        vec![(2.0, 2.0), (3.0, 3.0)],
        "Cross Platform 2",
        [0.0, 1.0, 0.0, 1.0],
    );

    let mixable_from_plugin = registry.create_mixable(chart2);
    assert!(mixable_from_plugin.is_some());
    assert!(mixable_from_plugin.unwrap().is_valid());
}
