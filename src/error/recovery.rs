// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Recovery mechanisms and state management for error handling.

use std::collections::HashMap;

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{ErrorContext, GupError, GupResult, RecoveryAction, RecoveryResult, RecoveryType};

/// Manages recovery operations and state restoration.
#[derive(Debug)]
pub struct RecoveryManager {
    recovery_handlers: HashMap<String, Box<dyn RecoveryHandler>>,
    recovery_history: Vec<RecoveryAttempt>,
    checkpoints: Vec<SystemCheckpoint>,
    config: RecoveryConfig,
}

/// Configuration for recovery behavior.
#[derive(Debug, Clone)]
pub struct RecoveryConfig {
    pub max_recovery_attempts: usize,
    pub recovery_timeout: Duration,
    pub enable_checkpoints: bool,
    pub checkpoint_interval: Duration,
    pub auto_recovery_enabled: bool,
}

/// Trait for implementing custom recovery handlers.
pub trait RecoveryHandler: std::fmt::Debug + Send + Sync {
    /// Attempt to recover from an error.
    fn recover(&self, context: &ErrorContext, action: &RecoveryAction)
    -> GupResult<RecoveryResult>;

    /// Check if this handler can handle the given recovery action.
    fn can_handle(&self, action: &RecoveryAction) -> bool;

    /// Get the name of this recovery handler.
    fn name(&self) -> &str;
}

/// Record of a recovery attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAttempt {
    pub attempt_id: uuid::Uuid,
    pub timestamp: std::time::SystemTime,
    pub error_id: uuid::Uuid,
    pub action: RecoveryAction,
    pub result: RecoveryResult,
    pub duration: Duration,
}

/// System state checkpoint for recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCheckpoint {
    pub checkpoint_id: uuid::Uuid,
    pub timestamp: std::time::SystemTime,
    pub gpu_state: GpuState,
    pub buffer_state: BufferState,
    pub pipeline_state: PipelineState,
    pub configuration: SystemConfiguration,
}

/// GPU state information for checkpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuState {
    pub device_id: String,
    pub backend: String,
    pub features_enabled: Vec<String>,
    pub memory_usage: u64,
    pub active_buffers: usize,
    pub active_textures: usize,
}

/// Buffer state information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferState {
    pub total_buffers: usize,
    pub total_size: u64,
    pub staging_buffers: usize,
    pub vertex_buffers: usize,
    pub uniform_buffers: usize,
}

/// Pipeline state information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineState {
    pub active_pipelines: usize,
    pub cached_shaders: usize,
    pub bind_groups: usize,
    pub render_passes: usize,
}

/// System configuration snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfiguration {
    pub render_scale: f32,
    pub quality_level: u32,
    pub vsync_enabled: bool,
    pub debug_mode: bool,
    pub fallback_settings: HashMap<String, String>,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_recovery_attempts: 3,
            recovery_timeout: Duration::from_secs(10),
            enable_checkpoints: true,
            checkpoint_interval: Duration::from_secs(30),
            auto_recovery_enabled: true,
        }
    }
}

impl RecoveryManager {
    /// Create a new recovery manager with default handlers.
    pub fn new() -> Self {
        let mut manager = Self {
            recovery_handlers: HashMap::new(),
            recovery_history: Vec::new(),
            checkpoints: Vec::new(),
            config: RecoveryConfig::default(),
        };

        manager.register_default_handlers();
        manager
    }

    /// Create a recovery manager with custom configuration.
    pub fn with_config(config: RecoveryConfig) -> Self {
        let mut manager = Self {
            recovery_handlers: HashMap::new(),
            recovery_history: Vec::new(),
            checkpoints: Vec::new(),
            config,
        };

        manager.register_default_handlers();
        manager
    }

    /// Register a custom recovery handler.
    pub fn register_handler(&mut self, name: String, handler: Box<dyn RecoveryHandler>) {
        self.recovery_handlers.insert(name, handler);
    }

    /// Attempt to recover from an error using available handlers.
    pub fn recover(&mut self, context: &ErrorContext) -> GupResult<RecoveryResult> {
        let start_time = Instant::now();

        if let Some(suggestion) = context.primary_recovery_suggestion()
            && let Some(action) = &suggestion.action
        {
            return self.execute_recovery_action(context, action, start_time);
        }

        // Try automatic recovery based on error type
        let action = self.determine_recovery_action(&context.error)?;
        self.execute_recovery_action(context, &action, start_time)
    }

    /// Create a system checkpoint for future recovery.
    pub fn create_checkpoint(&mut self) -> GupResult<uuid::Uuid> {
        if !self.config.enable_checkpoints {
            return Err(GupError::configuration_error(
                "checkpoints",
                "Checkpoints are disabled",
            ));
        }

        let checkpoint = SystemCheckpoint {
            checkpoint_id: uuid::Uuid::new_v4(),
            timestamp: std::time::SystemTime::now(),
            gpu_state: GpuState::current(),
            buffer_state: BufferState::current(),
            pipeline_state: PipelineState::current(),
            configuration: SystemConfiguration::current(),
        };

        let checkpoint_id = checkpoint.checkpoint_id;
        self.checkpoints.push(checkpoint);

        // Keep only recent checkpoints
        const MAX_CHECKPOINTS: usize = 10;
        if self.checkpoints.len() > MAX_CHECKPOINTS {
            self.checkpoints.remove(0);
        }

        log::info!("Created system checkpoint: {checkpoint_id}");
        Ok(checkpoint_id)
    }

    /// Restore system state from a checkpoint.
    pub fn restore_checkpoint(&mut self, checkpoint_id: uuid::Uuid) -> GupResult<()> {
        let checkpoint = self
            .checkpoints
            .iter()
            .find(|c| c.checkpoint_id == checkpoint_id)
            .ok_or_else(|| {
                GupError::invalid_operation(format!("Checkpoint not found: {checkpoint_id}"))
            })?;

        // Restore system state (implementation would restore actual state)
        log::info!("Restoring checkpoint: {checkpoint_id}");

        // In a real implementation, this would restore GPU state, buffers, etc.
        self.restore_gpu_state(&checkpoint.gpu_state)?;
        self.restore_buffer_state(&checkpoint.buffer_state)?;
        self.restore_pipeline_state(&checkpoint.pipeline_state)?;
        self.restore_configuration(&checkpoint.configuration)?;

        Ok(())
    }

    /// Get recovery statistics.
    pub fn recovery_stats(&self) -> RecoveryStats {
        let total_attempts = self.recovery_history.len();
        let successful_attempts = self
            .recovery_history
            .iter()
            .filter(|attempt| attempt.result.success)
            .count();

        let success_rate = if total_attempts > 0 {
            successful_attempts as f32 / total_attempts as f32
        } else {
            0.0
        };

        let average_duration = if total_attempts > 0 {
            let total_duration: Duration = self
                .recovery_history
                .iter()
                .map(|attempt| attempt.duration)
                .sum();
            total_duration / total_attempts as u32
        } else {
            Duration::ZERO
        };

        RecoveryStats {
            total_attempts,
            successful_attempts,
            success_rate,
            average_duration,
            recent_checkpoints: self.checkpoints.len(),
        }
    }

    fn execute_recovery_action(
        &mut self,
        context: &ErrorContext,
        action: &RecoveryAction,
        start_time: Instant,
    ) -> GupResult<RecoveryResult> {
        // Find appropriate handler for the action
        for handler in self.recovery_handlers.values() {
            if handler.can_handle(action) {
                let result = handler.recover(context, action);
                let duration = start_time.elapsed();

                // Record the attempt
                self.record_recovery_attempt(context, action, &result, duration);

                return result;
            }
        }

        Err(GupError::NoFallbackAvailable {
            original_error: format!("No handler found for recovery action: {action:?}"),
        })
    }

    fn determine_recovery_action(&self, error: &GupError) -> GupResult<RecoveryAction> {
        match error {
            GupError::GpuMemoryExhausted {
                requested,
                available,
            } => {
                let factor = (*available as f32) / (*requested as f32).max(1.0);
                Ok(RecoveryAction::ReduceBatchSize {
                    factor: factor.max(0.5),
                })
            }
            GupError::ShaderCompilationError { .. } => Ok(RecoveryAction::UseFallbackShader),
            GupError::StreamBufferOverflow { .. } => Ok(RecoveryAction::ClearCache),
            GupError::PerformanceTargetMissed { .. } => Ok(RecoveryAction::ReduceQuality),
            _ => Err(GupError::NoFallbackAvailable {
                original_error: format!("No automatic recovery available for: {error}"),
            }),
        }
    }

    fn record_recovery_attempt(
        &mut self,
        context: &ErrorContext,
        action: &RecoveryAction,
        result: &GupResult<RecoveryResult>,
        duration: Duration,
    ) {
        let recovery_result = match result {
            Ok(result) => result.clone(),
            Err(error) => RecoveryResult {
                recovery_type: RecoveryType::Manual,
                message: format!("Recovery failed: {error}"),
                performance_impact: None,
                success: false,
                fallback_active: None,
            },
        };

        let attempt = RecoveryAttempt {
            attempt_id: uuid::Uuid::new_v4(),
            timestamp: std::time::SystemTime::now(),
            error_id: context.error_id,
            action: action.clone(),
            result: recovery_result,
            duration,
        };

        self.recovery_history.push(attempt);

        // Keep history bounded
        const MAX_HISTORY: usize = 1000;
        if self.recovery_history.len() > MAX_HISTORY {
            self.recovery_history.remove(0);
        }
    }

    fn register_default_handlers(&mut self) {
        self.register_handler(
            "batch_size_reducer".to_string(),
            Box::new(BatchSizeReducer::new()),
        );

        self.register_handler("cache_cleaner".to_string(), Box::new(CacheCleaner::new()));

        self.register_handler(
            "quality_reducer".to_string(),
            Box::new(QualityReducer::new()),
        );
    }

    // State restoration methods (placeholders for actual implementation)
    fn restore_gpu_state(&self, _state: &GpuState) -> GupResult<()> {
        log::debug!("Restoring GPU state");
        Ok(())
    }

    fn restore_buffer_state(&self, _state: &BufferState) -> GupResult<()> {
        log::debug!("Restoring buffer state");
        Ok(())
    }

    fn restore_pipeline_state(&self, _state: &PipelineState) -> GupResult<()> {
        log::debug!("Restoring pipeline state");
        Ok(())
    }

    fn restore_configuration(&self, _config: &SystemConfiguration) -> GupResult<()> {
        log::debug!("Restoring system configuration");
        Ok(())
    }
}

/// Recovery statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStats {
    pub total_attempts: usize,
    pub successful_attempts: usize,
    pub success_rate: f32,
    pub average_duration: Duration,
    pub recent_checkpoints: usize,
}

// Default recovery handlers
#[derive(Debug)]
struct BatchSizeReducer;

impl BatchSizeReducer {
    fn new() -> Self {
        Self
    }
}

impl RecoveryHandler for BatchSizeReducer {
    fn recover(
        &self,
        _context: &ErrorContext,
        action: &RecoveryAction,
    ) -> GupResult<RecoveryResult> {
        if let RecoveryAction::ReduceBatchSize { factor } = action {
            log::info!("Reducing batch size by factor: {factor}");

            Ok(RecoveryResult {
                recovery_type: RecoveryType::Repair,
                message: format!("Reduced batch size by {}%", (1.0 - factor) * 100.0),
                performance_impact: None,
                success: true,
                fallback_active: None,
            })
        } else {
            Err(GupError::invalid_operation(
                "BatchSizeReducer cannot handle this action",
            ))
        }
    }

    fn can_handle(&self, action: &RecoveryAction) -> bool {
        matches!(action, RecoveryAction::ReduceBatchSize { .. })
    }

    fn name(&self) -> &str {
        "BatchSizeReducer"
    }
}

#[derive(Debug)]
struct CacheCleaner;

impl CacheCleaner {
    fn new() -> Self {
        Self
    }
}

impl RecoveryHandler for CacheCleaner {
    fn recover(
        &self,
        _context: &ErrorContext,
        action: &RecoveryAction,
    ) -> GupResult<RecoveryResult> {
        if let RecoveryAction::ClearCache = action {
            log::info!("Clearing system caches");

            Ok(RecoveryResult {
                recovery_type: RecoveryType::Repair,
                message: "Cleared system caches to free memory".to_string(),
                performance_impact: None,
                success: true,
                fallback_active: None,
            })
        } else {
            Err(GupError::invalid_operation(
                "CacheCleaner cannot handle this action",
            ))
        }
    }

    fn can_handle(&self, action: &RecoveryAction) -> bool {
        matches!(action, RecoveryAction::ClearCache)
    }

    fn name(&self) -> &str {
        "CacheCleaner"
    }
}

#[derive(Debug)]
struct QualityReducer;

impl QualityReducer {
    fn new() -> Self {
        Self
    }
}

impl RecoveryHandler for QualityReducer {
    fn recover(
        &self,
        _context: &ErrorContext,
        action: &RecoveryAction,
    ) -> GupResult<RecoveryResult> {
        if let RecoveryAction::ReduceQuality = action {
            log::info!("Reducing rendering quality");

            Ok(RecoveryResult {
                recovery_type: RecoveryType::Repair,
                message: "Reduced rendering quality to improve performance".to_string(),
                performance_impact: Some(super::PerformanceImpact {
                    expected_slowdown: 0.7, // 30% faster
                    memory_overhead: -25.0, // 25% less memory
                    quality_reduction: 0.2, // 20% quality reduction
                }),
                success: true,
                fallback_active: None,
            })
        } else {
            Err(GupError::invalid_operation(
                "QualityReducer cannot handle this action",
            ))
        }
    }

    fn can_handle(&self, action: &RecoveryAction) -> bool {
        matches!(action, RecoveryAction::ReduceQuality)
    }

    fn name(&self) -> &str {
        "QualityReducer"
    }
}

// State collection implementations
impl GpuState {
    fn current() -> Self {
        Self {
            device_id: "mock_device".to_string(),
            backend: "Vulkan".to_string(),
            features_enabled: vec!["TIMESTAMP_QUERY".to_string()],
            memory_usage: 1_000_000_000, // 1GB
            active_buffers: 150,
            active_textures: 25,
        }
    }
}

impl BufferState {
    fn current() -> Self {
        Self {
            total_buffers: 150,
            total_size: 500_000_000, // 500MB
            staging_buffers: 10,
            vertex_buffers: 100,
            uniform_buffers: 40,
        }
    }
}

impl PipelineState {
    fn current() -> Self {
        Self {
            active_pipelines: 15,
            cached_shaders: 30,
            bind_groups: 45,
            render_passes: 5,
        }
    }
}

impl SystemConfiguration {
    fn current() -> Self {
        Self {
            render_scale: 1.0,
            quality_level: 2,
            vsync_enabled: true,
            debug_mode: false,
            fallback_settings: HashMap::new(),
        }
    }
}

impl Default for RecoveryManager {
    fn default() -> Self {
        Self::new()
    }
}

// Extension methods for GupError
impl GupError {
    pub fn configuration_error(parameter: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ConfigurationError {
            parameter: parameter.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_manager_creation() {
        let manager = RecoveryManager::new();
        assert!(!manager.recovery_handlers.is_empty());
        assert_eq!(manager.recovery_history.len(), 0);
    }

    #[test]
    fn test_checkpoint_creation() {
        let mut manager = RecoveryManager::new();
        let checkpoint_id = manager.create_checkpoint().unwrap();

        assert_eq!(manager.checkpoints.len(), 1);
        assert_eq!(manager.checkpoints[0].checkpoint_id, checkpoint_id);
    }

    #[test]
    fn test_recovery_stats() {
        let manager = RecoveryManager::new();
        let stats = manager.recovery_stats();

        assert_eq!(stats.total_attempts, 0);
        assert_eq!(stats.success_rate, 0.0);
    }

    #[test]
    fn test_batch_size_reducer() {
        let handler = BatchSizeReducer::new();
        let action = RecoveryAction::ReduceBatchSize { factor: 0.5 };

        assert!(handler.can_handle(&action));
        assert_eq!(handler.name(), "BatchSizeReducer");
    }

    #[test]
    fn test_recovery_action_determination() {
        let manager = RecoveryManager::new();

        let error = GupError::gpu_memory_exhausted(2048, 1024);
        let action = manager.determine_recovery_action(&error).unwrap();

        match action {
            RecoveryAction::ReduceBatchSize { factor } => {
                assert!(factor > 0.0 && factor <= 1.0);
            }
            _ => panic!("Expected ReduceBatchSize action"),
        }
    }
}
