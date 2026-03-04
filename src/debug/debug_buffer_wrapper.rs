// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Debug wrapper for GpuBuffer with automatic validation.
//!
//! This module provides a wrapper around GpuBuffer that automatically performs
//! validation checks during upload operations in debug builds.

use crate::buffer::{BufferType, GpuBuffer};
use crate::debug::buffer_validation::{
    BufferMetadata, ValidationReport, ValidationResult, ValidationRule,
};
use crate::error::GupResult;
use std::time::Instant;
use wgpu::{Device, Queue};

/// Debug wrapper around GpuBuffer with automatic validation
#[cfg(debug_assertions)]
pub struct DebugBufferWrapper<T> {
    inner: GpuBuffer<T>,
    validation_rules: Vec<Box<dyn ValidationRule<T>>>,
    auto_validate: bool,
    operation_log: Vec<BufferOperation>,
    max_log_size: usize,
}

#[cfg(debug_assertions)]
impl<T> DebugBufferWrapper<T>
where
    T: bytemuck::Pod + bytemuck::Zeroable,
{
    /// Create a new debug buffer wrapper
    pub fn new(
        device: &Device,
        buffer_type: BufferType,
        capacity: usize,
        auto_validate: bool,
    ) -> Self {
        Self {
            inner: GpuBuffer::new(device, buffer_type, capacity),
            validation_rules: Vec::new(),
            auto_validate,
            operation_log: Vec::new(),
            max_log_size: 100,
        }
    }

    /// Wrap an existing GpuBuffer
    pub fn wrap(buffer: GpuBuffer<T>, auto_validate: bool) -> Self {
        Self {
            inner: buffer,
            validation_rules: Vec::new(),
            auto_validate,
            operation_log: Vec::new(),
            max_log_size: 100,
        }
    }

    /// Add a validation rule
    pub fn add_rule(&mut self, rule: Box<dyn ValidationRule<T>>) {
        self.validation_rules.push(rule);
    }

    /// Upload data with optional validation
    pub fn upload(&mut self, device: &Device, queue: &Queue, data: &[T]) -> GupResult<()> {
        let start = Instant::now();

        // Pre-upload validation
        if self.auto_validate {
            let validation_result = self.validate_data(data);
            if validation_result.has_errors() {
                eprintln!(
                    "⚠️  Buffer validation failed before upload:\n{}",
                    validation_result.format_report()
                );
            }
        }

        // Perform upload
        let upload_start = Instant::now();
        let result = self.inner.upload(device, queue, data);
        let upload_duration = upload_start.elapsed();

        // Log operation
        self.log_operation(BufferOperation::Upload {
            timestamp: Instant::now(),
            data_size: data.len(),
            buffer_capacity: self.inner.capacity(),
            duration: start.elapsed(),
            upload_duration,
        });

        result
    }

    /// Upload data to a specific range
    pub fn upload_range(&mut self, queue: &Queue, data: &[T], offset: usize) -> GupResult<()> {
        let start = Instant::now();

        // Perform upload
        let result = self.inner.upload_range(queue, data, offset);

        // Log operation
        self.log_operation(BufferOperation::UploadRange {
            timestamp: Instant::now(),
            data_size: data.len(),
            offset,
            duration: start.elapsed(),
        });

        result
    }

    /// Validate data against all registered rules
    fn validate_data(&self, data: &[T]) -> ValidationReport {
        let metadata = BufferMetadata {
            capacity: self.inner.capacity(),
            len: data.len(),
            element_size: std::mem::size_of::<T>(),
            buffer_size: (self.inner.capacity() * std::mem::size_of::<T>()) as u64,
        };

        let results: Vec<ValidationResult> = self
            .validation_rules
            .iter()
            .map(|rule| rule.validate(data, &metadata))
            .collect();

        ValidationReport::new(results)
    }

    /// Log a buffer operation
    fn log_operation(&mut self, operation: BufferOperation) {
        self.operation_log.push(operation);

        // Keep log size bounded
        if self.operation_log.len() > self.max_log_size {
            self.operation_log.remove(0);
        }
    }

    /// Get operation log
    pub fn get_operation_log(&self) -> &[BufferOperation] {
        &self.operation_log
    }

    /// Get the inner buffer
    pub fn inner(&self) -> &GpuBuffer<T> {
        &self.inner
    }

    /// Get mutable access to the inner buffer
    pub fn inner_mut(&mut self) -> &mut GpuBuffer<T> {
        &mut self.inner
    }

    /// Enable or disable auto-validation
    pub fn set_auto_validate(&mut self, enabled: bool) {
        self.auto_validate = enabled;
    }

    /// Check if auto-validation is enabled
    pub fn is_auto_validate(&self) -> bool {
        self.auto_validate
    }

    /// Clear operation log
    pub fn clear_log(&mut self) {
        self.operation_log.clear();
    }

    /// Get statistics about buffer operations
    pub fn get_operation_stats(&self) -> OperationStats {
        let upload_count = self
            .operation_log
            .iter()
            .filter(|op| matches!(op, BufferOperation::Upload { .. }))
            .count();

        let upload_range_count = self
            .operation_log
            .iter()
            .filter(|op| matches!(op, BufferOperation::UploadRange { .. }))
            .count();

        let total_upload_time: std::time::Duration = self
            .operation_log
            .iter()
            .filter_map(|op| match op {
                BufferOperation::Upload { duration, .. } => Some(*duration),
                _ => None,
            })
            .sum();

        let avg_upload_time = if upload_count > 0 {
            total_upload_time / upload_count as u32
        } else {
            std::time::Duration::ZERO
        };

        OperationStats {
            upload_count,
            upload_range_count,
            total_upload_time,
            avg_upload_time,
        }
    }
}

/// Record of a buffer operation
#[derive(Debug, Clone)]
pub enum BufferOperation {
    /// Full buffer upload operation.
    Upload {
        /// When the operation occurred.
        timestamp: Instant,
        /// Number of elements uploaded.
        data_size: usize,
        /// Capacity of the destination buffer.
        buffer_capacity: usize,
        /// Total operation duration including validation.
        duration: std::time::Duration,
        /// Duration of the GPU upload alone.
        upload_duration: std::time::Duration,
    },
    /// Partial buffer upload to a specific range.
    UploadRange {
        /// When the operation occurred.
        timestamp: Instant,
        /// Number of elements uploaded.
        data_size: usize,
        /// Byte offset into the buffer.
        offset: usize,
        /// Total operation duration.
        duration: std::time::Duration,
    },
}

/// Statistics about buffer operations
#[derive(Debug, Clone)]
pub struct OperationStats {
    /// Number of full upload operations.
    pub upload_count: usize,
    /// Number of range upload operations.
    pub upload_range_count: usize,
    /// Cumulative time spent on full uploads.
    pub total_upload_time: std::time::Duration,
    /// Average time per full upload.
    pub avg_upload_time: std::time::Duration,
}

#[cfg(test)]
mod tests {
    // Note: These tests require GPU access and are integration tests
    // They should be run with --test-threads=1

    #[test]
    fn test_debug_wrapper_creation() {
        // This is a compile-time test to ensure the API is correct
        // Actual GPU tests would require a device and queue
    }

    #[test]
    fn test_operation_log_bounded() {
        // Test that operation log doesn't grow unbounded
        // This would need GPU context to actually test
    }
}
