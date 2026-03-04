// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fallback management system for automatic error recovery.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::{ErrorCategory, ErrorContext, GupError, GupResult};

/// Manages fallback strategies for different error categories.
#[derive(Debug)]
pub struct FallbackManager {
    fallback_strategies: HashMap<ErrorCategory, Vec<FallbackStrategy>>,
    current_fallbacks: HashSet<FallbackType>,
    performance_monitor: PerformanceMonitor,
    config: FallbackConfig,
}

/// Configuration for fallback behavior.
#[derive(Debug, Clone)]
pub struct FallbackConfig {
    /// Whether automatic fallback activation is enabled.
    pub enable_automatic_fallbacks: bool,
    /// Maximum number of fallback attempts per error.
    pub max_fallback_attempts: usize,
    /// Timeout in seconds for a single fallback attempt.
    pub fallback_timeout_seconds: f64,
    /// Performance ratio threshold below which fallbacks are triggered.
    pub performance_threshold: f64,
}

/// Different types of fallback strategies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackStrategy {
    /// Fall back from GPU to CPU rendering.
    GpuToCpu,
    /// Fall back from WebGPU to WebGL rendering.
    WebGpuToWebGl,
    /// Reduce rendering quality from high to low.
    HighQualityToLowQuality,
    /// Simplify complex visualisation to a simpler form.
    ComplexToSimple,
    /// A named custom fallback strategy.
    CustomFallback(String),
}

/// Active fallback types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FallbackType {
    /// CPU-based software rendering is active.
    CpuRendering,
    /// WebGL rendering backend is active.
    WebGlRendering,
    /// Low-quality rendering mode is active.
    LowQuality,
    /// Simplified shader pipeline is active.
    SimpleShaders,
    /// A custom fallback identified by a numeric tag.
    Custom(u32),
}

/// Result of a fallback operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryResult {
    /// Type of recovery that was performed.
    pub recovery_type: RecoveryType,
    /// Human-readable description of the recovery outcome.
    pub message: String,
    /// Performance impact of the recovery, if measurable.
    pub performance_impact: Option<PerformanceImpact>,
    /// Whether the recovery was successful.
    pub success: bool,
    /// Fallback type that is now active, if any.
    pub fallback_active: Option<FallbackType>,
}

/// Types of recovery operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryType {
    /// Recovery through a fallback strategy.
    Fallback,
    /// Recovery through repairing the existing state.
    Repair,
    /// Recovery through restarting a component.
    Restart,
    /// Recovery requiring manual intervention.
    Manual,
}

/// Performance impact information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceImpact {
    /// Expected slowdown factor (values > 1.0 mean slower).
    pub expected_slowdown: f64,
    /// Memory overhead change as a percentage (negative means savings).
    pub memory_overhead: f64,
    /// Quality reduction as a fraction (0.0 to 1.0).
    pub quality_reduction: f32,
}

/// Performance monitoring for fallback decisions.
#[derive(Debug)]
pub struct PerformanceMonitor {
    baseline_fps: f64,
    current_fps: f64,
    frame_time_history: Vec<f64>,
    gpu_memory_usage: f64,
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enable_automatic_fallbacks: true,
            max_fallback_attempts: 3,
            fallback_timeout_seconds: 5.0,
            performance_threshold: 0.5, // 50% of baseline performance
        }
    }
}

impl FallbackManager {
    /// Create a new fallback manager with default strategies.
    pub fn new() -> Self {
        let mut manager = Self {
            fallback_strategies: HashMap::new(),
            current_fallbacks: HashSet::new(),
            performance_monitor: PerformanceMonitor::new(),
            config: FallbackConfig::default(),
        };

        manager.initialize_default_strategies();
        manager
    }

    /// Create a fallback manager with custom configuration.
    pub fn with_config(config: FallbackConfig) -> Self {
        let mut manager = Self {
            fallback_strategies: HashMap::new(),
            current_fallbacks: HashSet::new(),
            performance_monitor: PerformanceMonitor::new(),
            config,
        };

        manager.initialize_default_strategies();
        manager
    }

    /// Handle an error with appropriate fallback strategy.
    pub async fn handle_error(&mut self, error_context: ErrorContext) -> GupResult<RecoveryResult> {
        if !self.config.enable_automatic_fallbacks {
            return Err(error_context.error);
        }

        let error_category = error_context.error.category();

        if let Some(strategies) = self.fallback_strategies.get(&error_category).cloned() {
            for strategy in &strategies {
                match self.attempt_fallback(strategy, &error_context).await {
                    Ok(result) => {
                        log::info!("Successfully recovered using fallback: {strategy:?}");
                        return Ok(result);
                    }
                    Err(fallback_error) => {
                        log::warn!("Fallback {strategy:?} failed: {fallback_error}");
                        continue;
                    }
                }
            }
        }

        Err(error_context.error)
    }

    /// Add a custom fallback strategy for an error category.
    pub fn add_fallback_strategy(&mut self, category: ErrorCategory, strategy: FallbackStrategy) {
        self.fallback_strategies
            .entry(category)
            .or_default()
            .push(strategy);
    }

    /// Check if a fallback type is currently active.
    pub fn is_fallback_active(&self, fallback_type: FallbackType) -> bool {
        self.current_fallbacks.contains(&fallback_type)
    }

    /// Get current performance metrics.
    pub fn performance_metrics(&self) -> &PerformanceMonitor {
        &self.performance_monitor
    }

    /// Reset all active fallbacks.
    pub fn reset_fallbacks(&mut self) -> GupResult<()> {
        self.current_fallbacks.clear();
        log::info!("All fallbacks have been reset");
        Ok(())
    }

    async fn attempt_fallback(
        &mut self,
        strategy: &FallbackStrategy,
        context: &ErrorContext,
    ) -> GupResult<RecoveryResult> {
        match strategy {
            FallbackStrategy::GpuToCpu => self.enable_cpu_rendering_internal().await,
            FallbackStrategy::WebGpuToWebGl => self.enable_webgl_fallback().await,
            FallbackStrategy::HighQualityToLowQuality => self.reduce_rendering_quality().await,
            FallbackStrategy::ComplexToSimple => self.simplify_visualization().await,
            FallbackStrategy::CustomFallback(name) => {
                self.handle_custom_fallback(name, context).await
            }
        }
    }

    /// Enable CPU rendering as a fallback.
    pub async fn enable_cpu_rendering(&mut self) -> GupResult<RecoveryResult> {
        self.enable_cpu_rendering_internal().await
    }

    async fn enable_cpu_rendering_internal(&mut self) -> GupResult<RecoveryResult> {
        if self.current_fallbacks.contains(&FallbackType::CpuRendering) {
            return Err(GupError::FallbackAlreadyActive {
                fallback_type: "CpuRendering".to_string(),
            });
        }

        // Initialize CPU renderer (placeholder implementation)
        log::info!("Enabling CPU rendering fallback");

        self.current_fallbacks.insert(FallbackType::CpuRendering);

        Ok(RecoveryResult {
            recovery_type: RecoveryType::Fallback,
            message: "Switched to CPU rendering for compatibility".to_string(),
            performance_impact: Some(PerformanceImpact {
                expected_slowdown: 10.0, // 10x slower than GPU
                memory_overhead: 50.0,   // 50% more memory usage
                quality_reduction: 0.0,  // No quality reduction
            }),
            success: true,
            fallback_active: Some(FallbackType::CpuRendering),
        })
    }

    async fn enable_webgl_fallback(&mut self) -> GupResult<RecoveryResult> {
        if self
            .current_fallbacks
            .contains(&FallbackType::WebGlRendering)
        {
            return Err(GupError::FallbackAlreadyActive {
                fallback_type: "WebGlRendering".to_string(),
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            log::info!("Enabling WebGL rendering fallback");
            self.current_fallbacks.insert(FallbackType::WebGlRendering);

            Ok(RecoveryResult {
                recovery_type: RecoveryType::Fallback,
                message: "Switched to WebGL rendering backend".to_string(),
                performance_impact: Some(PerformanceImpact {
                    expected_slowdown: 2.0, // 2x slower than WebGPU
                    memory_overhead: 20.0,  // 20% more memory usage
                    quality_reduction: 0.1, // Slight quality reduction
                }),
                success: true,
                fallback_active: Some(FallbackType::WebGlRendering),
            })
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            Err(GupError::platform_not_supported("native", "WebGL fallback"))
        }
    }

    async fn reduce_rendering_quality(&mut self) -> GupResult<RecoveryResult> {
        log::info!("Reducing rendering quality for performance");
        self.current_fallbacks.insert(FallbackType::LowQuality);

        Ok(RecoveryResult {
            recovery_type: RecoveryType::Fallback,
            message: "Reduced rendering quality to improve performance".to_string(),
            performance_impact: Some(PerformanceImpact {
                expected_slowdown: 0.5, // 2x faster
                memory_overhead: -30.0, // 30% less memory usage
                quality_reduction: 0.3, // 30% quality reduction
            }),
            success: true,
            fallback_active: Some(FallbackType::LowQuality),
        })
    }

    async fn simplify_visualization(&mut self) -> GupResult<RecoveryResult> {
        log::info!("Simplifying visualization complexity");
        self.current_fallbacks.insert(FallbackType::SimpleShaders);

        Ok(RecoveryResult {
            recovery_type: RecoveryType::Fallback,
            message: "Simplified visualization for better compatibility".to_string(),
            performance_impact: Some(PerformanceImpact {
                expected_slowdown: 0.3, // 3x faster
                memory_overhead: -40.0, // 40% less memory usage
                quality_reduction: 0.2, // 20% quality reduction
            }),
            success: true,
            fallback_active: Some(FallbackType::SimpleShaders),
        })
    }

    async fn handle_custom_fallback(
        &mut self,
        fallback_name: &str,
        _context: &ErrorContext,
    ) -> GupResult<RecoveryResult> {
        log::info!("Attempting custom fallback: {fallback_name}");

        // Custom fallback implementation would go here
        match fallback_name {
            "emergency_cleanup" => Ok(RecoveryResult {
                recovery_type: RecoveryType::Repair,
                message: "Performed emergency resource cleanup".to_string(),
                performance_impact: None,
                success: true,
                fallback_active: None,
            }),
            _ => Err(GupError::NoFallbackAvailable {
                original_error: format!("Unknown custom fallback: {fallback_name}"),
            }),
        }
    }

    fn initialize_default_strategies(&mut self) {
        // GPU initialization errors
        self.fallback_strategies.insert(
            ErrorCategory::GpuInitialization,
            vec![FallbackStrategy::WebGpuToWebGl, FallbackStrategy::GpuToCpu],
        );

        // Resource exhaustion errors
        self.fallback_strategies.insert(
            ErrorCategory::ResourceExhaustion,
            vec![
                FallbackStrategy::ComplexToSimple,
                FallbackStrategy::HighQualityToLowQuality,
                FallbackStrategy::GpuToCpu,
            ],
        );

        // Shader compilation errors
        self.fallback_strategies.insert(
            ErrorCategory::ShaderCompilation,
            vec![
                FallbackStrategy::ComplexToSimple,
                FallbackStrategy::GpuToCpu,
            ],
        );

        // Platform compatibility errors
        self.fallback_strategies.insert(
            ErrorCategory::PlatformCompatibility,
            vec![FallbackStrategy::WebGpuToWebGl, FallbackStrategy::GpuToCpu],
        );

        // Performance errors
        self.fallback_strategies.insert(
            ErrorCategory::Performance,
            vec![
                FallbackStrategy::HighQualityToLowQuality,
                FallbackStrategy::ComplexToSimple,
            ],
        );
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceMonitor {
    /// Create a new performance monitor.
    pub fn new() -> Self {
        Self {
            baseline_fps: 60.0,
            current_fps: 60.0,
            frame_time_history: Vec::with_capacity(60),
            gpu_memory_usage: 0.0,
        }
    }

    /// Update performance metrics.
    pub fn update(&mut self, fps: f64, frame_time: f64, memory_usage: f64) {
        self.current_fps = fps;
        self.gpu_memory_usage = memory_usage;

        self.frame_time_history.push(frame_time);
        if self.frame_time_history.len() > 60 {
            self.frame_time_history.remove(0);
        }
    }

    /// Check if performance is below acceptable threshold.
    pub fn is_performance_degraded(&self, threshold: f64) -> bool {
        self.current_fps < self.baseline_fps * threshold
    }

    /// Get average frame time over recent history.
    pub fn average_frame_time(&self) -> f64 {
        if self.frame_time_history.is_empty() {
            return 0.0;
        }

        self.frame_time_history.iter().sum::<f64>() / self.frame_time_history.len() as f64
    }

    /// Get current FPS.
    pub fn current_fps(&self) -> f64 {
        self.current_fps
    }

    /// Get GPU memory usage.
    pub fn gpu_memory_usage(&self) -> f64 {
        self.gpu_memory_usage
    }
}

impl Default for FallbackManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fallback_manager_creation() {
        let manager = FallbackManager::new();
        assert!(
            manager
                .fallback_strategies
                .contains_key(&ErrorCategory::GpuInitialization)
        );
        assert!(
            manager
                .fallback_strategies
                .contains_key(&ErrorCategory::ResourceExhaustion)
        );
    }

    #[tokio::test]
    async fn test_gpu_memory_exhaustion_fallback() {
        let mut manager = FallbackManager::new();
        let error = GupError::gpu_memory_exhausted(2048, 1024);
        let context = ErrorContext::new(error);

        let result = manager.handle_error(context).await;
        assert!(result.is_ok());

        let recovery_result = result.unwrap();
        assert_eq!(recovery_result.recovery_type, RecoveryType::Fallback);
        assert!(recovery_result.success);
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
    }

    #[test]
    fn test_performance_monitor() {
        let mut monitor = PerformanceMonitor::new();

        monitor.update(45.0, 22.2, 0.75);

        assert_eq!(monitor.current_fps(), 45.0);
        assert_eq!(monitor.gpu_memory_usage(), 0.75);
        assert!(monitor.is_performance_degraded(0.8)); // 45 < 60 * 0.8
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
            GupError::FallbackAlreadyActive { .. } => {}
            _ => panic!("Expected FallbackAlreadyActive error"),
        }
    }
}
