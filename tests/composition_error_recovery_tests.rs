// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive tests for composition error recovery and diagnostics.
//!
//! These tests validate all error handling scenarios, recovery strategies,
//! and diagnostic tools for the composition system.

#![allow(clippy::field_reassign_with_default)]

use gup::mixable::composition_recovery::{
    CompositionFallbackType, ErrorHandlingPolicy, MixableErrorRecovery, RecoveryStrategy,
    RobustCompositionExecutor, debug,
};
use gup::{GupError, GupResult, Mixable, RenderContext};
use std::time::Duration;

/// Test component that can simulate various failure modes
#[derive(Debug, Clone)]
struct MockComponent {
    name: String,
    failure_mode: FailureMode,
    execution_count: std::sync::Arc<std::sync::Mutex<u32>>,
}

#[derive(Debug, Clone)]
enum FailureMode {
    Never,
    Always,
    FirstNTimes(u32),
    MemoryExhaustion,
    ShaderCompilation,
    PerformanceThreshold,
}

impl MockComponent {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            failure_mode: FailureMode::Never,
            execution_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
        }
    }

    fn with_failure_mode(name: &str, mode: FailureMode) -> Self {
        Self {
            name: name.to_string(),
            failure_mode: mode,
            execution_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
        }
    }
}

impl Mixable for MockComponent {
    type Output = ();

    fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
        let mut count = self.execution_count.lock().unwrap();
        *count += 1;
        let current_count = *count;

        match &self.failure_mode {
            FailureMode::Never => Ok(()),
            FailureMode::Always => Err(GupError::render_error(format!(
                "Component {} always fails",
                self.name
            ))),
            FailureMode::FirstNTimes(n) => {
                if current_count <= *n {
                    Err(GupError::render_error(format!(
                        "Component {} failing on attempt {}",
                        self.name, current_count
                    )))
                } else {
                    Ok(())
                }
            }
            FailureMode::MemoryExhaustion => Err(GupError::gpu_memory_exhausted(2048, 1024)),
            FailureMode::ShaderCompilation => Err(GupError::shader_compilation_failed(
                "vertex",
                "syntax error",
            )),
            FailureMode::PerformanceThreshold => {
                Err(GupError::performance_target_missed(16.67, 33.34))
            }
        }
    }

    fn description(&self) -> String {
        self.name.clone()
    }

    fn is_valid(&self) -> bool {
        !matches!(self.failure_mode, FailureMode::Always)
    }
}

#[tokio::test]
async fn test_error_recovery_skip_strategy() {
    let mut context = RenderContext::new().await.unwrap();

    let good_component = MockComponent::new("GoodComponent");
    let bad_component = MockComponent::with_failure_mode("BadComponent", FailureMode::Always);
    let mut composition = good_component.mix(bad_component);

    let mut policy = ErrorHandlingPolicy::default();
    policy.default_recovery = RecoveryStrategy::Skip;

    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor
        .execute_robust(&mut composition, &mut context)
        .await;

    assert!(result.success); // Should succeed despite failed component
    assert!(!result.errors.is_empty()); // Should record the error
}

#[tokio::test]
async fn test_fallback_rendering() {
    let mut context = RenderContext::new().await.unwrap();
    let mut failing_component =
        MockComponent::with_failure_mode("FailingComponent", FailureMode::Always);

    let mut policy = ErrorHandlingPolicy::default();
    policy.default_recovery = RecoveryStrategy::Fallback(CompositionFallbackType::Placeholder(
        "Component failed".to_string(),
    ));

    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor
        .execute_robust(&mut failing_component, &mut context)
        .await;

    assert!(result.success); // Should succeed with fallback
}

#[tokio::test]
async fn test_retry_strategy_success() {
    let mut context = RenderContext::new().await.unwrap();
    let mut component =
        MockComponent::with_failure_mode("RetryComponent", FailureMode::FirstNTimes(2));

    let mut policy = ErrorHandlingPolicy::default();
    policy.default_recovery = RecoveryStrategy::Retry {
        max_attempts: 3,
        backoff: Duration::from_millis(10),
    };

    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor.execute_robust(&mut component, &mut context).await;

    assert!(result.success); // Should succeed after retries
}

#[tokio::test]
async fn test_retry_strategy_exhaustion() {
    let mut context = RenderContext::new().await.unwrap();
    let mut component = MockComponent::with_failure_mode("AlwaysFailing", FailureMode::Always);

    let mut policy = ErrorHandlingPolicy::default();
    policy.default_recovery = RecoveryStrategy::Retry {
        max_attempts: 2,
        backoff: Duration::from_millis(10),
    };

    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor.execute_robust(&mut component, &mut context).await;

    // Should eventually succeed with fallback after retry exhaustion
    assert!(result.success);
}

#[tokio::test]
async fn test_memory_exhaustion_recovery() {
    let mut context = RenderContext::new().await.unwrap();
    let mut component =
        MockComponent::with_failure_mode("MemoryComponent", FailureMode::MemoryExhaustion);

    let policy = ErrorHandlingPolicy::default();
    let result = component.render_with_recovery(&mut context, &policy).await;

    assert!(result.success); // Should recover using skip strategy
}

#[tokio::test]
async fn test_shader_compilation_recovery() {
    let mut context = RenderContext::new().await.unwrap();
    let mut component =
        MockComponent::with_failure_mode("ShaderComponent", FailureMode::ShaderCompilation);

    let policy = ErrorHandlingPolicy::default();
    let result = component.render_with_recovery(&mut context, &policy).await;

    assert!(result.success); // Should recover using fallback
}

#[tokio::test]
async fn test_performance_threshold_recovery() {
    let mut context = RenderContext::new().await.unwrap();
    let mut component =
        MockComponent::with_failure_mode("SlowComponent", FailureMode::PerformanceThreshold);

    let policy = ErrorHandlingPolicy::default();
    let result = component.render_with_recovery(&mut context, &policy).await;

    assert!(result.success); // Should recover using quality reduction
}

#[tokio::test]
async fn test_component_specific_strategy() {
    let mut context = RenderContext::new().await.unwrap();
    let mut component = MockComponent::with_failure_mode("SpecialComponent", FailureMode::Always);

    let mut policy = ErrorHandlingPolicy::default();
    policy.component_strategies.insert(
        "SpecialComponent".to_string(),
        RecoveryStrategy::Fallback(CompositionFallbackType::SimpleGeometry),
    );

    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor.execute_robust(&mut component, &mut context).await;

    assert!(result.success); // Should use component-specific strategy
}

#[tokio::test]
async fn test_fail_fast_strategy() {
    let mut context = RenderContext::new().await.unwrap();
    let mut component = MockComponent::with_failure_mode("CriticalComponent", FailureMode::Always);

    let mut policy = ErrorHandlingPolicy::default();
    policy.default_recovery = RecoveryStrategy::Fail;

    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor.execute_robust(&mut component, &mut context).await;

    assert!(!result.success); // Should fail immediately
}

#[tokio::test]
async fn test_health_tracking() {
    let mut context = RenderContext::new().await.unwrap();

    // Test healthy component
    let mut good_component = MockComponent::new("HealthyComponent");
    let policy = ErrorHandlingPolicy::default();

    let mut executor = RobustCompositionExecutor::new(policy.clone());
    let result = executor
        .execute_robust(&mut good_component, &mut context)
        .await;

    assert!(result.success);
    assert!(result.health_status.overall_health > 0.0);

    // Test unhealthy component by using a Fail strategy to ensure failures count
    let mut bad_component =
        MockComponent::with_failure_mode("UnhealthyComponent", FailureMode::Always);

    // Use Fail strategy so failures are actually recorded as failures
    let mut fail_policy = ErrorHandlingPolicy::default();
    fail_policy.default_recovery = RecoveryStrategy::Fail;
    let mut fail_executor = RobustCompositionExecutor::new(fail_policy);

    // Execute multiple times to trigger unhealthy state (needs 3+ consecutive failures)
    for _ in 0..4 {
        let _result = fail_executor
            .execute_robust(&mut bad_component, &mut context)
            .await;
    }

    // Final execution to get updated health status
    let result = fail_executor
        .execute_robust(&mut bad_component, &mut context)
        .await;

    // Health should be affected after multiple failures
    let final_health = result.health_status.overall_health;
    assert!(final_health < 1.0);
}

#[tokio::test]
async fn test_performance_monitoring() {
    let mut context = RenderContext::new().await.unwrap();

    let component1 = MockComponent::new("FastComponent");
    let component2 = MockComponent::new("SlowComponent");
    let mut composition = component1.mix(component2);

    let policy = ErrorHandlingPolicy::default();
    let mut executor = RobustCompositionExecutor::new(policy);

    let result = executor
        .execute_robust(&mut composition, &mut context)
        .await;

    assert!(result.success);
    assert!(result.execution_time > Duration::ZERO);
    assert!(result.performance_metrics.total_execution_time > Duration::ZERO);
}

#[tokio::test]
async fn test_error_context_collection() {
    let mut context = RenderContext::new().await.unwrap();
    let mut component = MockComponent::with_failure_mode("ErrorComponent", FailureMode::Always);

    let mut policy = ErrorHandlingPolicy::default();
    policy.collect_context = true;

    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor.execute_robust(&mut component, &mut context).await;

    assert!(result.success); // Recovered
    assert!(!result.errors.is_empty()); // Error was recorded

    let error_record = &result.errors[0];
    assert_eq!(error_record.component_type, "unknown"); // Would be filled in real implementation
    assert!(error_record.recovery_action.is_some());
}

#[test]
fn test_debug_visualization() {
    let component1 = MockComponent::new("Component1");
    let component2 = MockComponent::with_failure_mode("Component2", FailureMode::Always);
    let composition = component1.mix(component2);

    let visualization = debug::CompositionVisualizer::visualize(&composition);

    assert!(visualization.contains("ComposedVisualization"));
    assert!(visualization.contains("Component1"));
    assert!(visualization.contains("Component2"));
}

#[test]
fn test_dot_graph_generation() {
    let component = MockComponent::new("TestComponent");
    let dot_graph = debug::CompositionVisualizer::to_dot_graph(&component);

    assert!(dot_graph.contains("digraph composition"));
    assert!(dot_graph.contains("TestComponent"));
    assert!(dot_graph.contains("color=green"));
}

#[test]
fn test_debug_session_functionality() {
    let component = MockComponent::new("SessionComponent");
    let mut session = debug::DebugSession::new(&component);

    session.add_breakpoint(12345);
    session.enable_step_mode();

    // Verify session state
    assert!(session.composition_tree.contains("SessionComponent"));
    assert_eq!(session.breakpoints.len(), 1);
    assert!(session.step_mode);
}

#[test]
fn test_composition_profiler() {
    let mut profiler = debug::CompositionProfiler::new();

    profiler.start_timing("render_operation");
    let duration = Duration::from_millis(15);
    profiler.end_timing(duration);

    profiler.start_timing("validation_operation");
    let duration2 = Duration::from_millis(5);
    profiler.end_timing(duration2);

    let report = profiler.generate_report();
    assert!(report.contains("render_operation"));
    assert!(report.contains("validation_operation"));
    assert!(report.contains("avg="));
    assert!(report.contains("min="));
    assert!(report.contains("max="));
    assert!(report.contains("calls="));
}

#[tokio::test]
async fn test_mixed_composition_recovery() {
    let mut context = RenderContext::new().await.unwrap();

    // Create a complex composition with mixed success/failure
    let good1 = MockComponent::new("Good1");
    let bad1 = MockComponent::with_failure_mode("Bad1", FailureMode::Always);
    let good2 = MockComponent::new("Good2");
    let bad2 = MockComponent::with_failure_mode("Bad2", FailureMode::MemoryExhaustion);

    let mut complex_composition = good1.mix(bad1).mix(good2.mix(bad2));

    let policy = ErrorHandlingPolicy::default();
    let mut executor = RobustCompositionExecutor::new(policy);

    let result = executor
        .execute_robust(&mut complex_composition, &mut context)
        .await;

    // Should succeed overall due to skip recovery strategy
    assert!(result.success);
    // Should have recorded errors from the failing components
    assert!(!result.errors.is_empty());
    // Should have performance metrics
    assert!(result.execution_time > Duration::ZERO);
}

#[tokio::test]
async fn test_recovery_recommendation_system() {
    let component = MockComponent::new("TestComponent");
    let recommendations = component.recovery_recommendations();

    assert!(!recommendations.is_empty());
    assert!(
        recommendations
            .iter()
            .any(|r| matches!(r, RecoveryStrategy::Skip))
    );
    assert!(
        recommendations
            .iter()
            .any(|r| matches!(r, RecoveryStrategy::Fallback(_)))
    );
    assert!(
        recommendations
            .iter()
            .any(|r| matches!(r, RecoveryStrategy::Retry { .. }))
    );
}

#[tokio::test]
async fn test_cascading_failure_recovery() {
    let mut context = RenderContext::new().await.unwrap();

    // Create a chain of components where failures cascade
    let c1 = MockComponent::with_failure_mode("C1", FailureMode::FirstNTimes(1));
    let c2 = MockComponent::with_failure_mode("C2", FailureMode::FirstNTimes(1));
    let c3 = MockComponent::new("C3");

    let mut chain = c1.mix(c2).mix(c3);

    let mut policy = ErrorHandlingPolicy::default();
    policy.default_recovery = RecoveryStrategy::Retry {
        max_attempts: 2,
        backoff: Duration::from_millis(10),
    };

    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor.execute_robust(&mut chain, &mut context).await;

    // Should eventually succeed after retries recover the failing components
    assert!(result.success);
}

#[tokio::test]
async fn test_context_dependent_recovery() {
    let mut context = RenderContext::new().await.unwrap();

    // Test recovery behavior under different context conditions
    let mut component = MockComponent::with_failure_mode("ContextComponent", FailureMode::Always);

    // Test with different viewport sizes
    let small_viewport = gup::Viewport {
        width: 100,
        height: 100,
        scale_factor: 1.0,
    };
    context.set_viewport(small_viewport).unwrap();

    let policy = ErrorHandlingPolicy::default();
    let mut executor = RobustCompositionExecutor::new(policy);
    let result = executor.execute_robust(&mut component, &mut context).await;

    assert!(result.success); // Should recover regardless of context
}
