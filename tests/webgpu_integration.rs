// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//

use gup::{GpuScatterPlot, Mixable, RenderContext};

#[tokio::test]
async fn test_render_context_initialization() {
    let context = RenderContext::new().await;
    assert!(
        context.is_ok(),
        "RenderContext should initialize successfully"
    );

    let context = context.unwrap();
    assert!(context.device().limits().max_texture_dimension_2d > 0);
}

#[tokio::test]
async fn test_render_context_with_viewport() {
    let viewport = gup::Viewport {
        width: 1024,
        height: 768,
        scale_factor: 2.0,
    };
    let context = RenderContext::with_viewport(viewport).await;
    assert!(context.is_ok());

    let context = context.unwrap();
    assert_eq!(context.viewport().width, 1024);
    assert_eq!(context.viewport().height, 768);
    assert_eq!(context.viewport().scale_factor, 2.0);
}

#[tokio::test]
async fn test_basic_gpu_rendering() {
    let mut context = RenderContext::new().await.unwrap();

    let mut scatter_plot = GpuScatterPlot::new(
        vec![(0.0, 0.0), (0.5, 0.5), (-0.5, -0.5)],
        [1.0, 0.0, 0.0, 1.0],
    );

    let result = scatter_plot.render(&mut context);
    assert!(result.is_ok(), "GPU rendering should succeed");
}

#[tokio::test]
async fn test_composed_gpu_rendering() {
    let mut context = RenderContext::new().await.unwrap();

    let plot1 = GpuScatterPlot::new(vec![(0.0, 0.0)], [1.0, 0.0, 0.0, 1.0]);
    let plot2 = GpuScatterPlot::new(vec![(0.5, 0.5)], [0.0, 1.0, 0.0, 1.0]);

    let mut composed = plot1.mix(plot2);
    let result = composed.render(&mut context);

    assert!(result.is_ok(), "Composed GPU rendering should succeed");
}

#[tokio::test]
async fn test_scatter_plot_validation() {
    let empty_plot = GpuScatterPlot::new(vec![], [1.0, 0.0, 0.0, 1.0]);
    assert!(
        !empty_plot.is_valid(),
        "Empty scatter plot should be invalid"
    );

    let valid_plot = GpuScatterPlot::new(vec![(0.0, 0.0)], [1.0, 0.0, 0.0, 1.0]);
    assert!(
        valid_plot.is_valid(),
        "Non-empty scatter plot should be valid"
    );
}

#[tokio::test]
async fn test_viewport_update() {
    let mut context = RenderContext::new().await.unwrap();

    let new_viewport = gup::Viewport {
        width: 1920,
        height: 1080,
        scale_factor: 1.5,
    };

    let result = context.set_viewport(new_viewport);
    assert!(result.is_ok(), "Viewport update should succeed");

    let updated_viewport = context.viewport();
    assert_eq!(updated_viewport.width, 1920);
    assert_eq!(updated_viewport.height, 1080);
    assert_eq!(updated_viewport.scale_factor, 1.5);
}

#[tokio::test]
async fn test_multiple_renders() {
    let mut context = RenderContext::new().await.unwrap();

    let mut scatter_plot = GpuScatterPlot::new(
        vec![(0.0, 0.0), (0.1, 0.1), (0.2, 0.2)],
        [0.0, 0.0, 1.0, 1.0],
    );

    // Render multiple times to test resource reuse
    for _ in 0..3 {
        let result = scatter_plot.render(&mut context);
        assert!(result.is_ok(), "Multiple renders should succeed");
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    async fn test_webgpu_in_browser() {
        let context = RenderContext::new().await;
        assert!(context.is_ok(), "WebGPU should initialize in browser");
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native_tests {
    use super::*;

    #[tokio::test]
    async fn test_native_webgpu() {
        let context = RenderContext::new().await;
        assert!(
            context.is_ok(),
            "WebGPU should initialize on native platforms"
        );
    }
}
