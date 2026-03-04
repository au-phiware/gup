// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Error context system providing detailed error information and recovery suggestions.

use std::collections::HashMap;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::GupError;

/// Comprehensive error context with system information and recovery suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// The error that occurred.
    pub error: GupError,
    /// Unique identifier for this error instance.
    pub error_id: Uuid,
    /// Timestamp when the error occurred.
    pub timestamp: SystemTime,
    /// Captured stack trace at the point of error.
    pub stack_trace: Vec<String>,
    /// System information collected at error time.
    pub system_info: SystemInfo,
    /// Suggested recovery actions.
    pub recovery_suggestions: Vec<RecoverySuggestion>,
    /// Additional key-value context pairs.
    pub additional_context: HashMap<String, String>,
}

/// System information collected at error time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    /// GPU adapter and device information.
    pub gpu_info: GpuInfo,
    /// Platform detection results.
    pub platform: Platform,
    /// Memory usage statistics.
    pub memory_info: MemoryInfo,
    /// Performance state at error time.
    pub performance_state: PerformanceState,
}

/// GPU-specific information for error diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    /// Name of the GPU adapter.
    pub adapter_name: String,
    /// Graphics backend in use.
    pub backend: String,
    /// Type of GPU device.
    pub device_type: String,
    /// PCI vendor identifier, if available.
    pub vendor_id: Option<u32>,
    /// PCI device identifier, if available.
    pub device_id: Option<u32>,
    /// Total GPU memory in bytes, if available.
    pub memory_size: Option<u64>,
    /// Enabled GPU features.
    pub features: Vec<String>,
    /// Device limits as key-value pairs.
    pub limits: HashMap<String, u64>,
}

/// Platform-specific information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    /// Operating system name.
    pub os: String,
    /// CPU architecture.
    pub arch: String,
    /// Combined target triple string.
    pub target: String,
    /// Whether WebGPU is available.
    pub webgpu_available: bool,
    /// Whether WebGL is available.
    pub webgl_available: bool,
}

/// Memory usage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Total system memory in bytes.
    pub total_system_memory: u64,
    /// Available system memory in bytes.
    pub available_system_memory: u64,
    /// GPU memory currently in use in bytes.
    pub gpu_memory_used: u64,
    /// Total GPU memory in bytes, if known.
    pub gpu_memory_total: Option<u64>,
    /// Number of active GPU buffers.
    pub buffer_count: usize,
    /// Number of active GPU textures.
    pub texture_count: usize,
}

/// Performance state at error time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceState {
    /// Current frames per second.
    pub current_fps: f64,
    /// Target frames per second.
    pub target_fps: f64,
    /// Current frame time in milliseconds.
    pub frame_time_ms: f64,
    /// CPU usage as a percentage.
    pub cpu_usage_percent: f32,
    /// Current memory pressure level.
    pub memory_pressure: MemoryPressure,
    /// Recent frame times for trend analysis.
    pub recent_frame_times: Vec<f64>,
}

/// Memory pressure levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryPressure {
    /// Memory usage is low and healthy.
    Low,
    /// Memory usage is moderate.
    Moderate,
    /// Memory usage is high and may cause issues.
    High,
    /// Memory usage is critically high.
    Critical,
}

/// Recovery suggestion with action information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySuggestion {
    /// Category of the recovery suggestion.
    pub suggestion_type: SuggestionType,
    /// Human-readable description of the suggestion.
    pub description: String,
    /// Concrete recovery action, if applicable.
    pub action: Option<RecoveryAction>,
    /// Estimated probability of success (0.0 to 1.0).
    pub success_probability: f32,
    /// Estimated time to execute the recovery.
    pub estimated_time: Duration,
    /// Whether this suggestion should be shown to the user.
    pub user_visible: bool,
}

/// Types of recovery suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionType {
    /// Recovery can be performed automatically without user intervention.
    AutomaticRecovery,
    /// Recovery requires a manual action by the user.
    UserAction,
    /// Recovery requires a configuration change.
    ConfigurationChange,
    /// Recovery requires a system-level prerequisite.
    SystemRequirement,
    /// Recovery involves switching to a fallback mode.
    Fallback,
    /// Recovery requires restarting a component or the application.
    Restart,
}

/// Specific recovery actions that can be taken.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// Reduce the data batch size by the given factor.
    ReduceBatchSize {
        /// Multiplicative factor to apply to batch size.
        factor: f32,
    },
    /// Switch to a simpler fallback shader.
    UseFallbackShader,
    /// Switch rendering backend to WebGL.
    SwitchToWebGL,
    /// Enable CPU-based rendering fallback.
    EnableCpuFallback,
    /// Clear internal caches to free memory.
    ClearCache,
    /// Reduce rendering quality settings.
    ReduceQuality,
    /// Restart the GPU device.
    RestartGpu,
    /// Restart the entire application.
    RestartApplication,
    /// A user-defined recovery action.
    Custom {
        /// Name of the custom action.
        action_name: String,
        /// Parameters for the custom action.
        parameters: HashMap<String, String>,
    },
}

/// Duration for recovery time estimates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Duration {
    /// Duration value in seconds.
    pub seconds: f64,
}

impl Duration {
    /// Create a new duration from a value in seconds.
    pub fn new(seconds: f64) -> Self {
        Self { seconds }
    }

    /// Create a new duration from a value in milliseconds.
    pub fn milliseconds(ms: f64) -> Self {
        Self {
            seconds: ms / 1000.0,
        }
    }

    /// Create a duration from a standard library `Duration`.
    pub fn from_std(duration: std::time::Duration) -> Self {
        Self {
            seconds: duration.as_secs_f64(),
        }
    }

    /// Convert this duration to a standard library `Duration`.
    pub fn to_std(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(self.seconds)
    }
}

impl ErrorContext {
    /// Create a new error context with system information collection.
    pub fn new(error: GupError) -> Self {
        let mut context = Self {
            error_id: Uuid::new_v4(),
            timestamp: SystemTime::now(),
            stack_trace: Self::capture_stack_trace(),
            system_info: SystemInfo::collect(),
            recovery_suggestions: Vec::new(),
            additional_context: HashMap::new(),
            error: error.clone(),
        };

        context.recovery_suggestions = context.generate_recovery_suggestions(&error);
        context
    }

    /// Add additional context information.
    pub fn add_context(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.additional_context.insert(key.into(), value.into());
    }

    /// Get human-readable error summary.
    pub fn summary(&self) -> String {
        format!(
            "Error {} ({}): {}",
            self.error_id,
            self.error.category(),
            self.error
        )
    }

    /// Get the primary recovery suggestion if available.
    pub fn primary_recovery_suggestion(&self) -> Option<&RecoverySuggestion> {
        self.recovery_suggestions
            .iter()
            .filter(|s| s.suggestion_type == SuggestionType::AutomaticRecovery)
            .max_by(|a, b| {
                a.success_probability
                    .partial_cmp(&b.success_probability)
                    .unwrap()
            })
    }

    /// Check if automatic recovery is possible.
    pub fn can_auto_recover(&self) -> bool {
        self.recovery_suggestions.iter().any(|s| {
            s.suggestion_type == SuggestionType::AutomaticRecovery && s.success_probability > 0.5
        })
    }

    fn capture_stack_trace() -> Vec<String> {
        // In a real implementation, this would capture actual stack traces
        // For now, we provide placeholder functionality
        vec![
            "gup::error::ErrorContext::new".to_string(),
            "gup::render::render_frame".to_string(),
            "application::main_loop".to_string(),
        ]
    }

    fn generate_recovery_suggestions(&self, error: &GupError) -> Vec<RecoverySuggestion> {
        match error {
            GupError::GpuMemoryExhausted {
                requested,
                available,
            } => {
                let reduction_factor = (*available as f32) / (*requested as f32);
                vec![
                    RecoverySuggestion {
                        suggestion_type: SuggestionType::AutomaticRecovery,
                        description: "Reduce data batch size and retry".to_string(),
                        action: Some(RecoveryAction::ReduceBatchSize {
                            factor: reduction_factor.max(0.5),
                        }),
                        success_probability: 0.8,
                        estimated_time: Duration::milliseconds(100.0),
                        user_visible: false,
                    },
                    RecoverySuggestion {
                        suggestion_type: SuggestionType::UserAction,
                        description: "Close other GPU-intensive applications".to_string(),
                        action: None,
                        success_probability: 0.6,
                        estimated_time: Duration::new(30.0),
                        user_visible: true,
                    },
                    RecoverySuggestion {
                        suggestion_type: SuggestionType::Fallback,
                        description: "Switch to CPU rendering".to_string(),
                        action: Some(RecoveryAction::EnableCpuFallback),
                        success_probability: 0.9,
                        estimated_time: Duration::new(1.0),
                        user_visible: true,
                    },
                ]
            }
            GupError::ShaderCompilationError { .. } => {
                vec![RecoverySuggestion {
                    suggestion_type: SuggestionType::AutomaticRecovery,
                    description: "Fall back to simpler shader implementation".to_string(),
                    action: Some(RecoveryAction::UseFallbackShader),
                    success_probability: 0.9,
                    estimated_time: Duration::milliseconds(50.0),
                    user_visible: false,
                }]
            }
            GupError::WebGpuNotAvailable { .. } => {
                vec![RecoverySuggestion {
                    suggestion_type: SuggestionType::Fallback,
                    description: "Switch to WebGL rendering backend".to_string(),
                    action: Some(RecoveryAction::SwitchToWebGL),
                    success_probability: 0.7,
                    estimated_time: Duration::new(2.0),
                    user_visible: true,
                }]
            }
            GupError::PerformanceTargetMissed { .. } => {
                vec![RecoverySuggestion {
                    suggestion_type: SuggestionType::AutomaticRecovery,
                    description: "Reduce rendering quality to improve performance".to_string(),
                    action: Some(RecoveryAction::ReduceQuality),
                    success_probability: 0.8,
                    estimated_time: Duration::milliseconds(10.0),
                    user_visible: false,
                }]
            }
            GupError::StreamBufferOverflow { .. } => {
                vec![RecoverySuggestion {
                    suggestion_type: SuggestionType::AutomaticRecovery,
                    description: "Clear buffer cache and reduce batch size".to_string(),
                    action: Some(RecoveryAction::ClearCache),
                    success_probability: 0.85,
                    estimated_time: Duration::milliseconds(200.0),
                    user_visible: false,
                }]
            }
            _ => Vec::new(),
        }
    }
}

impl SystemInfo {
    /// Collect current system information.
    pub fn collect() -> Self {
        Self {
            gpu_info: GpuInfo::collect(),
            platform: Platform::detect(),
            memory_info: MemoryInfo::collect(),
            performance_state: PerformanceState::current(),
        }
    }
}

impl GpuInfo {
    /// Collect GPU information from the current context.
    pub fn collect() -> Self {
        // In a real implementation, this would query the actual GPU adapter
        Self {
            adapter_name: "Mock GPU Adapter".to_string(),
            backend: "Vulkan".to_string(),
            device_type: "DiscreteGpu".to_string(),
            vendor_id: Some(0x10DE), // NVIDIA
            device_id: Some(0x1234),
            memory_size: Some(8_000_000_000), // 8GB
            features: vec![
                "TIMESTAMP_QUERY".to_string(),
                "PIPELINE_STATISTICS_QUERY".to_string(),
            ],
            limits: [
                ("max_texture_dimension_2d".to_string(), 8192),
                ("max_buffer_size".to_string(), 268_435_456),
            ]
            .into_iter()
            .collect(),
        }
    }
}

impl Platform {
    /// Detect current platform information.
    pub fn detect() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            target: format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
            webgpu_available: !cfg!(target_arch = "wasm32"),
            webgl_available: cfg!(target_arch = "wasm32"),
        }
    }
}

impl MemoryInfo {
    /// Collect current memory usage information.
    pub fn collect() -> Self {
        // In a real implementation, this would query actual memory usage
        Self {
            total_system_memory: 16_000_000_000,    // 16GB
            available_system_memory: 8_000_000_000, // 8GB available
            gpu_memory_used: 1_000_000_000,         // 1GB used
            gpu_memory_total: Some(8_000_000_000),  // 8GB total
            buffer_count: 150,
            texture_count: 25,
        }
    }

    /// Calculate memory pressure level.
    pub fn pressure_level(&self) -> MemoryPressure {
        let usage_ratio = self.gpu_memory_used as f32
            / self.gpu_memory_total.unwrap_or(self.gpu_memory_used) as f32;

        if usage_ratio > 0.9 {
            MemoryPressure::Critical
        } else if usage_ratio > 0.75 {
            MemoryPressure::High
        } else if usage_ratio > 0.5 {
            MemoryPressure::Moderate
        } else {
            MemoryPressure::Low
        }
    }
}

impl PerformanceState {
    /// Collect current performance metrics.
    pub fn current() -> Self {
        Self {
            current_fps: 60.0,
            target_fps: 60.0,
            frame_time_ms: 16.67, // ~60 FPS
            cpu_usage_percent: 25.0,
            memory_pressure: MemoryPressure::Low,
            recent_frame_times: vec![16.5, 16.8, 16.2, 17.1, 16.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_context_creation() {
        let error = GupError::gpu_memory_exhausted(2048, 1024);
        let context = ErrorContext::new(error);

        assert_eq!(
            context.error.category(),
            crate::ErrorCategory::ResourceExhaustion
        );
        assert!(!context.recovery_suggestions.is_empty());
        assert!(context.can_auto_recover());
    }

    #[test]
    fn test_recovery_suggestions() {
        let error = GupError::shader_compilation_failed("vertex", "syntax error");
        let context = ErrorContext::new(error);

        let primary = context.primary_recovery_suggestion();
        assert!(primary.is_some());

        let suggestion = primary.unwrap();
        assert_eq!(
            suggestion.suggestion_type,
            SuggestionType::AutomaticRecovery
        );
        assert!(suggestion.success_probability > 0.8);
    }

    #[test]
    fn test_memory_pressure() {
        let mut memory_info = MemoryInfo::collect();

        // Test High pressure (75% < usage <= 90%)
        memory_info.gpu_memory_used = 6_500_000_000; // 81.25% usage
        memory_info.gpu_memory_total = Some(8_000_000_000);
        assert_eq!(memory_info.pressure_level(), MemoryPressure::High);

        // Test Critical pressure (usage > 90%)
        memory_info.gpu_memory_used = 7_500_000_000; // 93.75% usage
        assert_eq!(memory_info.pressure_level(), MemoryPressure::Critical);
    }

    #[test]
    fn test_context_serialization() {
        let error = GupError::gpu_initialization_failed("Test error");
        let context = ErrorContext::new(error);

        let serialized = serde_json::to_string(&context).unwrap();
        let deserialized: ErrorContext = serde_json::from_str(&serialized).unwrap();

        assert_eq!(context.error_id, deserialized.error_id);
    }
}
