// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive error handling and resilience framework for Gup.
//!
//! This module provides a robust error handling system with automatic recovery
//! mechanisms, fallback strategies, and detailed error reporting to ensure
//! reliable operation across different platforms and scenarios.

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Re-export commonly used types
pub use cache::*;
pub use error_context::*;
pub use fallback::*;
pub use lazy_context::*;
pub use recovery::*;
pub use reporting::*;
pub use resource::*;

pub mod cache;
pub mod error_context;
pub mod fallback;
pub mod lazy_context;
pub mod recovery;
pub mod reporting;
pub mod resource;

/// Main error type for Gup operations with comprehensive error categories.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum GupError {
    // GPU and rendering errors
    /// GPU initialization failed.
    #[error("GPU initialization failed: {reason}")]
    GpuInitializationError {
        /// Description of why GPU initialization failed.
        reason: String,
    },

    /// Shader compilation failed.
    #[error("Shader compilation failed: {shader_type} - {error}")]
    ShaderCompilationError {
        /// Type of shader that failed to compile.
        shader_type: String,
        /// Compilation error message.
        error: String,
    },

    /// GPU memory has been exhausted.
    #[error("GPU memory exhausted: requested {requested} bytes, available {available} bytes")]
    GpuMemoryExhausted {
        /// Number of bytes requested.
        requested: usize,
        /// Number of bytes available.
        available: usize,
    },

    /// GPU resource creation failed.
    #[error("GPU resource creation failed: {resource_type} - {reason}")]
    GpuResourceCreationError {
        /// Type of GPU resource that failed to be created.
        resource_type: String,
        /// Description of why resource creation failed.
        reason: String,
    },

    // Data and type errors
    /// Invalid data format encountered.
    #[error("Invalid data format: {message}")]
    InvalidDataFormat {
        /// Description of the invalid format.
        message: String,
    },

    /// Type mismatch between expected and actual types.
    #[error("Type mismatch: expected {expected}, found {actual}")]
    TypeMismatch {
        /// Expected type name.
        expected: String,
        /// Actual type name found.
        actual: String,
    },

    /// Data validation failed.
    #[error("Data validation failed: {validation_error}")]
    DataValidationError {
        /// Description of the validation failure.
        validation_error: String,
    },

    /// Buffer size does not match the expected size.
    #[error("Buffer size mismatch: expected {expected}, found {actual}")]
    BufferSizeMismatch {
        /// Expected buffer size in bytes.
        expected: usize,
        /// Actual buffer size in bytes.
        actual: usize,
    },

    // Streaming and performance errors
    /// Stream buffer has overflowed its capacity.
    #[error("Stream buffer overflow: {buffer_size} exceeded")]
    StreamBufferOverflow {
        /// Size of the buffer that overflowed.
        buffer_size: usize,
    },

    /// Performance target was not met.
    #[error("Performance target missed: {target_ms}ms target, actual {actual_ms}ms")]
    PerformanceTargetMissed {
        /// Target frame time in milliseconds.
        target_ms: f64,
        /// Actual frame time in milliseconds.
        actual_ms: f64,
    },

    /// A resource limit has been exceeded.
    #[error("Resource limit exceeded: {limit_type} - {current} / {maximum}")]
    ResourceLimitExceeded {
        /// Type of resource limit that was exceeded.
        limit_type: String,
        /// Current resource usage count.
        current: usize,
        /// Maximum allowed resource count.
        maximum: usize,
    },

    // Platform and compatibility errors
    /// The current platform is not supported for the requested feature.
    #[error("Platform not supported: {platform} - {feature} not available")]
    PlatformNotSupported {
        /// Name of the unsupported platform.
        platform: String,
        /// Feature that is not available.
        feature: String,
    },

    /// WebGPU is not available on this platform.
    #[error("WebGPU not available: {fallback_suggestion}")]
    WebGpuNotAvailable {
        /// Suggestion for an alternative rendering path.
        fallback_suggestion: String,
    },

    /// A required feature is not supported on the current backend.
    #[error("Feature not supported: {feature} on {backend}")]
    FeatureNotSupported {
        /// Name of the unsupported feature.
        feature: String,
        /// Name of the backend lacking support.
        backend: String,
    },

    // Network and I/O errors
    /// A network operation failed.
    #[error("Network error: {error}")]
    NetworkError {
        /// Description of the network error.
        error: String,
    },

    /// A file I/O operation failed.
    #[error("File I/O error: {path} - {error}")]
    FileError {
        /// Path of the file involved.
        path: String,
        /// Description of the I/O error.
        error: String,
    },

    // System and configuration errors
    /// A configuration parameter is invalid or missing.
    #[error("Configuration error: {parameter} - {message}")]
    ConfigurationError {
        /// Name of the configuration parameter.
        parameter: String,
        /// Description of the configuration error.
        message: String,
    },

    /// A required system resource is unavailable.
    #[error("System resource unavailable: {resource} - {reason}")]
    SystemResourceUnavailable {
        /// Name of the unavailable system resource.
        resource: String,
        /// Reason the resource is unavailable.
        reason: String,
    },

    // Legacy error types for backward compatibility
    /// Legacy render error.
    #[error("Render error: {message}")]
    RenderError {
        /// Error message.
        message: String,
    },

    /// Legacy composition error.
    #[error("Composition error: {message}")]
    CompositionError {
        /// Error message.
        message: String,
    },

    /// Legacy resource error.
    #[error("Resource error: {message}")]
    ResourceError {
        /// Error message.
        message: String,
    },

    /// Legacy invalid operation error.
    #[error("Invalid operation: {message}")]
    InvalidOperation {
        /// Error message.
        message: String,
    },

    /// Legacy WebGPU error.
    #[error("WebGPU error: {message}")]
    WebGpuError {
        /// Error message.
        message: String,
    },

    /// Legacy buffer error.
    #[error("Buffer error: {message}")]
    BufferError {
        /// Error message.
        message: String,
    },

    /// Legacy validation error.
    #[error("Validation error: {message}")]
    ValidationError {
        /// Error message.
        message: String,
    },

    /// Legacy shader error.
    #[error("Shader error: {message}")]
    ShaderError {
        /// Error message.
        message: String,
    },

    // Fallback and recovery errors
    /// A fallback of the same type is already active.
    #[error("Fallback already active: {fallback_type}")]
    FallbackAlreadyActive {
        /// Type of fallback that is already active.
        fallback_type: String,
    },

    /// An attempted recovery operation failed.
    #[error("Recovery failed: {strategy} - {reason}")]
    RecoveryFailed {
        /// Name of the recovery strategy that failed.
        strategy: String,
        /// Reason the recovery failed.
        reason: String,
    },

    /// No fallback strategy is available for the error.
    #[error("No fallback available for error: {original_error}")]
    NoFallbackAvailable {
        /// Description of the original error.
        original_error: String,
    },
}

impl GupError {
    /// Create a new GPU initialization error.
    pub fn gpu_initialization_failed(reason: impl Into<String>) -> Self {
        Self::GpuInitializationError {
            reason: reason.into(),
        }
    }

    /// Create a new shader compilation error.
    pub fn shader_compilation_failed(
        shader_type: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self::ShaderCompilationError {
            shader_type: shader_type.into(),
            error: error.into(),
        }
    }

    /// Create a new GPU memory exhausted error.
    pub fn gpu_memory_exhausted(requested: usize, available: usize) -> Self {
        Self::GpuMemoryExhausted {
            requested,
            available,
        }
    }

    /// Create a new data validation error.
    pub fn data_validation_failed(validation_error: impl Into<String>) -> Self {
        Self::DataValidationError {
            validation_error: validation_error.into(),
        }
    }

    /// Create a new performance target missed error.
    pub fn performance_target_missed(target_ms: f64, actual_ms: f64) -> Self {
        Self::PerformanceTargetMissed {
            target_ms,
            actual_ms,
        }
    }

    /// Create a new platform not supported error.
    pub fn platform_not_supported(platform: impl Into<String>, feature: impl Into<String>) -> Self {
        Self::PlatformNotSupported {
            platform: platform.into(),
            feature: feature.into(),
        }
    }

    /// Get the error category for error handling strategies.
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::GpuInitializationError { .. }
            | Self::GpuResourceCreationError { .. }
            | Self::WebGpuError { .. } => ErrorCategory::GpuInitialization,

            Self::ShaderCompilationError { .. } | Self::ShaderError { .. } => {
                ErrorCategory::ShaderCompilation
            }

            Self::GpuMemoryExhausted { .. }
            | Self::ResourceLimitExceeded { .. }
            | Self::StreamBufferOverflow { .. }
            | Self::ResourceError { .. } => ErrorCategory::ResourceExhaustion,

            Self::InvalidDataFormat { .. }
            | Self::TypeMismatch { .. }
            | Self::DataValidationError { .. }
            | Self::BufferSizeMismatch { .. }
            | Self::ValidationError { .. } => ErrorCategory::DataValidation,

            Self::PerformanceTargetMissed { .. } => ErrorCategory::Performance,

            Self::PlatformNotSupported { .. }
            | Self::FeatureNotSupported { .. }
            | Self::WebGpuNotAvailable { .. } => ErrorCategory::PlatformCompatibility,

            Self::NetworkError { .. } | Self::FileError { .. } => ErrorCategory::IO,

            Self::ConfigurationError { .. } | Self::SystemResourceUnavailable { .. } => {
                ErrorCategory::Configuration
            }

            Self::RenderError { .. } | Self::CompositionError { .. } => ErrorCategory::Rendering,

            Self::BufferError { .. } => ErrorCategory::BufferManagement,

            Self::InvalidOperation { .. } => ErrorCategory::InvalidOperation,

            Self::FallbackAlreadyActive { .. }
            | Self::RecoveryFailed { .. }
            | Self::NoFallbackAvailable { .. } => ErrorCategory::Recovery,
        }
    }

    /// Get the severity level of this error.
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            Self::GpuInitializationError { .. }
            | Self::SystemResourceUnavailable { .. }
            | Self::WebGpuNotAvailable { .. } => ErrorSeverity::Critical,

            Self::GpuMemoryExhausted { .. }
            | Self::ResourceLimitExceeded { .. }
            | Self::ShaderCompilationError { .. } => ErrorSeverity::High,

            Self::PerformanceTargetMissed { .. }
            | Self::StreamBufferOverflow { .. }
            | Self::DataValidationError { .. } => ErrorSeverity::Medium,

            Self::InvalidDataFormat { .. }
            | Self::TypeMismatch { .. }
            | Self::NetworkError { .. } => ErrorSeverity::Low,

            _ => ErrorSeverity::Medium,
        }
    }

    /// Check if this error is recoverable through automatic means.
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::GpuMemoryExhausted { .. }
            | Self::ShaderCompilationError { .. }
            | Self::StreamBufferOverflow { .. }
            | Self::PerformanceTargetMissed { .. }
            | Self::ResourceLimitExceeded { .. } => true,

            Self::GpuInitializationError { .. }
            | Self::SystemResourceUnavailable { .. }
            | Self::PlatformNotSupported { .. }
            | Self::WebGpuNotAvailable { .. } => false,

            _ => false,
        }
    }

    /// Fast error classification for hot paths (const fn where possible).
    ///
    /// This method provides a fast path for error classification that can be
    /// optimized by the compiler. Use this in performance-critical code instead
    /// of the full `category()` method.
    pub const fn category_fast(&self) -> ErrorCategory {
        match self {
            Self::GpuInitializationError { .. }
            | Self::GpuResourceCreationError { .. }
            | Self::WebGpuError { .. } => ErrorCategory::GpuInitialization,

            Self::ShaderCompilationError { .. } | Self::ShaderError { .. } => {
                ErrorCategory::ShaderCompilation
            }

            Self::GpuMemoryExhausted { .. }
            | Self::ResourceLimitExceeded { .. }
            | Self::StreamBufferOverflow { .. }
            | Self::ResourceError { .. } => ErrorCategory::ResourceExhaustion,

            Self::InvalidDataFormat { .. }
            | Self::TypeMismatch { .. }
            | Self::DataValidationError { .. }
            | Self::BufferSizeMismatch { .. }
            | Self::ValidationError { .. } => ErrorCategory::DataValidation,

            Self::PerformanceTargetMissed { .. } => ErrorCategory::Performance,

            Self::PlatformNotSupported { .. }
            | Self::FeatureNotSupported { .. }
            | Self::WebGpuNotAvailable { .. } => ErrorCategory::PlatformCompatibility,

            Self::NetworkError { .. } | Self::FileError { .. } => ErrorCategory::IO,

            Self::ConfigurationError { .. } | Self::SystemResourceUnavailable { .. } => {
                ErrorCategory::Configuration
            }

            Self::RenderError { .. } | Self::CompositionError { .. } => ErrorCategory::Rendering,

            Self::BufferError { .. } => ErrorCategory::BufferManagement,

            Self::InvalidOperation { .. } => ErrorCategory::InvalidOperation,

            Self::FallbackAlreadyActive { .. }
            | Self::RecoveryFailed { .. }
            | Self::NoFallbackAvailable { .. } => ErrorCategory::Recovery,
        }
    }

    /// Whether this error needs full context creation with system information.
    ///
    /// Returns `true` for critical errors that benefit from detailed diagnostics,
    /// `false` for frequent, low-priority errors where context creation overhead
    /// is not justified.
    pub const fn needs_full_context(&self) -> bool {
        match self {
            // Critical errors need full context
            Self::GpuInitializationError { .. }
            | Self::SystemResourceUnavailable { .. }
            | Self::WebGpuNotAvailable { .. }
            | Self::GpuMemoryExhausted { .. }
            | Self::ShaderCompilationError { .. } => true,

            // Frequent, low-priority errors don't need full context
            Self::PerformanceTargetMissed { .. }
            | Self::DataValidationError { .. }
            | Self::InvalidDataFormat { .. } => false,

            // Medium priority errors - context may be useful
            _ => true,
        }
    }

    /// Whether this error is likely to occur frequently in hot paths.
    ///
    /// This helps determine if the error should use lazy context creation
    /// or other performance optimizations.
    pub const fn is_hot_path_error(&self) -> bool {
        matches!(
            self,
            Self::PerformanceTargetMissed { .. }
                | Self::DataValidationError { .. }
                | Self::InvalidDataFormat { .. }
                | Self::BufferSizeMismatch { .. }
        )
    }
}

/// Error categories for different handling strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// GPU device or adapter initialization errors.
    GpuInitialization,
    /// Shader compilation or linking errors.
    ShaderCompilation,
    /// Memory or resource exhaustion errors.
    ResourceExhaustion,
    /// Data format or validation errors.
    DataValidation,
    /// Performance target or frame-time errors.
    Performance,
    /// Platform or backend compatibility errors.
    PlatformCompatibility,
    /// Network or file I/O errors.
    IO,
    /// Configuration or system setup errors.
    Configuration,
    /// Rendering pipeline errors.
    Rendering,
    /// Buffer creation or management errors.
    BufferManagement,
    /// Invalid operation or usage errors.
    InvalidOperation,
    /// Recovery and fallback mechanism errors.
    Recovery,
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GpuInitialization => write!(f, "GPU Initialization"),
            Self::ShaderCompilation => write!(f, "Shader Compilation"),
            Self::ResourceExhaustion => write!(f, "Resource Exhaustion"),
            Self::DataValidation => write!(f, "Data Validation"),
            Self::Performance => write!(f, "Performance"),
            Self::PlatformCompatibility => write!(f, "Platform Compatibility"),
            Self::IO => write!(f, "I/O"),
            Self::Configuration => write!(f, "Configuration"),
            Self::Rendering => write!(f, "Rendering"),
            Self::BufferManagement => write!(f, "Buffer Management"),
            Self::InvalidOperation => write!(f, "Invalid Operation"),
            Self::Recovery => write!(f, "Recovery"),
        }
    }
}

/// Error severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Minor issue with minimal impact.
    Low,
    /// Moderate issue that may affect functionality.
    Medium,
    /// Significant issue requiring attention.
    High,
    /// Critical failure requiring immediate action.
    Critical,
}

/// Result type alias for Gup operations.
pub type GupResult<T> = Result<T, GupError>;

/// Convert from standard I/O errors.
impl From<std::io::Error> for GupError {
    fn from(error: std::io::Error) -> Self {
        Self::FileError {
            path: "unknown".to_string(),
            error: error.to_string(),
        }
    }
}

/// Convert from serde JSON errors.
impl From<serde_json::Error> for GupError {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidDataFormat {
            message: error.to_string(),
        }
    }
}

/// Backward compatibility constructors for legacy error types.
impl GupError {
    /// Create a legacy render error.
    pub fn render_error(message: impl Into<String>) -> Self {
        Self::RenderError {
            message: message.into(),
        }
    }

    /// Create a legacy composition error.
    pub fn composition_error(message: impl Into<String>) -> Self {
        Self::CompositionError {
            message: message.into(),
        }
    }

    /// Create a legacy resource error.
    pub fn resource_error(message: impl Into<String>) -> Self {
        Self::ResourceError {
            message: message.into(),
        }
    }

    /// Create a legacy invalid operation error.
    pub fn invalid_operation(message: impl Into<String>) -> Self {
        Self::InvalidOperation {
            message: message.into(),
        }
    }

    /// Create a legacy WebGPU error.
    pub fn webgpu_error(message: impl Into<String>) -> Self {
        Self::WebGpuError {
            message: message.into(),
        }
    }

    /// Create a legacy buffer error.
    pub fn buffer_error(message: impl Into<String>) -> Self {
        Self::BufferError {
            message: message.into(),
        }
    }

    /// Create a legacy validation error.
    pub fn validation_error(message: impl Into<String>) -> Self {
        Self::ValidationError {
            message: message.into(),
        }
    }

    /// Create a legacy shader error.
    pub fn shader_error(message: impl Into<String>) -> Self {
        Self::ShaderError {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categorization() {
        let gpu_error = GupError::gpu_initialization_failed("Mock GPU failure");
        assert_eq!(gpu_error.category(), ErrorCategory::GpuInitialization);
        assert_eq!(gpu_error.severity(), ErrorSeverity::Critical);
        assert!(!gpu_error.is_recoverable());

        let memory_error = GupError::gpu_memory_exhausted(2048, 1024);
        assert_eq!(memory_error.category(), ErrorCategory::ResourceExhaustion);
        assert_eq!(memory_error.severity(), ErrorSeverity::High);
        assert!(memory_error.is_recoverable());
    }

    #[test]
    fn test_error_serialization() {
        let error = GupError::shader_compilation_failed("vertex", "syntax error");
        let serialized = serde_json::to_string(&error).unwrap();
        let deserialized: GupError = serde_json::from_str(&serialized).unwrap();

        match deserialized {
            GupError::ShaderCompilationError { shader_type, error } => {
                assert_eq!(shader_type, "vertex");
                assert_eq!(error, "syntax error");
            }
            _ => panic!("Deserialization failed"),
        }
    }

    #[test]
    fn test_backward_compatibility() {
        let legacy_error = GupError::render_error("Legacy render failure");
        assert_eq!(legacy_error.category(), ErrorCategory::Rendering);

        let composition_error = GupError::composition_error("Composition failed");
        assert_eq!(composition_error.category(), ErrorCategory::Rendering);
    }

    #[test]
    fn test_error_display() {
        let error = GupError::gpu_memory_exhausted(2048, 1024);
        let display_string = format!("{error}");
        assert!(display_string.contains("GPU memory exhausted"));
        assert!(display_string.contains("2048"));
        assert!(display_string.contains("1024"));
    }
}
