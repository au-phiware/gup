// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive showcase of composition error recovery and diagnostic capabilities.
//!
//! This example demonstrates:
//! - Error recovery strategies (Skip, Retry, Fallback)
//! - Performance monitoring and bottleneck detection
//! - Component health tracking
//! - Visual debugging tools
//! - Interactive error diagnostics

#![allow(clippy::field_reassign_with_default)]

use gup::mixable::composition_recovery::{
    CompositionFallbackType, ErrorHandlingPolicy, RecoveryStrategy, RobustCompositionExecutor,
    debug,
};
use gup::{GupError, GupResult, Mixable, RenderContext};
use std::time::Duration;

/// Demonstrates different types of components with various failure modes
mod demo_components {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A reliable component that always succeeds
    #[derive(Debug)]
    pub struct ReliableComponent {
        name: String,
    }

    impl ReliableComponent {
        pub fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    impl Mixable for ReliableComponent {
        type Output = ();

        fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
            println!("✅ {} rendered successfully", self.name);
            Ok(())
        }

        fn description(&self) -> String {
            format!("ReliableComponent({})", self.name)
        }
    }

    /// A component that occasionally fails due to memory issues
    #[derive(Debug)]
    pub struct MemoryHungryComponent {
        name: String,
        failure_rate: f32,
        call_count: Arc<Mutex<u32>>,
    }

    impl MemoryHungryComponent {
        pub fn new(name: &str, failure_rate: f32) -> Self {
            Self {
                name: name.to_string(),
                failure_rate,
                call_count: Arc::new(Mutex::new(0)),
            }
        }
    }

    impl Mixable for MemoryHungryComponent {
        type Output = ();

        fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;

            // Simulate occasional memory exhaustion
            if (*count as f32 * self.failure_rate) % 1.0 > 0.7 {
                println!("🔴 {} failed with memory exhaustion", self.name);
                Err(GupError::gpu_memory_exhausted(2048, 1024))
            } else {
                println!("✅ {} rendered successfully", self.name);
                Ok(())
            }
        }

        fn description(&self) -> String {
            format!(
                "MemoryHungryComponent({}, {:.1}% failure)",
                self.name,
                self.failure_rate * 100.0
            )
        }

        fn is_valid(&self) -> bool {
            self.failure_rate < 0.5 // Consider components with >50% failure rate invalid
        }
    }

    /// A component with intermittent shader compilation issues
    #[derive(Debug)]
    pub struct ShaderComponent {
        name: String,
        complexity: u32,
        attempts: Arc<Mutex<u32>>,
    }

    impl ShaderComponent {
        pub fn new(name: &str, complexity: u32) -> Self {
            Self {
                name: name.to_string(),
                complexity,
                attempts: Arc::new(Mutex::new(0)),
            }
        }
    }

    impl Mixable for ShaderComponent {
        type Output = ();

        fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
            let mut attempts = self.attempts.lock().unwrap();
            *attempts += 1;

            // Fail first few attempts for complex shaders
            if self.complexity > 5 && *attempts <= 2 {
                println!(
                    "🔴 {} shader compilation failed (attempt {})",
                    self.name, *attempts
                );
                Err(GupError::shader_compilation_failed(
                    "fragment",
                    "Complex shader not supported",
                ))
            } else {
                println!("✅ {} shader compiled successfully", self.name);
                Ok(())
            }
        }

        fn description(&self) -> String {
            format!(
                "ShaderComponent({}, complexity: {})",
                self.name, self.complexity
            )
        }

        fn is_valid(&self) -> bool {
            self.complexity <= 10
        }
    }

    /// A component that performs poorly under load
    #[derive(Debug)]
    pub struct PerformanceSensitiveComponent {
        name: String,
        workload: u32,
    }

    impl PerformanceSensitiveComponent {
        pub fn new(name: &str, workload: u32) -> Self {
            Self {
                name: name.to_string(),
                workload,
            }
        }
    }

    impl Mixable for PerformanceSensitiveComponent {
        type Output = ();

        fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
            // Simulate processing time based on workload
            let processing_time = Duration::from_millis(self.workload as u64 * 10);
            std::thread::sleep(processing_time);

            if processing_time > Duration::from_millis(100) {
                println!(
                    "🟡 {} exceeded performance target ({:?})",
                    self.name, processing_time
                );
                Err(GupError::performance_target_missed(
                    16.67,
                    processing_time.as_millis() as f64,
                ))
            } else {
                println!("✅ {} rendered within performance budget", self.name);
                Ok(())
            }
        }

        fn description(&self) -> String {
            format!(
                "PerformanceSensitiveComponent({}, workload: {})",
                self.name, self.workload
            )
        }

        fn is_valid(&self) -> bool {
            self.workload <= 15
        }
    }
}

use demo_components::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Composition Error Recovery Showcase\n");

    let mut context = RenderContext::new().await?;

    // Scenario 1: Basic Error Recovery with Skip Strategy
    println!("📋 Scenario 1: Skip Strategy Recovery");
    demonstrate_skip_strategy(&mut context).await?;

    // Scenario 2: Retry Strategy for Transient Failures
    println!("\n📋 Scenario 2: Retry Strategy Recovery");
    demonstrate_retry_strategy(&mut context).await?;

    // Scenario 3: Fallback Rendering
    println!("\n📋 Scenario 3: Fallback Strategy Recovery");
    demonstrate_fallback_strategy(&mut context).await?;

    // Scenario 4: Complex Composition with Mixed Strategies
    println!("\n📋 Scenario 4: Complex Mixed Strategy Recovery");
    demonstrate_mixed_strategy_composition(&mut context).await?;

    // Scenario 5: Performance Monitoring and Bottleneck Detection
    println!("\n📋 Scenario 5: Performance Monitoring");
    demonstrate_performance_monitoring(&mut context).await?;

    // Scenario 6: Health Tracking and Diagnostics
    println!("\n📋 Scenario 6: Health Tracking");
    demonstrate_health_tracking(&mut context).await?;

    // Scenario 7: Visual Debugging Tools
    println!("\n📋 Scenario 7: Visual Debugging Tools");
    demonstrate_visual_debugging();

    // Scenario 8: Interactive Debugging Session
    println!("\n📋 Scenario 8: Interactive Debugging Session");
    demonstrate_debug_session();

    println!("\n🎉 All scenarios completed successfully!");
    println!("This demonstrates how the composition error recovery system");
    println!("provides robust, reliable visualization rendering even when");
    println!("individual components fail or perform poorly.");

    Ok(())
}

async fn demonstrate_skip_strategy(context: &mut RenderContext) -> GupResult<()> {
    println!("Creating a composition with reliable and unreliable components...");

    let reliable_chart = ReliableComponent::new("LineChart");
    let problematic_overlay = MemoryHungryComponent::new("DataOverlay", 0.8); // 80% failure rate
    let mut composition = reliable_chart.mix(problematic_overlay);

    let mut policy = ErrorHandlingPolicy::default();
    policy.default_recovery = RecoveryStrategy::Skip;

    println!("Executing with Skip strategy...");
    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor.execute_robust(&mut composition, context).await;

    println!(
        "✅ Result: {}",
        if result.success { "SUCCESS" } else { "FAILURE" }
    );
    println!("📊 Errors recorded: {}", result.errors.len());
    println!("⏱️  Execution time: {:?}", result.execution_time);
    println!(
        "🏥 Overall health: {:.1}%",
        result.health_status.overall_health * 100.0
    );

    Ok(())
}

async fn demonstrate_retry_strategy(context: &mut RenderContext) -> GupResult<()> {
    println!("Creating a shader component that fails initially but recovers...");

    let mut complex_shader = ShaderComponent::new("ComplexFragmentShader", 8);

    let mut policy = ErrorHandlingPolicy::default();
    policy.default_recovery = RecoveryStrategy::Retry {
        max_attempts: 3,
        backoff: Duration::from_millis(50),
    };

    println!("Executing with Retry strategy (max 3 attempts)...");
    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor.execute_robust(&mut complex_shader, context).await;

    println!(
        "✅ Result: {}",
        if result.success { "SUCCESS" } else { "FAILURE" }
    );
    println!("📊 Errors recorded: {}", result.errors.len());
    println!("⏱️  Execution time: {:?}", result.execution_time);

    Ok(())
}

async fn demonstrate_fallback_strategy(context: &mut RenderContext) -> GupResult<()> {
    println!("Creating a component that requires fallback rendering...");

    let mut performance_component = PerformanceSensitiveComponent::new("HighDetailChart", 20);

    let mut policy = ErrorHandlingPolicy::default();
    policy.default_recovery = RecoveryStrategy::Fallback(CompositionFallbackType::Placeholder(
        "High detail rendering unavailable".to_string(),
    ));

    println!("Executing with Fallback strategy...");
    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor
        .execute_robust(&mut performance_component, context)
        .await;

    println!(
        "✅ Result: {}",
        if result.success { "SUCCESS" } else { "FAILURE" }
    );
    println!("📊 Errors recorded: {}", result.errors.len());
    println!("⏱️  Execution time: {:?}", result.execution_time);

    Ok(())
}

async fn demonstrate_mixed_strategy_composition(context: &mut RenderContext) -> GupResult<()> {
    println!("Creating a complex composition with component-specific strategies...");

    let background = ReliableComponent::new("Background");
    let data_layer = MemoryHungryComponent::new("DataLayer", 0.3);
    let shader_overlay = ShaderComponent::new("EffectOverlay", 7);
    let performance_chart = PerformanceSensitiveComponent::new("DetailChart", 12);

    let mut complex_viz = background
        .mix(data_layer)
        .mix(shader_overlay.mix(performance_chart));

    let mut policy = ErrorHandlingPolicy::default();

    // Component-specific strategies
    policy
        .component_strategies
        .insert("MemoryHungryComponent".to_string(), RecoveryStrategy::Skip);
    policy.component_strategies.insert(
        "ShaderComponent".to_string(),
        RecoveryStrategy::Retry {
            max_attempts: 2,
            backoff: Duration::from_millis(25),
        },
    );
    policy.component_strategies.insert(
        "PerformanceSensitiveComponent".to_string(),
        RecoveryStrategy::Fallback(CompositionFallbackType::SimpleGeometry),
    );

    println!("Executing complex composition with mixed strategies...");
    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor.execute_robust(&mut complex_viz, context).await;

    println!(
        "✅ Result: {}",
        if result.success { "SUCCESS" } else { "FAILURE" }
    );
    println!("📊 Errors recorded: {}", result.errors.len());
    println!("⏱️  Execution time: {:?}", result.execution_time);
    println!(
        "🏥 Overall health: {:.1}%",
        result.health_status.overall_health * 100.0
    );

    // Show performance bottlenecks
    if !result.performance_metrics.bottlenecks.is_empty() {
        println!("\n🐌 Performance bottlenecks detected:");
        for bottleneck in &result.performance_metrics.bottlenecks {
            println!(
                "  • Component {}: {:.1}% of total time",
                bottleneck.component_id, bottleneck.percentage_of_total
            );
            for recommendation in &bottleneck.recommendations {
                println!("    💡 {recommendation}");
            }
        }
    }

    Ok(())
}

async fn demonstrate_performance_monitoring(context: &mut RenderContext) -> GupResult<()> {
    println!("Setting up performance monitoring with profiler...");

    let mut profiler = debug::CompositionProfiler::new();

    // Simulate different rendering operations
    profiler.start_timing("background_render");
    let background = ReliableComponent::new("Background");
    let mut bg_copy = background;
    let _ = bg_copy.render(context);
    profiler.end_timing(Duration::from_millis(5));

    profiler.start_timing("data_processing");
    let data_component = PerformanceSensitiveComponent::new("DataProcessor", 8);
    let mut data_copy = data_component;
    let _ = data_copy.render(context);
    profiler.end_timing(Duration::from_millis(80));

    profiler.start_timing("effect_rendering");
    let effect_component = ShaderComponent::new("EffectShader", 3);
    let mut effect_copy = effect_component;
    let _ = effect_copy.render(context);
    profiler.end_timing(Duration::from_millis(15));

    println!("\n📈 Performance Report:");
    println!("{}", profiler.generate_report());

    Ok(())
}

async fn demonstrate_health_tracking(context: &mut RenderContext) -> GupResult<()> {
    println!("Demonstrating component health tracking over multiple executions...");

    let reliable = ReliableComponent::new("ReliableChart");
    let unreliable = MemoryHungryComponent::new("UnreliableOverlay", 0.6);
    let mut composition = reliable.mix(unreliable);

    let policy = ErrorHandlingPolicy::default();
    let mut executor = RobustCompositionExecutor::new(policy);

    println!("Executing composition multiple times to track health...");
    for i in 1..=5 {
        let result = executor.execute_robust(&mut composition, context).await;
        println!(
            "Execution {}: {} (Health: {:.1}%)",
            i,
            if result.success {
                "✅ SUCCESS"
            } else {
                "❌ FAILURE"
            },
            result.health_status.overall_health * 100.0
        );

        if !result.health_status.unhealthy_components.is_empty() {
            println!(
                "  🏥 Unhealthy components: {:?}",
                result.health_status.unhealthy_components
            );
        }
    }

    Ok(())
}

fn demonstrate_visual_debugging() {
    println!("Creating visual debugging representations...");

    let chart1 = ReliableComponent::new("ScatterPlot");
    let chart2 = MemoryHungryComponent::new("Heatmap", 0.4);
    let shader = ShaderComponent::new("BloomEffect", 6);

    let composition = chart1.mix(chart2.mix(shader));

    println!("\n🌳 Composition Tree Visualization:");
    let tree = debug::CompositionVisualizer::visualize(&composition);
    println!("{tree}");

    println!("🔗 DOT Graph (for Graphviz):");
    let dot_graph = debug::CompositionVisualizer::to_dot_graph(&composition);
    println!("{dot_graph}");

    // In a real implementation, you could save this to a file and render with Graphviz:
    // dot -Tpng composition.dot -o composition.png
}

fn demonstrate_debug_session() {
    println!("Setting up interactive debugging session...");

    let chart = ReliableComponent::new("InteractiveChart");
    let overlay = MemoryHungryComponent::new("DataOverlay", 0.3);
    let composition = chart.mix(overlay);

    let mut debug_session = debug::DebugSession::new(&composition);

    // Add breakpoints and configure debugging
    debug_session.add_breakpoint(12345);
    debug_session.add_breakpoint(67890);
    debug_session.enable_step_mode();

    println!("🔍 Debug Session Configuration:");
    println!("  • Breakpoints: {:?}", debug_session.breakpoints);
    println!(
        "  • Step mode: {}",
        if debug_session.step_mode {
            "Enabled"
        } else {
            "Disabled"
        }
    );

    println!("\n🌳 Captured Composition Tree:");
    debug_session.print_tree();

    println!("In a real debugging session, you would be able to:");
    println!("  • Step through component execution");
    println!("  • Inspect component state at breakpoints");
    println!("  • Modify recovery strategies on the fly");
    println!("  • View real-time performance metrics");
}
