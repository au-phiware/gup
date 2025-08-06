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
    pub error: GupError,
    pub error_id: Uuid,
    pub timestamp: SystemTime,
    pub stack_trace: Vec<String>,
    pub system_info: SystemInfo,
    pub recovery_suggestions: Vec<RecoverySuggestion>,
    pub additional_context: HashMap<String, String>,
}

/// System information collected at error time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub gpu_info: GpuInfo,
    pub platform: Platform,
    pub memory_info: MemoryInfo,
    pub performance_state: PerformanceState,
}

/// GPU-specific information for error diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub adapter_name: String,
    pub backend: String,
    pub device_type: String,
    pub vendor_id: Option<u32>,
    pub device_id: Option<u32>,
    pub memory_size: Option<u64>,
    pub features: Vec<String>,
    pub limits: HashMap<String, u64>,
}

/// Platform-specific information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub os: String,
    pub arch: String,
    pub target: String,
    pub webgpu_available: bool,
    pub webgl_available: bool,
}

/// Memory usage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub total_system_memory: u64,
    pub available_system_memory: u64,
    pub gpu_memory_used: u64,
    pub gpu_memory_total: Option<u64>,
    pub buffer_count: usize,
    pub texture_count: usize,
}

/// Performance state at error time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceState {
    pub current_fps: f64,
    pub target_fps: f64,
    pub frame_time_ms: f64,
    pub cpu_usage_percent: f32,
    pub memory_pressure: MemoryPressure,
    pub recent_frame_times: Vec<f64>,
}

/// Memory pressure levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryPressure {
    Low,
    Moderate,
    High,
    Critical,
}

/// Recovery suggestion with action information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySuggestion {
    pub suggestion_type: SuggestionType,
    pub description: String,
    pub action: Option<RecoveryAction>,
    pub success_probability: f32,
    pub estimated_time: Duration,
    pub user_visible: bool,
}

/// Types of recovery suggestions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionType {
    AutomaticRecovery,
    UserAction,
    ConfigurationChange,
    SystemRequirement,
    Fallback,
    Restart,
}

/// Specific recovery actions that can be taken.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    ReduceBatchSize {
        factor: f32,
    },
    UseFallbackShader,
    SwitchToWebGL,
    EnableCpuFallback,
    ClearCache,
    ReduceQuality,
    RestartGpu,
    RestartApplication,
    Custom {
        action_name: String,
        parameters: HashMap<String, String>,
    },
}

/// Duration for recovery time estimates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Duration {
    pub seconds: f64,
}

impl Duration {
    pub fn new(seconds: f64) -> Self {
        Self { seconds }
    }

    pub fn milliseconds(ms: f64) -> Self {
        Self {
            seconds: ms / 1000.0,
        }
    }

    pub fn from_std(duration: std::time::Duration) -> Self {
        Self {
            seconds: duration.as_secs_f64(),
        }
    }

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
