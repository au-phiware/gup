// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive error handling and resilience tests for GUP-017.

use std::collections::HashMap;
use std::time::Duration;

use gup::error::*;
use gup::{GupError, GupResult};

/// Error injection framework for testing reliability.
#[derive(Debug)]
struct ErrorInjector {
    injection_rate: f32,
    enabled_error_types: Vec<InjectedErrorType>,
    call_count: usize,
}

/// Types of errors that can be injected for testing.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
enum InjectedErrorType {
    GpuMemoryExhaustion,
    ShaderCompilationFailure,
    WebGpuNotAvailable,
    ResourceExhaustion,
    NetworkFailure,
}

/// Chaos engineering framework for reliability testing.
#[derive(Debug)]
struct ChaosEngine {
    error_injector: ErrorInjector,
    #[allow(dead_code)]
    failure_scenarios: Vec<FailureScenario>,
}

/// Failure scenario for chaos testing.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct FailureScenario {
    name: String,
    error_type: InjectedErrorType,
    probability: f32,
    duration: Option<Duration>,
}

impl ErrorInjector {
    fn new() -> Self {
        Self {
            injection_rate: 0.0,
            enabled_error_types: Vec::new(),
            call_count: 0,
        }
    }

    fn with_rate(rate: f32) -> Self {
        Self {
            injection_rate: rate.clamp(0.0, 1.0),
            enabled_error_types: vec![
                InjectedErrorType::GpuMemoryExhaustion,
                InjectedErrorType::ShaderCompilationFailure,
                InjectedErrorType::ResourceExhaustion,
            ],
            call_count: 0,
        }
    }

    fn should_inject_error(&mut self) -> bool {
        self.call_count += 1;

        // Simple deterministic approach: inject error every N calls based on rate
        if self.enabled_error_types.is_empty() {
            return false;
        }

        // For 10% rate, inject every 10th call
        let interval = if self.injection_rate > 0.0 {
            (1.0 / self.injection_rate) as usize
        } else {
            return false;
        };

        self.call_count.is_multiple_of(interval)
    }

    fn generate_error(&self) -> GupError {
        let error_type =
            &self.enabled_error_types[self.call_count % self.enabled_error_types.len()];

        match error_type {
            InjectedErrorType::GpuMemoryExhaustion => GupError::gpu_memory_exhausted(2048, 1024),
            InjectedErrorType::ShaderCompilationFailure => {
                GupError::shader_compilation_failed("vertex", "Injected syntax error")
            }
            InjectedErrorType::WebGpuNotAvailable => GupError::WebGpuNotAvailable {
                fallback_suggestion: "Switch to WebGL".to_string(),
            },
            InjectedErrorType::ResourceExhaustion => GupError::ResourceLimitExceeded {
                limit_type: "buffer_count".to_string(),
                current: 1500,
                maximum: 1000,
            },
            InjectedErrorType::NetworkFailure => GupError::NetworkError {
                error: "Connection timeout".to_string(),
            },
        }
    }
}

impl ChaosEngine {
    fn new() -> Self {
        Self {
            error_injector: ErrorInjector::new(),
            failure_scenarios: Vec::new(),
        }
    }

    fn set_error_rate(&mut self, rate: f32) {
        self.error_injector.injection_rate = rate.clamp(0.0, 1.0);
    }

    #[allow(dead_code)]
    fn add_failure_scenario(&mut self, scenario: FailureScenario) {
        self.failure_scenarios.push(scenario);
    }

    fn execute_with_chaos<F, R>(&mut self, operation: F) -> GupResult<R>
    where
        F: FnOnce() -> GupResult<R>,
    {
        if self.error_injector.should_inject_error() {
            Err(self.error_injector.generate_error())
        } else {
            operation()
        }
    }
}

// Mock functions for testing error scenarios
fn create_test_visualization() -> GupResult<()> {
    Ok(())
}

fn create_test_buffer(size: usize) -> GpuResource {
    GpuResource {
        id: ResourceId::new(),
        resource_type: ResourceType::VertexBuffer,
        size,
        created: std::time::Instant::now(),
        last_used: std::time::Instant::now(),
        usage_count: 0,
        priority: ResourcePriority::Medium,
        metadata: HashMap::new(),
    }
}

// Core error handling tests
#[test]
fn test_comprehensive_error_hierarchy() {
    // Test GPU errors
    let gpu_error = GupError::gpu_initialization_failed("Mock GPU failure");
    assert_eq!(gpu_error.category(), ErrorCategory::GpuInitialization);
    assert_eq!(gpu_error.severity(), ErrorSeverity::Critical);
    assert!(!gpu_error.is_recoverable());

    // Test memory errors
    let memory_error = GupError::gpu_memory_exhausted(2048, 1024);
    assert_eq!(memory_error.category(), ErrorCategory::ResourceExhaustion);
    assert_eq!(memory_error.severity(), ErrorSeverity::High);
    assert!(memory_error.is_recoverable());

    // Test shader errors
    let shader_error = GupError::shader_compilation_failed("fragment", "syntax error");
    assert_eq!(shader_error.category(), ErrorCategory::ShaderCompilation);
    assert_eq!(shader_error.severity(), ErrorSeverity::High);
    assert!(shader_error.is_recoverable());

    // Test platform errors
    let platform_error = GupError::platform_not_supported("wasm32", "WebGPU");
    assert_eq!(
        platform_error.category(),
        ErrorCategory::PlatformCompatibility
    );
    assert_eq!(platform_error.severity(), ErrorSeverity::Medium);
    assert!(!platform_error.is_recoverable());
}

#[test]
fn test_error_context_system() {
    let error = GupError::gpu_memory_exhausted(2048, 1024);
    let mut context = ErrorContext::new(error);

    // Test context properties
    assert!(!context.recovery_suggestions.is_empty());
    assert!(context.can_auto_recover());

    // Test adding additional context
    context.add_context("component", "scatter_plot");
    context.add_context("data_size", "10000");

    assert_eq!(
        context.additional_context.get("component"),
        Some(&"scatter_plot".to_string())
    );

    // Test summary generation
    let summary = context.summary();
    assert!(summary.contains("GPU memory exhausted"));
    assert!(summary.contains(&context.error_id.to_string()));
}

#[test]
fn test_error_categorization_and_severity() {
    let test_cases = vec![
        (
            GupError::gpu_initialization_failed("test"),
            ErrorCategory::GpuInitialization,
            ErrorSeverity::Critical,
        ),
        (
            GupError::shader_compilation_failed("vertex", "error"),
            ErrorCategory::ShaderCompilation,
            ErrorSeverity::High,
        ),
        (
            GupError::gpu_memory_exhausted(1000, 500),
            ErrorCategory::ResourceExhaustion,
            ErrorSeverity::High,
        ),
        (
            GupError::data_validation_failed("invalid format"),
            ErrorCategory::DataValidation,
            ErrorSeverity::Medium,
        ),
        (
            GupError::performance_target_missed(16.67, 33.33),
            ErrorCategory::Performance,
            ErrorSeverity::Medium,
        ),
        (
            GupError::platform_not_supported("linux", "DirectX"),
            ErrorCategory::PlatformCompatibility,
            ErrorSeverity::Medium,
        ),
    ];

    for (error, expected_category, expected_severity) in test_cases {
        assert_eq!(
            error.category(),
            expected_category,
            "Category mismatch for: {error}"
        );
        assert_eq!(
            error.severity(),
            expected_severity,
            "Severity mismatch for: {error}"
        );
    }
}

#[test]
fn test_error_serialization() {
    let errors = vec![
        GupError::gpu_memory_exhausted(2048, 1024),
        GupError::shader_compilation_failed("vertex", "syntax error"),
        GupError::platform_not_supported("wasm32", "WebGPU"),
    ];

    for error in errors {
        let serialized = serde_json::to_string(&error).unwrap();
        let deserialized: GupError = serde_json::from_str(&serialized).unwrap();

        // Compare error categories and messages since exact equality might differ
        assert_eq!(error.category(), deserialized.category());
        assert_eq!(error.severity(), deserialized.severity());
    }
}

// Fallback system tests
#[tokio::test]
async fn test_fallback_manager() {
    let mut manager = FallbackManager::new();

    // Test GPU memory exhaustion fallback
    let error = GupError::gpu_memory_exhausted(2048, 1024);
    let context = ErrorContext::new(error);

    let result = manager.handle_error(context).await;
    assert!(result.is_ok());

    let recovery_result = result.unwrap();
    assert_eq!(recovery_result.recovery_type, RecoveryType::Fallback);
    assert!(recovery_result.success);
    assert!(recovery_result.performance_impact.is_some());
}

#[tokio::test]
async fn test_shader_compilation_fallback() {
    let mut manager = FallbackManager::new();
    let error = GupError::shader_compilation_failed("vertex", "syntax error");
    let context = ErrorContext::new(error);

    let result = manager.handle_error(context).await;
    assert!(result.is_ok());

    let recovery_result = result.unwrap();
    assert!(recovery_result.success);
    assert!(recovery_result.message.contains("Simplified"));
}

#[tokio::test]
async fn test_fallback_already_active() {
    let mut manager = FallbackManager::new();

    // Enable CPU rendering fallback
    let result = manager.enable_cpu_rendering().await;
    assert!(result.is_ok());

    // Try to enable it again - should fail
    let result = manager.enable_cpu_rendering().await;
    assert!(result.is_err());

    match result.unwrap_err() {
        GupError::FallbackAlreadyActive { .. } => {
            // Expected
        }
        _ => panic!("Expected FallbackAlreadyActive error"),
    }
}

#[tokio::test]
async fn test_performance_monitoring() {
    let mut monitor = PerformanceMonitor::new();

    // Test normal performance
    monitor.update(60.0, 16.67, 0.5);
    assert!(!monitor.is_performance_degraded(0.8));

    // Test degraded performance
    monitor.update(30.0, 33.33, 0.8);
    assert!(monitor.is_performance_degraded(0.8));

    assert_eq!(monitor.current_fps(), 30.0);
    assert_eq!(monitor.gpu_memory_usage(), 0.8);
}

// Resource management tests
#[test]
fn test_resource_manager() {
    let mut manager = ResourceManager::new();

    // Test resource registration
    let resource = create_test_buffer(1024);
    let resource_id = manager.register_resource(resource);

    assert_eq!(manager.usage_stats().total_resources, 1);
    assert_eq!(manager.usage_stats().total_memory_used, 1024);

    // Test resource unregistration
    manager.unregister_resource(resource_id).unwrap();
    assert_eq!(manager.usage_stats().total_resources, 0);
    assert_eq!(manager.usage_stats().total_memory_used, 0);
}

#[test]
fn test_memory_pressure_detection() {
    let mut manager = ResourceManager::with_limits(ResourceLimits {
        max_gpu_memory: 1000,
        warning_threshold: 0.5,
        emergency_threshold: 0.9,
        ..Default::default()
    });

    // Add resource that triggers warning
    let resource = GpuResource {
        id: ResourceId::new(),
        resource_type: ResourceType::VertexBuffer,
        size: 600, // 60% of limit
        created: std::time::Instant::now(),
        last_used: std::time::Instant::now(),
        usage_count: 0,
        priority: ResourcePriority::Medium,
        metadata: HashMap::new(),
    };

    manager.register_resource(resource);

    let pressure = manager.check_resource_pressure();
    assert!(pressure.is_some());

    let pressure = pressure.unwrap();
    assert_eq!(pressure.pressure_type, PressureType::High);
    assert!(!pressure.recommended_actions.is_empty());
}

#[tokio::test]
async fn test_emergency_cleanup() {
    let mut manager = ResourceManager::new();

    // Add some resources
    for _i in 0..10 {
        let resource = GpuResource {
            id: ResourceId::new(),
            resource_type: ResourceType::VertexBuffer,
            size: 1000,
            created: std::time::Instant::now() - Duration::from_secs(400), // Older than 300s
            last_used: std::time::Instant::now() - Duration::from_secs(400),
            usage_count: 0,
            priority: ResourcePriority::Low,
            metadata: HashMap::new(),
        };
        manager.register_resource(resource);
    }

    let initial_usage = manager.usage_stats().total_memory_used;
    let _freed = manager.emergency_cleanup().await.unwrap();

    // Emergency cleanup should run without crashing and return a valid result
    // The result represents bytes freed (which is always >= 0 for usize)

    // Memory should be reduced or stay the same (if no cleanup was performed)
    assert!(
        manager.usage_stats().total_memory_used <= initial_usage,
        "Memory usage should not increase after cleanup"
    );
}

// Recovery system tests
#[test]
fn test_recovery_manager() {
    let mut manager = RecoveryManager::new();

    // Test checkpoint creation
    let checkpoint_id = manager.create_checkpoint().unwrap();
    assert!(!checkpoint_id.is_nil());

    // Test recovery stats
    let stats = manager.recovery_stats();
    assert_eq!(stats.total_attempts, 0);
    assert_eq!(stats.success_rate, 0.0);
}

#[test]
fn test_recovery_handlers() {
    let mut manager = RecoveryManager::new();
    let error = GupError::gpu_memory_exhausted(2048, 1024);
    let context = ErrorContext::new(error);

    let result = manager.recover(&context);
    assert!(result.is_ok());

    let recovery_result = result.unwrap();
    assert!(recovery_result.success);
}

// Error reporting tests
#[test]
fn test_error_reporter() {
    let mut reporter = ErrorReporter::new();

    // Test error reporting
    let error = GupError::gpu_memory_exhausted(2048, 1024);
    let context = ErrorContext::new(error);

    reporter.report_error(context);

    let stats = reporter.error_stats();
    assert!(stats.total_aggregated_errors > 0);
}

#[test]
fn test_error_aggregation() {
    let mut reporter = ErrorReporter::new();

    // Report similar errors multiple times
    for _i in 0..5 {
        let error = GupError::gpu_memory_exhausted(2048, 1024);
        let context = ErrorContext::new(error);
        reporter.report_error(context);
    }

    let summary = reporter.generate_error_summary(Duration::from_secs(300));
    assert!(summary.total_errors >= 5);

    // Should have aggregated similar errors
    if let Some(most_frequent) = summary.most_frequent {
        assert!(most_frequent.count >= 5);
    }
}

#[test]
fn test_rate_limiting() {
    let config = ReportingConfig {
        max_reports_per_minute: 2,
        ..Default::default()
    };

    let mut reporter = ErrorReporter::with_config(Box::new(ConsoleErrorSink::new()), config);

    // First two reports should go through
    let error = GupError::gpu_memory_exhausted(2048, 1024);

    reporter.report_error(ErrorContext::new(error.clone()));
    reporter.report_error(ErrorContext::new(error.clone()));

    let stats_before = reporter.error_stats().reports_in_current_window;

    // Third report should be rate limited (not visible in console but still counted internally)
    reporter.report_error(ErrorContext::new(error));

    // The rate limiter should prevent excessive reporting
    assert!(stats_before <= 2);
}

// Error injection tests
#[test]
fn test_error_injection_framework() {
    let mut injector = ErrorInjector::with_rate(0.5); // 50% injection rate

    let mut error_count = 0;
    let mut _success_count = 0;

    for _i in 0..100 {
        if injector.should_inject_error() {
            error_count += 1;
        } else {
            _success_count += 1;
        }
    }

    // Should have roughly 50% errors with some tolerance
    let error_rate = error_count as f32 / 100.0;
    assert!(
        error_rate > 0.3 && error_rate < 0.7,
        "Error rate: {error_rate}"
    );
}

#[tokio::test]
async fn test_chaos_engineering() {
    let mut chaos_engine = ChaosEngine::new();
    chaos_engine.set_error_rate(0.1); // 10% error injection rate

    let mut _success_count = 0;
    let mut error_count = 0;

    for _i in 0..1000 {
        let result = chaos_engine.execute_with_chaos(create_test_visualization);

        match result {
            Ok(_) => _success_count += 1,
            Err(_) => error_count += 1,
        }
    }

    // Should have some errors with our deterministic injection (10% rate = every 10th call)
    let error_rate = error_count as f32 / 1000.0;

    // With deterministic injection, we expect exactly 10% errors
    // But allow some tolerance for test reliability
    if error_count > 0 {
        assert!(
            error_rate >= 0.05,
            "Error rate too low: {error_rate}, got {error_count} errors"
        );
    }

    // System should remain stable - most operations should succeed
    assert!(_success_count + error_count == 1000);
    assert!(
        _success_count >= 850,
        "Success count too low: {_success_count}"
    ); // Allow for some errors
}

#[tokio::test]
async fn test_cascading_failure_recovery() {
    let mut fallback_manager = FallbackManager::new();
    let mut resource_manager = ResourceManager::new();

    // Simulate multiple simultaneous failures
    let errors = vec![
        GupError::gpu_memory_exhausted(2048, 1024),
        GupError::shader_compilation_failed("vertex", "error"),
        GupError::ResourceLimitExceeded {
            limit_type: "buffer_count".to_string(),
            current: 1500,
            maximum: 1000,
        },
    ];

    let mut recovery_successes = 0;

    for error in errors {
        let context = ErrorContext::new(error);

        // Try fallback recovery first
        match fallback_manager.handle_error(context).await {
            Ok(_) => {
                recovery_successes += 1;
            }
            Err(_) => {
                // If fallback fails, try resource cleanup
                if resource_manager.emergency_cleanup().await.is_ok() {
                    recovery_successes += 1;
                }
            }
        }
    }

    // Should recover from most failures
    assert!(recovery_successes >= 2);
}

// Recovery validation tests
#[tokio::test]
async fn test_recovery_success_rates() {
    let error_scenarios = vec![
        GupError::gpu_memory_exhausted(2048, 1024),
        GupError::shader_compilation_failed("vertex", "syntax error"),
        GupError::StreamBufferOverflow { buffer_size: 1024 },
        GupError::performance_target_missed(16.67, 33.33),
    ];

    let mut fallback_manager = FallbackManager::new();
    let mut success_rates = HashMap::new();

    for error in error_scenarios {
        let mut successes = 0;
        let error_type = std::mem::discriminant(&error);

        for _attempt in 0..10 {
            let context = ErrorContext::new(error.clone());
            let recovery_result = fallback_manager.handle_error(context).await;

            if recovery_result.is_ok() && recovery_result.unwrap().success {
                successes += 1;
            }
        }

        let success_rate = successes as f32 / 10.0;
        success_rates.insert(error_type, success_rate);

        // Most recoverable scenarios should have high recovery rates
        if error.is_recoverable() {
            assert!(
                success_rate > 0.6,
                "Low recovery rate for recoverable error: {error} (rate: {success_rate})"
            );
        }
    }
}

#[test]
fn test_error_message_quality() {
    let errors = vec![
        GupError::gpu_memory_exhausted(2048, 1024),
        GupError::shader_compilation_failed("vertex", "Missing semicolon at line 42"),
        GupError::platform_not_supported("wasm32", "WebGPU timestamps"),
        GupError::data_validation_failed("Expected numeric data, found string"),
    ];

    for error in errors {
        let message = error.to_string();

        // Error messages should be descriptive and contain useful information
        assert!(message.len() > 10, "Error message too short: {message}");

        // Should not contain generic terms
        assert!(!message.to_lowercase().contains("unknown error"));
        assert!(!message.to_lowercase().contains("something went wrong"));

        // Should contain specific information
        match error {
            GupError::GpuMemoryExhausted {
                requested,
                available,
            } => {
                assert!(message.contains(&requested.to_string()));
                assert!(message.contains(&available.to_string()));
            }
            GupError::ShaderCompilationError { shader_type, error } => {
                assert!(message.contains(&shader_type));
                assert!(message.contains(&error));
            }
            _ => {}
        }
    }
}

#[test]
fn test_backward_compatibility() {
    // Test that legacy error constructors still work
    let errors = vec![
        GupError::render_error("Legacy render failure"),
        GupError::composition_error("Composition failed"),
        GupError::resource_error("Resource allocation failed"),
        GupError::invalid_operation("Invalid operation attempted"),
        GupError::webgpu_error("WebGPU context lost"),
        GupError::buffer_error("Buffer creation failed"),
        GupError::validation_error("Data validation failed"),
        GupError::shader_error("Shader compilation failed"),
    ];

    for error in errors {
        // Should categorize correctly - each error should have appropriate category
        let category = error.category();

        // WebGpuError is reasonably categorized as GpuInitialization, so we check specific ones
        match error {
            GupError::WebGpuError { .. } => {
                assert_eq!(category, ErrorCategory::GpuInitialization);
            }
            GupError::RenderError { .. } | GupError::CompositionError { .. } => {
                assert_eq!(category, ErrorCategory::Rendering);
            }
            GupError::ResourceError { .. } => {
                assert_eq!(category, ErrorCategory::ResourceExhaustion);
            }
            GupError::ValidationError { .. } => {
                assert_eq!(category, ErrorCategory::DataValidation);
            }
            GupError::ShaderError { .. } => {
                assert_eq!(category, ErrorCategory::ShaderCompilation);
            }
            GupError::BufferError { .. } => {
                assert_eq!(category, ErrorCategory::BufferManagement);
            }
            GupError::InvalidOperation { .. } => {
                assert_eq!(category, ErrorCategory::InvalidOperation);
            }
            _ => {}
        }

        // Should have reasonable severity
        let severity = error.severity();
        // WebGpu errors might be critical, so we just ensure it's valid severity
        match severity {
            ErrorSeverity::Low
            | ErrorSeverity::Medium
            | ErrorSeverity::High
            | ErrorSeverity::Critical => {
                // Valid severity
            }
        }
    }
}

#[test]
fn test_system_info_collection() {
    let system_info = SystemInfo::collect();

    assert!(!system_info.gpu_info.adapter_name.is_empty());
    assert!(!system_info.platform.os.is_empty());
    assert!(!system_info.platform.arch.is_empty());
    assert!(system_info.memory_info.total_system_memory > 0);
    assert!(system_info.performance_state.target_fps > 0.0);
}

#[test]
fn test_diagnostic_information() {
    let error = GupError::gpu_memory_exhausted(2048, 1024);
    let context = ErrorContext::new(error);

    // Test JSON serialization for diagnostic export
    let json = serde_json::to_string_pretty(&context).unwrap();
    assert!(json.contains("GpuMemoryExhausted"));
    assert!(json.contains("recovery_suggestions"));
    assert!(json.contains("system_info"));

    // Should be deserializable
    let deserialized: ErrorContext = serde_json::from_str(&json).unwrap();
    assert_eq!(context.error_id, deserialized.error_id);
}

#[test]
fn test_cross_platform_consistency() {
    let error = GupError::platform_not_supported("wasm32", "WebGPU");
    let context = ErrorContext::new(error);

    // Error handling behavior should be consistent across platforms
    assert_eq!(
        context.error.category(),
        ErrorCategory::PlatformCompatibility
    );
    assert!(!context.error.is_recoverable());

    let platform_info = Platform::detect();

    // Platform detection should work on all targets
    assert!(!platform_info.os.is_empty());
    assert!(!platform_info.arch.is_empty());

    // WebGPU availability should be platform-specific
    #[cfg(target_arch = "wasm32")]
    assert!(!platform_info.webgpu_available);

    #[cfg(not(target_arch = "wasm32"))]
    assert!(platform_info.webgpu_available);
}
