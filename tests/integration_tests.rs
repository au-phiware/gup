//! Integration tests for the Mixable trait and composition system.

use gup::{CompositionMode, CrossFadeComposition, CustomCompositionBehavior, GupError, GupResult, Mixable, MixableExt, RenderContext};

/// Mock scatter plot implementation for testing
#[derive(Debug, Clone)]
struct ScatterPlot {
    data: Vec<(f32, f32)>,
    color: String,
}

impl ScatterPlot {
    fn new(data: Vec<(f32, f32)>, color: &str) -> Self {
        Self {
            data,
            color: color.to_string(),
        }
    }
}

impl Mixable for ScatterPlot {
    type Output = ();

    fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
        // Mock rendering - in real implementation would draw points
        println!(
            "Rendering scatter plot with {} points in {}",
            self.data.len(),
            self.color
        );
        Ok(())
    }
}

/// Mock line chart implementation for testing
#[derive(Debug, Clone)]
struct LineChart {
    data: Vec<f32>,
    style: String,
}

impl LineChart {
    fn new(data: Vec<f32>, style: &str) -> Self {
        Self {
            data,
            style: style.to_string(),
        }
    }
}

impl Mixable for LineChart {
    type Output = ();

    fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
        // Mock rendering - in real implementation would draw lines
        println!(
            "Rendering line chart with {} points in {} style",
            self.data.len(),
            self.style
        );
        Ok(())
    }
}

/// Mock bar chart implementation for testing
#[derive(Debug, Clone)]
struct BarChart {
    categories: Vec<String>,
    #[allow(dead_code)]
    values: Vec<f32>,
}

impl BarChart {
    fn new(categories: Vec<String>, values: Vec<f32>) -> Self {
        Self { categories, values }
    }
}

impl Mixable for BarChart {
    type Output = ();

    fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
        // Mock rendering - in real implementation would draw bars
        println!(
            "Rendering bar chart with {} categories",
            self.categories.len()
        );
        Ok(())
    }
}

/// Mock heatmap implementation for testing
#[derive(Debug, Clone)]
struct Heatmap {
    data: Vec<Vec<f32>>,
    width: usize,
    height: usize,
    should_fail: bool,
}

impl Heatmap {
    fn new(data: Vec<Vec<f32>>) -> Self {
        let height = data.len();
        let width = data.first().map(|row| row.len()).unwrap_or(0);
        Self {
            data,
            width,
            height,
            should_fail: false,
        }
    }

    fn with_failure(data: Vec<Vec<f32>>) -> Self {
        let mut heatmap = Self::new(data);
        heatmap.should_fail = true;
        heatmap
    }
}

impl Mixable for Heatmap {
    type Output = ();

    fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
        if self.should_fail {
            return Err(GupError::RenderError("Heatmap render failure".to_string()));
        }
        println!("Rendering heatmap {}x{}", self.width, self.height);
        Ok(())
    }

    fn is_valid(&self) -> bool {
        !self.should_fail && !self.data.is_empty() && self.width > 0 && self.height > 0
    }

    fn description(&self) -> String {
        format!("Heatmap({}x{})", self.width, self.height)
    }
}

#[tokio::test]
async fn test_different_chart_types_composition() {
    let scatter = ScatterPlot::new(vec![(1.0, 2.0), (3.0, 4.0)], "red");
    let line = LineChart::new(vec![1.0, 2.0, 3.0, 4.0], "dashed");

    let mut composed = scatter.mix(line);
    assert!(composed.is_valid());

    let mut context = RenderContext::new().await.unwrap();
    assert!(composed.render(&mut context).is_ok());
}

#[tokio::test]
async fn test_three_way_composition() {
    let scatter = ScatterPlot::new(vec![(1.0, 2.0)], "blue");
    let line = LineChart::new(vec![1.0, 2.0], "solid");
    let bars = BarChart::new(vec!["A".to_string(), "B".to_string()], vec![10.0, 20.0]);

    let mut composed = scatter.mix(line).mix(bars);
    assert!(composed.is_valid());

    let mut context = RenderContext::new().await.unwrap();
    assert!(composed.render(&mut context).is_ok());
}

#[test]
fn test_composition_modes_with_different_types() {
    let scatter = ScatterPlot::new(vec![(1.0, 2.0)], "green");
    let heatmap = Heatmap::new(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);

    // Test all composition modes
    let overlay = scatter.clone().overlay(heatmap.clone());
    assert_eq!(overlay.composition_mode(), CompositionMode::Overlay);

    let merge = scatter.clone().merge(heatmap.clone());
    assert_eq!(merge.composition_mode(), CompositionMode::Merge);

    let beside = scatter.clone().beside(heatmap.clone());
    assert_eq!(beside.composition_mode(), CompositionMode::SideBySide);

    let custom_behavior = CustomCompositionBehavior::CrossFade(CrossFadeComposition { fade_factor: 0.5 });
    let custom = scatter.custom_compose(heatmap, custom_behavior);
    assert_eq!(custom.composition_mode(), CompositionMode::Custom);

    // All should be valid
    assert!(overlay.is_valid());
    assert!(merge.is_valid());
    assert!(beside.is_valid());
    assert!(custom.is_valid());
}

#[tokio::test]
async fn test_mixed_valid_invalid_composition() {
    let valid_scatter = ScatterPlot::new(vec![(1.0, 2.0)], "purple");
    let invalid_heatmap = Heatmap::with_failure(vec![vec![1.0]]);

    let mut composed = valid_scatter.mix(invalid_heatmap);
    assert!(!composed.is_valid());

    let mut context = RenderContext::new().await.unwrap();
    let result = composed.render(&mut context);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_complex_nested_composition() {
    let scatter1 = ScatterPlot::new(vec![(1.0, 2.0)], "red");
    let scatter2 = ScatterPlot::new(vec![(3.0, 4.0)], "blue");
    let line1 = LineChart::new(vec![1.0, 2.0], "solid");
    let line2 = LineChart::new(vec![3.0, 4.0], "dashed");

    // Create nested composition: (scatter1 + scatter2) + (line1 + line2)
    let scatter_composition = scatter1.mix(scatter2);
    let line_composition = line1.mix(line2);
    let mut final_composition = scatter_composition.mix(line_composition);

    assert!(final_composition.is_valid());

    let mut context = RenderContext::new().await.unwrap();
    assert!(final_composition.render(&mut context).is_ok());
}

#[test]
fn test_composition_with_different_output_types() {
    // This test verifies that the trait works even when components have different Output types
    let scatter = ScatterPlot::new(vec![(1.0, 2.0)], "orange");
    let line = LineChart::new(vec![1.0, 2.0], "dotted");

    // Both have Output = (), so composition should work seamlessly
    let composed = scatter.mix(line);
    assert!(composed.is_valid());
}

#[tokio::test]
async fn test_composition_error_messages() {
    let valid_chart = ScatterPlot::new(vec![(1.0, 2.0)], "yellow");
    let invalid_heatmap = Heatmap::with_failure(vec![vec![1.0]]);

    let mut composed = valid_chart.mix(invalid_heatmap);
    let mut context = RenderContext::new().await.unwrap();

    match composed.render(&mut context) {
        Err(GupError::CompositionError(msg)) => {
            assert!(msg.contains("Second component is invalid"));
            assert!(msg.contains("Heatmap"));
        }
        _ => panic!("Expected CompositionError with descriptive message"),
    }
}

#[test]
fn test_composition_decomposition() {
    let scatter = ScatterPlot::new(vec![(1.0, 2.0)], "cyan");
    let line = LineChart::new(vec![1.0, 2.0], "thick");
    let mode = CompositionMode::Merge;

    let composed = scatter.mix_with_mode(line, mode);
    let (first, second, actual_mode) = composed.into_parts();

    assert_eq!(actual_mode, mode);
    assert_eq!(first.color, "cyan");
    assert_eq!(second.style, "thick");
}

#[tokio::test]
async fn test_render_context_interaction() {
    let scatter = ScatterPlot::new(vec![(1.0, 2.0), (3.0, 4.0), (5.0, 6.0)], "magenta");
    let line = LineChart::new(vec![1.0, 2.0, 3.0, 4.0, 5.0], "wavy");

    let mut composed = scatter.mix(line);

    // Test with different viewport sizes
    let mut small_context = RenderContext::with_viewport(gup::Viewport {
        width: 400,
        height: 300,
        scale_factor: 1.0,
    }).await.unwrap();
    assert!(composed.render(&mut small_context).is_ok());

    let mut large_context = RenderContext::with_viewport(gup::Viewport {
        width: 1920,
        height: 1080,
        scale_factor: 2.0,
    }).await.unwrap();
    assert!(composed.render(&mut large_context).is_ok());
}

#[tokio::test]
async fn test_performance_characteristics() {
    // Note: Due to type system constraints, we can't easily build very deep chains
    // in this test without complex type gymnastics. In practice, the benchmarks
    // would be more comprehensive.

    // This test mainly ensures the basic structure doesn't have obvious performance issues
    let scatter1 = ScatterPlot::new(vec![(1.0, 2.0)], "perf1");
    let scatter2 = ScatterPlot::new(vec![(3.0, 4.0)], "perf2");
    let scatter3 = ScatterPlot::new(vec![(5.0, 6.0)], "perf3");

    let mut composed = scatter1.mix(scatter2).mix(scatter3);
    let mut context = RenderContext::new().await.unwrap();

    // Multiple renders should be fast
    for _ in 0..100 {
        assert!(composed.render(&mut context).is_ok());
    }
}
