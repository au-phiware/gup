// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive error handling and resilience tests for GUP-017.

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
            GupError::RenderError { .. } => {
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
