// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU buffer content inspection and analysis tools.
//!
//! This module provides utilities for inspecting GPU buffer contents through staging buffers,
//! enabling debugging of GPU data transfer and processing issues.

use crate::debug::buffer_validation::{BufferMetadata, ValidationReport, ValidationRule};
use crate::error::{GupError, GupResult};
use futures_channel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::marker::PhantomData;
use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Device, MapMode, PollType,
    Queue,
};

/// GPU buffer inspector for content analysis and debugging
#[derive(Debug)]
pub struct GpuBufferInspector {
    device: Device,
    queue: Queue,
    /// Cache of staging buffers for reuse
    staging_buffer_cache: HashMap<u64, Buffer>,
}

impl GpuBufferInspector {
    /// Create a new GPU buffer inspector
    pub fn new(device: &Device, queue: &Queue) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            staging_buffer_cache: HashMap::new(),
        }
    }

    /// Dump buffer contents to a JSON file for inspection
    pub async fn dump_buffer<T>(&mut self, buffer: &Buffer, output_path: &str) -> GupResult<()>
    where
        T: bytemuck::Pod + bytemuck::Zeroable + Serialize,
    {
        let data = self.read_buffer::<T>(buffer).await?;
        let json = serde_json::to_string_pretty(&data).map_err(|e| {
            GupError::validation_error(format!("Failed to serialize buffer data: {e}"))
        })?;

        std::fs::write(output_path, json)
            .map_err(|e| GupError::resource_error(format!("Failed to write buffer dump: {e}")))?;

        Ok(())
    }

    /// Dump buffer contents to a CSV file for spreadsheet analysis
    pub async fn dump_buffer_csv<T>(&mut self, buffer: &Buffer, output_path: &str) -> GupResult<()>
    where
        T: bytemuck::Pod + bytemuck::Zeroable + Serialize,
    {
        let data = self.read_buffer::<T>(buffer).await?;

        let mut writer = csv::Writer::from_path(output_path)
            .map_err(|e| GupError::resource_error(format!("Failed to create CSV writer: {e}")))?;

        for item in &data {
            writer
                .serialize(item)
                .map_err(|e| GupError::validation_error(format!("Failed to write CSV row: {e}")))?;
        }

        writer
            .flush()
            .map_err(|e| GupError::resource_error(format!("Failed to flush CSV writer: {e}")))?;

        Ok(())
    }

    /// Read buffer contents into a `Vec<T>`
    pub async fn read_buffer<T>(&mut self, buffer: &Buffer) -> GupResult<Vec<T>>
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        let buffer_size = buffer.size();
        let element_size = std::mem::size_of::<T>() as u64;

        if !buffer_size.is_multiple_of(element_size) {
            return Err(GupError::validation_error(format!(
                "Buffer size {buffer_size} is not a multiple of element size {element_size}"
            )));
        }

        let element_count = (buffer_size / element_size) as usize;

        // Get or create staging buffer (need to split borrowing)
        let buffer_size_key = buffer_size;
        let staging_buffer_exists = self.staging_buffer_cache.contains_key(&buffer_size_key);

        if !staging_buffer_exists {
            let staging_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some(&format!("staging_buffer_{buffer_size}")),
                size: buffer_size,
                usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.staging_buffer_cache
                .insert(buffer_size_key, staging_buffer);
        }

        // Copy from GPU buffer to staging buffer
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("buffer_inspector_copy"),
            });

        let staging_buffer = self.staging_buffer_cache.get(&buffer_size_key).unwrap();
        encoder.copy_buffer_to_buffer(buffer, 0, staging_buffer, 0, buffer_size);
        let submission_index = self.queue.submit([encoder.finish()]);

        // Wait for copy to complete
        let _ = self
            .device
            .poll(PollType::WaitForSubmissionIndex(submission_index));

        // Map buffer and read data
        let staging_buffer = self.staging_buffer_cache.get(&buffer_size_key).unwrap();
        let buffer_slice = staging_buffer.slice(..);

        let (sender, receiver) = futures_channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        let _ = self.device.poll(PollType::Wait);

        receiver
            .await
            .map_err(|_| {
                GupError::resource_error("Failed to receive buffer mapping result".to_string())
            })?
            .map_err(|e| GupError::resource_error(format!("Buffer mapping failed: {e:?}")))?;

        let data = buffer_slice.get_mapped_range();
        let typed_data: &[T] = bytemuck::cast_slice(&data);
        let result = typed_data.to_vec();

        drop(data);
        staging_buffer.unmap();

        if result.len() != element_count {
            return Err(GupError::validation_error(format!(
                "Read {} elements, expected {}",
                result.len(),
                element_count
            )));
        }

        Ok(result)
    }

    /// Compare two buffers and report differences
    pub async fn compare_buffers<T>(
        &mut self,
        buffer_a: &Buffer,
        buffer_b: &Buffer,
        _tolerance: f32,
    ) -> GupResult<BufferComparisonResult<T>>
    where
        T: bytemuck::Pod + bytemuck::Zeroable + PartialEq + Clone + std::fmt::Debug,
    {
        let data_a = self.read_buffer::<T>(buffer_a).await?;
        let data_b = self.read_buffer::<T>(buffer_b).await?;

        if data_a.len() != data_b.len() {
            return Ok(BufferComparisonResult {
                is_identical: false,
                length_mismatch: Some((data_a.len(), data_b.len())),
                differences: Vec::new(),
                similarity_percentage: 0.0,
                phantom: PhantomData,
            });
        }

        let mut differences = Vec::new();
        let mut identical_count = 0;

        for (index, (a, b)) in data_a.iter().zip(data_b.iter()).enumerate() {
            // For numeric types, we might want to implement tolerance-based comparison
            // For now, use exact equality
            if a == b {
                identical_count += 1;
            } else {
                differences.push(BufferElementDifference {
                    index,
                    value_a: format!("{a:?}"),
                    value_b: format!("{b:?}"),
                });

                // Limit number of reported differences to prevent huge output
                if differences.len() >= 100 {
                    break;
                }
            }
        }

        let similarity_percentage = if data_a.is_empty() {
            100.0
        } else {
            (identical_count as f32 / data_a.len() as f32) * 100.0
        };

        Ok(BufferComparisonResult {
            is_identical: differences.is_empty(),
            length_mismatch: None,
            differences,
            similarity_percentage,
            phantom: PhantomData,
        })
    }

    /// Analyze buffer for common patterns and anomalies
    pub async fn analyze_buffer<T>(&mut self, buffer: &Buffer) -> GupResult<BufferAnalysis>
    where
        T: bytemuck::Pod + bytemuck::Zeroable + Clone + std::fmt::Debug,
    {
        let data = self.read_buffer::<T>(buffer).await?;

        if data.is_empty() {
            return Ok(BufferAnalysis {
                element_count: 0,
                unique_values: 0,
                has_zero_values: false,
                has_nan_values: false,
                has_infinite_values: false,
                memory_usage_bytes: 0,
                anomalies: Vec::new(),
            });
        }

        let mut unique_values = std::collections::HashSet::new();
        let mut has_zero_values = false;
        let mut has_nan_values = false;
        let mut has_infinite_values = false;
        let mut anomalies = Vec::new();

        // Analyze numeric patterns for f32 data
        let bytes = bytemuck::cast_slice::<T, u8>(&data);
        let floats = bytemuck::cast_slice::<u8, f32>(bytes);

        for (index, &value) in floats.iter().enumerate() {
            if value == 0.0 {
                has_zero_values = true;
            }
            if value.is_nan() {
                has_nan_values = true;
                anomalies.push(format!("NaN value at float index {index}"));
            }
            if value.is_infinite() {
                has_infinite_values = true;
                anomalies.push(format!("Infinite value at float index {index}: {value}"));
            }
        }

        // Count unique values (limited to prevent performance issues)
        for item in data.iter().take(10000) {
            unique_values.insert(format!("{item:?}"));
        }

        Ok(BufferAnalysis {
            element_count: data.len(),
            unique_values: unique_values.len(),
            has_zero_values,
            has_nan_values,
            has_infinite_values,
            memory_usage_bytes: buffer.size(),
            anomalies,
        })
    }

    /// Clear staging buffer cache to free GPU memory
    pub fn clear_cache(&mut self) {
        self.staging_buffer_cache.clear();
    }

    /// Get statistics about cached staging buffers
    pub fn get_cache_stats(&self) -> StagingBufferStats {
        let total_buffers = self.staging_buffer_cache.len();
        let total_memory: u64 = self.staging_buffer_cache.values().map(|b| b.size()).sum();

        StagingBufferStats {
            buffer_count: total_buffers,
            total_memory_bytes: total_memory,
        }
    }

    /// Validate buffer contents using provided validation rules
    pub async fn validate_buffer<T>(
        &mut self,
        buffer: &Buffer,
        capacity: usize,
        rules: Vec<Box<dyn ValidationRule<T>>>,
    ) -> GupResult<ValidationReport>
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        let data = self.read_buffer::<T>(buffer).await?;
        let metadata = BufferMetadata {
            capacity,
            len: data.len(),
            element_size: std::mem::size_of::<T>(),
            buffer_size: buffer.size(),
        };

        let mut results = Vec::new();
        for rule in rules {
            results.push(rule.validate(&data, &metadata));
        }

        Ok(ValidationReport::new(results))
    }

    /// Create a formatted statistical summary of buffer contents
    pub async fn create_statistical_summary<T>(&mut self, buffer: &Buffer) -> GupResult<String>
    where
        T: bytemuck::Pod + bytemuck::Zeroable + Clone + std::fmt::Debug,
    {
        let data = self.read_buffer::<T>(buffer).await?;
        if data.is_empty() {
            return Ok("Empty buffer".to_string());
        }

        // Try to interpret as floats for statistical analysis
        let bytes = bytemuck::cast_slice::<T, u8>(&data);
        let floats = bytemuck::cast_slice::<u8, f32>(bytes);

        let mut output = String::new();
        output.push_str("=== Buffer Statistical Summary ===\n");
        output.push_str(&format!("Element count: {}\n", data.len()));
        output.push_str(&format!("Buffer size: {} bytes\n", buffer.size()));
        output.push_str(&format!(
            "Element size: {} bytes\n",
            std::mem::size_of::<T>()
        ));
        output.push_str("\nFloat value statistics:\n");

        // Calculate basic statistics
        let valid_floats: Vec<f32> = floats.iter().filter(|f| f.is_finite()).copied().collect();

        if !valid_floats.is_empty() {
            let min = valid_floats.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let max = valid_floats
                .iter()
                .fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let sum: f32 = valid_floats.iter().sum();
            let mean = sum / valid_floats.len() as f32;

            let variance: f32 = valid_floats
                .iter()
                .map(|&v| (v - mean).powi(2))
                .sum::<f32>()
                / valid_floats.len() as f32;
            let std_dev = variance.sqrt();

            output.push_str(&format!("  Min: {:.6}\n", min));
            output.push_str(&format!("  Max: {:.6}\n", max));
            output.push_str(&format!("  Mean: {:.6}\n", mean));
            output.push_str(&format!("  Std Dev: {:.6}\n", std_dev));
            output.push_str(&format!("  Valid values: {}\n", valid_floats.len()));
        }

        let nan_count = floats.iter().filter(|f| f.is_nan()).count();
        let inf_count = floats.iter().filter(|f| f.is_infinite()).count();
        let zero_count = floats.iter().filter(|&&f| f == 0.0).count();

        output.push_str(&format!("  NaN values: {}\n", nan_count));
        output.push_str(&format!("  Infinite values: {}\n", inf_count));
        output.push_str(&format!("  Zero values: {}\n", zero_count));

        Ok(output)
    }
}

/// Result of buffer comparison analysis
#[derive(Debug, Clone)]
pub struct BufferComparisonResult<T> {
    /// Whether the two buffers are identical.
    pub is_identical: bool,
    /// Length mismatch as (buffer_a_len, buffer_b_len), if applicable.
    pub length_mismatch: Option<(usize, usize)>,
    /// Individual element differences found.
    pub differences: Vec<BufferElementDifference>,
    /// Percentage of identical elements.
    pub similarity_percentage: f32,
    phantom: PhantomData<T>,
}

/// Individual element difference in buffer comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferElementDifference {
    /// Index of the differing element.
    pub index: usize,
    /// Debug representation of the value in buffer A.
    pub value_a: String,
    /// Debug representation of the value in buffer B.
    pub value_b: String,
}

/// Analysis results for buffer content patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferAnalysis {
    /// Number of elements in the buffer.
    pub element_count: usize,
    /// Approximate number of unique values.
    pub unique_values: usize,
    /// Whether the buffer contains zero values.
    pub has_zero_values: bool,
    /// Whether the buffer contains NaN float values.
    pub has_nan_values: bool,
    /// Whether the buffer contains infinite float values.
    pub has_infinite_values: bool,
    /// Total buffer size in bytes.
    pub memory_usage_bytes: u64,
    /// Descriptions of detected anomalies.
    pub anomalies: Vec<String>,
}

/// Statistics about staging buffer cache usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagingBufferStats {
    /// Number of cached staging buffers.
    pub buffer_count: usize,
    /// Total GPU memory used by staging buffers.
    pub total_memory_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_analysis_empty() {
        let analysis = BufferAnalysis {
            element_count: 0,
            unique_values: 0,
            has_zero_values: false,
            has_nan_values: false,
            has_infinite_values: false,
            memory_usage_bytes: 0,
            anomalies: Vec::new(),
        };

        assert_eq!(analysis.element_count, 0);
        assert_eq!(analysis.unique_values, 0);
        assert!(!analysis.has_zero_values);
        assert!(analysis.anomalies.is_empty());
    }

    #[test]
    fn test_buffer_element_difference() {
        let diff = BufferElementDifference {
            index: 42,
            value_a: "1.0".to_string(),
            value_b: "2.0".to_string(),
        };

        assert_eq!(diff.index, 42);
        assert_eq!(diff.value_a, "1.0");
        assert_eq!(diff.value_b, "2.0");
    }

    #[test]
    fn test_staging_buffer_stats() {
        let stats = StagingBufferStats {
            buffer_count: 5,
            total_memory_bytes: 1024 * 1024,
        };

        assert_eq!(stats.buffer_count, 5);
        assert_eq!(stats.total_memory_bytes, 1024 * 1024);
    }

    #[test]
    fn test_buffer_comparison_result() {
        let result: BufferComparisonResult<f32> = BufferComparisonResult {
            is_identical: false,
            length_mismatch: Some((100, 200)),
            differences: vec![BufferElementDifference {
                index: 0,
                value_a: "1.0".to_string(),
                value_b: "2.0".to_string(),
            }],
            similarity_percentage: 85.5,
            phantom: PhantomData,
        };

        assert!(!result.is_identical);
        assert_eq!(result.length_mismatch, Some((100, 200)));
        assert_eq!(result.differences.len(), 1);
        assert_eq!(result.similarity_percentage, 85.5);
    }
}
