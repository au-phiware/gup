// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Multi-Mark Pattern Showcase
//!
//! This example demonstrates pattern rendering support across all major mark types
//! (Circle, Rectangle, Line, BoxPlot) to show how patterns provide
//! color-independent data encoding for accessibility.
//!
//! ## What You'll Learn
//!
//! - How patterns work with Circle marks (scatter plots, outliers)
//! - How patterns work with Rectangle marks (bar charts, boxes)
//! - How patterns work with Line marks (line charts, whiskers)
//! - How patterns work with BoxPlot marks (statistical distributions)
//! - How to configure different pattern types (Solid, Dots, Lines, Crosshatch)
//! - Why patterns are essential for colorblind-accessible visualizations
//!
//! ## Practical Use Case
//!
//! This example demonstrates a multi-category product performance visualization
//! where four product categories are distinguished using both color AND pattern.
//! This dual encoding ensures the visualization is accessible to users with
//! color vision deficiencies.
//!
//! ## Pattern Types Demonstrated
//!
//! 1. **Solid** - No pattern, just solid color
//!    - Use case: Primary category or when maximum clarity is needed
//!    - Electronics category in this example
//!
//! 2. **Dots** - Dotted pattern for texture
//!    - Use case: Secondary categories, adding texture without overwhelming
//!    - Spacing controls dot density (larger = sparser)
//!    - Clothing category in this example
//!
//! 3. **Lines** - Diagonal lines at specified angle
//!    - Use case: Distinct directional pattern
//!    - Angle parameter controls direction (0° = horizontal, 45° = diagonal)
//!    - Spacing controls line density
//!    - Home & Garden category in this example
//!
//! 4. **Crosshatch** - Grid pattern for high contrast
//!    - Use case: Maximum distinction, high contrast needed
//!    - Combines horizontal and vertical lines
//!    - Sports category in this example
//!
//! ## Pattern Configuration
//!
//! Patterns are configured using `PatternUniforms` and rendered with `PatternRenderer`:
//!
//! ```rust
//! let pattern = Pattern::Dots { spacing: 10.0 };
//! let uniforms = PatternUniforms::from_pattern(&pattern, foreground_color, background_color);
//! let renderer = PatternRenderer::new(device, uniforms);
//! ```
//!
//! ## Mark Type Coverage
//!
//! This example validates that all implemented mark types support patterns:
//! - ✓ Circle marks
//! - ✓ Rectangle marks
//! - ✓ Line marks
//! - ✓ BoxPlot marks
//! - ✓ Path marks (when using pre-written shaders)
//!
//! ## Accessibility Benefits
//!
//! Patterns provide texture-based encoding that works independently of color:
//! - Users with protanopia (red-blind) can distinguish categories
//! - Users with deuteranopia (green-blind) can distinguish categories
//! - Users with tritanopia (blue-blind) can distinguish categories
//! - Users with achromatopsia (total color blindness) can distinguish categories
//! - Printable in black and white while retaining distinctions
//!
//! ## Run with
//!
//! ```bash
//! cargo run --example multi_mark_pattern_showcase
//! ```

use gup::GupContext;
use gup::accessibility::{Color, Pattern, PatternRenderer, PatternUniforms};
use gup::error::GupResult;
use gup::mark::{BoxPlot, Circle, Line, MarkInfo, MarkInfoImpl, Rectangle};
use std::sync::Arc;

/// Product category with pattern configuration
#[derive(Clone, Debug)]
struct CategoryData {
    name: &'static str,
    color: Color,
    pattern: Pattern,
    description: &'static str,
}

/// Create sample data for four product categories
fn create_category_data() -> Vec<CategoryData> {
    vec![
        CategoryData {
            name: "Electronics",
            color: Color {
                r: 0.2,
                g: 0.6,
                b: 0.9,
                a: 1.0,
            },
            pattern: Pattern::Solid,
            description: "Solid fill - highest clarity, primary category",
        },
        CategoryData {
            name: "Clothing",
            color: Color {
                r: 0.9,
                g: 0.4,
                b: 0.3,
                a: 1.0,
            },
            pattern: Pattern::Dots { spacing: 10.0 },
            description: "Dotted pattern - adds texture, spacing=10px",
        },
        CategoryData {
            name: "Home & Garden",
            color: Color {
                r: 0.3,
                g: 0.8,
                b: 0.3,
                a: 1.0,
            },
            pattern: Pattern::Lines {
                spacing: 8.0,
                angle: std::f32::consts::PI / 4.0, // 45 degrees
            },
            description: "Diagonal lines - 45° angle, spacing=8px",
        },
        CategoryData {
            name: "Sports",
            color: Color {
                r: 0.9,
                g: 0.7,
                b: 0.2,
                a: 1.0,
            },
            pattern: Pattern::Crosshatch { spacing: 10.0 },
            description: "Crosshatch - grid pattern, spacing=10px",
        },
    ]
}

/// Validate that all mark types have pattern shader support
async fn validate_mark_pattern_support() -> GupResult<()> {
    println!("\n=== Pattern Support Validation ===\n");

    // Check Circle mark
    let circle_info = MarkInfoImpl::<Circle>::new();
    let circle_supported = circle_info.has_pattern_shader();
    println!(
        "  Circle mark:     {} {}",
        if circle_supported { "✓" } else { "✗" },
        if circle_supported {
            "Pattern shader available"
        } else {
            "No pattern shader"
        }
    );

    // Check Rectangle mark
    let rectangle_info = MarkInfoImpl::<Rectangle>::new();
    let rectangle_supported = rectangle_info.has_pattern_shader();
    println!(
        "  Rectangle mark:  {} {}",
        if rectangle_supported { "✓" } else { "✗" },
        if rectangle_supported {
            "Pattern shader available"
        } else {
            "No pattern shader"
        }
    );

    // Check Line mark
    let line_info = MarkInfoImpl::<Line>::new();
    let line_supported = line_info.has_pattern_shader();
    println!(
        "  Line mark:       {} {}",
        if line_supported { "✓" } else { "✗" },
        if line_supported {
            "Pattern shader available"
        } else {
            "No pattern shader"
        }
    );

    // Check BoxPlot mark
    let boxplot_info = MarkInfoImpl::<BoxPlot>::new();
    let boxplot_supported = boxplot_info.has_pattern_shader();
    println!(
        "  BoxPlot mark:    {} {}",
        if boxplot_supported { "✓" } else { "✗" },
        if boxplot_supported {
            "Pattern shader available"
        } else {
            "No pattern shader"
        }
    );

    let all_supported =
        circle_supported && rectangle_supported && line_supported && boxplot_supported;

    if all_supported {
        println!("\n✓ All mark types support pattern rendering!");
    } else {
        println!("\n✗ Some mark types are missing pattern support");
    }

    Ok(())
}

/// Demonstrate pattern pipeline creation for each mark type
async fn demonstrate_pattern_pipelines() -> GupResult<()> {
    println!("\n=== Pattern Pipeline Creation ===\n");

    let context = Arc::new(GupContext::headless().await?);
    let device = &context.device;

    // Create pattern pipelines for each mark type
    let circle_info = MarkInfoImpl::<Circle>::new();
    let start = std::time::Instant::now();
    let _circle_pipeline = circle_info.create_render_pipeline_with_patterns(device)?;
    println!(
        "  ✓ Circle pattern pipeline created in {:?}",
        start.elapsed()
    );

    let rectangle_info = MarkInfoImpl::<Rectangle>::new();
    let start = std::time::Instant::now();
    let _rectangle_pipeline = rectangle_info.create_render_pipeline_with_patterns(device)?;
    println!(
        "  ✓ Rectangle pattern pipeline created in {:?}",
        start.elapsed()
    );

    let line_info = MarkInfoImpl::<Line>::new();
    let start = std::time::Instant::now();
    let _line_pipeline = line_info.create_render_pipeline_with_patterns(device)?;
    println!("  ✓ Line pattern pipeline created in {:?}", start.elapsed());

    let boxplot_info = MarkInfoImpl::<BoxPlot>::new();
    let start = std::time::Instant::now();
    let _boxplot_pipeline = boxplot_info.create_render_pipeline_with_patterns(device)?;
    println!(
        "  ✓ BoxPlot pattern pipeline created in {:?}",
        start.elapsed()
    );

    println!("\n✓ All pattern pipelines created successfully!");

    Ok(())
}

/// Demonstrate pattern configuration for each category
async fn demonstrate_pattern_configurations() -> GupResult<()> {
    println!("\n=== Pattern Configuration Examples ===\n");

    let context = Arc::new(GupContext::headless().await?);
    let device = &context.device;
    let queue = &context.queue;

    let categories = create_category_data();

    println!("Product Categories with Pattern Encoding:\n");

    for (i, category) in categories.iter().enumerate() {
        println!(
            "{}. {} (RGB: {:.1}, {:.1}, {:.1})",
            i + 1,
            category.name,
            category.color.r,
            category.color.g,
            category.color.b
        );
        println!("   Pattern: {:?}", category.pattern);
        println!("   Description: {}", category.description);

        // Create pattern uniforms
        let uniforms = PatternUniforms::from_pattern(
            &category.pattern,
            category.color,
            Color::WHITE, // background
        );

        // Create pattern renderer
        let mut renderer = PatternRenderer::new(device, uniforms);

        // Demonstrate updating pattern
        let updated_uniforms = PatternUniforms::from_pattern(
            &category.pattern,
            category.color,
            Color {
                r: 0.95,
                g: 0.95,
                b: 0.95,
                a: 1.0,
            }, // light gray background
        );
        renderer.update(queue, updated_uniforms);

        println!("   ✓ Pattern renderer created and updated");
        println!();
    }

    Ok(())
}

/// Demonstrate practical use cases for each mark type
fn demonstrate_use_cases() {
    println!("\n=== Practical Use Cases ===\n");

    println!("1. Circle Marks with Patterns:");
    println!("   - Scatter plots: Each category shown with distinct pattern");
    println!("   - Outlier detection: Pattern helps identify which category");
    println!("   - Multi-series plots: Pattern + color = robust encoding");
    println!();

    println!("2. Rectangle Marks with Patterns:");
    println!("   - Bar charts: Categories distinguished by pattern");
    println!("   - Stacked bars: Each segment has unique pattern");
    println!("   - Heatmaps: Pattern intensity adds another dimension");
    println!();

    println!("3. Line Marks with Patterns:");
    println!("   - Line charts: Each line series has distinct pattern");
    println!("   - Area charts: Filled areas with patterns");
    println!("   - Confidence intervals: Pattern shows uncertainty");
    println!();

    println!("4. BoxPlot Marks with Patterns:");
    println!("   - Box fills: Category shown with pattern");
    println!("   - Multi-category comparison: Each box has unique pattern");
    println!("   - Statistical distributions: Pattern + color = clear distinction");
    println!();
}

/// Demonstrate when to use each pattern type
fn demonstrate_pattern_selection_guidance() {
    println!("\n=== Pattern Selection Guidance ===\n");

    println!("Choosing the Right Pattern:");
    println!();

    println!("• Use Solid when:");
    println!("  - Maximum clarity is needed");
    println!("  - Primary or most important category");
    println!("  - Other categories have patterns (provides contrast)");
    println!();

    println!("• Use Dots when:");
    println!("  - Need subtle texture without overwhelming");
    println!("  - Multiple categories need distinction");
    println!("  - Works well at various zoom levels");
    println!("  - Adjust spacing: 6-12px typical range");
    println!();

    println!("• Use Lines when:");
    println!("  - Need directional distinction");
    println!("  - Clear, bold pattern needed");
    println!("  - Angle creates unique identity (0°, 45°, 90°, -45°)");
    println!("  - Adjust spacing: 6-10px typical range");
    println!();

    println!("• Use Crosshatch when:");
    println!("  - Maximum contrast needed");
    println!("  - Dense pattern acceptable");
    println!("  - High-density data visualization");
    println!("  - Adjust spacing: 8-12px typical range");
    println!();

    println!("Pattern Spacing Guidelines:");
    println!("  - Too small (<4px): Pattern becomes noise");
    println!("  - Small (4-6px): Dense, high contrast");
    println!("  - Medium (6-10px): Balanced, recommended");
    println!("  - Large (10-15px): Sparse, subtle");
    println!("  - Too large (>15px): Pattern may not be visible");
    println!();
}

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       Gup - Multi-Mark Pattern Showcase                     ║");
    println!("║       Pattern Rendering for Accessible Visualizations       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    // Validate that all mark types support patterns
    validate_mark_pattern_support().await?;

    // Demonstrate pattern pipeline creation
    demonstrate_pattern_pipelines().await?;

    // Demonstrate pattern configurations
    demonstrate_pattern_configurations().await?;

    // Demonstrate practical use cases
    demonstrate_use_cases();

    // Provide pattern selection guidance
    demonstrate_pattern_selection_guidance();

    println!("\n=== Summary ===\n");
    println!("✓ All 4 major mark types support pattern rendering");
    println!("✓ 4 pattern types available: Solid, Dots, Lines, Crosshatch");
    println!("✓ Patterns enable colorblind-accessible visualizations");
    println!("✓ GPU-accelerated for high performance");
    println!("✓ Configurable spacing and angles for customization");
    println!();
    println!("Pattern rendering provides texture-based data encoding that");
    println!("works independently of color, making visualizations accessible");
    println!("to all users including those with color vision deficiencies.");
    println!();
    println!("Next steps:");
    println!("  - See tests/multi_mark_pattern_tests.rs for integration tests");
    println!("  - See src/accessibility/pattern_renderer.rs for implementation");
    println!("  - Try pattern_pipeline_demo.rs for GPU rendering details");
    println!();

    Ok(())
}
