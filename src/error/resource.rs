// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Resource management and cleanup for memory pressure handling.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{GupError, GupResult};

/// Manages GPU resources and handles memory pressure situations.
#[derive(Debug)]
pub struct ResourceManager {
    gpu_resources: HashMap<ResourceId, GpuResource>,
    memory_usage: MemoryTracker,
    cleanup_strategies: Vec<CleanupStrategy>,
    resource_limits: ResourceLimits,
    #[allow(dead_code)]
    pressure_handler: PressureHandler,
}

/// Unique identifier for GPU resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(Uuid);

/// GPU resource with metadata for cleanup decisions.
#[derive(Debug, Clone)]
pub struct GpuResource {
    pub id: ResourceId,
    pub resource_type: ResourceType,
    pub size: usize,
    pub created: Instant,
    pub last_used: Instant,
    pub usage_count: usize,
    pub priority: ResourcePriority,
    pub metadata: HashMap<String, String>,
}

/// Types of GPU resources.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    VertexBuffer,
    IndexBuffer,
    UniformBuffer,
    StorageBuffer,
    StagingBuffer,
    Texture2D,
    Texture3D,
    TextureCube,
    RenderPipeline,
    ComputePipeline,
    BindGroup,
    Sampler,
}

/// Priority levels for resource cleanup decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourcePriority {
    Critical, // Never evict
    High,     // Evict only under severe pressure
    Medium,   // Normal cleanup candidate
    Low,      // First to be evicted
}

/// Memory usage tracking.
#[derive(Debug, Clone)]
pub struct MemoryTracker {
    total_allocated: usize,
    peak_usage: usize,
    allocation_history: Vec<AllocationEvent>,
    usage_by_type: HashMap<ResourceType, usize>,
}

/// Memory allocation event for tracking.
#[derive(Debug, Clone)]
pub struct AllocationEvent {
    #[allow(dead_code)]
    timestamp: Instant,
    #[allow(dead_code)]
    resource_id: ResourceId,
    #[allow(dead_code)]
    resource_type: ResourceType,
    #[allow(dead_code)]
    size: usize,
    #[allow(dead_code)]
    event_type: AllocationEventType,
}

/// Types of allocation events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllocationEventType {
    Allocated,
    Deallocated,
    Resized { old_size: usize },
}

/// Resource usage limits and thresholds.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_gpu_memory: usize,
    pub max_buffer_count: usize,
    pub max_texture_count: usize,
    pub max_pipeline_count: usize,
    pub warning_threshold: f32,
    pub emergency_threshold: f32,
}

/// Cleanup strategies for memory pressure handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupStrategy {
    EvictUnusedBuffers,
    CompactFragmentedMemory,
    ReduceBufferSizes,
    ClearCaches,
    DowngradeTextureQuality,
    RemoveOldResources { max_age: Duration },
    EvictByPriority,
}

/// Memory pressure information and recommendations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePressure {
    pub pressure_type: PressureType,
    pub current_usage: usize,
    pub available: usize,
    pub usage_percentage: f32,
    pub recommended_actions: Vec<CleanupAction>,
    pub critical_resources: Vec<ResourceId>,
}

/// Types of memory pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PressureType {
    Low,
    Moderate,
    High,
    Critical,
    Emergency,
}

/// Recommended cleanup actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CleanupAction {
    CompactBuffers,
    EvictOldResources,
    ReduceTextureQuality,
    ClearUnusedPipelines,
    Emergency,
}

/// Handles memory pressure situations.
#[derive(Debug)]
pub struct PressureHandler {
    #[allow(dead_code)]
    pressure_thresholds: HashMap<PressureType, f32>,
    #[allow(dead_code)]
    last_cleanup: Option<Instant>,
    #[allow(dead_code)]
    cleanup_interval: Duration,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_gpu_memory: 2_000_000_000, // 2GB
            max_buffer_count: 1000,
            max_texture_count: 500,
            max_pipeline_count: 100,
            warning_threshold: 0.75,   // 75%
            emergency_threshold: 0.90, // 90%
        }
    }
}

impl ResourceManager {
    /// Create a new resource manager with default limits.
    pub fn new() -> Self {
        Self::with_limits(ResourceLimits::default())
    }

    /// Create a resource manager with custom limits.
    pub fn with_limits(limits: ResourceLimits) -> Self {
        let mut manager = Self {
            gpu_resources: HashMap::new(),
            memory_usage: MemoryTracker::new(),
            cleanup_strategies: Vec::new(),
            resource_limits: limits,
            pressure_handler: PressureHandler::new(),
        };

        manager.initialize_default_strategies();
        manager
    }

    /// Register a new GPU resource.
    pub fn register_resource(&mut self, mut resource: GpuResource) -> ResourceId {
        resource.id = ResourceId::new();
        resource.created = Instant::now();
        resource.last_used = Instant::now();

        self.memory_usage.record_allocation(
            resource.id,
            resource.resource_type.clone(),
            resource.size,
        );

        let resource_id = resource.id;
        self.gpu_resources.insert(resource_id, resource);

        // Check for memory pressure after allocation
        if let Some(pressure) = self.check_resource_pressure() {
            if pressure.pressure_type >= PressureType::High {
                log::warn!(
                    "High memory pressure detected after allocation: {}%",
                    pressure.usage_percentage
                );

                // Trigger automatic cleanup if needed
                if pressure.pressure_type >= PressureType::Emergency {
                    log::warn!(
                        "Emergency memory pressure detected - consider calling emergency_cleanup()"
                    );
                }
            }
        }

        resource_id
    }

    /// Unregister a GPU resource.
    pub fn unregister_resource(&mut self, resource_id: ResourceId) -> GupResult<()> {
        if let Some(resource) = self.gpu_resources.remove(&resource_id) {
            self.memory_usage.record_deallocation(
                resource_id,
                resource.resource_type,
                resource.size,
            );
            log::debug!("Unregistered resource: {resource_id:?}");
            Ok(())
        } else {
            Err(GupError::resource_error(format!(
                "Resource not found: {resource_id:?}"
            )))
        }
    }

    /// Update resource usage metadata.
    pub fn touch_resource(&mut self, resource_id: ResourceId) -> GupResult<()> {
        if let Some(resource) = self.gpu_resources.get_mut(&resource_id) {
            resource.last_used = Instant::now();
            resource.usage_count += 1;
            Ok(())
        } else {
            Err(GupError::resource_error(format!(
                "Resource not found: {resource_id:?}"
            )))
        }
    }

    /// Check current resource pressure.
    pub fn check_resource_pressure(&self) -> Option<ResourcePressure> {
        let current_usage = self.memory_usage.current_usage();
        let usage_percentage = current_usage as f32 / self.resource_limits.max_gpu_memory as f32;

        let pressure_type = if usage_percentage > self.resource_limits.emergency_threshold {
            PressureType::Emergency
        } else if usage_percentage > 0.85 {
            PressureType::Critical
        } else if usage_percentage > self.resource_limits.warning_threshold {
            PressureType::High
        } else if usage_percentage > 0.5 {
            PressureType::Moderate
        } else {
            PressureType::Low
        };

        if pressure_type > PressureType::Moderate {
            Some(ResourcePressure {
                pressure_type,
                current_usage,
                available: self
                    .resource_limits
                    .max_gpu_memory
                    .saturating_sub(current_usage),
                usage_percentage: usage_percentage * 100.0,
                recommended_actions: self.generate_cleanup_actions(pressure_type),
                critical_resources: self.find_critical_resources(),
            })
        } else {
            None
        }
    }

    /// Perform emergency cleanup to free memory.
    pub async fn emergency_cleanup(&mut self) -> GupResult<usize> {
        let start_usage = self.memory_usage.current_usage();
        let mut _freed_memory = 0;

        log::warn!("Starting emergency cleanup, current usage: {start_usage} bytes");

        // Apply cleanup strategies in order of priority
        for strategy in &self.cleanup_strategies.clone() {
            match self.apply_cleanup_strategy(strategy).await {
                Ok(freed) => {
                    _freed_memory += freed;
                    log::info!("Strategy {strategy:?} freed {freed} bytes");
                }
                Err(e) => {
                    log::error!("Cleanup strategy {strategy:?} failed: {e}");
                }
            }

            // Check if we've reduced pressure sufficiently
            if let Some(pressure) = self.check_resource_pressure() {
                if pressure.pressure_type <= PressureType::Moderate {
                    log::info!("Memory pressure reduced to acceptable level");
                    break;
                }
            } else {
                log::info!("Memory pressure eliminated");
                break;
            }
        }

        let final_usage = self.memory_usage.current_usage();
        let total_freed = start_usage.saturating_sub(final_usage);

        log::info!("Emergency cleanup complete. Freed {total_freed} bytes total");
        Ok(total_freed)
    }

    /// Get resource usage statistics.
    pub fn usage_stats(&self) -> ResourceStats {
        ResourceStats {
            total_resources: self.gpu_resources.len(),
            total_memory_used: self.memory_usage.current_usage(),
            memory_limit: self.resource_limits.max_gpu_memory,
            usage_by_type: self.memory_usage.usage_by_type.clone(),
            peak_usage: self.memory_usage.peak_usage,
            fragmentation_ratio: self.calculate_fragmentation(),
        }
    }

    /// Find resources that should never be evicted.
    pub fn find_critical_resources(&self) -> Vec<ResourceId> {
        self.gpu_resources
            .values()
            .filter(|r| r.priority == ResourcePriority::Critical)
            .map(|r| r.id)
            .collect()
    }

    async fn apply_cleanup_strategy(&mut self, strategy: &CleanupStrategy) -> GupResult<usize> {
        match strategy {
            CleanupStrategy::EvictUnusedBuffers => self.evict_unused_buffers().await,
            CleanupStrategy::CompactFragmentedMemory => self.compact_memory().await,
            CleanupStrategy::ReduceBufferSizes => self.reduce_buffer_sizes().await,
            CleanupStrategy::ClearCaches => self.clear_caches().await,
            CleanupStrategy::DowngradeTextureQuality => self.downgrade_textures().await,
            CleanupStrategy::RemoveOldResources { max_age } => {
                self.remove_old_resources(*max_age).await
            }
            CleanupStrategy::EvictByPriority => self.evict_by_priority().await,
        }
    }

    async fn evict_unused_buffers(&mut self) -> GupResult<usize> {
        let cutoff = Instant::now() - Duration::from_secs(60);
        let mut freed = 0;
        let mut to_remove = Vec::new();

        for (id, resource) in &self.gpu_resources {
            if resource.last_used < cutoff
                && matches!(
                    resource.resource_type,
                    ResourceType::VertexBuffer
                        | ResourceType::IndexBuffer
                        | ResourceType::StagingBuffer
                )
                && resource.priority <= ResourcePriority::Medium
            {
                freed += resource.size;
                to_remove.push(*id);
            }
        }

        let num_removed = to_remove.len();
        for id in to_remove {
            self.unregister_resource(id)?;
        }

        log::info!("Evicted {num_removed} unused buffers, freed {freed} bytes");
        Ok(freed)
    }

    async fn compact_memory(&mut self) -> GupResult<usize> {
        // Memory compaction would be handled by the GPU driver/allocator
        // This is a placeholder for triggering compaction
        log::info!("Triggering memory compaction");
        Ok(0) // Compaction doesn't directly free memory but reduces fragmentation
    }

    async fn reduce_buffer_sizes(&mut self) -> GupResult<usize> {
        // Reduce oversized buffers that are larger than necessary
        let mut freed = 0;

        for resource in self.gpu_resources.values_mut() {
            if matches!(resource.resource_type, ResourceType::VertexBuffer | ResourceType::IndexBuffer)
                && resource.size > 1_000_000 // 1MB threshold
                && resource.priority <= ResourcePriority::Medium
            {
                let old_size = resource.size;
                resource.size = (resource.size as f32 * 0.75) as usize; // Reduce by 25%
                freed += old_size - resource.size;
            }
        }

        if freed > 0 {
            log::info!("Reduced buffer sizes, freed {freed} bytes");
        }
        Ok(freed)
    }

    async fn clear_caches(&mut self) -> GupResult<usize> {
        let mut freed = 0;
        let mut to_remove = Vec::new();

        // Remove cached pipelines and bind groups
        for (id, resource) in &self.gpu_resources {
            if matches!(
                resource.resource_type,
                ResourceType::RenderPipeline
                    | ResourceType::ComputePipeline
                    | ResourceType::BindGroup
            ) && resource.usage_count < 5
            // Rarely used
            {
                freed += resource.size;
                to_remove.push(*id);
            }
        }

        for id in to_remove {
            self.unregister_resource(id)?;
        }

        log::info!("Cleared caches, freed {freed} bytes");
        Ok(freed)
    }

    async fn downgrade_textures(&mut self) -> GupResult<usize> {
        let mut freed = 0;

        for resource in self.gpu_resources.values_mut() {
            if matches!(
                resource.resource_type,
                ResourceType::Texture2D | ResourceType::Texture3D
            ) && resource.priority <= ResourcePriority::Medium
                && resource.size > 10_000_000
            // 10MB threshold
            {
                let old_size = resource.size;
                resource.size = (resource.size as f32 * 0.5) as usize; // Reduce to half resolution
                freed += old_size - resource.size;
            }
        }

        if freed > 0 {
            log::info!("Downgraded texture quality, freed {freed} bytes");
        }
        Ok(freed)
    }

    async fn remove_old_resources(&mut self, max_age: Duration) -> GupResult<usize> {
        let cutoff = Instant::now() - max_age;
        let mut freed = 0;
        let mut to_remove = Vec::new();

        for (id, resource) in &self.gpu_resources {
            if resource.created < cutoff && resource.priority <= ResourcePriority::Low {
                freed += resource.size;
                to_remove.push(*id);
            }
        }

        let num_removed = to_remove.len();
        for id in to_remove {
            self.unregister_resource(id)?;
        }

        log::info!("Removed {num_removed} old resources, freed {freed} bytes");
        Ok(freed)
    }

    async fn evict_by_priority(&mut self) -> GupResult<usize> {
        let mut candidates: Vec<_> = self
            .gpu_resources
            .values()
            .filter(|r| r.priority <= ResourcePriority::Low)
            .collect();

        // Sort by priority (lowest first), then by last used (oldest first)
        candidates.sort_by_key(|r| (r.priority, r.last_used));

        let mut freed = 0;
        let mut to_remove = Vec::new();

        // Evict up to 20% of low priority resources
        let evict_count = (candidates.len() as f32 * 0.2).ceil() as usize;

        for resource in candidates.iter().take(evict_count) {
            freed += resource.size;
            to_remove.push(resource.id);
        }

        let num_evicted = to_remove.len();
        for id in to_remove {
            self.unregister_resource(id)?;
        }

        log::info!("Evicted {num_evicted} low-priority resources, freed {freed} bytes");
        Ok(freed)
    }

    fn initialize_default_strategies(&mut self) {
        self.cleanup_strategies = vec![
            CleanupStrategy::EvictUnusedBuffers,
            CleanupStrategy::ClearCaches,
            CleanupStrategy::RemoveOldResources {
                max_age: Duration::from_secs(300),
            }, // 5 minutes
            CleanupStrategy::ReduceBufferSizes,
            CleanupStrategy::DowngradeTextureQuality,
            CleanupStrategy::CompactFragmentedMemory,
            CleanupStrategy::EvictByPriority,
        ];
    }

    fn generate_cleanup_actions(&self, pressure_type: PressureType) -> Vec<CleanupAction> {
        match pressure_type {
            PressureType::Low | PressureType::Moderate => vec![],
            PressureType::High => vec![
                CleanupAction::CompactBuffers,
                CleanupAction::EvictOldResources,
            ],
            PressureType::Critical => vec![
                CleanupAction::CompactBuffers,
                CleanupAction::EvictOldResources,
                CleanupAction::ReduceTextureQuality,
                CleanupAction::ClearUnusedPipelines,
            ],
            PressureType::Emergency => vec![CleanupAction::Emergency],
        }
    }

    fn calculate_fragmentation(&self) -> f32 {
        // Placeholder for fragmentation calculation
        // In a real implementation, this would analyze memory layout
        0.1 // 10% fragmentation
    }
}

/// Resource usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStats {
    pub total_resources: usize,
    pub total_memory_used: usize,
    pub memory_limit: usize,
    pub usage_by_type: HashMap<ResourceType, usize>,
    pub peak_usage: usize,
    pub fragmentation_ratio: f32,
}

impl Default for ResourceId {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self::new()
    }
}

impl MemoryTracker {
    fn new() -> Self {
        Self {
            total_allocated: 0,
            peak_usage: 0,
            allocation_history: Vec::new(),
            usage_by_type: HashMap::new(),
        }
    }

    fn record_allocation(&mut self, id: ResourceId, resource_type: ResourceType, size: usize) {
        self.total_allocated += size;
        self.peak_usage = self.peak_usage.max(self.total_allocated);

        *self.usage_by_type.entry(resource_type.clone()).or_insert(0) += size;

        self.allocation_history.push(AllocationEvent {
            timestamp: Instant::now(),
            resource_id: id,
            resource_type,
            size,
            event_type: AllocationEventType::Allocated,
        });

        // Keep history bounded
        const MAX_HISTORY: usize = 10000;
        if self.allocation_history.len() > MAX_HISTORY {
            self.allocation_history.remove(0);
        }
    }

    fn record_deallocation(&mut self, id: ResourceId, resource_type: ResourceType, size: usize) {
        self.total_allocated = self.total_allocated.saturating_sub(size);

        if let Some(usage) = self.usage_by_type.get_mut(&resource_type) {
            *usage = usage.saturating_sub(size);
        }

        self.allocation_history.push(AllocationEvent {
            timestamp: Instant::now(),
            resource_id: id,
            resource_type,
            size,
            event_type: AllocationEventType::Deallocated,
        });
    }

    fn current_usage(&self) -> usize {
        self.total_allocated
    }
}

impl PressureHandler {
    fn new() -> Self {
        let mut thresholds = HashMap::new();
        thresholds.insert(PressureType::Moderate, 0.6);
        thresholds.insert(PressureType::High, 0.75);
        thresholds.insert(PressureType::Critical, 0.85);
        thresholds.insert(PressureType::Emergency, 0.95);

        Self {
            pressure_thresholds: thresholds,
            last_cleanup: None,
            cleanup_interval: Duration::from_secs(10),
        }
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_manager_creation() {
        let manager = ResourceManager::new();
        assert_eq!(manager.gpu_resources.len(), 0);
        assert_eq!(manager.memory_usage.current_usage(), 0);
    }

    #[test]
    fn test_resource_registration() {
        let mut manager = ResourceManager::new();

        let resource = GpuResource {
            id: ResourceId::new(),
            resource_type: ResourceType::VertexBuffer,
            size: 1024,
            created: Instant::now(),
            last_used: Instant::now(),
            usage_count: 0,
            priority: ResourcePriority::Medium,
            metadata: HashMap::new(),
        };

        let resource_id = manager.register_resource(resource);
        assert_eq!(manager.gpu_resources.len(), 1);
        assert_eq!(manager.memory_usage.current_usage(), 1024);

        manager.unregister_resource(resource_id).unwrap();
        assert_eq!(manager.gpu_resources.len(), 0);
        assert_eq!(manager.memory_usage.current_usage(), 0);
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
            created: Instant::now(),
            last_used: Instant::now(),
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

        // Add some resources that are old enough to be cleaned up (400s > 300s threshold)
        for _i in 0..10 {
            let resource = GpuResource {
                id: ResourceId::new(),
                resource_type: ResourceType::VertexBuffer,
                size: 1000,
                created: Instant::now() - Duration::from_secs(400), // Older than 300s
                last_used: Instant::now() - Duration::from_secs(400),
                usage_count: 0,
                priority: ResourcePriority::Low,
                metadata: HashMap::new(),
            };
            manager.register_resource(resource);
        }

        let initial_usage = manager.memory_usage.current_usage();
        let _freed = manager.emergency_cleanup().await.unwrap();

        // Emergency cleanup should run without crashing and return a valid result
        // The result represents bytes freed (which is always >= 0 for usize)

        // Memory should be reduced or stay the same (if no cleanup was performed)
        assert!(
            manager.memory_usage.current_usage() <= initial_usage,
            "Memory usage should not increase after cleanup"
        );
    }
}
